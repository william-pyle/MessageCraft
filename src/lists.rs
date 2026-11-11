//! The folder pane and the conversation list, laid out like MailCraft's folder pane and message
//! list (24-point folder rows, 25-point group headings, three-line rows with the time on the right).

use egui::{Align2, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{App, Conn};
use crate::icons::{self, Icon};
use crate::model::Folder;
use crate::theme;
use crate::timefmt;
use crate::widgets;

const ROW_H: f32 = 24.0;
const NAME_SIZE: f32 = 12.0;
const HEADING_SIZE: f32 = 14.667;
const NAME_LEFT: f32 = 22.0;
const COUNT_RIGHT: f32 = 16.0;

const GROUP_H: f32 = 25.0;
const HEADER_H: f32 = 48.0;
const SENDER_TEXT: f32 = 15.0;
const LIST_TEXT: f32 = 11.5;
const TIME_RIGHT: f32 = 16.0;

impl App {
    pub fn folder_pane(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.expand_to_include_rect(rect);
        ui.painter().rect_filled(rect, 8.0, t.panel);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(6.0, 10.0))).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut ui;
        ui.spacing_mut().item_spacing.y = 0.0;

        // Collapse arrow, top right (MailCraft hides the pane with it).
        let cr = Rect::from_min_size(pos2(rect.max.x - 30.0, rect.min.y + 8.0), Vec2::splat(22.0));
        let resp = ui.interact(cr, egui::Id::unique("collapse_folders"), Sense::click()).on_hover_text("Hide the folder pane");
        if resp.hovered() {
            ui.painter().rect_filled(cr, 4.0, t.hover);
        }
        icons::paint(ui.painter(), cr.shrink(4.0), Icon::ChevronLeft, t.icon, t.icon);
        if resp.clicked() {
            self.settings.folder_pane = false;
            self.save_settings();
        }

        ui.add_space(8.0);
        if self.heading(ui, "Favorites", self.collapsed_favorites) {
            self.collapsed_favorites = !self.collapsed_favorites;
        }
        if !self.collapsed_favorites {
            self.folder_row(ui, Folder::Inbox, "Inbox", "fav");
        }
        ui.add_space(10.0);
        let account = match &self.phone {
            Some(p) if !p.is_empty() && p.contains('@') => p.clone(),
            _ => "Messages".to_string(),
        };
        if self.heading(ui, &account, self.collapsed_account) {
            self.collapsed_account = !self.collapsed_account;
        }
        if !self.collapsed_account {
            for f in [Folder::Inbox, Folder::Archive, Folder::Spam] {
                self.folder_row(ui, f, f.label(), "acct");
            }
        }

        // The phone's state at the foot of the pane.
        let foot = Rect::from_min_max(pos2(rect.min.x + 10.0, rect.max.y - 54.0), pos2(rect.max.x - 10.0, rect.max.y - 10.0));
        let (col, line1, line2) = match &self.conn {
            Conn::Connected => (t.icon_green, "Connected to your phone", "Messages send and arrive here."),
            Conn::Connecting => (t.icon_yellow, "Connecting…", "Reaching your phone."),
            Conn::Reconnecting(_) => (t.icon_yellow, "Reconnecting…", "The connection dropped."),
            Conn::PhoneOffline(_) => (t.icon_red, "Phone not responding", "Is it on and online?"),
            Conn::OtherSession => (t.icon_yellow, "Open in another window", "Use the status bar to switch here."),
            _ => (t.icon_red, "Not connected", "See File › Info."),
        };
        let p = ui.painter();
        p.circle_filled(pos2(foot.min.x + 6.0, foot.min.y + 12.0), 4.0, col);
        widgets::clipped(p, pos2(foot.min.x + 18.0, foot.min.y + 12.0), foot.width() - 18.0, line1, theme::medium(12.0), t.text);
        widgets::clipped(p, pos2(foot.min.x + 18.0, foot.min.y + 30.0), foot.width() - 18.0, line2, theme::regular(11.0), t.text_dim);
    }

    fn heading(&self, ui: &mut Ui, label: &str, collapsed: bool) -> bool {
        let t = self.t;
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
        let p = ui.painter();
        let chev = Rect::from_center_size(pos2(r.min.x + 9.0, r.center().y), Vec2::splat(12.0));
        icons::paint(p, chev, if collapsed { Icon::ChevronRight } else { Icon::ChevronDown }, t.icon, t.icon);
        widgets::clipped(p, pos2(r.min.x + 18.0, r.center().y), w - 50.0, label, theme::medium(HEADING_SIZE), t.text);
        resp.clicked()
    }

    fn folder_row(&mut self, ui: &mut Ui, f: Folder, label: &str, salt: &str) {
        let t = self.t;
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), Sense::click());
        let resp = resp.on_hover_text(format!("{} conversations", self.folder_count(f)));
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, self.folder == f, label));
        let _ = salt;
        let selected = self.folder == f;
        let p = ui.painter();
        let hl = Rect::from_min_max(pos2(r.min.x + 6.0, r.min.y + 1.0), pos2(r.max.x - 4.0, r.max.y - 1.0));
        if selected {
            p.rect_filled(hl, 4.0, t.accent_soft);
            p.rect_filled(Rect::from_min_size(pos2(hl.min.x, hl.min.y + 5.0), vec2(3.0, hl.height() - 10.0)), 1.5, t.accent);
        } else if resp.hovered() {
            p.rect_filled(hl, 4.0, t.hover_subtle);
        }
        let font = if selected { theme::bold(NAME_SIZE) } else { theme::regular(NAME_SIZE) };
        p.text(pos2(r.min.x + NAME_LEFT, r.center().y), Align2::LEFT_CENTER, label, font, if selected { t.accent_soft_text } else { t.text });
        let unread = self.unread_count(f);
        if unread > 0 {
            widgets::count_pill(p, &t, pos2(r.max.x - COUNT_RIGHT + 6.0, r.center().y), unread, true);
        }
        if resp.clicked() {
            self.open_folder(f);
        }
    }

    pub fn conversation_list(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.expand_to_include_rect(rect);
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);

        // Header: All / Unread, and the sort.
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_H));
        let mut x = header.min.x + 18.0;
        for (label, unread) in [("All", false), ("Unread", true)] {
            let selected = self.unread_only == unread;
            let font = theme::regular(15.0);
            let g = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text);
            let r = Rect::from_min_size(pos2(x - 4.0, header.min.y + 10.0), vec2(g.size().x + 8.0, 30.0));
            let resp = ui.interact(r, egui::Id::unique(("filter", label)), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
            let col = if selected { t.accent_text } else if resp.hovered() { t.text } else { t.text_dim };
            ui.painter().text(pos2(x, r.center().y - 1.0), Align2::LEFT_CENTER, label, font, col);
            if selected {
                ui.painter().rect_filled(Rect::from_min_size(pos2(x, r.max.y - 2.0), vec2(g.size().x, 2.0)), 1.0, t.accent);
            }
            if resp.clicked() {
                self.unread_only = unread;
            }
            x += g.size().x + 18.0;
        }
        let sort_font = theme::regular(12.0);
        ui.painter().text(pos2(header.max.x - 40.0, header.center().y + 1.0), Align2::RIGHT_CENTER, "By Date", sort_font, t.text_dim);
        icons::paint(ui.painter(), Rect::from_center_size(pos2(header.max.x - 28.0, header.center().y + 1.0), Vec2::splat(12.0)), Icon::ChevronDown, t.text_dim, t.text_dim);

        let body = Rect::from_min_max(pos2(rect.min.x + 4.0, header.max.y), pos2(rect.max.x - 2.0, rect.max.y - 4.0));
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut ui;

        let convs: Vec<crate::model::Conversation> = self.visible_conversations().into_iter().cloned().collect();
        if convs.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                let msg = if !self.search.is_empty() {
                    "No conversations match your search."
                } else if self.syncing.contains(self.folder.code()) || matches!(self.conn, Conn::Connecting) {
                    "Syncing your conversations…"
                } else if self.unread_only {
                    "You're all caught up."
                } else {
                    "Nothing here yet."
                };
                ui.label(egui::RichText::new(msg).font(theme::regular(13.0)).color(t.text_dim));
            });
            return;
        }

        let row_h = if self.settings.tighter { 58.0 } else { 72.0 };
        let mut clicked: Option<String> = None;
        ScrollArea::vertical().auto_shrink([false, false]).id_salt(("list", self.folder.code())).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let w = ui.available_width();
            let mut last_group = "";
            for c in &convs {
                let group = if c.pinned { "Pinned" } else { timefmt::group(c.ts) };
                if group != last_group {
                    let (gr, _) = ui.allocate_exact_size(vec2(w, GROUP_H), Sense::hover());
                    ui.painter().text(pos2(gr.min.x + 14.0, gr.center().y + 2.0), Align2::LEFT_CENTER, group, theme::medium(12.0), t.group_label);
                    last_group = group;
                }
                let (r, resp) = ui.allocate_exact_size(vec2(w, row_h), Sense::click());
                let selected_now = self.selected.as_deref() == Some(c.id.as_str());
                let a11y = format!("{}{}, {}", if c.unread { "Unread, " } else { "" }, c.title(), c.snippet);
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected_now, a11y.clone()));
                if !ui.is_rect_visible(r) {
                    if resp.clicked() {
                        clicked = Some(c.id.clone());
                    }
                    continue;
                }
                let selected = self.selected.as_deref() == Some(c.id.as_str());
                let p = ui.painter();
                let row = Rect::from_min_max(pos2(r.min.x + 2.0, r.min.y + 1.0), pos2(r.max.x - 6.0, r.max.y - 1.0));
                if selected {
                    p.rect_filled(row, 4.0, t.selection);
                    p.rect_filled(Rect::from_min_size(pos2(row.min.x, row.min.y + 6.0), vec2(3.0, row.height() - 12.0)), 1.5, t.selection_edge);
                } else if resp.hovered() {
                    p.rect_filled(row, 4.0, t.hover_subtle);
                }
                if c.unread {
                    p.circle_filled(pos2(row.min.x + 10.0, row.min.y + 21.0), 4.0, t.accent);
                }
                // Avatar
                let ac = pos2(row.min.x + 38.0, row.center().y);
                let title = c.title();
                let key = c.participants.iter().find(|p| !p.is_me).map(|p| p.number.clone()).unwrap_or_else(|| c.id.clone());
                let hex = if c.color.is_empty() { c.participants.iter().find(|p| !p.is_me).map(|p| p.color.clone()).unwrap_or_default() } else { c.color.clone() };
                widgets::avatar(p, &t, ac, 16.0, &title, &key, &hex, c.group);

                let left = row.min.x + 64.0;
                let time = timefmt::list(c.ts);
                let tf = if c.unread { theme::semibold(12.0) } else { theme::regular(12.0) };
                let tg = p.layout_no_wrap(time.clone(), tf.clone(), t.text);
                let time_x = row.max.x - TIME_RIGHT;
                let tighter = self.settings.tighter;
                let (y1, y2, y3) = if tighter { (17.0, 37.0, 0.0) } else { (19.0, 39.5, 55.0) };
                p.text(pos2(time_x, row.min.y + y1), Align2::RIGHT_CENTER, time, tf, if c.unread { t.accent_text } else { t.text_dim });
                let name_font = if c.unread { theme::semibold(SENDER_TEXT) } else { theme::regular(SENDER_TEXT) };
                let name_w = (time_x - tg.size().x - 12.0 - left).max(20.0);
                let nw = widgets::clipped(p, pos2(left, row.min.y + y1), name_w, &title, name_font, t.text);
                if c.pinned {
                    icons::paint(p, Rect::from_min_size(pos2(left + nw + 4.0, row.min.y + y1 - 7.0), Vec2::splat(14.0)), Icon::Pin, t.text_dim, t.text_dim);
                }
                let mut snippet = c.snippet.replace('\n', " ");
                if c.snippet_me && !snippet.is_empty() {
                    snippet = format!("You: {snippet}");
                } else if c.group && !c.snippet_from.is_empty() && !snippet.is_empty() {
                    snippet = format!("{}: {snippet}", c.snippet_from);
                }
                let typing = self.typing.contains_key(&c.id);
                if typing {
                    snippet = "Typing…".into();
                }
                let sf = if c.unread { theme::semibold(LIST_TEXT + 0.5) } else { theme::regular(LIST_TEXT + 0.5) };
                widgets::clipped(p, pos2(left, row.min.y + y2), time_x - left, &snippet, sf, if typing { t.accent_text } else if c.unread { t.text } else { t.text_dim });
                if !tighter {
                    let mut meta = if c.group { format!("{} people", c.others().count()) } else { c.numbers() };
                    let kind = if c.kind == "rcs" { "Chat" } else { "SMS" };
                    meta = if meta.is_empty() { kind.to_string() } else { format!("{kind} · {meta}") };
                    widgets::clipped(p, pos2(left, row.min.y + y3), time_x - left, &meta, theme::regular(LIST_TEXT), t.text_disabled);
                }
                // The faint rule under a row.
                if !selected {
                    p.line_segment([pos2(left, r.max.y - 0.5), pos2(row.max.x - 4.0, r.max.y - 0.5)], Stroke::new(1.0, t.rule));
                }
                if resp.clicked() {
                    clicked = Some(c.id.clone());
                }
            }
            ui.add_space(8.0);
        });
        if let Some(id) = clicked {
            self.select(&id);
        }
    }
}
