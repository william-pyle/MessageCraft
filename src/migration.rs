//! Offline, lossless profile cutover. Legacy names here are persisted contracts.
use std::{collections::BTreeMap, fs::{self, File, OpenOptions}, io::{Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type Result<T> = std::result::Result<T, String>;
const RECEIPT: &str = "migration-receipt.json";
#[derive(Serialize, Deserialize)]
struct Receipt { version: u32, source: PathBuf, files: BTreeMap<String, String> }
fn err(e: impl std::fmt::Display) -> String { e.to_string() }

fn plain_path(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path).map_err(err)?;
    let mut linked = m.file_type().is_symlink();
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        linked |= m.file_attributes() & 0x400 != 0;
    }
    if linked { Err("Profile migration refuses links or reparse points; preserve the original and resolve the path.".into()) } else { Ok(()) }
}
fn inventory(root: &Path, relative: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    plain_path(&root.join(relative))?;
    for entry in fs::read_dir(root.join(relative)).map_err(err)? {
        let entry = entry.map_err(err)?;
        plain_path(&entry.path())?;
        let rel = relative.join(entry.file_name());
        if entry.file_type().map_err(err)?.is_dir() { inventory(root, &rel, files)?; }
        else if entry.file_type().map_err(err)?.is_file() { files.push(rel); }
        else { return Err("Profile contains an unsupported file type.".into()); }
    }
    files.sort(); Ok(())
}
fn hash(file: &mut File) -> Result<String> {
    file.seek(SeekFrom::Start(0)).map_err(err)?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop { let n = file.read(&mut buf).map_err(err)?; if n == 0 { break; } digest.update(&buf[..n]); }
    Ok(format!("{:x}", digest.finalize()))
}
fn validate_json(dir: &Path) -> Result<()> {
    for name in ["ui.json", "cache.json"] {
        let path = dir.join(name);
        if !path.exists() { continue; }
        let v: serde_json::Value = serde_json::from_slice(&fs::read(path).map_err(err)?)
            .map_err(|_| format!("{name} is invalid JSON. Preserve it and recover from a verified backup; no empty profile will replace it."))?;
        if !v.is_object() { return Err(format!("{name} must be a JSON object.")); }
        if name == "ui.json" { serde_json::from_value::<crate::model::Settings>(v).map_err(|_| "Saved settings have invalid field types.".to_string())?; }
        else {
            if let Some(c) = v.get("conversations") { serde_json::from_value::<Vec<crate::model::Conversation>>(c.clone()).map_err(|_| "Saved conversations have invalid field types.".to_string())?; }
            if let Some(t) = v.get("threads") { serde_json::from_value::<BTreeMap<String, Vec<crate::model::Message>>>(t.clone()).map_err(|_| "Saved threads have invalid field types.".to_string())?; }
        }
    }
    for pending in ["cache.tmp", "session.bin.tmp", "ui.json.pending"] {
        if dir.join(pending).exists() { return Err(format!("An interrupted save ({pending}) needs recovery before startup. Preserve both files; do not overwrite them.")); }
    }
    Ok(())
}
struct Lock(PathBuf);
impl Drop for Lock { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }

pub fn prepare_at(roaming: &Path) -> Result<PathBuf> {
    let old = roaming.join("MessageCraft");
    let new = roaming.join("MessageSolutions");
    let stage = roaming.join("MessageSolutions.migration-staging");
    let lock = roaming.join("MessageSolutions.migration-lock");
    fs::create_dir_all(roaming).map_err(err)?;
    plain_path(roaming)?;
    if stage.exists() || lock.exists() { return Err("An interrupted migration or another migration is present. Preserve migration-staging/migration-lock and follow docs/rename-migration.md before retrying.".into()); }
    if !old.exists() {
        if new.exists() { plain_path(&new)?; validate_json(&new)?; }
        else { fs::create_dir(&new).map_err(err)?; }
        return Ok(new);
    }
    #[cfg(not(windows))]
    return Err("Offline profile migration requires Windows exclusive file sharing.".into());
    let guard = OpenOptions::new().write(true).create_new(true).open(&lock).map_err(|_| "Another migration is running; no files were replaced.".to_string())?;
    let _guard = Lock(lock); drop(guard);
    plain_path(&old)?;
    let source = old.canonicalize().map_err(err)?;
    let mut files = Vec::new(); inventory(&old, Path::new(""), &mut files)?;
    let mut handles = Vec::new(); let mut fingerprint = BTreeMap::new();
    for rel in &files {
        let mut options = OpenOptions::new(); options.read(true);
        #[cfg(windows)] { use std::os::windows::fs::OpenOptionsExt; options.share_mode(0); }
        let mut file = options.open(old.join(rel)).map_err(|_| "Existing profile is open or inaccessible. Close its consumers yourself in a maintenance window, then retry. Originals were not changed.".to_string())?;
        fingerprint.insert(rel.to_str().ok_or("Profile filenames must be Unicode")?.to_string(), hash(&mut file)?);
        handles.push(file);
    }
    if new.exists() {
        plain_path(&new)?;
        let receipt: Receipt = serde_json::from_slice(&fs::read(new.join(RECEIPT)).map_err(|_| "Both old and new profiles exist without a migration receipt. Preserve both; choose recovery explicitly.".to_string())?).map_err(|_| "Invalid migration receipt; preserve both profiles.".to_string())?;
        if receipt.version != 1 || receipt.source != source || receipt.files != fingerprint { return Err("The retained source changed or the receipt does not match. Preserve both profiles; automatic merge is refused.".into()); }
        validate_json(&new)?;
        return Ok(new);
    }
    if fingerprint.contains_key(RECEIPT) { return Err("Legacy profile already has a migration receipt; resolve its history explicitly.".into()); }
    fs::create_dir(&stage).map_err(err)?;
    for ((rel, input), expected) in files.iter().zip(handles.iter_mut()).zip(fingerprint.values()) {
        let target = stage.join(rel);
        fs::create_dir_all(target.parent().unwrap()).map_err(err)?;
        let mut output = OpenOptions::new().create_new(true).read(true).write(true).open(target).map_err(err)?;
        input.seek(SeekFrom::Start(0)).map_err(err)?;
        std::io::copy(input, &mut output).map_err(err)?; output.sync_all().map_err(err)?;
        if &hash(&mut output)? != expected { return Err("Profile copy hash mismatch; staged copy was not activated.".into()); }
    }
    validate_json(&stage)?;
    let mut after = Vec::new(); inventory(&old, Path::new(""), &mut after)?;
    if after != files { return Err("Source changed during migration; staged copy was not activated.".into()); }
    let receipt = Receipt { version: 1, source, files: fingerprint };
    let mut output = OpenOptions::new().create_new(true).write(true).open(stage.join(RECEIPT)).map_err(err)?;
    output.write_all(&serde_json::to_vec_pretty(&receipt).map_err(err)?).map_err(err)?; output.sync_all().map_err(err)?; drop(output);
    fs::rename(stage, &new).map_err(err)?;
    Ok(new)
}
pub fn prepare() -> Result<PathBuf> {
    let roaming = std::env::var_os("APPDATA").ok_or("APPDATA is missing; refusing to read or create a profile in the working directory.")?;
    prepare_at(Path::new(&roaming))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("messagesolutions-migration-{}", contactsolutions_store::new_id()));
            fs::create_dir_all(p.join("MessageCraft/media")).unwrap();
            fs::write(p.join("MessageCraft/ui.json"), br#"{"appearance":"Dark","zoom":135,"future":{"keep":true}}"#).unwrap();
            fs::write(p.join("MessageCraft/cache.json"), br#"{"conversations":[{"id":"stable-conversation"}],"threads":{"stable-conversation":[{"id":"stable-message","conversation_id":"stable-conversation","text":"synthetic"}]}}"#).unwrap();
            fs::write(p.join("MessageCraft/session.bin"), b"synthetic opaque ciphertext").unwrap();
            fs::write(p.join("MessageCraft/media/keep.bin"), [1,2,3]).unwrap(); Self(p)
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    #[test]
    fn copy_preserves_all_bytes_ids_settings_and_source_and_detects_divergence() {
        let f=Fixture::new(); let dest=prepare_at(&f.0).unwrap();
        for file in ["ui.json","cache.json","session.bin","media/keep.bin"] { assert_eq!(fs::read(dest.join(file)).unwrap(), fs::read(f.0.join("MessageCraft").join(file)).unwrap()); }
        let settings: crate::model::Settings=serde_json::from_slice(&fs::read(dest.join("ui.json")).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(settings).unwrap()["future"]["keep"], true);
        assert_eq!(prepare_at(&f.0).unwrap(),dest);
        fs::write(f.0.join("MessageCraft/late"),b"late").unwrap();
        assert!(prepare_at(&f.0).unwrap_err().contains("retained source changed"));
    }
    #[test]
    fn collision_and_interruption_preserve_both_sides() {
        let f=Fixture::new(); fs::create_dir(f.0.join("MessageSolutions")).unwrap();
        assert!(prepare_at(&f.0).unwrap_err().contains("Both old and new"));
        let f=Fixture::new(); fs::create_dir(f.0.join("MessageSolutions.migration-staging")).unwrap();
        assert!(prepare_at(&f.0).unwrap_err().contains("interrupted"));
        fs::rename(f.0.join("MessageSolutions.migration-staging"),f.0.join("preserved-attempt")).unwrap();
        assert!(prepare_at(&f.0).is_ok());
        let f=Fixture::new(); fs::write(f.0.join("MessageSolutions.migration-lock"),b"").unwrap();
        assert!(prepare_at(&f.0).is_err());
    }
    #[test]
    fn corrupt_or_pending_state_never_activates_a_profile() {
        for (file,content) in [("ui.json","invalid"),("cache.json","[]"),("ui.json","{\"zoom\":\"bad\"}"),("session.bin.tmp","partial")] {
            let f=Fixture::new(); fs::write(f.0.join("MessageCraft").join(file),content).unwrap();
            assert!(prepare_at(&f.0).is_err()); assert!(!f.0.join("MessageSolutions").exists());
            assert_eq!(fs::read_to_string(f.0.join("MessageCraft").join(file)).unwrap(),content);
        }
    }
    #[cfg(windows)]
    #[test]
    fn locked_source_is_not_copied() {
        use std::os::windows::fs::OpenOptionsExt;
        let f=Fixture::new(); let _held=OpenOptions::new().read(true).share_mode(0).open(f.0.join("MessageCraft/session.bin")).unwrap();
        assert!(prepare_at(&f.0).unwrap_err().contains("open or inaccessible")); assert!(!f.0.join("MessageSolutions").exists());
    }
    #[test]
    fn new_profile_works_without_legacy_data() {
        let f=Fixture::new(); fs::rename(f.0.join("MessageCraft"),f.0.join("retained-elsewhere")).unwrap();
        assert!(prepare_at(&f.0).unwrap().is_dir());
    }
}
