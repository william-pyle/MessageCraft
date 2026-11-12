//! Contacts shared with MailCraft, through MailCraft's own control channel
//! (github.com/UCS-Care-Group/mailcraft, docs/control-protocol.md).
//!
//! MailCraft started with a control port (`mailcraft.exe --control <port>`) listens on 127.0.0.1
//! and writes a random key to `%APPDATA%\MailCraft\control-key.local-<port>` while it runs. Here we
//! read that key, make MailCraft prove it holds it (HMAC over our nonce, so the key is never sent
//! to some other program that took the port), then send `people.*` commands. Changes go straight
//! into MailCraft's contacts, and MailCraft shows them at once. Nothing is copied to disk here.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;

pub const DEFAULT_PORT: u16 = 7991;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Labeled {
    pub label: String,
    pub value: String,
}

/// One contact as MailCraft gives it (`people.get`); the fields MessageCraft shows and edits.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Contact {
    pub id: i64,
    pub name: String,
    pub first: String,
    pub last: String,
    pub display_name: String,
    pub company: String,
    pub title: String,
    pub emails: Vec<Labeled>,
    pub phones: Vec<Labeled>,
    pub notes: String,
    pub favorite: bool,
    pub kept_at: Value,
}

impl Contact {
    pub fn label(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let n = format!("{} {}", self.first, self.last).trim().to_string();
        if !n.is_empty() {
            n
        } else if !self.company.is_empty() {
            self.company.clone()
        } else {
            self.emails.first().map(|e| e.value.clone()).unwrap_or_default()
        }
    }

    /// What `people.new` / `people.edit` take.
    pub fn to_params(&self) -> Value {
        let clean = |v: &[Labeled]| -> Vec<Value> {
            v.iter().filter(|l| !l.value.trim().is_empty()).map(|l| json!({"label": if l.label.is_empty() { "other" } else { &l.label }, "value": l.value.trim()})).collect()
        };
        json!({
            "first": self.first.trim(),
            "last": self.last.trim(),
            "company": self.company.trim(),
            "title": self.title.trim(),
            "emails": clean(&self.emails),
            "phones": clean(&self.phones),
            "notes": self.notes,
            "favorite": self.favorite,
        })
    }
}

/// The digits of a phone number that identify it: the last ten (so +1 555… and (555)… match).
pub fn phone_key(number: &str) -> Option<String> {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 7 {
        return None;
    }
    Some(digits[digits.len().saturating_sub(10)..].to_string())
}

fn settings_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default().join("MailCraft")
}

/// The ports MailCraft is listening on for this user, newest key file first.
fn sharing_ports() -> Vec<(u16, PathBuf)> {
    let mut found: Vec<(u16, PathBuf, std::time::SystemTime)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(settings_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(port) = name.strip_prefix("control-key.local-").and_then(|p| p.parse::<u16>().ok()) {
                let when = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                found.push((port, e.path(), when));
            }
        }
    }
    found.sort_by(|a, b| b.2.cmp(&a.2));
    found.into_iter().map(|(p, path, _)| (p, path)).collect()
}

/// Where MailCraft is installed, if it is.
pub fn mailcraft_exe() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for var in ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            let base = PathBuf::from(base);
            candidates.push(base.join("MailCraft").join("mailcraft.exe"));
            candidates.push(base.join("Programs").join("MailCraft").join("mailcraft.exe"));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Whether MailCraft is running at all (with or without sharing).
pub fn mailcraft_running() -> bool {
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/FI", "IMAGENAME eq mailcraft.exe", "/NH"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output().map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("mailcraft.exe")).unwrap_or(false)
}

/// Start MailCraft with contact sharing on (when it isn't running already).
pub fn open_mailcraft_sharing() -> Result<(), String> {
    let exe = mailcraft_exe().ok_or("MailCraft isn't installed on this computer.")?;
    std::process::Command::new(exe).arg("--control").arg(DEFAULT_PORT.to_string()).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct Conn {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next: u64,
}

impl Conn {
    fn open() -> Result<Conn, String> {
        let ports = sharing_ports();
        if ports.is_empty() {
            return Err("off".into());
        }
        let mut last = String::new();
        for (port, key_path) in ports {
            match Conn::open_port(port, &key_path) {
                Ok(c) => return Ok(c),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn open_port(port: u16, key_path: &PathBuf) -> Result<Conn, String> {
        let key = std::fs::read_to_string(key_path).map_err(|e| e.to_string())?.trim().to_string();
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| "off".to_string())?;
        stream.set_read_timeout(Some(Duration::from_secs(20))).ok();
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut c = Conn { stream, reader, next: 1 };
        // MailCraft proves it holds the key before we send it.
        let mut nonce = [0u8; 32];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let nonce = hex(&nonce);
        let r = c.raw(json!({"id": 0, "method": "auth.challenge", "params": {"nonce": nonce}}))?;
        let proof = r.get("result").and_then(|v| v.get("proof")).and_then(|v| v.as_str()).unwrap_or("");
        let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).map_err(|e| e.to_string())?;
        mac.update(format!("mailcraft-control-v1 {nonce}").as_bytes());
        let expected = hex(&mac.finalize().into_bytes());
        if proof.len() != expected.len() || !proof.bytes().zip(expected.bytes()).fold(true, |ok, (a, b)| ok & (a == b)) {
            return Err("Something other than MailCraft answered on MailCraft's port.".into());
        }
        let r = c.raw(json!({"id": 1, "method": "auth", "key": key}))?;
        if r.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            return Err("MailCraft didn't accept the connection.".into());
        }
        Ok(c)
    }

    fn raw(&mut self, msg: Value) -> Result<Value, String> {
        let mut line = msg.to_string();
        line.push('\n');
        self.stream.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        let mut answer = String::new();
        self.reader.read_line(&mut answer).map_err(|e| e.to_string())?;
        if answer.is_empty() {
            return Err("MailCraft closed the connection.".into());
        }
        serde_json::from_str(&answer).map_err(|e| e.to_string())
    }

    fn run(&mut self, command: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let r = self.raw(json!({"id": self.next, "method": "engine.execute", "params": {"command": command, "params": params}}))?;
        if r.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(r.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(r.get("error").and_then(|v| v.as_str()).unwrap_or("MailCraft refused that").to_string())
        }
    }

    fn all_contacts(&mut self) -> Result<Vec<Contact>, String> {
        let list = self.run("people.list", json!({"account": "all", "query": ""}))?;
        let ids: Vec<i64> = list.get("contacts").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|c| c.get("id").and_then(|v| v.as_i64())).collect()).unwrap_or_default();
        let mut out = Vec::with_capacity(ids.len());
        for id in ids.into_iter().take(5000) {
            if let Ok(v) = self.run("people.get", json!({"id": id})) {
                if let Ok(c) = serde_json::from_value::<Contact>(v) {
                    out.push(c);
                }
            }
        }
        out.sort_by_key(|c| c.label().to_lowercase());
        Ok(out)
    }
}

pub enum Request {
    Load,
    Save(Contact),
    Delete(i64),
}

pub enum Reply {
    /// Not shared: MailCraft isn't running with sharing on (or something went wrong: the text).
    Off(Option<String>),
    Loaded(Vec<Contact>),
    Saved(i64),
    Deleted(i64),
    Failed(String),
}

/// A worker thread that talks to MailCraft, so the window never waits on it.
pub struct Link {
    tx: Sender<Request>,
    pub rx: Receiver<Reply>,
}

impl Link {
    pub fn start(ctx: egui::Context) -> Link {
        let (tx, req_rx) = channel::<Request>();
        let (rep_tx, rx) = channel::<Reply>();
        std::thread::spawn(move || {
            let mut conn: Option<Conn> = None;
            for req in req_rx {
                if conn.is_none() {
                    match Conn::open() {
                        Ok(c) => conn = Some(c),
                        Err(e) => {
                            let _ = rep_tx.send(Reply::Off(if e == "off" { None } else { Some(e) }));
                            ctx.request_repaint();
                            continue;
                        }
                    }
                }
                let Some(c) = conn.as_mut() else { continue };
                let reply = match req {
                    Request::Load => c.all_contacts().map(Reply::Loaded),
                    Request::Save(contact) => {
                        let mut p = contact.to_params();
                        let r = if contact.id > 0 {
                            p["id"] = json!(contact.id);
                            c.run("people.edit", p).map(|_| contact.id)
                        } else {
                            c.run("people.new", p).map(|v| v.get("id").and_then(|i| i.as_i64()).unwrap_or(0))
                        };
                        r.map(Reply::Saved)
                    }
                    Request::Delete(id) => c.run("people.delete", json!({"id": id})).map(|_| Reply::Deleted(id)),
                };
                let reply = match reply {
                    Ok(r) => r,
                    Err(e) => {
                        // A dropped connection (MailCraft closed): try afresh next time.
                        if e.contains("closed") || e.contains("os error") || e.contains("timed out") {
                            conn = None;
                            Reply::Off(None)
                        } else {
                            Reply::Failed(e)
                        }
                    }
                };
                let _ = rep_tx.send(reply);
                ctx.request_repaint();
            }
        });
        Link { tx, rx }
    }

    pub fn send(&self, r: Request) {
        let _ = self.tx.send(r);
    }
}
