//! The MessageCraft window: its state, the events from the phone, and the frame loop.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use egui::{ColorImage, TextureHandle, TextureOptions};
use serde_json::{Value, json};

use crate::bridge::Bridge;
use crate::model::{Appearance, Conversation, Folder, Message, Settings};
use crate::theme::Tokens;
use crate::timefmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Conn {
    Starting,
    NoSession,
    Pairing,
    Connecting,
    Connected,
    Reconnecting(String),
    PhoneOffline(String),
    OtherSession,
    Error(String),
    HelperMissing(String),
}

impl Conn {
    pub fn paired_view(&self) -> bool {
        !matches!(self, Conn::NoSession | Conn::Pairing | Conn::HelperMissing(_) | Conn::Starting)
    }
}

#[derive(Default)]
pub struct Thread {
    pub messages: Vec<Message>,
    pub loading: bool,
    pub loading_older: bool,
    pub loaded: bool,
    pub failed: bool,
    pub has_more: bool,
    pub cursor: Option<(String, i64)>,
    /// Scroll to the newest message on the next frame.
    pub scroll_to_end: bool,
}

#[derive(Default, Clone)]
pub struct Draft {
    pub text: String,
    pub files: Vec<PathBuf>,
    pub reply_to: Option<String>,
}

#[allow(dead_code)]
pub enum MediaSlot {
    Loading,
    Decoding,
    Ready(TextureHandle),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RibbonTab {
    Home,
    View,
    Help,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Backstage {
    Info,
    Options,
    About,
}

#[derive(Default)]
pub struct NewChat {
    pub numbers: String,
    pub group_name: String,
    pub busy: bool,
    pub focus: bool,
}

pub struct Notice {
    pub text: String,
    pub error: bool,
    pub until: Instant,
}

pub struct App {
    pub bridge: Bridge,
    pub conn: Conn,
    pub qr: Option<(String, Vec<bool>, usize, Instant)>,
    pub qr_expired: bool,
    pub auto_pair_started: bool,
    pub phone: Option<String>,
    pub convs: HashMap<String, Conversation>,
    pub folder: Folder,
    pub unread_only: bool,
    pub selected: Option<String>,
    pub threads: HashMap<String, Thread>,
    pub drafts: HashMap<String, Draft>,
    pub media: HashMap<String, MediaSlot>,
    pub media_paths: HashMap<String, PathBuf>,
    pub open_when_ready: HashSet<String>,
    decoded_tx: Sender<(String, Result<ColorImage, String>)>,
    decoded_rx: Receiver<(String, Result<ColorImage, String>)>,
    pub search: String,
    pub ribbon: RibbonTab,
    pub backstage: Option<Backstage>,
    pub settings: Settings,
    pub notice: Option<Notice>,
    pub syncing: HashSet<String>,
    pub last_sync: Option<i64>,
    pub new_chat: Option<NewChat>,
    pub confirm_unpair: bool,
    pub typing: HashMap<String, Instant>,
    pub focus_composer: bool,
    pub focus_search: bool,
    pub collapsed_favorites: bool,
    pub collapsed_account: bool,
    cache_dirty: bool,
    cache_saved: Instant,
    last_title: String,
    pub t: Tokens,
    pub sent_count: u64,
    pub demo: bool,
}

const CACHE_FILE: &str = "cache.json";
const SETTINGS_FILE: &str = "ui.json";

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::install_fonts(&cc.egui_ctx);
        crate::theme::apply(&cc.egui_ctx);
        let mut bridge = Bridge::new();
        let settings: Settings = std::fs::read(bridge.data_dir.join(SETTINGS_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let (decoded_tx, decoded_rx) = channel();
        let mut app = App {
            conn: Conn::Starting,
            qr: None,
            qr_expired: false,
            auto_pair_started: false,
            phone: None,
            convs: HashMap::new(),
            folder: Folder::Inbox,
            unread_only: false,
            selected: None,
            threads: HashMap::new(),
            drafts: HashMap::new(),
            media: HashMap::new(),
            media_paths: HashMap::new(),
            open_when_ready: HashSet::new(),
            decoded_tx,
            decoded_rx,
            search: String::new(),
            ribbon: RibbonTab::Home,
            backstage: None,
            settings,
            notice: None,
            syncing: HashSet::new(),
            last_sync: None,
            new_chat: None,
            confirm_unpair: false,
            typing: HashMap::new(),
            focus_composer: false,
            focus_search: false,
            collapsed_favorites: false,
            collapsed_account: false,
            cache_dirty: false,
            cache_saved: Instant::now(),
            last_title: String::new(),
            t: Tokens::dark(),
            sent_count: 0,
            demo: false,
            bridge: Bridge::new(),
        };
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--demo") {
            // Sample conversations, no phone and nothing saved: for trying the window out.
            app.demo = true;
            if args.iter().any(|a| a == "--light") {
                app.settings.appearance = Appearance::Light;
            } else if args.iter().any(|a| a == "--dark") {
                app.settings.appearance = Appearance::Dark;
            }
            app.apply_settings(&cc.egui_ctx);
            crate::demo::fill(&mut app);
            app.bridge = bridge;
            return app;
        }
        app.load_cache();
        app.apply_settings(&cc.egui_ctx);
        bridge.start(&cc.egui_ctx);
        bridge.send(json!({"cmd": "connect"}));
        app.bridge = bridge;
        app
    }

    pub fn apply_settings(&self, ctx: &egui::Context) {
        ctx.set_theme(match self.settings.appearance {
            Appearance::System => egui::ThemePreference::System,
            Appearance::Light => egui::ThemePreference::Light,
            Appearance::Dark => egui::ThemePreference::Dark,
        });
        ctx.set_zoom_factor(self.settings.zoom.clamp(80, 200) as f32 / 100.0);
    }

    pub fn save_settings(&self) {
        if self.demo {
            return;
        }
        if let Ok(b) = serde_json::to_vec_pretty(&self.settings) {
            let _ = std::fs::write(self.bridge.data_dir.join(SETTINGS_FILE), b);
        }
    }

    fn load_cache(&mut self) {
        let Ok(bytes) = std::fs::read(self.bridge.data_dir.join(CACHE_FILE)) else { return };
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else { return };
        if let Some(arr) = v.get("conversations").and_then(|a| a.as_array()) {
            for c in arr {
                if let Ok(c) = serde_json::from_value::<Conversation>(c.clone()) {
                    self.convs.insert(c.id.clone(), c);
                }
            }
        }
        if let Some(obj) = v.get("threads").and_then(|a| a.as_object()) {
            for (id, msgs) in obj {
                if let Ok(messages) = serde_json::from_value::<Vec<Message>>(msgs.clone()) {
                    self.threads.insert(id.clone(), Thread { messages, ..Default::default() });
                }
            }
        }
        self.phone = v.get("phone").and_then(|p| p.as_str()).map(str::to_string);
    }

    pub fn save_cache(&mut self) {
        if self.demo {
            return;
        }
        let convs: Vec<&Conversation> = self.convs.values().collect();
        let mut threads = serde_json::Map::new();
        for (id, t) in &self.threads {
            let keep: Vec<&Message> = t.messages.iter().filter(|m| !m.local).rev().take(60).collect::<Vec<_>>().into_iter().rev().collect();
            if !keep.is_empty() {
                threads.insert(id.clone(), serde_json::to_value(keep).unwrap_or(Value::Null));
            }
        }
        let v = json!({"conversations": convs, "threads": threads, "phone": self.phone});
        if let Ok(b) = serde_json::to_vec(&v) {
            let path = self.bridge.data_dir.join(CACHE_FILE);
            let tmp = path.with_extension("tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
        self.cache_dirty = false;
        self.cache_saved = Instant::now();
    }

    fn clear_local_data(&mut self) {
        self.convs.clear();
        self.threads.clear();
        self.drafts.clear();
        self.media.clear();
        self.media_paths.clear();
        self.selected = None;
        self.phone = None;
        let _ = std::fs::remove_file(self.bridge.data_dir.join(CACHE_FILE));
        let media = self.bridge.data_dir.join("media");
        if let Ok(rd) = std::fs::read_dir(&media) {
            for e in rd.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    pub fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notice = Some(Notice { text: text.into(), error, until: Instant::now() + Duration::from_secs(if error { 8 } else { 4 }) });
    }

    // -----------------------------------------------------------------------------------------
    // Commands

    pub fn sync_now(&mut self) {
        if !self.conn.paired_view() {
            return;
        }
        self.bridge.send(json!({"cmd": "sync", "folder": self.folder.code(), "count": 150}));
        if let Some(id) = self.selected.clone() {
            self.load_thread(&id, true);
        }
    }

    pub fn select(&mut self, id: &str) {
        if self.selected.as_deref() == Some(id) {
            return;
        }
        self.selected = Some(id.to_string());
        let needs = self.threads.get(id).map(|t| !t.loaded && !t.loading).unwrap_or(true);
        if needs {
            self.load_thread(id, false);
        }
        if let Some(t) = self.threads.get_mut(id) {
            t.scroll_to_end = true;
        }
        self.focus_composer = true;
    }

    pub fn load_thread(&mut self, id: &str, refresh: bool) {
        let t = self.threads.entry(id.to_string()).or_default();
        if t.loading && !refresh {
            return;
        }
        t.loading = true;
        t.failed = false;
        self.bridge.send(json!({"cmd": "messages", "conversation_id": id, "count": 50}));
    }

    pub fn load_older(&mut self, id: &str) {
        let Some(t) = self.threads.get_mut(id) else { return };
        let Some((cid, cts)) = t.cursor.clone() else { return };
        if t.loading_older {
            return;
        }
        t.loading_older = true;
        self.bridge.send(json!({"cmd": "messages", "conversation_id": id, "count": 50, "before_id": cid, "before_ts": cts}));
    }

    pub fn send_draft(&mut self, conv_id: &str) {
        let draft = self.drafts.get(conv_id).cloned().unwrap_or_default();
        let text = draft.text.trim_end().to_string();
        if text.trim().is_empty() && draft.files.is_empty() {
            return;
        }
        if !matches!(self.conn, Conn::Connected) {
            self.notify("MessageCraft isn't connected to your phone right now. Your message is still in the box.", true);
            return;
        }
        self.sent_count += 1;
        let tmp_id = format!("mc{}{}", timefmt::now_ms(), self.sent_count);
        let files: Vec<String> = draft.files.iter().map(|p| p.display().to_string()).collect();
        let local = Message {
            id: tmp_id.clone(),
            conversation_id: conv_id.to_string(),
            ts: timefmt::now_ms(),
            from_me: true,
            text: text.clone(),
            status: "sending".into(),
            tmp_id: tmp_id.clone(),
            local: true,
            media: if draft.files.is_empty() {
                None
            } else {
                Some(
                    draft
                        .files
                        .iter()
                        .map(|p| crate::model::Media {
                            name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                            mime: "local/file".into(),
                            ..Default::default()
                        })
                        .collect(),
                )
            },
            ..Default::default()
        };
        let t = self.threads.entry(conv_id.to_string()).or_default();
        t.messages.push(local);
        t.scroll_to_end = true;
        let mut cmd = json!({"cmd": "send", "conversation_id": conv_id, "text": text, "tmp_id": tmp_id, "files": files});
        if let Some(r) = &draft.reply_to {
            cmd["reply_to"] = json!(r);
        }
        if self.demo {
            if let Some(m) = self.threads.get_mut(conv_id).and_then(|t| t.messages.last_mut()) {
                m.status = "delivered".into();
                m.local = false;
            }
        }
        self.bridge.send(cmd);
        if let Some(c) = self.convs.get_mut(conv_id) {
            c.snippet = if text.is_empty() { "Attachment".into() } else { text };
            c.snippet_me = true;
            c.ts = timefmt::now_ms();
        }
        self.drafts.insert(conv_id.to_string(), Draft::default());
    }

    pub fn request_media(&mut self, id: &str, key: &str, mime: &str) {
        if id.is_empty() || self.media.contains_key(id) {
            return;
        }
        self.media.insert(id.to_string(), MediaSlot::Loading);
        self.bridge.send(json!({"cmd": "media", "media_id": id, "key": key, "mime": mime}));
    }

    pub fn open_media(&mut self, id: &str, key: &str, mime: &str) {
        if let Some(path) = self.media_paths.get(id).cloned() {
            open_path(&path);
            return;
        }
        self.open_when_ready.insert(id.to_string());
        if !self.media.contains_key(id) {
            self.media.insert(id.to_string(), MediaSlot::Loading);
            self.bridge.send(json!({"cmd": "media", "media_id": id, "key": key, "mime": mime}));
        }
        self.notify("Downloading the attachment from your phone…", false);
    }

    pub fn pair(&mut self) {
        self.qr = None;
        self.qr_expired = false;
        self.conn = Conn::Pairing;
        self.bridge.send(json!({"cmd": "pair"}));
    }

    pub fn unpair(&mut self) {
        self.bridge.send(json!({"cmd": "unpair"}));
        self.clear_local_data();
        self.auto_pair_started = false;
    }

    pub fn restart_helper(&mut self, ctx: &egui::Context) {
        self.conn = Conn::Starting;
        self.bridge.start(ctx);
        self.bridge.send(json!({"cmd": "connect"}));
    }

    pub fn start_new_chat(&mut self) {
        let Some(nc) = &mut self.new_chat else { return };
        let numbers: Vec<String> = nc
            .numbers
            .split([',', ';', '\n'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if numbers.is_empty() {
            return;
        }
        nc.busy = true;
        let group = nc.group_name.clone();
        self.bridge.send(json!({"cmd": "new_chat", "numbers": numbers, "group_name": group}));
    }

    // -----------------------------------------------------------------------------------------
    // Events

    fn handle_events(&mut self, ctx: &egui::Context) {
        let mut n = 0;
        while let Ok(ev) = self.bridge.rx.try_recv() {
            self.on_event(ctx, ev);
            n += 1;
            if n > 500 {
                ctx.request_repaint();
                break;
            }
        }
        while let Ok((id, result)) = self.decoded_rx.try_recv() {
            match result {
                Ok(img) => {
                    let tex = ctx.load_texture(format!("media-{id}"), img, TextureOptions::LINEAR);
                    self.media.insert(id, MediaSlot::Ready(tex));
                }
                Err(e) => {
                    self.media.insert(id, MediaSlot::Failed(e));
                }
            }
        }
    }

    fn on_event(&mut self, ctx: &egui::Context, ev: Value) {
        let kind = ev.get("ev").and_then(|v| v.as_str()).unwrap_or("");
        let s = |k: &str| ev.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        match kind {
            "state" => {
                let detail = s("detail");
                let new = match s("state").as_str() {
                    "no_session" => Conn::NoSession,
                    "pairing" => Conn::Pairing,
                    "connecting" => Conn::Connecting,
                    "connected" => Conn::Connected,
                    "reconnecting" => Conn::Reconnecting(detail),
                    "phone_offline" => Conn::PhoneOffline(detail),
                    "other_session" => Conn::OtherSession,
                    "error" => Conn::Error(detail),
                    _ => return,
                };
                if new == Conn::NoSession && !self.auto_pair_started && self.qr.is_none() {
                    // Nothing paired yet: show a code to scan straight away.
                    self.auto_pair_started = true;
                    self.conn = Conn::NoSession;
                    self.pair();
                    return;
                }
                if new == Conn::Connected && self.conn != Conn::Connected {
                    self.qr = None;
                }
                self.conn = new;
            }
            "qr" => {
                let url = s("url");
                if let Ok(code) = qrcode::QrCode::new(url.as_bytes()) {
                    let w = code.width();
                    let modules: Vec<bool> = code.to_colors().into_iter().map(|c| c == qrcode::Color::Dark).collect();
                    self.qr = Some((url, modules, w, Instant::now()));
                    self.qr_expired = false;
                    self.conn = Conn::Pairing;
                }
            }
            "qr_expired" => {
                self.qr = None;
                self.qr_expired = true;
            }
            "paired" => {
                self.qr = None;
                self.phone = Some(s("phone"));
                self.conn = Conn::Connecting;
                self.notify("Paired with your phone. Syncing your conversations…", false);
            }
            "phone" => {
                let id = s("id");
                if !id.is_empty() {
                    self.phone = Some(id);
                }
            }
            "unpaired" => {
                let reason = s("reason");
                self.clear_local_data();
                self.auto_pair_started = false;
                if !reason.is_empty() {
                    self.notify(reason, true);
                }
            }
            "syncing" => {
                self.syncing.insert(s("folder"));
            }
            "synced" => {
                self.syncing.remove(&s("folder"));
                if ev.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    self.last_sync = Some(timefmt::now_ms());
                }
            }
            "conversations" => {
                let folder = s("folder");
                if let Some(items) = ev.get("items").and_then(|v| v.as_array()) {
                    let fresh: Vec<Conversation> = items.iter().filter_map(|i| serde_json::from_value(i.clone()).ok()).collect();
                    let ids: HashSet<String> = fresh.iter().map(|c| c.id.clone()).collect();
                    // Conversations that left this folder on the phone leave it here too.
                    self.convs.retain(|id, c| c.folder != folder || ids.contains(id));
                    for c in fresh {
                        let changed = self.convs.get(&c.id).map(|old| old.latest_msg_id != c.latest_msg_id).unwrap_or(false);
                        let id = c.id.clone();
                        self.convs.insert(id.clone(), c);
                        if changed && self.selected.as_deref() == Some(&id) {
                            self.load_thread(&id, true);
                        }
                    }
                    self.cache_dirty = true;
                }
            }
            "conversation" => {
                if let Some(item) = ev.get("item") {
                    if let Ok(c) = serde_json::from_value::<Conversation>(item.clone()) {
                        let id = c.id.clone();
                        let opened = ev.get("opened").and_then(|v| v.as_bool()).unwrap_or(false);
                        self.convs.insert(id.clone(), c);
                        self.cache_dirty = true;
                        if opened {
                            self.new_chat = None;
                            self.folder = Folder::Inbox;
                            self.selected = None;
                            self.select(&id);
                        }
                    }
                }
            }
            "messages" => {
                let cid = s("conversation_id");
                let older = ev.get("older").and_then(|v| v.as_bool()).unwrap_or(false);
                let failed = ev.get("failed").and_then(|v| v.as_bool()).unwrap_or(false);
                let items: Vec<Message> = ev
                    .get("items")
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|i| serde_json::from_value(i.clone()).ok()).collect())
                    .unwrap_or_default();
                let has_more = ev.get("has_more").and_then(|v| v.as_bool()).unwrap_or(false);
                let cursor = ev.get("cursor_id").and_then(|v| v.as_str()).map(|id| (id.to_string(), ev.get("cursor_ts").and_then(|v| v.as_i64()).unwrap_or(0)));
                let t = self.threads.entry(cid.clone()).or_default();
                if failed {
                    t.loading = false;
                    t.loading_older = false;
                    t.failed = !t.loaded;
                    return;
                }
                if older {
                    t.loading_older = false;
                    let have: HashSet<String> = t.messages.iter().map(|m| m.id.clone()).collect();
                    let mut merged: Vec<Message> = items.into_iter().filter(|m| !have.contains(&m.id)).collect();
                    merged.append(&mut t.messages);
                    t.messages = merged;
                } else {
                    t.loading = false;
                    let first_load = !t.loaded;
                    // Keep messages typed here that the phone hasn't confirmed yet.
                    let pending: Vec<Message> = t.messages.drain(..).filter(|m| m.local && !items.iter().any(|i| i.tmp_id == m.tmp_id)).collect();
                    t.messages = items;
                    t.messages.extend(pending);
                    t.loaded = true;
                    if first_load {
                        t.scroll_to_end = true;
                    }
                }
                t.messages.sort_by_key(|m| m.ts);
                t.has_more = has_more;
                if cursor.is_some() {
                    t.cursor = cursor;
                }
                self.cache_dirty = true;
            }
            "message" => {
                let Some(item) = ev.get("item") else { return };
                let Ok(m) = serde_json::from_value::<Message>(item.clone()) else { return };
                let old = ev.get("old").and_then(|v| v.as_bool()).unwrap_or(false);
                let cid = m.conversation_id.clone();
                let incoming_new = !m.from_me && !old && !m.deleted;
                let preview = m.preview();
                let ts = m.ts;
                let from_me = m.from_me;
                if let Some(t) = self.threads.get_mut(&cid) {
                    if let Some(pos) = t.messages.iter().position(|x| x.id == m.id || (!m.tmp_id.is_empty() && x.tmp_id == m.tmp_id)) {
                        if m.deleted {
                            t.messages.remove(pos);
                        } else {
                            t.messages[pos] = m;
                        }
                    } else if !m.deleted {
                        t.messages.push(m);
                        t.messages.sort_by_key(|x| x.ts);
                        t.scroll_to_end = true;
                    }
                }
                if let Some(c) = self.convs.get_mut(&cid) {
                    if ts >= c.ts && !preview.is_empty() {
                        c.ts = ts;
                        c.snippet = preview.clone();
                        c.snippet_me = from_me;
                    }
                }
                if incoming_new {
                    self.typing.remove(&cid);
                    let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
                    let viewing = focused && self.selected.as_deref() == Some(cid.as_str());
                    if let Some(c) = self.convs.get_mut(&cid) {
                        if !viewing {
                            c.unread = true;
                        }
                    }
                    if !focused {
                        ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
                    }
                }
                self.cache_dirty = true;
            }
            "send_result" => {
                let ok = ev.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                if !ok {
                    let tmp = s("tmp_id");
                    let err = s("error");
                    if let Some(t) = self.threads.get_mut(&s("conversation_id")) {
                        if let Some(m) = t.messages.iter_mut().find(|m| m.tmp_id == tmp) {
                            m.failed = true;
                            m.local_error = err.clone();
                            m.status = "failed".into();
                        }
                    }
                    self.notify(if err.is_empty() { "The message couldn't be sent.".to_string() } else { err }, true);
                }
            }
            "typing" => {
                let cid = s("conversation_id");
                if ev.get("typing").and_then(|v| v.as_bool()).unwrap_or(false) {
                    self.typing.insert(cid, Instant::now() + Duration::from_secs(15));
                } else {
                    self.typing.remove(&cid);
                }
            }
            "media" => {
                let id = s("media_id");
                if ev.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let path = PathBuf::from(s("path"));
                    self.media_paths.insert(id.clone(), path.clone());
                    if self.open_when_ready.remove(&id) {
                        open_path(&path);
                    }
                    let is_image = matches!(
                        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
                        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "jfif")
                    );
                    if is_image {
                        self.media.insert(id.clone(), MediaSlot::Decoding);
                        let tx = self.decoded_tx.clone();
                        let ctx = ctx.clone();
                        std::thread::spawn(move || {
                            let result = decode_image(&path);
                            let _ = tx.send((id, result));
                            ctx.request_repaint();
                        });
                    } else {
                        self.media.insert(id, MediaSlot::Failed(String::new()));
                    }
                } else {
                    let err = s("error");
                    if self.open_when_ready.remove(&id) {
                        self.notify(format!("The attachment couldn't be downloaded: {err}"), true);
                    }
                    self.media.insert(id, MediaSlot::Failed(err));
                }
            }
            "deleted" => {
                let mid = s("message_id");
                for t in self.threads.values_mut() {
                    t.messages.retain(|m| m.id != mid);
                }
            }
            "phone_alert" => self.notify(s("text"), false),
            "error" => {
                let ctxname = s("context");
                let msg = s("message");
                if ctxname == "new_chat" {
                    if let Some(nc) = &mut self.new_chat {
                        nc.busy = false;
                    }
                }
                if ctxname == "messages" {
                    // shown in the reading pane
                    return;
                }
                if !msg.is_empty() {
                    self.notify(msg, true);
                }
            }
            "bridge_missing" => {
                self.conn = Conn::HelperMissing(s("message"));
            }
            "bridge_exit" => {
                if self.bridge.running {
                    self.bridge.running = false;
                    self.conn = Conn::HelperMissing("MessageCraft's connection helper stopped unexpectedly.".into());
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------------------------
    // Lists

    pub fn visible_conversations(&self) -> Vec<&Conversation> {
        let q = self.search.trim().to_lowercase();
        let mut v: Vec<&Conversation> = self
            .convs
            .values()
            .filter(|c| c.folder == self.folder.code())
            .filter(|c| !self.unread_only || c.unread)
            .filter(|c| {
                q.is_empty()
                    || c.title().to_lowercase().contains(&q)
                    || c.snippet.to_lowercase().contains(&q)
                    || c.participants.iter().any(|p| p.number.replace([' ', '-', '(', ')'], "").contains(&q.replace([' ', '-', '(', ')'], "")) || p.name.to_lowercase().contains(&q))
            })
            .collect();
        v.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.ts.cmp(&a.ts)));
        v
    }

    pub fn unread_count(&self, folder: Folder) -> usize {
        self.convs.values().filter(|c| c.folder == folder.code() && c.unread).count()
    }

    pub fn folder_count(&self, folder: Folder) -> usize {
        self.convs.values().filter(|c| c.folder == folder.code()).count()
    }

    fn mark_read_if_viewing(&mut self, ctx: &egui::Context) {
        let Some(id) = self.selected.clone() else { return };
        let focused = ctx.input(|i| i.viewport().focused).unwrap_or(true);
        if !focused || !matches!(self.conn, Conn::Connected) || self.backstage.is_some() {
            return;
        }
        let Some(c) = self.convs.get(&id) else { return };
        if !c.unread {
            return;
        }
        let latest = self
            .threads
            .get(&id)
            .and_then(|t| t.messages.iter().rev().find(|m| !m.local).map(|m| m.id.clone()))
            .unwrap_or_else(|| c.latest_msg_id.clone());
        if latest.is_empty() {
            return;
        }
        self.bridge.send(json!({"cmd": "mark_read", "conversation_id": id, "message_id": latest}));
        if let Some(c) = self.convs.get_mut(&id) {
            c.unread = false;
        }
        self.cache_dirty = true;
    }

    pub fn mark_selected_unread_toggle(&mut self) {
        let Some(id) = self.selected.clone() else { return };
        if let Some(c) = self.convs.get_mut(&id) {
            if c.unread {
                let latest = c.latest_msg_id.clone();
                c.unread = false;
                self.bridge.send(json!({"cmd": "mark_read", "conversation_id": id, "message_id": latest}));
            }
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        let typing_somewhere = ctx.memory(|m| m.focused().is_some());
        let (ctrl_n, f5, ctrl_f, esc, up, down) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::N),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F5) || i.consume_key(egui::Modifiers::NONE, egui::Key::F9),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::F) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::E),
                i.key_pressed(egui::Key::Escape),
                !typing_somewhere && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                !typing_somewhere && i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            )
        });
        if ctrl_n && self.conn.paired_view() {
            self.new_chat = Some(NewChat { focus: true, ..Default::default() });
        }
        if f5 {
            self.sync_now();
        }
        if ctrl_f {
            self.focus_search = true;
        }
        if esc {
            if self.new_chat.is_some() {
                self.new_chat = None;
            } else if self.confirm_unpair {
                self.confirm_unpair = false;
            } else if self.backstage.is_some() {
                self.backstage = None;
            }
        }
        if up || down {
            let ids: Vec<String> = self.visible_conversations().iter().map(|c| c.id.clone()).collect();
            if !ids.is_empty() {
                let pos = self.selected.as_ref().and_then(|s| ids.iter().position(|i| i == s));
                let next = match (pos, up) {
                    (None, _) => 0,
                    (Some(p), true) => p.saturating_sub(1),
                    (Some(p), false) => (p + 1).min(ids.len() - 1),
                };
                let id = ids[next].clone();
                self.select(&id);
                self.focus_composer = false;
            }
        }
    }

    fn dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if dropped.is_empty() {
            return;
        }
        if let Some(id) = self.selected.clone() {
            self.drafts.entry(id).or_default().files.extend(dropped);
            self.focus_composer = true;
        } else {
            self.notify("Open a conversation first, then drop files on it to attach them.", false);
        }
    }

    fn window_title(&mut self, ctx: &egui::Context) {
        let unread = self.unread_count(Folder::Inbox);
        let mut title = format!("{} - MessageCraft", self.folder.label());
        if let Some(id) = &self.selected {
            if let Some(c) = self.convs.get(id) {
                title = format!("{} - MessageCraft", c.title());
            }
        }
        if unread > 0 {
            title = format!("({unread}) {title}");
        }
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}

pub fn open_path(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

fn decode_image(path: &std::path::Path) -> Result<ColorImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    // Big photos are shown scaled down; the full file opens with a click.
    let img = if img.width() > 1600 || img.height() > 1600 { img.thumbnail(1600, 1600) } else { img };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.t = Tokens::for_dark(ui.visuals().dark_mode);
        self.handle_events(&ctx);
        self.keyboard(&ctx);
        self.dropped_files(&ctx);
        self.typing.retain(|_, until| *until > Instant::now());
        if self.notice.as_ref().is_some_and(|n| n.until < Instant::now()) {
            self.notice = None;
        }
        self.mark_read_if_viewing(&ctx);
        self.window_title(&ctx);

        self.draw(ui);

        if self.cache_dirty && self.cache_saved.elapsed() > Duration::from_secs(8) {
            self.save_cache();
        }
        // Keep clocks, the QR countdown and notices moving.
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn on_exit(&mut self) {
        self.save_cache();
        self.save_settings();
        self.bridge.stop();
    }
}
