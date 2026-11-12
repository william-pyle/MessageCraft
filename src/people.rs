//! People: MailCraft's contacts, seen and edited from MessageCraft (through `mailcraft.rs`), laid
//! out like MailCraft's People module: the list on the left, the card on the right.

use std::collections::HashMap;

use egui::{Align2, Margin, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::App;
use crate::icons::{self, Icon};
use crate::mailcraft::{self, Contact, Labeled, Request};
use crate::theme;
use crate::widgets;

#[derive(Clone, PartialEq)]
pub enum ContactsState {
    /// Not asked yet.
    Unknown,
    Loading,
    /// MailCraft isn't sharing its contacts right now (with why, when it's more than "not on").
    Off(Option<String>),
    Ready,
}

pub struct People {
    pub open: bool,
    pub state: ContactsState,
    pub contacts: Vec<Contact>,
    pub by_phone: HashMap<String, String>,
    pub selected: Option<i64>,
    pub search: String,
    /// The card being edited (id 0: a new contact).
    pub editing: Option<Contact>,
    pub busy: bool,
    pub confirm_delete: Option<i64>,
    pub link: Option<mailcraft::Link>,
    /// Ask again then (MailCraft was just opened and needs a moment to start sharing).
    pub retry_at: Option<std::time::Instant>,
    pub retries: u32,
    /// MessageCraft just started MailCraft with sharing on: wait for it a little.
    pub launched: bool,
}

impl Default for People {
    fn default() -> Self {
        People {
            open: false,
            state: ContactsState::Unknown,
            contacts: Vec::new(),
            by_phone: HashMap::new(),
            selected: None,
            search: String::new(),
            editing: None,
            busy: false,
            confirm_delete: None,
            link: None,
            retry_at: None,
            retries: 0,
            launched: false,
        }
    }
}

impl People {
    pub fn index(&mut self) {
        self.by_phone.clear();
        for c in &self.contacts {
            let name = c.label();
            if name.is_empty() {
                continue;
            }
            for p in &c.phones {
                if let Some(k) = mailcraft::phone_key(&p.value) {
                    self.by_phone.entry(k).or_insert_with(|| name.clone());
                }
            }
        }
    }

    pub fn name_for(&self, number: &str) -> Option<&str> {
        mailcraft::phone_key(number).and_then(|k| self.by_phone.get(&k)).map(String::as_str)
    }

    pub fn contact_for(&self, number: &str) -> Option<&Contact> {
        let key = mailcraft::phone_key(number)?;
        self.contacts.iter().find(|c| c.phones.iter().any(|p| mailcraft::phone_key(&p.value).as_deref() == Some(key.as_str())))
    }

}

/// Text that is only a phone number (no letters): a name worth replacing with a contact's.
pub fn looks_like_number(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_alphabetic()) && s.chars().filter(|c| c.is_ascii_digit()).count() >= 3
}

impl App {
    pub fn contacts_load(&mut self) {
        self.people.retry_at = None;
        if let Some(link) = &self.people.link {
            if self.people.state != ContactsState::Ready {
                self.people.state = ContactsState::Loading;
            }
            link.send(Request::Load);
        }
    }

    /// Answers from MailCraft.
    pub fn contacts_events(&mut self) {
        let mut replies = Vec::new();
        if let Some(link) = &self.people.link {
            while let Ok(r) = link.rx.try_recv() {
                replies.push(r);
            }
        }
        if self.people.retry_at.is_some_and(|t| t <= std::time::Instant::now()) {
            self.contacts_load();
        }
        for r in replies {
            match r {
                mailcraft::Reply::Off(why) => {
                    self.people.busy = false;
                    // MailCraft was just opened: give it a few more tries to start sharing.
                    if self.people.launched && why.is_none() && self.people.retries < 8 {
                        self.people.retries += 1;
                        self.people.retry_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                    } else {
                        self.people.retries = 0;
                        self.people.launched = false;
                        self.people.state = ContactsState::Off(why);
                    }
                }
                mailcraft::Reply::Loaded(list) => {
                    self.people.retries = 0;
                    self.people.launched = false;
                    self.people.contacts = list;
                    self.people.state = ContactsState::Ready;
                    self.people.index();
                    self.apply_contact_names();
                }
                mailcraft::Reply::Saved(id) => {
                    self.people.busy = false;
                    self.people.editing = None;
                    self.people.selected = Some(id);
                    self.notify("Saved to MailCraft's contacts.", false);
                    self.contacts_load();
                }
                mailcraft::Reply::Deleted(id) => {
                    self.people.busy = false;
                    self.people.contacts.retain(|c| c.id != id);
                    self.people.index();
                    self.people.selected = None;
                    self.notify("Deleted from MailCraft's contacts.", false);
                }
                mailcraft::Reply::Failed(e) => {
                    self.people.busy = false;
                    self.notify(format!("MailCraft said: {e}"), true);
                }
            }
        }
    }

    /// Show contact names for conversations the phone lists only by number.
    pub fn apply_contact_names(&mut self) {
        if self.people.by_phone.is_empty() {
            return;
        }
        let people = &self.people;
        for c in self.convs.values_mut() {
            for p in c.participants.iter_mut().filter(|p| !p.is_me) {
                if p.name.is_empty() || looks_like_number(&p.name) {
                    if let Some(n) = people.name_for(&p.number) {
                        p.name = n.to_string();
                    }
                }
            }
            if !c.group && (c.name.is_empty() || looks_like_number(&c.name)) {
                if let Some(n) = c.participants.iter().find(|p| !p.is_me).and_then(|p| people.name_for(&p.number)) {
                    c.name = n.to_string();
                }
            }
        }
    }

    /// Start (or open) a conversation with a contact's number.
    pub fn text_number(&mut self, number: &str) {
        let key = mailcraft::phone_key(number);
        let existing = self
            .convs
            .values()
            .filter(|c| !c.group)
            .find(|c| c.participants.iter().any(|p| !p.is_me && mailcraft::phone_key(&p.number) == key))
            .map(|c| (c.id.clone(), c.folder.clone()));
        self.people.open = false;
        if let Some((id, folder)) = existing {
            self.folder = match folder.as_str() {
                "archive" => crate::model::Folder::Archive,
                "spam" => crate::model::Folder::Spam,
                _ => crate::model::Folder::Inbox,
            };
            self.selected = None;
            self.select(&id);
        } else if self.conn.paired_view() {
            self.new_chat = Some(crate::app::NewChat { numbers: number.to_string(), busy: true, ..Default::default() });
            self.start_new_chat();
        } else {
            self.notify("Pair MessageCraft with your phone first.", true);
        }
    }

    /// Add the person in a conversation to MailCraft's contacts.
    pub fn add_contact_from(&mut self, name: &str, number: &str) {
        let (first, last) = if looks_like_number(name) || name.is_empty() {
            (String::new(), String::new())
        } else {
            match name.rsplit_once(' ') {
                Some((f, l)) => (f.to_string(), l.to_string()),
                None => (name.to_string(), String::new()),
            }
        };
        self.people.open = true;
        self.backstage = None;
        self.people.selected = None;
        self.people.editing = Some(Contact { first, last, phones: vec![Labeled { label: "mobile".into(), value: number.to_string() }], ..Default::default() });
        self.contacts_load();
    }

    // -----------------------------------------------------------------------------------------

    pub fn people_list(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.expand_to_include_rect(rect);
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);

        let header = Rect::from_min_size(rect.min, vec2(rect.width(), 48.0));
        ui.painter().text(pos2(header.min.x + 18.0, header.center().y), Align2::LEFT_CENTER, "Contacts", theme::regular(15.0), t.accent_text);
        ui.painter().rect_filled(Rect::from_min_size(pos2(header.min.x + 18.0, header.center().y + 13.0), vec2(62.0, 2.0)), 1.0, t.accent);
        ui.painter().text(pos2(header.max.x - 16.0, header.center().y), Align2::RIGHT_CENTER, "from MailCraft", theme::regular(12.0), t.text_dim);

        // Search
        let sr = Rect::from_min_size(pos2(rect.min.x + 12.0, header.max.y + 2.0), vec2(rect.width() - 24.0, 30.0));
        ui.painter().rect_filled(sr, 6.0, t.input);
        ui.painter().rect_stroke(sr, 6.0, Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
        icons::paint(ui.painter(), Rect::from_min_size(pos2(sr.min.x + 8.0, sr.center().y - 7.0), Vec2::splat(14.0)), Icon::Search, t.text_dim, t.text_dim);
        let mut su = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(pos2(sr.min.x + 28.0, sr.min.y + 3.0), pos2(sr.max.x - 6.0, sr.max.y - 3.0))));
        su.add(
            egui::TextEdit::singleline(&mut self.people.search)
                .frame(egui::Frame::NONE)
                .hint_text(egui::RichText::new("Search contacts").color(t.text_dim))
                .font(theme::regular(12.5))
                .desired_width(sr.width() - 36.0),
        );

        let body = Rect::from_min_max(pos2(rect.min.x + 4.0, sr.max.y + 8.0), pos2(rect.max.x - 2.0, rect.max.y - 4.0));
        let mut u = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut u;
        match &self.people.state {
            ContactsState::Ready => {}
            ContactsState::Loading | ContactsState::Unknown => {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| ui.label(egui::RichText::new("Reading MailCraft's contacts…").font(theme::regular(13.0)).color(t.text_dim)));
                return;
            }
            ContactsState::Off(_) => {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| ui.label(egui::RichText::new("MailCraft isn't sharing its contacts.").font(theme::regular(13.0)).color(t.text_dim)));
                return;
            }
        }
        let q = self.people.search.trim().to_lowercase();
        let qd: String = q.chars().filter(|c| c.is_ascii_digit()).collect();
        let shown: Vec<Contact> = self
            .people
            .contacts
            .iter()
            .filter(|c| {
                q.is_empty()
                    || c.label().to_lowercase().contains(&q)
                    || c.company.to_lowercase().contains(&q)
                    || c.emails.iter().any(|e| e.value.to_lowercase().contains(&q))
                    || (!qd.is_empty() && c.phones.iter().any(|p| p.value.chars().filter(|c| c.is_ascii_digit()).collect::<String>().contains(&qd)))
            })
            .cloned()
            .collect();
        if shown.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| ui.label(egui::RichText::new(if q.is_empty() { "No contacts in MailCraft yet." } else { "No contacts match." }).font(theme::regular(13.0)).color(t.text_dim)));
            return;
        }
        let mut clicked = None;
        ScrollArea::vertical().auto_shrink([false, false]).id_salt("people_list").show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let w = ui.available_width();
            let mut last_letter = None;
            for c in &shown {
                let label = c.label();
                let letter = label.chars().next().map(|ch| ch.to_ascii_uppercase()).filter(|ch| ch.is_alphabetic()).unwrap_or('#');
                if last_letter != Some(letter) {
                    let (gr, _) = ui.allocate_exact_size(vec2(w, 25.0), Sense::hover());
                    ui.painter().text(pos2(gr.min.x + 14.0, gr.center().y + 2.0), Align2::LEFT_CENTER, letter.to_string(), theme::medium(12.0), t.group_label);
                    last_letter = Some(letter);
                }
                let (r, resp) = ui.allocate_exact_size(vec2(w, 56.0), Sense::click());
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, self.people.selected == Some(c.id), label.clone()));
                if !ui.is_rect_visible(r) {
                    if resp.clicked() {
                        clicked = Some(c.id);
                    }
                    continue;
                }
                let sel = self.people.selected == Some(c.id) && self.people.editing.as_ref().is_none_or(|e| e.id == c.id);
                let p = ui.painter();
                let row = Rect::from_min_max(pos2(r.min.x + 2.0, r.min.y + 1.0), pos2(r.max.x - 6.0, r.max.y - 1.0));
                if sel {
                    p.rect_filled(row, 4.0, t.selection);
                    p.rect_filled(Rect::from_min_size(pos2(row.min.x, row.min.y + 6.0), vec2(3.0, row.height() - 12.0)), 1.5, t.selection_edge);
                } else if resp.hovered() {
                    p.rect_filled(row, 4.0, t.hover_subtle);
                }
                let key = c.emails.first().map(|e| e.value.clone()).or_else(|| c.phones.first().map(|p| p.value.clone())).unwrap_or_else(|| label.clone());
                widgets::avatar(p, &t, pos2(row.min.x + 26.0, row.center().y), 15.0, &label, &key, "", false);
                let left = row.min.x + 50.0;
                widgets::clipped(p, pos2(left, row.min.y + 19.0), row.max.x - left - 30.0, &label, theme::regular(14.0), t.text);
                let sub = c.phones.first().map(|p| p.value.clone()).or_else(|| c.emails.first().map(|e| e.value.clone())).unwrap_or_else(|| c.company.clone());
                widgets::clipped(p, pos2(left, row.min.y + 38.0), row.max.x - left - 30.0, &sub, theme::regular(11.5), t.text_dim);
                if c.favorite {
                    icons::paint(p, Rect::from_center_size(pos2(row.max.x - 16.0, row.center().y), Vec2::splat(14.0)), Icon::Star, t.icon_yellow, t.icon_yellow);
                }
                if resp.clicked() {
                    clicked = Some(c.id);
                }
            }
            ui.add_space(8.0);
        });
        if let Some(id) = clicked {
            self.people.selected = Some(id);
            self.people.editing = None;
        }
    }

    pub fn people_card(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);

        if let ContactsState::Off(why) = self.people.state.clone() {
            self.sharing_off_card(ui, rect, why);
            return;
        }
        if self.people.editing.is_some() {
            self.contact_editor(ui, rect);
            return;
        }
        let Some(c) = self.people.selected.and_then(|id| self.people.contacts.iter().find(|c| c.id == id)).cloned() else {
            let ce = rect.center();
            icons::paint(ui.painter(), Rect::from_center_size(pos2(ce.x, ce.y - 40.0), Vec2::splat(56.0)), Icon::People, t.text_disabled, t.accent);
            ui.painter().text(pos2(ce.x, ce.y + 8.0), Align2::CENTER_TOP, "Select a contact to see it", theme::medium(15.0), t.text);
            ui.painter().text(pos2(ce.x, ce.y + 32.0), Align2::CENTER_TOP, "These are MailCraft's contacts: changes here show in MailCraft too.", theme::regular(12.5), t.text_dim);
            return;
        };

        let inner = rect.shrink2(vec2(28.0, 24.0));
        let mut u = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut u;
        let label = c.label();
        ui.horizontal(|ui| {
            let (ar, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
            let key = c.emails.first().map(|e| e.value.clone()).or_else(|| c.phones.first().map(|p| p.value.clone())).unwrap_or_else(|| label.clone());
            widgets::avatar(ui.painter(), &t, ar.center(), 30.0, &label, &key, "", false);
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(&label).font(theme::semibold(22.0)).color(t.text));
                let role = [c.title.as_str(), c.company.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(", ");
                if !role.is_empty() {
                    ui.label(egui::RichText::new(role).font(theme::regular(13.0)).color(t.text_dim));
                }
            });
        });
        ui.add_space(12.0);
        let mut text_to: Option<String> = None;
        let mut edit = false;
        let mut delete = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let phone = c.phones.first().map(|p| p.value.clone());
            if widgets::primary_button(ui, &t, "Text", phone.is_some()).clicked() {
                text_to = phone;
            }
            if widgets::secondary_button(ui, &t, "Edit").clicked() {
                edit = true;
            }
            if widgets::secondary_button(ui, &t, "Delete").clicked() {
                delete = true;
            }
        });
        ui.add_space(16.0);
        ScrollArea::vertical().auto_shrink([false, false]).id_salt(("card", c.id)).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let section = |ui: &mut Ui, title: &str| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(title).font(theme::semibold(13.5)).color(t.text));
                let w = ui.available_width().min(560.0);
                let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
                ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, t.rule));
            };
            if !c.phones.is_empty() {
                section(ui, "Phone");
                for p in &c.phones {
                    let mut clicked = false;
                    ui.horizontal(|ui| {
                        ui.add_sized([90.0, 20.0], egui::Label::new(egui::RichText::new(cap(&p.label)).font(theme::regular(12.5)).color(t.text_dim)));
                        if ui.link(egui::RichText::new(&p.value).font(theme::regular(13.5))).on_hover_text("Text this number").clicked() {
                            clicked = true;
                        }
                    });
                    if clicked {
                        text_to = Some(p.value.clone());
                    }
                }
            }
            if !c.emails.is_empty() {
                section(ui, "Email");
                for e in &c.emails {
                    ui.horizontal(|ui| {
                        ui.add_sized([90.0, 20.0], egui::Label::new(egui::RichText::new(cap(&e.label)).font(theme::regular(12.5)).color(t.text_dim)));
                        ui.label(egui::RichText::new(&e.value).font(theme::regular(13.5)).color(t.text));
                    });
                }
            }
            if !c.notes.trim().is_empty() {
                section(ui, "Notes");
                ui.add(egui::Label::new(egui::RichText::new(&c.notes).font(theme::regular(13.0)).color(t.text)).wrap());
            }
        });
        if let Some(n) = text_to {
            self.text_number(&n);
        }
        if edit {
            self.people.editing = Some(c.clone());
        }
        if delete {
            self.people.confirm_delete = Some(c.id);
        }
    }

    fn contact_editor(&mut self, ui: &mut Ui, rect: Rect) {
        let t = self.t;
        let inner = rect.shrink2(vec2(28.0, 24.0));
        let mut u = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut u;
        let busy = self.people.busy;
        let Some(c) = self.people.editing.as_mut() else { return };
        let title = if c.id == 0 { "New contact" } else { "Edit contact" };
        ui.label(egui::RichText::new(title).font(theme::semibold(20.0)).color(t.text));
        ui.label(egui::RichText::new("Saved into MailCraft's contacts.").font(theme::regular(12.5)).color(t.text_dim));
        ui.add_space(10.0);
        let mut save = false;
        let mut cancel = false;
        ScrollArea::vertical().auto_shrink([false, false]).max_height(inner.height() - 110.0).id_salt("editor").show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let field = |ui: &mut Ui, label: &str, value: &mut String| {
                ui.horizontal(|ui| {
                    ui.add_sized([110.0, 28.0], egui::Label::new(egui::RichText::new(label).font(theme::regular(13.0)).color(t.text_dim)));
                    ui.add(egui::TextEdit::singleline(value).desired_width(320.0).font(theme::regular(13.5)).margin(Margin::symmetric(8, 5)));
                });
            };
            field(ui, "First name", &mut c.first);
            field(ui, "Last name", &mut c.last);
            field(ui, "Company", &mut c.company);
            field(ui, "Job title", &mut c.title);
            ui.add_space(4.0);
            labeled_list(ui, &t, "Phone", &mut c.phones, &["mobile", "home", "work", "other"]);
            labeled_list(ui, &t, "Email", &mut c.emails, &["home", "work", "other"]);
            ui.horizontal(|ui| {
                ui.add_sized([110.0, 28.0], egui::Label::new(egui::RichText::new("Notes").font(theme::regular(13.0)).color(t.text_dim)));
                ui.add(egui::TextEdit::multiline(&mut c.notes).desired_width(320.0).desired_rows(3).font(theme::regular(13.0)).margin(Margin::symmetric(8, 5)));
            });
            ui.horizontal(|ui| {
                ui.add_space(114.0);
                ui.checkbox(&mut c.favorite, egui::RichText::new("Favorite").font(theme::regular(13.0)));
            });
        });
        ui.add_space(10.0);
        let ok = !(c.first.trim().is_empty() && c.last.trim().is_empty() && c.company.trim().is_empty() && c.emails.iter().all(|e| e.value.trim().is_empty()));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if widgets::primary_button(ui, &t, if busy { "Saving…" } else { "Save" }, ok && !busy).clicked() {
                save = true;
            }
            if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                cancel = true;
            }
            if !ok {
                ui.label(egui::RichText::new("A contact needs a name, a company or an email address.").font(theme::regular(12.0)).color(t.text_dim));
            }
        });
        if save {
            let mut draft = c.clone();
            if let Some(link) = &self.people.link {
                self.people.busy = true;
                link.send(Request::Save(draft));
            } else {
                if draft.id == 0 {
                    draft.id = self.people.contacts.iter().map(|c| c.id).max().unwrap_or(0) + 1;
                }
                draft.name = String::new();
                let id = draft.id;
                self.people.contacts.retain(|c| c.id != id);
                self.people.contacts.push(draft);
                self.people.contacts.sort_by_key(|c| c.label().to_lowercase());
                self.people.index();
                self.people.editing = None;
                self.people.selected = Some(id);
                self.apply_contact_names();
            }
        }
        if cancel {
            self.people.editing = None;
        }
    }

    fn sharing_off_card(&mut self, ui: &mut Ui, rect: Rect, why: Option<String>) {
        let t = self.t;
        let card = Rect::from_center_size(rect.center(), vec2((rect.width() - 80.0).clamp(300.0, 560.0), 320.0));
        let mut u = ui.new_child(egui::UiBuilder::new().max_rect(card).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut u;
        ui.spacing_mut().item_spacing.y = 10.0;
        ui.label(egui::RichText::new("Use your MailCraft contacts").font(theme::semibold(20.0)).color(t.text));
        let installed = mailcraft::mailcraft_exe().is_some();
        let running = mailcraft::mailcraft_running();
        let text = if !installed {
            "MailCraft isn't installed on this computer. Install it to share its contacts with MessageCraft.".to_string()
        } else if running {
            "MailCraft is open, but it isn't sharing its contacts. Close MailCraft, then click the button below: MessageCraft opens it again with contact sharing on.".to_string()
        } else {
            "MessageCraft can show and edit the contacts in MailCraft, and use their names for your conversations. MailCraft shares them while it runs with sharing on.".to_string()
        };
        ui.add(egui::Label::new(egui::RichText::new(text).font(theme::regular(13.5)).color(t.text_dim)).wrap());
        if let Some(w) = why {
            ui.add(egui::Label::new(egui::RichText::new(w).font(theme::regular(12.5)).color(t.red)).wrap());
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new("Sharing uses MailCraft's own local connection: only programs on this computer, run by you, can use it, and MailCraft checks a secret key it keeps for you.")
                    .font(theme::regular(12.0))
                    .color(t.text_dim),
            )
            .wrap(),
        );
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            if installed && widgets::primary_button(ui, &t, "Open MailCraft with contact sharing", !running).clicked() {
                match mailcraft::open_mailcraft_sharing() {
                    Ok(()) => {
                        self.people.launched = true;
                        self.people.retries = 0;
                        self.people.state = ContactsState::Loading;
                        self.people.retry_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
                    }
                    Err(e) => self.notify(e, true),
                }
            }
            if widgets::secondary_button(ui, &t, "Try again").clicked() {
                self.people.state = ContactsState::Loading;
                self.contacts_load();
            }
        });
    }

    pub fn people_ribbon(&mut self, ui: &mut Ui) {
        let t = self.t;
        let ready = self.people.state == ContactsState::Ready;
        let sel = self.people.selected.and_then(|id| self.people.contacts.iter().find(|c| c.id == id)).cloned();
        if widgets::ribbon_button(ui, &t, Icon::Person, "New Contact", false, ready, false).clicked() {
            self.people.editing = Some(Contact { phones: vec![Labeled { label: "mobile".into(), value: String::new() }], ..Default::default() });
        }
        widgets::ribbon_rule(ui, &t);
        if widgets::ribbon_button(ui, &t, Icon::Chat, "Text", false, sel.as_ref().is_some_and(|c| !c.phones.is_empty()), false).clicked() {
            if let Some(n) = sel.as_ref().and_then(|c| c.phones.first()).map(|p| p.value.clone()) {
                self.text_number(&n);
            }
        }
        if widgets::ribbon_button(ui, &t, Icon::Copy, "Edit", false, sel.is_some(), false).clicked() {
            self.people.editing = sel.clone();
        }
        if widgets::ribbon_button(ui, &t, Icon::Trash, "Delete", false, sel.is_some(), false).clicked() {
            self.people.confirm_delete = sel.as_ref().map(|c| c.id);
        }
        widgets::ribbon_rule(ui, &t);
        if widgets::ribbon_button(ui, &t, Icon::Sync, "Refresh", self.people.state == ContactsState::Loading, true, false).clicked() {
            self.contacts_load();
        }
        if widgets::ribbon_button(ui, &t, Icon::Chat, "Back to Messages", false, true, false).clicked() {
            self.people.open = false;
        }
    }

    pub fn people_dialogs(&mut self, ctx: &egui::Context) {
        let t = self.t;
        let Some(id) = self.people.confirm_delete else { return };
        let name = self.people.contacts.iter().find(|c| c.id == id).map(|c| c.label()).unwrap_or_default();
        let mut close = false;
        let mut go = false;
        let modal = egui::Modal::new(egui::Id::unique("delete_contact")).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label(egui::RichText::new(format!("Delete {name}?")).font(theme::semibold(17.0)).color(t.text));
            ui.add(egui::Label::new(egui::RichText::new("The contact is deleted from MailCraft too.").font(theme::regular(13.0)).color(t.text_dim)).wrap());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &t, "Delete", true).clicked() {
                    go = true;
                }
                if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                    close = true;
                }
            });
        });
        if go {
            if let Some(link) = &self.people.link {
                self.people.busy = true;
                link.send(Request::Delete(id));
            } else {
                self.people.contacts.retain(|c| c.id != id);
                self.people.index();
                self.people.selected = None;
            }
            self.people.confirm_delete = None;
        } else if close || modal.should_close() {
            self.people.confirm_delete = None;
        }
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Other".into(),
    }
}

fn labeled_list(ui: &mut Ui, t: &theme::Tokens, title: &str, items: &mut Vec<Labeled>, labels: &[&str]) {
    let mut remove = None;
    for (i, item) in items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_sized([110.0, 28.0], egui::Label::new(egui::RichText::new(if i == 0 { title } else { "" }).font(theme::regular(13.0)).color(t.text_dim)));
            egui::ComboBox::from_id_salt((title, i)).width(84.0).selected_text(cap(&item.label)).show_ui(ui, |ui| {
                for l in labels {
                    ui.selectable_value(&mut item.label, l.to_string(), cap(l));
                }
            });
            ui.add(egui::TextEdit::singleline(&mut item.value).desired_width(226.0).font(theme::regular(13.5)).margin(Margin::symmetric(8, 5)));
            if widgets::icon_button(ui, t, Icon::Close, 24.0, "Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        items.remove(i);
    }
    ui.horizontal(|ui| {
        ui.add_space(114.0);
        if ui.link(egui::RichText::new(format!("Add {}", title.to_lowercase())).font(theme::regular(12.5))).clicked() {
            items.push(Labeled { label: labels[0].to_string(), value: String::new() });
        }
    });
}
