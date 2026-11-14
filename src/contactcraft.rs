//! Contacts from ContactCraft (github.com/william-pyle/ContactCraft): its shared contact store,
//! `%APPDATA%\ContactCraft\contacts.db`, read through ContactCraft's own `contactcraft-store`
//! crate. Nothing has to be running. A worker thread owns the store (it isn't `Sync`); the window
//! asks through a channel and never waits on the disk.
//!
//! Contacts are read-only here. Adding or changing one starts ContactCraft's own editor window
//! (`ContactCraft.exe --edit-phone <number>`, `--edit-contact <id>`, `--new-contact`), which
//! saves into the same store; MessageCraft sees the change through the store's revision.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub use contactcraft_store::Contact;
use contactcraft_store::Store;

/// What the worker sends back.
pub enum Reply {
    /// The store couldn't be opened (with why).
    Unavailable(String),
    /// Everything, after opening or after a change elsewhere. `dir` is the store's folder.
    All { contacts: Vec<Paths>, exe: Option<PathBuf> },
    /// ContactCraft's editor window closed.
    Edited(EditOutcome),
}

/// A contact with the files its picture and theme use, resolved by the store.
#[derive(Clone)]
pub struct Paths {
    pub contact: Contact,
    pub photo: Option<PathBuf>,
    pub background: Option<PathBuf>,
    pub photo_image: Option<egui::ColorImage>,
    pub background_image: Option<egui::ColorImage>,
}

pub enum EditOutcome {
    Saved(String),
    Cancelled,
    Failed(String),
}

pub enum Request {
    /// Look at the store again now (it checks the revision every two seconds anyway).
    Refresh,
    /// Open ContactCraft's editor with these arguments; the answer is `Edited`.
    Edit(Vec<String>),
}

pub struct Link {
    tx: Sender<Request>,
    pub rx: Receiver<Reply>,
}

fn decode(path: &PathBuf) -> Option<egui::ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.thumbnail(1600, 1600).to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize], image.as_raw(),
    ))
}

fn load(store: &Store) -> Result<Vec<Paths>, String> {
    Ok(store
        .list()?
        .into_iter()
        .map(|c| {
            let photo = store.photo_path(&c);
            let background = store.background_path(&c);
            Paths {
                photo_image: photo.as_ref().and_then(decode),
                background_image: background.as_ref().and_then(decode),
                photo, background, contact: c,
            }
        })
        .collect())
}

impl Link {
    pub fn start(ctx: egui::Context) -> Link {
        let (tx, req_rx) = channel::<Request>();
        let (rep_tx, rx) = channel::<Reply>();
        std::thread::spawn(move || {
            let send = |r: Reply| {
                let _ = rep_tx.send(r);
                ctx.request_repaint();
            };
            let mut store: Option<Store> = None;
            let mut revision = i64::MIN;
            let mut next_try = Instant::now();
            loop {
                if store.is_none() && Instant::now() >= next_try {
                    match Store::open_default() {
                        Ok(s) => store = Some(s),
                        Err(e) => {
                            send(Reply::Unavailable(e));
                            next_try = Instant::now() + Duration::from_secs(30);
                        }
                    }
                }
                if let Some(s) = &store {
                    // Changes from any Craft app move the revision.
                    let r = s.revision();
                    if r != revision {
                        revision = r;
                        match load(s) {
                            Ok(contacts) => send(Reply::All { contacts, exe: s.contactcraft_exe() }),
                            Err(e) => send(Reply::Unavailable(e)),
                        }
                    }
                }
                match req_rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(Request::Refresh) => revision = i64::MIN,
                    Ok(Request::Edit(args)) => {
                        let exe = store.as_ref().and_then(|s| s.contactcraft_exe());
                        let Some(exe) = exe else {
                            send(Reply::Edited(EditOutcome::Failed("ContactCraft isn't installed on this computer.".into())));
                            continue;
                        };
                        // Its editor runs in its own thread so the store keeps being watched.
                        let tx = rep_tx.clone();
                        let ctx = ctx.clone();
                        std::thread::spawn(move || {
                            let outcome = run_editor(&exe, &args);
                            let _ = tx.send(Reply::Edited(outcome));
                            ctx.request_repaint();
                        });
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        });
        Link { tx, rx }
    }

    pub fn send(&self, r: Request) {
        let _ = self.tx.send(r);
    }
}

fn run_editor(exe: &PathBuf, args: &[String]) -> EditOutcome {
    let out = std::process::Command::new(exe).args(args).stdin(std::process::Stdio::null()).output();
    match out {
        Err(e) => EditOutcome::Failed(format!("ContactCraft couldn't start: {e}")),
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            match out.status.code() {
                Some(0) => EditOutcome::Saved(text.strip_prefix("saved ").unwrap_or("").trim().to_string()),
                Some(1) => EditOutcome::Cancelled,
                _ => EditOutcome::Failed(text.strip_prefix("error ").map(str::to_string).unwrap_or_else(|| "ContactCraft couldn't open that contact.".into())),
            }
        }
    }
}

pub fn phone_key(number: &str) -> Option<String> {
    contactcraft_store::phone_key(number)
}

/// `#RRGGBB` to a colour; empty or malformed means "MessageCraft's own" (None).
pub fn color(s: &str) -> Option<egui::Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}
