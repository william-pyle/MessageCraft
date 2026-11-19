//! Synthetic fixtures only. No environment variables or owner data are accessed.
use std::{fs, path::PathBuf};
use contactsolutions_store::{Contact, ContactGroup, GroupMember, Store, migration::{migrate_directory, prepare_profile_at, SETTINGS_FILE}, vcard};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("contactsolutions-migration-{}", contactsolutions_store::new_id()));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn old(&self) -> PathBuf { self.0.join("ContactCraft") }
    fn new_path(&self) -> PathBuf { self.0.join("ContactSolutions") }
    fn settings(&self) {
        fs::create_dir_all(self.old()).unwrap();
        fs::write(self.old().join("contactcraft-ui.json"), br#"{"zoom":150,"appearance":"Dark","futureSetting":{"preserve":true}}"#).unwrap();
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

#[test]
fn profile_migration_preserves_ids_settings_photos_and_originals() {
    let f = Fixture::new(); f.settings();
    let store = Store::open(&f.old()).unwrap();
    let mut c = Contact::new(); c.name.given = "Synthetic".into(); c.extra.insert("futureField".into(), serde_json::json!({"keep":42}));
    store.save(&mut c).unwrap();
    let mut group = ContactGroup::new("Fixture"); group.members.push(GroupMember::Contact { id: c.id.clone() });
    store.save_group(&mut group).unwrap();
    store.set_meta("contactcraft.exe", Some("old.exe")).unwrap();
    fs::write(f.old().join("photos/picture.bin"), [1,2,3,4]).unwrap();
    drop(store);
    let db_before = fs::read(f.old().join("contacts.db")).unwrap();
    assert_eq!(prepare_profile_at(&f.0).unwrap(), f.new_path());
    assert_eq!(fs::read(f.old().join("contacts.db")).unwrap(), db_before);
    assert_eq!(fs::read(f.new_path().join(SETTINGS_FILE)).unwrap(), fs::read(f.old().join("contactcraft-ui.json")).unwrap());
    assert!(!f.new_path().join("contactcraft-ui.json").exists());
    assert_eq!(fs::read(f.new_path().join("photos/picture.bin")).unwrap(), [1,2,3,4]);
    let migrated = Store::open(&f.new_path()).unwrap();
    assert_eq!(serde_json::to_value(migrated.get(&c.id).unwrap().unwrap()).unwrap(), serde_json::to_value(&c).unwrap());
    assert_eq!(migrated.get_group(&group.id).unwrap().unwrap().members, group.members);
    assert!(migrated.meta("contactcraft.exe").is_none());
    drop(migrated);
    prepare_profile_at(&f.0).unwrap(); // Receipt makes repeat startup safe.
    fs::write(f.old().join("late-write"), b"conflict").unwrap();
    assert!(prepare_profile_at(&f.0).unwrap_err().contains("source changed"));
}

#[test]
fn both_existing_folders_are_never_silently_merged() {
    let f = Fixture::new(); f.settings();
    fs::create_dir(f.new_path()).unwrap(); fs::write(f.new_path().join("keep"), b"new").unwrap();
    assert!(prepare_profile_at(&f.0).unwrap_err().contains("Both old and new"));
    assert_eq!(fs::read(f.new_path().join("keep")).unwrap(), b"new");
    assert!(f.old().join("contactcraft-ui.json").exists());
}

#[test]
fn malformed_and_colliding_settings_do_not_activate_a_migration() {
    let f = Fixture::new(); f.settings();
    fs::write(f.old().join("contactcraft-ui.json"), b"invalid").unwrap();
    assert!(prepare_profile_at(&f.0).unwrap_err().contains("invalid JSON"));
    assert!(!f.new_path().exists());
    f.settings(); fs::write(f.old().join(SETTINGS_FILE), b"{}").unwrap();
    assert!(prepare_profile_at(&f.0).unwrap_err().contains("Both settings"));
}

#[test]
fn interrupted_migration_requires_explicit_recovery_and_can_retry() {
    let f = Fixture::new(); f.settings();
    let stage = f.0.join("ContactSolutions.migration-staging");
    fs::create_dir(&stage).unwrap(); fs::write(stage.join("partial"), b"recover").unwrap();
    assert!(prepare_profile_at(&f.0).unwrap_err().contains("interrupted"));
    // Recovery preserves the failed attempt separately before retrying.
    fs::rename(&stage, f.0.join("failed-attempt-backup")).unwrap();
    prepare_profile_at(&f.0).unwrap();
    assert_eq!(fs::read(f.0.join("failed-attempt-backup/partial")).unwrap(), b"recover");
}

#[test]
fn copied_wal_is_recovered_without_touching_source() {
    let f = Fixture::new(); fs::create_dir(f.old()).unwrap();
    let db = f.old().join("contacts.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE proof(id TEXT); INSERT INTO proof VALUES('stable-id');").unwrap();
    let base = fs::read(&db).unwrap();
    let wal = fs::read(f.old().join("contacts.db-wal")).unwrap();
    drop(conn);
    fs::write(&db, &base).unwrap(); fs::write(f.old().join("contacts.db-wal"), &wal).unwrap();
    prepare_profile_at(&f.0).unwrap();
    assert_eq!(fs::read(&db).unwrap(), base);
    assert_eq!(fs::read(f.old().join("contacts.db-wal")).unwrap(), wal);
    let copied = rusqlite::Connection::open(f.new_path().join("contacts.db")).unwrap();
    assert_eq!(copied.query_row("SELECT id FROM proof", [], |r| r.get::<_,String>(0)).unwrap(), "stable-id");
}

#[test]
fn corrupt_database_leaves_source_and_staging_for_recovery() {
    let f = Fixture::new(); f.settings(); fs::write(f.old().join("contacts.db"), b"not sqlite").unwrap();
    assert!(prepare_profile_at(&f.0).is_err()); assert!(!f.new_path().exists());
    assert_eq!(fs::read(f.old().join("contacts.db")).unwrap(), b"not sqlite");
    assert!(f.0.join("ContactSolutions.migration-staging").exists());
}

#[test]
fn same_or_nested_paths_are_rejected() {
    let f = Fixture::new(); f.settings();
    assert!(migrate_directory(&f.old(), &f.old()).unwrap_err().contains("separate"));
    assert!(migrate_directory(&f.old(), &f.old().join("nested")).unwrap_err().contains("separate"));
}

#[test]
fn custom_store_migration_keeps_settings_in_roaming_profile() {
    let f = Fixture::new(); f.settings();
    let old_store = f.0.join("custom-old"); let new_store = f.0.join("custom-new");
    let store = Store::open(&old_store).unwrap(); let mut contact = Contact::new();
    contact.name.given = "Custom fixture".into(); store.save(&mut contact).unwrap(); drop(store);
    assert!(contactsolutions_store::migration::prepare_store_at(&f.0, None, Some(&old_store)).is_err());
    assert!(!f.new_path().exists());
    let selected = contactsolutions_store::migration::prepare_store_at(&f.0, Some(&new_store), Some(&old_store)).unwrap();
    assert_eq!(selected, new_store);
    assert!(f.new_path().join(SETTINGS_FILE).exists());
    assert!(!new_store.join(SETTINGS_FILE).exists());
    assert_eq!(Store::open(&new_store).unwrap().get(&contact.id).unwrap().unwrap().id, contact.id);
    // Rollback source still contains the original contact, independently of the new store.
    assert_eq!(Store::open(&old_store).unwrap().get(&contact.id).unwrap().unwrap().id, contact.id);
}

#[test]
fn destination_io_failure_never_changes_source() {
    let f = Fixture::new(); f.settings();
    let blocked_parent = f.0.join("not-a-directory"); fs::write(&blocked_parent, b"keep").unwrap();
    assert!(migrate_directory(&f.old(), &blocked_parent.join("new")).is_err());
    assert_eq!(fs::read(&blocked_parent).unwrap(), b"keep");
    assert!(f.old().join("contactcraft-ui.json").exists());
    assert!(!f.new_path().exists());
}

#[cfg(windows)]
#[test]
fn open_source_files_prevent_migration_without_closing_their_owner() {
    let f = Fixture::new(); f.settings();
    let file = fs::File::open(f.old().join("contactcraft-ui.json")).unwrap();
    assert!(prepare_profile_at(&f.0).unwrap_err().contains("open or inaccessible"));
    assert!(!f.new_path().exists()); drop(file);
    prepare_profile_at(&f.0).unwrap();
}

#[test]
fn legacy_vcard_extension_reads_but_new_exports_use_new_name() {
    let mut c = Contact::new(); c.name.given = "Fixture".into(); c.theme.accent = "#123456".into();
    c.id = "existing-contact-id".into();
    c.extra.insert("retained".into(), serde_json::json!([1,2,3]));
    let current = vcard::export(&[c.clone()], |_| None);
    assert!(current.contains("X-CONTACTSOLUTIONS-JSON")); assert!(!current.contains("X-CONTACTCRAFT-JSON"));
    for text in [current.clone(), current.replace("X-CONTACTSOLUTIONS-JSON", "X-CONTACTCRAFT-JSON")] {
        let imported = vcard::import(&text);
        // Export import deliberately allocates new IDs; folder migration preserves existing IDs.
        assert!(imported[0].0.id.is_empty()); assert_eq!(imported[0].0.extra, c.extra); assert_eq!(imported[0].0.theme, c.theme);
    }
}
