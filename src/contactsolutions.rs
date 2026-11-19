//! Contacts from ContactSolutions (github.com/william-pyle/ContactSolutions): its shared contact store,
//! `%APPDATA%\ContactSolutions\contacts.db`, read through ContactSolutions's own `contactsolutions-store`
//! crate. Nothing has to be running. A worker thread owns the store (it isn't `Sync`); the window
//! asks through a channel and never waits on the disk.
//!
//! Contacts are read-only here. Adding or changing one starts ContactSolutions's own editor window
//! (`ContactSolutions.exe --edit-phone <number>`, `--edit-contact <id>`, `--new-contact`), which
//! saves into the same store; MessageSolutions sees the change through the store's revision.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub use contactsolutions_store::{Contact, ContactGroup};
use contactsolutions_store::Store;

/// What the worker sends back.
pub enum Reply {
    /// The store couldn't be opened (with why).
    Unavailable(String),
    /// Everything, after opening or after a change elsewhere. `dir` is the store's folder.
    All { contacts: Vec<Paths>, groups: Vec<GroupPaths>, exe: Option<PathBuf> },
    /// ContactSolutions's editor window closed.
    Edited(EditOutcome),
}

/// A contact with the files its picture and theme use, resolved by the store.
#[derive(Clone)]
pub struct Paths {
    pub contact: Contact,
    pub photo: Option<PathBuf>,
    pub background: Option<PathBuf>,
    pub banner: Option<PathBuf>,
    pub photo_image: Option<egui::ColorImage>,
    pub background_image: Option<egui::ColorImage>,
    pub banner_image: Option<egui::ColorImage>,
}

#[derive(Clone)]
pub struct GroupPaths {
    pub group: ContactGroup,
    pub background: Option<PathBuf>,
    pub banner: Option<PathBuf>,
    pub background_image: Option<egui::ColorImage>,
    pub banner_image: Option<egui::ColorImage>,
}

pub enum EditOutcome {
    Saved(String),
    SavedGroup,
    Cancelled,
    Failed(String),
}

pub enum Request {
    /// Look at the store again now (it checks the revision every two seconds anyway).
    Refresh,
    /// Open ContactSolutions's editor with these arguments; the answer is `Edited`.
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

fn load(store: &Store) -> Result<(Vec<Paths>, Vec<GroupPaths>), String> {
    let contacts = store
        .list()?
        .into_iter()
        .map(|c| {
            let photo = store.photo_path(&c);
            let background = store.background_path(&c);
            let banner = store.banner_path(&c.theme);
            Paths {
                photo_image: photo.as_ref().and_then(decode),
                background_image: background.as_ref().and_then(decode),
                banner_image: banner.as_ref().and_then(decode),
                photo, background, banner, contact: c,
            }
        })
        .collect();
    let groups = store.list_groups()?.into_iter().map(|group| {
        let background = store.theme_background_path(&group.theme);
        let banner = store.banner_path(&group.theme);
        GroupPaths {
            background_image: background.as_ref().and_then(decode),
            banner_image: banner.as_ref().and_then(decode),
            group, background, banner,
        }
    }).collect();
    Ok((contacts, groups))
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
                            Ok((contacts, groups)) => send(Reply::All { contacts, groups, exe: s.contactsolutions_exe() }),
                            Err(e) => send(Reply::Unavailable(e)),
                        }
                    }
                }
                match req_rx.recv_timeout(Duration::from_secs(2)) {
                    Ok(Request::Refresh) => revision = i64::MIN,
                    Ok(Request::Edit(args)) => {
                        let exe = store.as_ref().and_then(|s| s.contactsolutions_exe());
                        let Some(exe) = exe else {
                            send(Reply::Edited(EditOutcome::Failed("ContactSolutions isn't installed on this computer.".into())));
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
    let group = args.iter().any(|arg| arg == "--new-group" || arg == "--open-group");
    let out = std::process::Command::new(exe).args(args).stdin(std::process::Stdio::null()).output();
    match out {
        Err(e) => EditOutcome::Failed(format!("ContactSolutions couldn't start: {e}")),
        Ok(out) => editor_result(group, out.status.code(), &String::from_utf8_lossy(&out.stdout)),
    }
}

fn editor_result(group: bool, code: Option<i32>, output: &str) -> EditOutcome {
    let text = output.trim();
    match code {
        Some(0) if group => EditOutcome::SavedGroup,
        Some(0) => EditOutcome::Saved(text.strip_prefix("saved ").unwrap_or("").trim().to_string()),
        Some(1) => EditOutcome::Cancelled,
        _ => EditOutcome::Failed(text.strip_prefix("error ").map(str::to_string).unwrap_or_else(|| format!("ContactSolutions couldn't open that {}.", if group { "group" } else { "contact" }))),
    }
}

pub fn phone_key(number: &str) -> Option<String> {
    contactsolutions_store::phone_key(number)
}

/// `#RRGGBB` to a colour; empty or malformed means "MessageSolutions's own" (None).
pub fn color(s: &str) -> Option<egui::Color32> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(egui::Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    #[test]
    fn released_store_preserves_contacts_groups_media_discovery_and_revisions() {
        let dir=std::env::temp_dir().join(format!("messagesolutions-contact-contract-{}",contactsolutions_store::new_id()));
        let writer=Store::open(&dir).unwrap();
        let mut c=Contact::new(); c.name.given="Synthetic".into();
        c.phones.push(contactsolutions_store::Labeled::new("mobile","+1 202 555 0100"));
        c.extra.insert("original-id-note".into(),serde_json::json!("keep"));
        writer.save(&mut c).unwrap();
        let mut group=ContactGroup::new("Synthetic group"); group.add_contact(&c.id);
        writer.set_banner(&mut group.theme,&group.id,b"\x89PNGsynthetic").unwrap(); writer.save_group(&mut group).unwrap();
        let exe=dir.join("ContactSolutions.exe"); std::fs::write(&exe,b"synthetic discovery only, never executed").unwrap();
        writer.set_meta("contactsolutions.exe",Some(exe.to_str().unwrap())).unwrap();
        let reader=Store::open(&dir).unwrap(); let (contacts,groups)=load(&reader).unwrap();
        assert_eq!(contacts[0].contact.id,c.id); assert_eq!(groups[0].group.id,group.id);
        assert_eq!(groups[0].group.members,group.members); assert!(groups[0].banner.as_ref().unwrap().exists());
        assert_eq!(reader.contactsolutions_exe(),Some(exe));
        assert_eq!(reader.find_by_phone("202-555-0100").unwrap().unwrap().id,c.id);
        let before=reader.revision(); c.name.given="Updated fixture".into(); writer.save(&mut c).unwrap();
        assert!(reader.revision()>before); assert_eq!(load(&reader).unwrap().0[0].contact.name.given,"Updated fixture");
        drop(reader); drop(writer); std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn published_editor_exit_protocol_is_preserved() {
        assert!(matches!(editor_result(false,Some(0),"saved stable-contact\r\n"),EditOutcome::Saved(id) if id=="stable-contact"));
        assert!(matches!(editor_result(true,Some(0),"saved stable-group"),EditOutcome::SavedGroup));
        assert!(matches!(editor_result(false,Some(1),""),EditOutcome::Cancelled));
        assert!(matches!(editor_result(false,Some(2),"error synthetic failure"),EditOutcome::Failed(e) if e=="synthetic failure"));
    }
}
