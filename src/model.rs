//! What the window keeps: conversations, their messages, and the person's settings.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Participant {
    pub id: String,
    pub name: String,
    pub first: String,
    pub number: String,
    pub is_me: bool,
    pub color: String,
}

impl Participant {
    pub fn label(&self) -> &str {
        if !self.name.is_empty() { &self.name } else if !self.first.is_empty() { &self.first } else { &self.number }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Conversation {
    pub id: String,
    pub name: String,
    pub snippet: String,
    pub snippet_me: bool,
    pub snippet_from: String,
    /// Milliseconds since 1970.
    pub ts: i64,
    pub unread: bool,
    pub group: bool,
    pub pinned: bool,
    pub read_only: bool,
    pub color: String,
    pub folder: String,
    pub kind: String,
    pub outgoing_id: String,
    pub latest_msg_id: String,
    pub participants: Vec<Participant>,
    /// Number explicitly chosen when starting a chat, for contact photos and names.
    pub contact_number: String,
}

impl Conversation {
    pub fn contact_number(&self) -> &str {
        if !self.contact_number.is_empty() { &self.contact_number }
        else { self.others().next().map(|p| p.number.as_str()).unwrap_or("") }
    }
    pub fn title(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let others: Vec<&str> = self.participants.iter().filter(|p| !p.is_me).map(|p| p.label()).collect();
        if others.is_empty() { "(No name)".into() } else { others.join(", ") }
    }
    pub fn others(&self) -> impl Iterator<Item = &Participant> {
        self.participants.iter().filter(|p| !p.is_me)
    }
    pub fn numbers(&self) -> String {
        let n: Vec<&str> = self.others().map(|p| p.number.as_str()).filter(|n| !n.is_empty()).collect();
        n.join(", ")
    }
    pub fn participant(&self, id: &str) -> Option<&Participant> {
        self.participants.iter().find(|p| p.id == id)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Media {
    pub id: String,
    pub key: String,
    pub thumb_id: String,
    pub thumb_key: String,
    pub name: String,
    pub mime: String,
    pub size: i64,
    pub w: i64,
    pub h: i64,
}

impl Media {
    pub fn is_image(&self) -> bool {
        self.mime.starts_with("image/")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Reaction {
    pub emoji: String,
    pub count: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub participant_id: String,
    pub ts: i64,
    pub from_me: bool,
    pub text: String,
    pub status: String,
    pub status_text: String,
    pub failed: bool,
    pub deleted: bool,
    pub tmp_id: String,
    pub media: Option<Vec<Media>>,
    pub reactions: Option<Vec<Reaction>>,
    pub subject: String,
    pub sender_name: String,
    pub reply_to: String,
    /// Set on this side only: a message typed here that the phone hasn't confirmed yet.
    #[serde(skip)]
    pub local: bool,
    #[serde(skip)]
    pub local_error: String,
}

impl Message {
    pub fn media(&self) -> &[Media] {
        self.media.as_deref().unwrap_or(&[])
    }
    pub fn reactions(&self) -> &[Reaction] {
        self.reactions.as_deref().unwrap_or(&[])
    }
    pub fn preview(&self) -> String {
        if !self.text.is_empty() {
            return self.text.clone();
        }
        match self.media().first() {
            Some(m) if m.is_image() => "Photo".into(),
            Some(m) if m.mime.starts_with("video/") => "Video".into(),
            Some(m) if m.mime.starts_with("audio/") => "Audio".into(),
            Some(_) => "Attachment".into(),
            None => String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: Appearance,
    pub folder_pane: bool,
    pub tighter: bool,
    pub zoom: u16,
    pub enter_sends: bool,
    pub preview_lines: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { appearance: Appearance::System, folder_pane: true, tighter: false, zoom: 100, enter_sends: true, preview_lines: 1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folder {
    Inbox,
    Archive,
    Spam,
}

impl Folder {
    pub fn code(self) -> &'static str {
        match self {
            Folder::Inbox => "inbox",
            Folder::Archive => "archive",
            Folder::Spam => "spam",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Folder::Inbox => "Inbox",
            Folder::Archive => "Archived",
            Folder::Spam => "Spam & blocked",
        }
    }
}
