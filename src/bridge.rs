//! The connection helper: `messagecraft-bridge.exe` beside MessageCraft.exe does the talking to
//! Google Messages for web. This side starts it, sends it commands and reads its events, one JSON
//! object per line each way.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

pub struct Bridge {
    child: Option<Child>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    pub rx: Receiver<Value>,
    tx: Sender<Value>,
    pub data_dir: PathBuf,
    pub running: bool,
}

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("MessageCraft");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn helper_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    dir.join("messagecraft-bridge.exe")
}

impl Bridge {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Bridge { child: None, stdin: None, rx, tx, data_dir: data_dir(), running: false }
    }

    /// Start the helper; events arrive on `rx` and wake the window.
    pub fn start(&mut self, ctx: &egui::Context) {
        self.stop();
        let path = helper_path();
        let mut cmd = Command::new(&path);
        cmd.arg("--data").arg(&self.data_dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        match cmd.spawn() {
            Ok(mut child) => {
                let stdout = child.stdout.take();
                self.stdin = child.stdin.take().map(|s| Arc::new(Mutex::new(s)));
                self.child = Some(child);
                self.running = true;
                if let Some(stdout) = stdout {
                    let tx = self.tx.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            let Ok(line) = line else { break };
                            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                                if tx.send(v).is_err() {
                                    return;
                                }
                                ctx.request_repaint();
                            }
                        }
                        let _ = tx.send(json!({"ev": "bridge_exit"}));
                        ctx.request_repaint();
                    });
                }
            }
            Err(e) => {
                self.running = false;
                let _ = self.tx.send(json!({
                    "ev": "bridge_missing",
                    "message": format!("MessageCraft's connection helper couldn't start ({}): {e}", path.display()),
                }));
            }
        }
    }

    /// Feed an event in as if the helper had sent it (used by the Google sign-in window).
    pub fn sender(&self) -> Sender<Value> {
        self.tx.clone()
    }

    pub fn send(&self, cmd: Value) {
        let Some(stdin) = &self.stdin else { return };
        let stdin = stdin.clone();
        // Writes are tiny; a thread keeps a stuck pipe from ever holding the window.
        std::thread::spawn(move || {
            if let Ok(mut s) = stdin.lock() {
                let mut line = cmd.to_string();
                line.push('\n');
                let _ = s.write_all(line.as_bytes());
                let _ = s.flush();
            }
        });
    }

    pub fn stop(&mut self) {
        self.stdin = None; // closing stdin asks the helper to shut down
        if let Some(mut child) = self.child.take() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(50)),
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        self.running = false;
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop();
    }
}
