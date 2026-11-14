//! The store: one SQLite database (`contacts.db`) and a `photos` folder, in
//! `%APPDATA%\ContactCraft` (or the folder `CONTACTCRAFT_DATA` names).
//!
//! SQLite in WAL mode lets several programs read and write at once, so ContactCraft, MessageCraft
//! and MailCraft can all open it, whether ContactCraft is running or not. Every change bumps the
//! store's `revision`; a program that keeps contacts in memory checks [`Store::revision`] now and
//! then and reloads when it moved.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::model::{Contact, phone_key};

pub type Result<T> = std::result::Result<T, String>;

const SCHEMA_VERSION: i64 = 1;

pub struct Store {
    conn: Connection,
    dir: PathBuf,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// A new random id: 32 hex digits.
pub fn new_id() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl Store {
    /// Where the shared store lives: `CONTACTCRAFT_DATA`, else `%APPDATA%\ContactCraft`.
    pub fn default_dir() -> PathBuf {
        if let Some(d) = std::env::var_os("CONTACTCRAFT_DATA") {
            return PathBuf::from(d);
        }
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("ContactCraft")
    }

    /// Open (and create, the first time) the shared store.
    pub fn open_default() -> Result<Store> {
        Store::open(&Store::default_dir())
    }

    pub fn open(dir: &Path) -> Result<Store> {
        std::fs::create_dir_all(dir.join("photos")).map_err(err)?;
        let conn = Connection::open(dir.join("contacts.db")).map_err(err)?;
        conn.busy_timeout(std::time::Duration::from_secs(8)).map_err(err)?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(err)?;
        conn.pragma_update(None, "synchronous", "NORMAL").map_err(err)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS contacts (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL DEFAULT '',
                sort_key TEXT NOT NULL DEFAULT '',
                emails TEXT NOT NULL DEFAULT '',
                phones TEXT NOT NULL DEFAULT '',
                favorite INTEGER NOT NULL DEFAULT 0,
                photo TEXT NOT NULL DEFAULT '',
                theme_accent TEXT NOT NULL DEFAULT '',
                google_resource TEXT NOT NULL DEFAULT '',
                dirty INTEGER NOT NULL DEFAULT 1,
                deleted INTEGER NOT NULL DEFAULT 0,
                rev INTEGER NOT NULL DEFAULT 0,
                created TEXT NOT NULL DEFAULT '',
                updated TEXT NOT NULL DEFAULT '',
                data TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS contacts_sort ON contacts(sort_key);
            CREATE INDEX IF NOT EXISTS contacts_google ON contacts(google_resource);
            CREATE INDEX IF NOT EXISTS contacts_rev ON contacts(rev);
            CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .map_err(err)?;
        conn.execute("INSERT OR IGNORE INTO meta(key, value) VALUES ('schema', ?1)", params![SCHEMA_VERSION.to_string()]).map_err(err)?;
        conn.execute("INSERT OR IGNORE INTO meta(key, value) VALUES ('revision', '0')", []).map_err(err)?;
        Ok(Store { conn, dir: dir.to_path_buf() })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn photos_dir(&self) -> PathBuf {
        self.dir.join("photos")
    }

    /// The store's change counter: it moves whenever any program changes a contact.
    pub fn revision(&self) -> i64 {
        self.conn
            .query_row("SELECT value FROM meta WHERE key='revision'", [], |r| r.get::<_, String>(0))
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    }

    fn rows(&self, sql: &str, p: impl rusqlite::Params) -> Result<Vec<Contact>> {
        let mut st = self.conn.prepare(sql).map_err(err)?;
        let it = st.query_map(p, |r| r.get::<_, String>(0)).map_err(err)?;
        let mut out = Vec::new();
        for row in it {
            let json = row.map_err(err)?;
            if let Ok(c) = serde_json::from_str::<Contact>(&json) {
                out.push(c);
            }
        }
        Ok(out)
    }

    /// Every contact, sorted the way they file.
    pub fn list(&self) -> Result<Vec<Contact>> {
        self.rows("SELECT data FROM contacts WHERE deleted=0 ORDER BY sort_key COLLATE NOCASE", [])
    }

    pub fn get(&self, id: &str) -> Result<Option<Contact>> {
        let json: Option<String> = self.conn.query_row("SELECT data FROM contacts WHERE id=?1 AND deleted=0", params![id], |r| r.get(0)).optional().map_err(err)?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }

    /// The contact with this phone number (any format: the last ten digits are compared).
    pub fn find_by_phone(&self, number: &str) -> Result<Option<Contact>> {
        let Some(k) = phone_key(number) else { return Ok(None) };
        Ok(self.rows("SELECT data FROM contacts WHERE deleted=0 AND phones LIKE ?1 ORDER BY favorite DESC LIMIT 1", params![format!("% {k} %")])?.into_iter().next())
    }

    /// The contact with this email address (case doesn't matter).
    pub fn find_by_email(&self, email: &str) -> Result<Option<Contact>> {
        let e = email.trim().to_lowercase();
        if e.is_empty() {
            return Ok(None);
        }
        Ok(self.rows("SELECT data FROM contacts WHERE deleted=0 AND emails LIKE ?1 ORDER BY favorite DESC LIMIT 1", params![format!("% {e} %")])?.into_iter().next())
    }

    /// Contacts whose name, company, emails, phones, address, labels or notes hold `query`.
    pub fn search(&self, query: &str) -> Result<Vec<Contact>> {
        let q = query.trim().to_lowercase();
        let all = self.list()?;
        if q.is_empty() {
            return Ok(all);
        }
        Ok(all.into_iter().filter(|c| c.search_text().contains(&q)).collect())
    }

    /// Contacts changed after `revision` (deleted ones too, with their `deleted` flag in the
    /// returned pair), for programs that keep a copy in memory.
    pub fn changes_since(&self, revision: i64) -> Result<Vec<(Contact, bool)>> {
        let mut st = self.conn.prepare("SELECT data, deleted FROM contacts WHERE rev>?1 ORDER BY rev").map_err(err)?;
        let it = st.query_map(params![revision], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))).map_err(err)?;
        let mut out = Vec::new();
        for row in it {
            let (json, deleted) = row.map_err(err)?;
            if let Ok(c) = serde_json::from_str::<Contact>(&json) {
                out.push((c, deleted));
            }
        }
        Ok(out)
    }

    /// Every label in use, sorted.
    pub fn labels(&self) -> Result<Vec<String>> {
        let mut v: Vec<String> = self.list()?.into_iter().flat_map(|c| c.labels).collect();
        v.sort_by_key(|l| l.to_lowercase());
        v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        Ok(v)
    }

    fn write(&self, c: &Contact, dirty: bool, deleted: bool) -> Result<()> {
        let json = serde_json::to_string(c).map_err(err)?;
        let wrap = |v: Vec<String>| if v.is_empty() { String::new() } else { format!(" {} ", v.join(" ")) };
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        tx.execute("UPDATE meta SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT) WHERE key='revision'", []).map_err(err)?;
        let rev: i64 = tx.query_row("SELECT CAST(value AS INTEGER) FROM meta WHERE key='revision'", [], |r| r.get(0)).map_err(err)?;
        tx.execute(
            "INSERT INTO contacts(id, display_name, sort_key, emails, phones, favorite, photo, theme_accent, google_resource, dirty, deleted, rev, created, updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET display_name=?2, sort_key=?3, emails=?4, phones=?5, favorite=?6, photo=?7, theme_accent=?8,
               google_resource=?9, dirty=?10, deleted=?11, rev=?12, created=?13, updated=?14, data=?15",
            params![
                c.id,
                c.display_name(),
                c.sort_key(),
                wrap(c.email_keys()),
                wrap(c.phone_keys()),
                c.favorite as i64,
                c.photo,
                c.theme.accent,
                c.google.as_ref().map(|g| g.resource_name.clone()).unwrap_or_default(),
                dirty as i64,
                deleted as i64,
                rev,
                c.created,
                c.updated,
                json
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }

    /// Save a contact (new or changed). Gives a new one its id, and stamps the times. It's marked
    /// as changed, so the next Google sync sends it.
    pub fn save(&self, c: &mut Contact) -> Result<()> {
        let stamp = now();
        if c.id.is_empty() {
            c.id = new_id();
        }
        if c.created.is_empty() {
            c.created = stamp.clone();
        }
        if c.kind.is_empty() {
            c.kind = "individual".into();
        }
        c.updated = stamp;
        self.write(c, true, false)
    }

    /// Save what a sync brought in, without marking it to be sent back.
    pub fn save_synced(&self, c: &mut Contact) -> Result<()> {
        if c.id.is_empty() {
            c.id = new_id();
        }
        if c.created.is_empty() {
            c.created = now();
        }
        if c.updated.is_empty() {
            c.updated = now();
        }
        self.write(c, false, false)
    }

    /// Delete a contact. One that is in Google Contacts is kept as a marker until the next sync
    /// deletes it there too.
    pub fn delete(&self, id: &str) -> Result<()> {
        let Some(mut c) = self.get(id)? else { return Ok(()) };
        self.remove_photo_file(&c.photo);
        c.photo.clear();
        if c.google.as_ref().is_some_and(|g| !g.resource_name.is_empty()) {
            c.updated = now();
            self.write(&c, true, true)
        } else {
            self.purge(id)
        }
    }

    /// Remove a contact's row for good (no marker).
    pub fn purge(&self, id: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        tx.execute("UPDATE meta SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT) WHERE key='revision'", []).map_err(err)?;
        tx.execute("DELETE FROM contacts WHERE id=?1", params![id]).map_err(err)?;
        tx.commit().map_err(err)
    }

    /// Contacts changed here since they were last synced (deleted ones are in [`Store::deleted`]).
    pub fn changed(&self) -> Result<Vec<Contact>> {
        self.rows("SELECT data FROM contacts WHERE dirty=1 AND deleted=0", [])
    }

    /// Deleted contacts still waiting to be deleted in Google Contacts.
    pub fn deleted(&self) -> Result<Vec<Contact>> {
        self.rows("SELECT data FROM contacts WHERE deleted=1", [])
    }

    pub fn by_google_resource(&self, resource: &str) -> Result<Option<Contact>> {
        Ok(self.rows("SELECT data FROM contacts WHERE google_resource=?1 LIMIT 1", params![resource])?.into_iter().next())
    }

    /// Whether the row is marked changed-since-sync.
    pub fn is_dirty(&self, id: &str) -> bool {
        self.conn.query_row("SELECT dirty FROM contacts WHERE id=?1", params![id], |r| r.get::<_, i64>(0)).map(|d| d != 0).unwrap_or(false)
    }

    /// A value kept for a program (sync tokens and the like). Keys are `<app>.<name>`.
    pub fn meta(&self, key: &str) -> Option<String> {
        self.conn.query_row("SELECT value FROM meta WHERE key=?1", params![key], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set_meta(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.conn.execute("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=?2", params![key, v]).map(|_| ()).map_err(err),
            None => self.conn.execute("DELETE FROM meta WHERE key=?1", params![key]).map(|_| ()).map_err(err),
        }
    }

    /// Where ContactCraft.exe is, so another app can start it to create or edit a contact
    /// (`--new-contact`, `--edit-contact <id>`, `--edit-phone <number>`...). ContactCraft
    /// records its own location each time it starts; the standard install folder is tried too.
    pub fn contactcraft_exe(&self) -> Option<PathBuf> {
        if let Some(p) = self.meta("contactcraft.exe").map(PathBuf::from).filter(|p| p.is_file()) {
            return Some(p);
        }
        ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"]
            .iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .flat_map(|b| [b.join("ContactCraft").join("ContactCraft.exe"), b.join("Programs").join("ContactCraft").join("ContactCraft.exe")])
            .find(|p| p.is_file())
    }

    // --- photos ------------------------------------------------------------------------------

    /// The full path of a contact's picture, when it has one.
    pub fn photo_path(&self, c: &Contact) -> Option<PathBuf> {
        if c.photo.is_empty() || c.photo.contains(['/', '\\']) || c.photo.contains("..") {
            return None;
        }
        let p = self.photos_dir().join(&c.photo);
        p.is_file().then_some(p)
    }

    /// Store a picture for a contact (PNG, JPEG, GIF or WebP bytes) and point the contact at it.
    /// The contact itself still has to be saved.
    pub fn set_photo(&self, c: &mut Contact, bytes: &[u8]) -> Result<()> {
        let ext = sniff_image(bytes).ok_or("That file isn't a PNG, JPEG, GIF or WebP picture.")?;
        if c.id.is_empty() {
            c.id = new_id();
        }
        let hash: String = Sha256::digest(bytes).iter().take(6).map(|b| format!("{b:02x}")).collect();
        let name = format!("{}-{hash}.{ext}", c.id);
        std::fs::write(self.photos_dir().join(&name), bytes).map_err(err)?;
        if c.photo != name {
            self.remove_photo_file(&c.photo);
        }
        c.photo = name;
        Ok(())
    }

    pub fn clear_photo(&self, c: &mut Contact) {
        self.remove_photo_file(&c.photo);
        c.photo.clear();
    }

    /// Where a theme's files live: `backgrounds` for pictures, `fonts` for font files the person
    /// added (Windows' own fonts stay in `C:\Windows\Fonts`).
    pub fn theme_dir(&self, kind: &str) -> PathBuf {
        let d = self.dir.join(if kind == "fonts" { "fonts" } else { "backgrounds" });
        let _ = std::fs::create_dir_all(&d);
        d
    }

    /// The full path of the contact's background picture, when it has one.
    pub fn background_path(&self, c: &Contact) -> Option<PathBuf> {
        let n = &c.theme.background_image;
        if n.is_empty() || n.contains(['/', '\\']) || n.contains("..") {
            return None;
        }
        let p = self.theme_dir("backgrounds").join(n);
        p.is_file().then_some(p)
    }

    /// The full path of the contact's font file: one added to the store, else Windows' own.
    pub fn font_path(&self, c: &Contact) -> Option<PathBuf> {
        let n = &c.theme.font_file;
        if n.is_empty() || n.contains(['/', '\\']) || n.contains("..") {
            return None;
        }
        let own = self.theme_dir("fonts").join(n);
        if own.is_file() {
            return Some(own);
        }
        let win = std::env::var_os("WINDIR").map(PathBuf::from).unwrap_or_else(|| "C:\\Windows".into()).join("Fonts").join(n);
        win.is_file().then_some(win)
    }

    /// Store a background picture for a contact's theme (the contact still has to be saved).
    pub fn set_background(&self, c: &mut Contact, bytes: &[u8]) -> Result<()> {
        let ext = sniff_image(bytes).ok_or("That file isn't a PNG, JPEG, GIF or WebP picture.")?;
        if c.id.is_empty() {
            c.id = new_id();
        }
        let hash: String = Sha256::digest(bytes).iter().take(6).map(|b| format!("{b:02x}")).collect();
        let name = format!("{}-{hash}.{ext}", c.id);
        std::fs::write(self.theme_dir("backgrounds").join(&name), bytes).map_err(err)?;
        c.theme.background_image = name;
        Ok(())
    }

    /// Copy a font file (.ttf, .otf or .ttc) into the store so every Craft app can use it, and
    /// point the contact's theme at it.
    pub fn add_font(&self, c: &mut Contact, file: &Path) -> Result<()> {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc") {
            return Err("Pick a font file (.ttf, .otf or .ttc).".into());
        }
        let name = file.file_name().and_then(|n| n.to_str()).ok_or("That file has no name.")?.to_string();
        std::fs::copy(file, self.theme_dir("fonts").join(&name)).map_err(err)?;
        c.theme.font_file = name;
        if c.theme.font.is_empty() {
            c.theme.font = file.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        }
        Ok(())
    }

    fn remove_photo_file(&self, name: &str) {
        if !name.is_empty() && !name.contains(['/', '\\']) && !name.contains("..") {
            let _ = std::fs::remove_file(self.photos_dir().join(name));
        }
    }
}

/// The picture format from its first bytes.
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.starts_with(b"GIF8") {
        Some("gif")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}
