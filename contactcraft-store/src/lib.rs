//! # ContactCraft's shared contact store
//!
//! ContactCraft keeps its contacts where every Craft app can reach them, whether ContactCraft is
//! running or not: a SQLite database and a folder of pictures in `%APPDATA%\ContactCraft`.
//! This crate is the way in. Add it to another Craft app:
//!
//! ```toml
//! contactcraft-store = { path = "<ContactCraft>/app/store" }
//! ```
//!
//! and then:
//!
//! ```no_run
//! use contactcraft_store::Store;
//! let store = Store::open_default().unwrap();
//! if let Some(c) = store.find_by_phone("+1 555 010 2030").unwrap() {
//!     println!("{} — {}", c.display_name(), c.theme.accent);
//!     let picture = store.photo_path(&c);
//! }
//! ```
//!
//! Changes made through the store show in ContactCraft at once (it watches
//! [`Store::revision`]). ContactCraft never connects to Google: contacts come in and go out only
//! through files (vCard, CSV, JSON).
//! See `docs/shared-contacts.md` for the database layout, for programs not written in Rust.

pub mod cleanup;
pub mod csv;
pub mod model;
pub mod store;
pub mod vcard;

pub use model::*;
pub use store::{Store, new_id, now, sniff_image};
