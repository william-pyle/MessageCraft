//! One-time, offline migration. Legacy names here identify existing data, not supported old apps.
//! The source remains untouched. A receipt detects later writes to the retained source.
use std::{collections::BTreeMap, fs::{self, File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, String>;
const RECEIPT: &str = "migration-receipt.json";
const OLD_SETTINGS: &str = "contactcraft-ui.json";
pub const SETTINGS_FILE: &str = "contactsolutions-ui.json";

#[derive(Serialize, Deserialize)]
struct Receipt {
    version: u32,
    source: PathBuf,
    files: BTreeMap<String, String>,
}

fn error(e: impl std::fmt::Display) -> String { e.to_string() }

fn inventory(dir: &Path, relative: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir.join(relative)).map_err(error)? {
        let entry = entry.map_err(error)?;
        let kind = entry.file_type().map_err(error)?;
        if kind.is_symlink() { return Err("Migration refuses symbolic links or junctions; originals are unchanged.".into()); }
        #[cfg(windows)] {
            use std::os::windows::fs::MetadataExt;
            if entry.metadata().map_err(error)?.file_attributes() & 0x400 != 0 {
                return Err("Migration refuses reparse points; originals are unchanged.".into());
            }
        }
        let path = relative.join(entry.file_name());
        if kind.is_dir() { inventory(dir, &path, out)?; }
        else if kind.is_file() { out.push(path); }
        else { return Err("Migration found an unsupported file type.".into()); }
    }
    out.sort();
    Ok(())
}

// Hold all source files against writes/deletion for the entire snapshot on Windows.
fn snapshot(source: &Path) -> Result<(Vec<File>, BTreeMap<String, Vec<u8>>)> {
    let mut paths = Vec::new();
    inventory(source, Path::new(""), &mut paths)?;
    let mut handles = Vec::new();
    for path in &paths {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)] { use std::os::windows::fs::OpenOptionsExt; options.share_mode(0); }
        handles.push(options.open(source.join(path)).map_err(|_| "Existing data is open or inaccessible. Close its consumers yourself before migration; nothing was moved.".to_string())?);
    }
    let mut files = BTreeMap::new();
    for (path, file) in paths.iter().zip(handles.iter_mut()) {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(error)?;
        files.insert(path.to_str().ok_or("Migration requires Unicode filenames")?.to_string(), bytes);
    }
    let mut after = Vec::new();
    inventory(source, Path::new(""), &mut after)?;
    if paths != after { return Err("Source changed during migration. Retry only when its consumers are closed.".into()); }
    Ok((handles, files))
}

fn hashes(files: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, String> {
    files.iter().map(|(path, bytes)| (path.clone(), format!("{:x}", Sha256::digest(bytes)))).collect()
}

struct Lock(PathBuf);
impl Drop for Lock { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }

/// Copy legacy data into a sibling staging directory and publish only after validation.
/// A failed/interrupted staging directory is retained for explicit recovery, never overwritten.
pub fn migrate_directory(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(not(windows))]
    return Err("Offline migration is supported only on Windows, where source files can be held exclusively.".into());
    let parent = destination.parent().ok_or("Migration destination needs a parent")?;
    fs::create_dir_all(parent).map_err(error)?;
    let name = destination.file_name().ok_or("Migration destination needs a name")?.to_string_lossy();
    let staging = parent.join(format!("{name}.migration-staging"));
    let lock_path = parent.join(format!("{name}.migration-lock"));
    let lock_file = OpenOptions::new().write(true).create_new(true).open(&lock_path)
        .map_err(|_| "Migration is already running or an interrupted migration needs recovery (migration-lock).".to_string())?;
    let _lock = Lock(lock_path);
    drop(lock_file);
    if staging.exists() { return Err("An interrupted migration-staging folder exists. Preserve it and the original; follow the recovery guide before retrying.".into()); }
    if !source.is_dir() { return Err("The migration source directory does not exist.".into()); }
    let source_metadata = fs::symlink_metadata(source).map_err(error)?;
    if source_metadata.file_type().is_symlink() { return Err("Migration refuses a linked source directory.".into()); }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if source_metadata.file_attributes() & 0x400 != 0 { return Err("Migration refuses a reparse-point source directory.".into()); }
    }
    let canonical_source = source.canonicalize().map_err(error)?;
    let canonical_destination = if destination.exists() { destination.canonicalize().map_err(error)? }
        else { parent.canonicalize().map_err(error)?.join(destination.file_name().unwrap()) };
    if canonical_destination.starts_with(&canonical_source) || canonical_source.starts_with(&canonical_destination) {
        return Err("Migration source and destination must be separate, non-nested folders.".into());
    }
    let (_handles, files) = snapshot(source)?;
    let fingerprint = hashes(&files);
    if destination.exists() {
        let receipt: Receipt = serde_json::from_slice(&fs::read(destination.join(RECEIPT))
            .map_err(|_| "Both old and new data folders exist without a migration receipt. Neither was changed; resolve the collision explicitly.".to_string())?).map_err(error)?;
        if receipt.version != 1 || receipt.source != canonical_source || receipt.files != fingerprint {
            return Err("The retained source changed or the migration receipt does not match. Neither store was merged; preserve both and resolve the conflict.".into());
        }
        return Ok(());
    }
    if files.contains_key(RECEIPT) { return Err("Source already contains a migration receipt; resolve the migration history explicitly.".into()); }
    if files.contains_key(OLD_SETTINGS) && files.contains_key(SETTINGS_FILE) {
        return Err("Both settings filenames exist in the source. Resolve this collision before migration.".into());
    }
    if let Some(bytes) = files.get(OLD_SETTINGS).or_else(|| files.get(SETTINGS_FILE)) {
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| format!("Existing settings are invalid JSON: {e}"))?;
        if !value.is_object() { return Err("Existing settings must be a JSON object.".into()); }
    }
    fs::create_dir(&staging).map_err(error)?;
    for (path, bytes) in &files {
        let target = staging.join(if path == OLD_SETTINGS { SETTINGS_FILE } else { path });
        fs::create_dir_all(target.parent().unwrap()).map_err(error)?;
        let mut output = OpenOptions::new().create_new(true).write(true).open(&target).map_err(error)?;
        output.write_all(bytes).map_err(error)?;
        output.sync_all().map_err(error)?;
        if fs::read(&target).map_err(error)? != *bytes { return Err("Migration copy verification failed; originals remain unchanged.".into()); }
    }
    if staging.join("contacts.db").exists() {
        // Opening the copied database replays its copied WAL; never open/write the original.
        let conn = rusqlite::Connection::open(staging.join("contacts.db")).map_err(error)?;
        let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(error)?;
        if integrity != "ok" { return Err("Copied database integrity check failed; originals remain unchanged.".into()); }
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").map_err(error)?;
        // Discovery metadata is not contact content. Do not point the new app at an old executable.
        let has_meta: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='meta')", [], |r| r.get(0)).map_err(error)?;
        if has_meta { conn.execute("DELETE FROM meta WHERE key='contactcraft.exe'", []).map_err(error)?; }
        conn.close().map_err(|(_, e)| error(e))?;
    }
    let receipt = Receipt { version: 1, source: canonical_source, files: fingerprint };
    let mut file = OpenOptions::new().write(true).create_new(true).open(staging.join(RECEIPT)).map_err(error)?;
    file.write_all(&serde_json::to_vec_pretty(&receipt).map_err(error)?).map_err(error)?;
    file.sync_all().map_err(error)?;
    drop(file);
    let mut after = Vec::new();
    inventory(source, Path::new(""), &mut after)?;
    let mut after_names: Vec<_> = after.iter().map(|p| p.to_string_lossy().to_string()).collect();
    after_names.sort();
    if after_names != files.keys().cloned().collect::<Vec<_>>() {
        return Err("Source file inventory changed; staged copy was not activated.".into());
    }
    fs::rename(&staging, destination).map_err(error)?;
    Ok(())
}

pub fn prepare_profile_at(roaming: &Path) -> Result<PathBuf> {
    let destination = roaming.join("ContactSolutions");
    let source = roaming.join("ContactCraft");
    if source.exists() { migrate_directory(&source, &destination)?; }
    else if roaming.join("ContactSolutions.migration-staging").exists() || roaming.join("ContactSolutions.migration-lock").exists() {
        return Err("An interrupted profile migration needs recovery before startup.".into());
    }
    Ok(destination)
}

pub fn prepare_profile() -> Result<PathBuf> {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from).ok_or("APPDATA is not set; refusing to create data in the working directory.")?;
    prepare_profile_at(&roaming)
}

pub fn prepare_store() -> Result<PathBuf> {
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from).ok_or("APPDATA is not set; refusing to create data in the working directory.")?;
    let target = std::env::var_os("CONTACTSOLUTIONS_DATA").map(PathBuf::from);
    let legacy = std::env::var_os("CONTACTCRAFT_DATA").map(PathBuf::from);
    prepare_store_at(&roaming, target.as_deref(), legacy.as_deref())
}

/// Pure path selection entry point, also used by isolated migration tests.
pub fn prepare_store_at(roaming: &Path, target: Option<&Path>, legacy: Option<&Path>) -> Result<PathBuf> {
    if legacy.is_some() && target.is_none() {
        return Err("Legacy CONTACTCRAFT_DATA is set. Set CONTACTSOLUTIONS_DATA to a separate destination for guarded migration; the source will be retained.".into());
    }
    let profile = prepare_profile_at(roaming)?;
    if let Some(old) = legacy {
        migrate_directory(old, target.unwrap())?;
    }
    Ok(target.map(Path::to_path_buf).unwrap_or(profile))
}
