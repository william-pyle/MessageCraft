//! The window's frame in MailCraft's layout: the title bar with search, the tab strip, the ribbon,
//! the module rail down the left, and the status bar.

use egui::{Align2, Color32, Frame, Margin, Panel, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{App, Backstage, Conn, NewChat, RibbonTab};
use crate::icons::{self, Icon};
use crate::model::{Appearance, Folder};
use crate::theme;
use crate::timefmt;
use crate::widgets::{self, icon_button, ribbon_button, ribbon_rule};

pub const TITLE_BAR: f32 = 48.0;
const SEARCH_BOX: f32 = 34.0;
const TAB_H: f32 = 30.0;
const BAND_H: f32 = 48.0;
const STATUS_BAR_H: f32 = 30.0;
const RAIL_W: f32 = 48.0;

impl App {
    pub fn draw(&mut self, ui: &mut Ui) {
        let t = self.t;
        Panel::top("title_bar").exact_size(TITLE_BAR).frame(Frame::new().fill(t.title_bar)).show(ui, |ui| self.title_bar(ui));
        Panel::bottom("status_bar").exact_size(STATUS_BAR_H).frame(Frame::new().fill(t.status_bar)).show(ui, |ui| self.status_bar(ui));
        Panel::left("rail").exact_size(RAIL_W).resizable(false).show_separator_line(false).frame(Frame::new().fill(t.nav)).show(ui, |ui| self.rail(ui));
        Panel::top("tabs").exact_size(TAB_H).show_separator_line(false).frame(Frame::new().fill(t.tab_strip)).show(ui, |ui| self.tab_strip(ui));

        if let Some(page) = self.backstage {
            egui::CentralPanel::no_frame().show(ui, |ui| self.backstage_view(ui, page));
        } else {
            Panel::top("ribbon")
                .exact_size(BAND_H + 12.0)
                .show_separator_line(false)
                .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 4, right: 10, top: 5, bottom: 7 }))
                .show(ui, |ui| self.ribbon(ui));
            if self.conn.paired_view() {
                if self.settings.folder_pane {
                    Panel::left("folders")
                        .default_size(240.0)
                        .size_range(170.0..=360.0)
                        .resizable(true)
                        .show_separator_line(false)
                        .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 0, right: 6, top: 0, bottom: 8 }))
                        .show(ui, |ui| self.folder_pane(ui));
                }
                Panel::left("list")
                    .default_size(440.0)
                    .size_range(300.0..=720.0)
                    .resizable(true)
                    .show_separator_line(false)
                    .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 0, right: 6, top: 0, bottom: 8 }))
                    .show(ui, |ui| self.conversation_list(ui));
                egui::CentralPanel::no_frame()
                    .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 0, right: 10, top: 0, bottom: 8 }))
                    .show(ui, |ui| self.reading_pane(ui));
            } else {
                egui::CentralPanel::no_frame()
                    .frame(Frame::new().fill(t.canvas).inner_margin(Margin { left: 4, right: 10, top: 0, bottom: 8 }))
                    .show(ui, |ui| self.pairing_view(ui));
            }
        }

        let ctx = ui.ctx().clone();
        self.dialogs(&ctx);
        self.notice_overlay(&ctx);
    }

    // -----------------------------------------------------------------------------------------

    fn title_bar(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter().clone();
        p.line_segment([pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)], Stroke::new(1.0, t.border.linear_multiply(0.6)));

        // App mark: MailCraft's blue tile, with a speech bubble.
        let mark = Rect::from_min_size(pos2(rect.min.x + 12.0, rect.center().y - 12.0), Vec2::splat(24.0));
        p.rect_filled(mark, 6.0, t.accent);
        icons::paint(&p, mark.shrink(4.0), Icon::Chat, Color32::WHITE, Color32::WHITE);

        // Quick access: sync and new conversation.
        let mut qat = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(mark.max.x + 10.0, rect.center().y - 14.0), vec2(80.0, 28.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        qat.spacing_mut().item_spacing.x = 2.0;
        if icon_button(&mut qat, &t, Icon::Sync, 28.0, "Sync now (F5)").clicked() {
            self.sync_now();
        }
        if icon_button(&mut qat, &t, Icon::NewChat, 28.0, "New conversation (Ctrl+N)").clicked() && self.conn.paired_view() {
            self.new_chat = Some(NewChat { focus: true, ..Default::default() });
        }

        // Search, centred like MailCraft's.
        let w = (rect.width() * 0.36).clamp(260.0, 540.0);
        let sr = Rect::from_center_size(pos2(rect.center().x, rect.center().y), vec2(w, SEARCH_BOX));
        let focused = ui.memory(|m| m.focused() == Some(egui::Id::unique("search_box")));
        p.rect_filled(sr, 6.0, t.input);
        p.rect_stroke(sr, 6.0, Stroke::new(1.0, if focused { t.accent } else { t.border_strong }), StrokeKind::Inside);
        icons::paint(&p, Rect::from_min_size(pos2(sr.min.x + 10.0, sr.center().y - 8.0), Vec2::splat(16.0)), Icon::Search, t.text_dim, t.text_dim);
        let edit_rect = Rect::from_min_max(pos2(sr.min.x + 34.0, sr.min.y + 2.0), pos2(sr.max.x - if self.search.is_empty() { 120.0 } else { 34.0 }, sr.max.y - 2.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let edit = egui::TextEdit::singleline(&mut self.search)
            .id(egui::Id::unique("search_box"))
            .frame(Frame::NONE)
            .hint_text(egui::RichText::new("Search conversations and people").color(t.text_dim))
            .font(theme::regular(13.0))
            .text_color(t.text)
            .desired_width(edit_rect.width());
        let resp = child.add(edit);
        if self.focus_search {
            resp.request_focus();
            self.focus_search = false;
        }
        if self.search.is_empty() {
            p.text(pos2(sr.max.x - 12.0, sr.center().y), Align2::RIGHT_CENTER, self.folder.label(), theme::regular(12.0), t.text_dim);
        } else {
            let cr = Rect::from_center_size(pos2(sr.max.x - 17.0, sr.center().y), Vec2::splat(22.0));
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(cr));
            if icon_button(&mut c, &t, Icon::Close, 22.0, "Clear search").clicked() {
                self.search.clear();
            }
        }

        // Right: the phone's state, and the person's avatar.
        let right = Rect::from_min_max(pos2(sr.max.x + 12.0, rect.min.y), pos2(rect.max.x - 12.0, rect.max.y));
        let mut r = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::right_to_left(egui::Align::Center)));
        r.spacing_mut().item_spacing.x = 8.0;
        let (ar, aresp) = r.allocate_exact_size(Vec2::splat(30.0), Sense::click());
        let label = self.phone.clone().unwrap_or_else(|| "Me".into());
        widgets::avatar(r.painter(), &t, ar.center(), 14.0, "Me", &label, "", false);
        if aresp.on_hover_text("Account and phone").clicked() {
            self.backstage = Some(Backstage::Info);
        }
        let (dot, text, icon) = match &self.conn {
            Conn::Connected => (t.icon_green, "Phone connected", Icon::Phone),
            Conn::Connecting | Conn::Starting => (t.icon_yellow, "Connecting…", Icon::Phone),
            Conn::Reconnecting(_) => (t.icon_yellow, "Reconnecting…", Icon::Phone),
            Conn::PhoneOffline(_) => (t.icon_red, "Phone not responding", Icon::Phone),
            Conn::OtherSession => (t.icon_yellow, "Open elsewhere", Icon::Phone),
            Conn::Error(_) | Conn::HelperMissing(_) => (t.icon_red, "Not connected", Icon::Warning),
            Conn::NoSession | Conn::Pairing => (t.text_disabled, "Pair phone", Icon::Phone),
        };
        if widgets::chip(&mut r, &t, icon, text, Some(dot)).on_hover_text("Phone and pairing").clicked() {
            self.backstage = Some(Backstage::Info);
        }
    }

    fn tab_strip(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        let mut x = rect.min.x + 12.0;
        let font = theme::regular(13.333);
        let tabs: [(&str, Option<RibbonTab>); 4] = [("File", None), ("Home", Some(RibbonTab::Home)), ("View", Some(RibbonTab::View)), ("Help", Some(RibbonTab::Help))];
        for (label, tab) in tabs {
            let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text);
            let w = galley.size().x + 22.0;
            let r = Rect::from_min_size(pos2(x, rect.min.y), vec2(w, rect.height()));
            let resp = ui.interact(r, egui::Id::unique(("tab", label)), Sense::click());
            let selected = match tab {
                None => self.backstage.is_some(),
                Some(tb) => self.backstage.is_none() && self.ribbon == tb,
            };
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, label));
            if resp.hovered() && !selected {
                ui.painter().rect_filled(r.shrink2(vec2(2.0, 3.0)), 4.0, t.hover);
            }
            let col = if selected { t.text } else { t.text_dim };
            let f = if selected { theme::semibold(13.333) } else { font.clone() };
            ui.painter().text(pos2(r.center().x, r.center().y), Align2::CENTER_CENTER, label, f, col);
            if selected {
                let gw = galley.size().x;
                let ul = Rect::from_center_size(pos2(r.center().x, r.max.y - 3.0), vec2(gw, 2.0));
                ui.painter().rect_filled(ul, 1.0, t.accent);
            }
            if resp.clicked() {
                match tab {
                    None => {
                        self.backstage = if self.backstage.is_some() { None } else { Some(Backstage::Info) };
                    }
                    Some(tb) => {
                        self.backstage = None;
                        self.ribbon = tb;
                    }
                }
            }
            x += w + 2.0;
        }
    }

    fn ribbon(&mut self, ui: &mut Ui) {
        let t = self.t;
        let sheet = ui.max_rect();
        ui.painter().rect_filled(sheet, 8.0, t.ribbon);
        ui.painter().rect_stroke(sheet, 8.0, t.ribbon_edge, StrokeKind::Inside);
        let inner = sheet.shrink2(vec2(8.0, (sheet.height() - 32.0) / 2.0));
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
        ui.spacing_mut().item_spacing.x = 2.0;
        let ui = &mut ui;
        let paired = self.conn.paired_view();
        let connected = matches!(self.conn, Conn::Connected);
        let has_sel = self.selected.is_some();
        match self.ribbon {
            RibbonTab::Home => {
                if ribbon_button(ui, &t, Icon::NewChat, "New Conversation", false, paired, false).clicked() {
                    self.new_chat = Some(NewChat { focus: true, ..Default::default() });
                }
                ribbon_rule(ui, &t);
                if ribbon_button(ui, &t, Icon::Reply, "Reply", false, has_sel, false).clicked() {
                    self.focus_composer = true;
                }
                if ribbon_button(ui, &t, Icon::Attach, "Attach File", false, has_sel, false).clicked() {
                    if let Some(id) = self.selected.clone() {
                        if let Some(files) = rfd::FileDialog::new().set_title("Attach files").pick_files() {
                            self.drafts.entry(id).or_default().files.extend(files);
                            self.focus_composer = true;
                        }
                    }
                }
                ribbon_rule(ui, &t);
                let unread = self.selected.as_ref().and_then(|id| self.convs.get(id)).is_some_and(|c| c.unread);
                if ribbon_button(ui, &t, Icon::MarkRead, "Mark as Read", false, has_sel && unread && connected, false).clicked() {
                    self.mark_selected_unread_toggle();
                }
                if ribbon_button(ui, &t, Icon::Sync, "Sync Now", !self.syncing.is_empty(), paired, false).clicked() {
                    self.sync_now();
                }
                ribbon_rule(ui, &t);
                for f in [Folder::Inbox, Folder::Archive, Folder::Spam] {
                    let icon = match f {
                        Folder::Inbox => Icon::Inbox,
                        Folder::Archive => Icon::Archive,
                        Folder::Spam => Icon::Spam,
                    };
                    if ribbon_button(ui, &t, icon, f.label(), self.folder == f, paired, false).clicked() {
                        self.open_folder(f);
                    }
                }
            }
            RibbonTab::View => {
                if ribbon_button(ui, &t, Icon::FolderPane, "Folder Pane", self.settings.folder_pane, true, false).clicked() {
                    self.settings.folder_pane = !self.settings.folder_pane;
                    self.save_settings();
                }
                if ribbon_button(ui, &t, Icon::Spacing, "Tighter Spacing", self.settings.tighter, true, false).clicked() {
                    self.settings.tighter = !self.settings.tighter;
                    self.save_settings();
                }
                if ribbon_button(ui, &t, Icon::Inbox, "Unread Only", self.unread_only, true, false).clicked() {
                    self.unread_only = !self.unread_only;
                }
                ribbon_rule(ui, &t);
                for (a, label) in [(Appearance::System, "System Theme"), (Appearance::Light, "Light"), (Appearance::Dark, "Dark")] {
                    if ribbon_button(ui, &t, Icon::Theme, label, self.settings.appearance == a, true, false).clicked() {
                        self.settings.appearance = a;
                        self.apply_settings(ui.ctx());
                        self.save_settings();
                    }
                }
                ribbon_rule(ui, &t);
                if ribbon_button(ui, &t, Icon::Zoom, &format!("Zoom {}%", self.settings.zoom), false, true, true).clicked() {
                    let presets = [80u16, 90, 100, 110, 125, 150, 175, 200];
                    let i = presets.iter().position(|z| *z == self.settings.zoom).unwrap_or(2);
                    self.settings.zoom = presets[(i + 1) % presets.len()];
                    self.apply_settings(ui.ctx());
                    self.save_settings();
                }
                if ribbon_button(ui, &t, Icon::Undo, "Reset View", false, true, false).clicked() {
                    self.settings = crate::model::Settings { appearance: self.settings.appearance, ..Default::default() };
                    self.apply_settings(ui.ctx());
                    self.save_settings();
                }
            }
            RibbonTab::Help => {
                if ribbon_button(ui, &t, Icon::Phone, "How to Pair", false, true, false).clicked() {
                    self.backstage = Some(Backstage::Info);
                }
                if ribbon_button(ui, &t, Icon::Copy, "Open Data Folder", false, true, false).clicked() {
                    crate::app::open_path(&self.bridge.data_dir);
                }
                if ribbon_button(ui, &t, Icon::Info, "About MessageCraft", false, true, false).clicked() {
                    self.backstage = Some(Backstage::About);
                }
            }
        }
    }

    pub fn open_folder(&mut self, f: Folder) {
        if self.folder != f {
            self.folder = f;
            self.selected = None;
            if self.conn.paired_view() {
                self.bridge.send(serde_json::json!({"cmd": "sync", "folder": f.code(), "count": 150}));
            }
        }
    }

    fn rail(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        let items = [(Icon::Chat, "Messages", true), (Icon::People, "People (from your conversations)", false), (Icon::Gear, "Options", false)];
        let mut y = rect.min.y + 4.0 + TAB_H;
        for (i, (icon, tip, _)) in items.iter().enumerate() {
            let r = Rect::from_min_size(pos2(rect.min.x, y), vec2(RAIL_W, 48.0));
            let resp = ui.interact(r, egui::Id::unique(("rail", i)), Sense::click()).on_hover_text(*tip);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, *tip));
            let active = match i {
                0 => self.backstage.is_none(),
                2 => self.backstage == Some(Backstage::Options),
                _ => false,
            };
            if resp.hovered() {
                ui.painter().rect_filled(r.shrink(6.0), 6.0, t.hover);
            }
            if active {
                ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 2.0, r.center().y - 10.0), vec2(3.0, 20.0)), 1.5, t.accent);
            }
            let ir = Rect::from_center_size(r.center(), Vec2::splat(22.0));
            icons::paint(ui.painter(), ir, *icon, if active { t.accent } else { t.icon }, if active { t.accent } else { t.icon });
            if resp.clicked() {
                match i {
                    0 => self.backstage = None,
                    1 => {
                        self.backstage = None;
                        self.focus_search = true;
                    }
                    _ => self.backstage = Some(Backstage::Options),
                }
            }
            y += 48.0;
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        let p = ui.painter();
        let font = theme::regular(12.0);
        let y = rect.center().y;
        if self.conn.paired_view() {
            let n = self.folder_count(self.folder);
            let u = self.unread_count(self.folder);
            let left = format!("{n} conversations");
            let lw = p.text(pos2(rect.min.x + 12.0, y), Align2::LEFT_CENTER, left, font.clone(), t.text_dim).width();
            p.text(pos2(rect.min.x + 12.0 + lw + 28.0, y), Align2::LEFT_CENTER, format!("Unread: {u}"), font.clone(), t.text_dim);
        }
        let (text, col) = match &self.conn {
            Conn::Connected if !self.syncing.is_empty() => ("Syncing with your phone…".to_string(), t.text_dim),
            Conn::Connected => match self.last_sync {
                Some(ms) => (format!("Up to date — {}", timefmt::clock(ms)), t.text_dim),
                None => ("Connected".into(), t.text_dim),
            },
            Conn::Starting => ("Starting…".into(), t.text_dim),
            Conn::Connecting => ("Connecting to your phone…".into(), t.text_dim),
            Conn::Pairing => ("Waiting for you to scan the QR code".into(), t.text_dim),
            Conn::NoSession => ("Not paired".into(), t.text_dim),
            Conn::Reconnecting(_) => ("Connection lost — reconnecting…".into(), t.orange),
            Conn::PhoneOffline(_) => ("Your phone isn't responding".into(), t.orange),
            Conn::OtherSession => ("Google Messages is open in another window".into(), t.orange),
            Conn::Error(e) => (format!("Not connected: {e}"), t.red),
            Conn::HelperMissing(_) => ("The connection helper isn't running".into(), t.red),
        };
        let right_x = rect.max.x - 12.0;
        let mut text_right = right_x;
        if matches!(self.conn, Conn::OtherSession | Conn::Error(_) | Conn::PhoneOffline(_) | Conn::HelperMissing(_)) {
            let label = if matches!(self.conn, Conn::OtherSession) { "Use here" } else { "Retry" };
            let g = p.layout_no_wrap(label.to_string(), theme::semibold(12.0), t.accent_text);
            let br = Rect::from_min_max(pos2(right_x - g.size().x - 12.0, rect.min.y + 4.0), pos2(right_x, rect.max.y - 4.0));
            let resp = ui.interact(br, egui::Id::unique("status_action"), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(br, 4.0, t.hover);
            }
            p.text(br.center(), Align2::CENTER_CENTER, label, theme::semibold(12.0), t.accent_text);
            if resp.clicked() {
                if matches!(self.conn, Conn::HelperMissing(_)) {
                    let ctx = ui.ctx().clone();
                    self.restart_helper(&ctx);
                } else {
                    self.conn = Conn::Connecting;
                    self.bridge.send(serde_json::json!({"cmd": "reconnect"}));
                }
            }
            text_right = br.min.x - 10.0;
        }
        ui.painter().text(pos2(text_right, y), Align2::RIGHT_CENTER, text, font, col);
    }

    fn notice_overlay(&mut self, ctx: &egui::Context) {
        let Some(n) = &self.notice else { return };
        let t = self.t;
        let text = n.text.clone();
        let error = n.error;
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("notice"))
            .order(egui::Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, vec2(-18.0, -(STATUS_BAR_H + 14.0)))
            .show(ctx, |ui| {
                Frame::new()
                    .fill(t.menu)
                    .stroke(Stroke::new(1.0, if error { t.red.linear_multiply(0.7) } else { t.border_strong }))
                    .corner_radius(8)
                    .shadow(egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: t.shadow_popup })
                    .inner_margin(Margin::symmetric(14, 10))
                    .show(ui, |ui| {
                        ui.set_max_width((screen.width() * 0.4).clamp(260.0, 460.0));
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                            icons::paint(ui.painter(), r, if error { Icon::Warning } else { Icon::Info }, if error { t.red } else { t.icon }, if error { t.red } else { t.accent });
                            ui.add(egui::Label::new(egui::RichText::new(text).font(theme::regular(12.5)).color(t.text)).wrap());
                            if icon_button(ui, &t, Icon::Close, 20.0, "Dismiss").clicked() {
                                self.notice = None;
                            }
                        });
                    });
            });
    }
}
