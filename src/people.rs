//! ContactCraft contacts, indexed by phone number. ContactCraft owns all editing.
use std::collections::HashMap;
use egui::{Color32, Rect, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2, pos2, vec2};
use crate::app::App;
use crate::contactcraft::{self, Contact, Paths, Request, Reply, EditOutcome};
use crate::icons::Icon;
use crate::{theme, widgets};

#[derive(Clone, PartialEq)]
pub enum ContactsState { Unknown, Off(String), Ready }

pub struct People {
    pub open: bool,
    pub state: ContactsState,
    pub contacts: Vec<Paths>,
    pub by_phone: HashMap<String, usize>,
    pub selected: Option<String>,
    pub link: Option<contactcraft::Link>,
    pub exe: Option<std::path::PathBuf>,
    pub photos: HashMap<String, TextureHandle>,
    pub backgrounds: HashMap<String, TextureHandle>,
}
impl Default for People {
    fn default() -> Self {
        Self { open: false, state: ContactsState::Unknown, contacts: Vec::new(), by_phone: HashMap::new(), selected: None, link: None, exe: None, photos: HashMap::new(), backgrounds: HashMap::new() }
    }
}
impl People {
    pub fn index(&mut self) {
        self.by_phone.clear();
        for (i, p) in self.contacts.iter().enumerate() {
            for k in p.contact.phone_keys() { self.by_phone.entry(k).or_insert(i); }
        }
    }
    pub fn paths_for(&self, number: &str) -> Option<&Paths> {
        self.by_phone.get(&contactcraft::phone_key(number)?).and_then(|i| self.contacts.get(*i))
    }
    pub fn contact_for(&self, number: &str) -> Option<&Contact> { self.paths_for(number).map(|p| &p.contact) }
    pub fn name_for(&self, number: &str) -> Option<String> { self.contact_for(number).map(Contact::display_name) }
    pub fn selected_contact(&self) -> Option<&Contact> {
        self.contacts.iter().find(|p| self.selected.as_ref() == Some(&p.contact.id)).map(|p| &p.contact)
    }
    pub fn background_for(&self, number: &str) -> Option<&TextureHandle> {
        self.paths_for(number).and_then(|p| p.background.as_ref()).and_then(|p| self.backgrounds.get(&p.to_string_lossy().to_string()))
    }
    pub fn avatar(&self, ui: &Ui, center: egui::Pos2, radius: f32, name: &str, number: &str, hex: &str, group: bool, t: &theme::Tokens) {
        if !group {
            if let Some(p) = self.paths_for(number) {
                let c = &p.contact;
                let r = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
                let corner = match c.theme.avatar_shape.as_str() { "square" => 0, "rounded" => (radius * 0.35) as u8, _ => radius as u8 };
                if let Some(tex) = p.photo.as_ref().and_then(|p| self.photos.get(&p.to_string_lossy().to_string())) {
                    egui::Image::new((tex.id(), r.size())).corner_radius(corner).paint_at(ui, r);
                    return;
                }
                let fill = contactcraft::color(&c.theme.accent).unwrap_or_else(|| theme::avatar_color(number, hex));
                ui.painter().rect_filled(r, corner, fill);
                ui.painter().text(center, egui::Align2::CENTER_CENTER, c.initials(), theme::semibold(radius * 0.78), Color32::WHITE);
                return;
            }
        }
        widgets::avatar(ui.painter(), t, center, radius, name, number, hex, group);
    }
}
pub fn looks_like_number(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(char::is_alphabetic) && s.chars().filter(char::is_ascii_digit).count() >= 3
}
impl App {
    pub fn contacts_load(&mut self) {
        if let Some(link) = &self.people.link { link.send(Request::Refresh); }
    }
    pub fn contacts_events(&mut self, ctx: &egui::Context) {
        let mut replies = Vec::new();
        if let Some(link) = &self.people.link { while let Ok(r) = link.rx.try_recv() { replies.push(r); } }
        for reply in replies {
            match reply {
                Reply::Unavailable(e) => self.people.state = ContactsState::Off(e),
                Reply::All { contacts, exe } => {
                    self.people.photos.clear();
                    self.people.backgrounds.clear();
                    for p in &contacts {
                        if let (Some(path), Some(img)) = (&p.photo, &p.photo_image) {
                            self.people.photos.insert(path.to_string_lossy().to_string(), ctx.load_texture(path.to_string_lossy(), img.clone(), Default::default()));
                        }
                        if let (Some(path), Some(img)) = (&p.background, &p.background_image) {
                            self.people.backgrounds.insert(path.to_string_lossy().to_string(), ctx.load_texture(path.to_string_lossy(), img.clone(), Default::default()));
                        }
                    }
                    self.people.contacts = contacts;
                    self.people.exe = exe;
                    self.people.state = ContactsState::Ready;
                    self.people.index();
                    self.apply_contact_names();
                }
                Reply::Edited(EditOutcome::Saved(id)) => {
                    if !id.is_empty() { self.people.selected = Some(id); }
                    self.contacts_load();
                }
                Reply::Edited(EditOutcome::Cancelled) => {}
                Reply::Edited(EditOutcome::Failed(e)) => self.notify(e, true),
            }
        }
    }
    pub fn apply_contact_names(&mut self) {
        let people = &self.people;
        for c in self.convs.values_mut() {
            for p in c.participants.iter_mut().filter(|p| !p.is_me) {
                if let Some(n) = people.name_for(&p.number) { p.name = n; }
                else if looks_like_number(&p.name) { p.name.clear(); }
            }
            if !c.group {
                if let Some(n) = people.name_for(c.contact_number()) { c.name = n; }
                else if looks_like_number(&c.name) { c.name.clear(); }
            }
        }
    }
    pub fn text_number(&mut self, number: &str) {
        let key = contactcraft::phone_key(number);
        let existing = self.convs.values().filter(|c| !c.group).find(|c| c.participants.iter().any(|p| !p.is_me && contactcraft::phone_key(&p.number) == key)).map(|c| (c.id.clone(), c.folder.clone()));
        self.people.open = false;
        if let Some((id, folder)) = existing {
            self.folder = match folder.as_str() { "archive" => crate::model::Folder::Archive, "spam" => crate::model::Folder::Spam, _ => crate::model::Folder::Inbox };
            self.selected = None;
            self.select(&id);
        } else if self.conn.paired_view() {
            self.new_chat = Some(crate::app::NewChat { numbers: number.to_string(), busy: true, ..Default::default() });
            self.start_new_chat();
        } else { self.notify("Pair MessageCraft with your phone first.", true); }
    }
    pub fn add_contact_from(&mut self, _name: &str, number: &str) { self.edit_contact(vec!["--edit-phone".into(), number.into()]); }
    fn edit_contact(&mut self, args: Vec<String>) {
        if self.people.exe.is_none() { self.notify("ContactCraft's editor is not installed on this computer.", true); }
        else if let Some(link) = &self.people.link { link.send(Request::Edit(args)); }
    }
    pub fn people_list(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
        ui.label(egui::RichText::new("Contacts").font(theme::semibold(15.0)).color(t.text));
        match &self.people.state {
            ContactsState::Off(e) => { ui.label(format!("Couldn't read ContactCraft contacts: {e}")); return; }
            ContactsState::Unknown => { ui.label("Reading ContactCraft contacts…"); return; }
            ContactsState::Ready => {}
        }
        let q = self.search.trim().to_lowercase();
        ScrollArea::vertical().show(ui, |ui| {
            for p in &self.people.contacts {
                let c = &p.contact;
                let name = c.display_name();
                if !q.is_empty() && !c.search_text().contains(&q) { continue; }
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 55.0), Sense::click());
                if self.people.selected.as_ref() == Some(&c.id) { ui.painter().rect_filled(r, 4.0, t.selection); }
                let number = c.phones.first().map(|p| p.value.as_str()).unwrap_or("");
                self.people.avatar(ui, pos2(r.min.x + 25.0, r.center().y), 16.0, &name, number, "", false, &t);
                widgets::clipped(ui.painter(), pos2(r.min.x + 50.0, r.min.y + 19.0), r.width() - 65.0, &name, theme::regular(14.0), t.text);
                widgets::clipped(ui.painter(), pos2(r.min.x + 50.0, r.min.y + 38.0), r.width() - 65.0, number, theme::regular(11.5), t.text_dim);
                if resp.clicked() { self.people.selected = Some(c.id.clone()); }
            }
        });
    }
    pub fn people_card(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
        let Some(c) = self.people.selected_contact().cloned() else { ui.centered_and_justified(|ui| ui.label("Select a ContactCraft contact")); return; };
        let name = c.display_name();
        let accent = contactcraft::color(&c.theme.accent).unwrap_or_else(|| theme::avatar_color(&c.id, &c.id));
        let (banner, _) = ui.allocate_exact_size(vec2(ui.available_width(), 155.0), Sense::hover());
        ui.painter().rect_filled(banner, egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, accent);
        let number = c.phones.first().map(|p| p.value.as_str()).unwrap_or("");
        self.people.avatar(ui, pos2(banner.min.x + 85.0, banner.max.y - 8.0), 52.0, &name, number, "", false, &t);
        ui.painter().text(pos2(banner.min.x + 155.0, banner.max.y - 35.0), egui::Align2::LEFT_CENTER, &name, theme::semibold(27.0), Color32::WHITE);
        ui.add_space(19.0);
        let mut text_to = None;
        let mut edit = false;
        let mut theme_edit = false;
        ui.horizontal(|ui| {
            ui.add_space(155.0);
            if widgets::primary_button(ui, &t, "Edit", self.people.exe.is_some()).clicked() { edit = true; }
            if widgets::secondary_button(ui, &t, "Theme").clicked() { theme_edit = true; }
            if widgets::secondary_button(ui, &t, "Text").clicked() { text_to = c.phones.first().map(|p| p.value.clone()); }
            if c.favorite { ui.label(egui::RichText::new("★").color(t.accent).size(22.0)); }
        });
        ui.add_space(22.0);
        ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(850.0);
            if !c.phones.is_empty() || !c.emails.is_empty() {
                detail_heading(ui, &t, "Contact");
                for p in &c.phones {
                    detail_row(ui, &t, &p.label, &p.value, true, || text_to = Some(p.value.clone()));
                }
                for e in &c.emails { detail_row(ui, &t, &e.label, &e.address, false, || {}); }
            }
            if !c.addresses.is_empty() {
                detail_heading(ui, &t, "Addresses");
                for a in &c.addresses { detail_row(ui, &t, &a.label, &a.lines().join(", "), false, || {}); }
            }
            if !c.organizations.is_empty() || !c.occupation.is_empty() {
                detail_heading(ui, &t, "Work");
                for o in &c.organizations {
                    detail_row(ui, &t, "Company", &o.company, false, || {});
                    detail_row(ui, &t, "Title", &o.title, false, || {});
                    detail_row(ui, &t, "Department", &o.department, false, || {});
                }
                detail_row(ui, &t, "Occupation", &c.occupation, false, || {});
            }
            if !c.websites.is_empty() || !c.social_profiles.is_empty() {
                detail_heading(ui, &t, "Online");
                for site in &c.websites { detail_row(ui, &t, &site.label, &site.value, false, || {}); }
                for profile in &c.social_profiles { detail_row(ui, &t, &profile.label, &profile.value, false, || {}); }
            }
            if !c.birthday.is_empty() || !c.anniversary.is_empty() || !c.events.is_empty() {
                detail_heading(ui, &t, "Dates");
                detail_row(ui, &t, "Birthday", &c.birthday, false, || {});
                detail_row(ui, &t, "Anniversary", &c.anniversary, false, || {});
                for event in &c.events { detail_row(ui, &t, &event.label, &event.value, false, || {}); }
            }
            if !c.notes.is_empty() || !c.custom_fields.is_empty() {
                detail_heading(ui, &t, "More fields");
                for field in &c.custom_fields { detail_row(ui, &t, &field.label, &field.value, false, || {}); }
                detail_row(ui, &t, "Notes", &c.notes, false, || {});
            }
        });
        if let Some(n) = text_to { self.text_number(&n); }
        if edit { self.edit_contact(vec!["--edit-contact".into(), c.id.clone()]); }
        if theme_edit { self.edit_contact(vec!["--edit-contact".into(), c.id, "--theme-tab".into()]); }
    }
    pub fn people_ribbon(&mut self, ui: &mut Ui) {
        let t = self.t;
        if widgets::ribbon_button(ui, &t, Icon::Person, "New Contact", false, self.people.exe.is_some(), false).clicked() { self.edit_contact(vec!["--new-contact".into()]); }
        widgets::ribbon_rule(ui, &t);
        let selected = self.people.selected_contact().cloned();
        if widgets::ribbon_button(ui, &t, Icon::Chat, "Text", false, selected.as_ref().is_some_and(|c| !c.phones.is_empty()), false).clicked() {
            if let Some(n) = selected.as_ref().and_then(|c| c.phones.first()) { self.text_number(&n.value); }
        }
        if widgets::ribbon_button(ui, &t, Icon::Copy, "Edit", false, selected.is_some() && self.people.exe.is_some(), false).clicked() {
            if let Some(c) = selected { self.edit_contact(vec!["--edit-contact".into(), c.id]); }
        }
        widgets::ribbon_rule(ui, &t);
        if widgets::ribbon_button(ui, &t, Icon::Sync, "Refresh", false, true, false).clicked() { self.contacts_load(); }
        if widgets::ribbon_button(ui, &t, Icon::Chat, "Back to Messages", false, true, false).clicked() { self.people.open = false; }
    }
    pub fn people_dialogs(&mut self, _ctx: &egui::Context) {}
}

fn detail_heading(ui: &mut Ui, t: &theme::Tokens, title: &str) {
    ui.add_space(11.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(egui::RichText::new(title).font(theme::semibold(15.0)).color(t.text));
    });
    ui.separator();
}

fn detail_row(ui: &mut Ui, t: &theme::Tokens, label: &str, value: &str, link: bool, mut on_click: impl FnMut()) {
    if value.trim().is_empty() { return; }
    ui.horizontal_wrapped(|ui| {
        ui.set_min_height(28.0);
        ui.add_sized(vec2(160.0, 24.0), egui::Label::new(egui::RichText::new(if label.is_empty() { "Other" } else { label }).color(t.text_dim)));
        if link {
            if ui.link(value).clicked() { on_click(); }
        } else {
            ui.label(egui::RichText::new(value).color(t.text));
        }
    });
}
