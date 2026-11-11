//! File view (MailCraft's "backstage"): the phone and pairing, options, about; and the dialogs.

use egui::{Align2, Frame, Margin, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{App, Backstage, Conn};
use crate::icons::{self, Icon};
use crate::model::Appearance;
use crate::theme;
use crate::widgets;

impl App {
    pub fn backstage_view(&mut self, ui: &mut Ui, page: Backstage) {
        let t = self.t;
        let rect = ui.max_rect();
        let column = Rect::from_min_max(rect.min, pos2(rect.min.x + 220.0, rect.max.y));
        ui.painter().rect_filled(column, 0.0, t.backstage_column);
        ui.painter().line_segment([pos2(column.max.x, column.min.y), pos2(column.max.x, column.max.y)], Stroke::new(1.0, t.border));

        let mut y = column.min.y + 14.0;
        // Back
        let back = Rect::from_min_size(pos2(column.min.x + 10.0, y), vec2(column.width() - 20.0, 34.0));
        let resp = ui.interact(back, egui::Id::unique("bs_back"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(back, 6.0, t.hover);
        }
        icons::paint(ui.painter(), Rect::from_min_size(pos2(back.min.x + 8.0, back.center().y - 9.0), Vec2::splat(18.0)), Icon::ChevronLeft, t.icon, t.icon);
        ui.painter().text(pos2(back.min.x + 34.0, back.center().y), Align2::LEFT_CENTER, "Back to messages", theme::regular(13.333), t.text);
        if resp.clicked() {
            self.backstage = None;
        }
        y += 50.0;
        for (p, label, icon) in [(Backstage::Info, "Info", Icon::Phone), (Backstage::Options, "Options", Icon::Gear), (Backstage::About, "About", Icon::Info)] {
            let r = Rect::from_min_size(pos2(column.min.x + 10.0, y), vec2(column.width() - 20.0, 36.0));
            let resp = ui.interact(r, egui::Id::unique(("bs", label)), Sense::click());
            let sel = page == p;
            if sel {
                ui.painter().rect_filled(r, 6.0, t.accent_soft);
                ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x, r.min.y + 8.0), vec2(3.0, 20.0)), 1.5, t.accent);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 6.0, t.hover);
            }
            icons::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 12.0, r.center().y - 9.0), Vec2::splat(18.0)), icon, if sel { t.accent_soft_text } else { t.icon }, t.accent);
            ui.painter().text(pos2(r.min.x + 40.0, r.center().y), Align2::LEFT_CENTER, label, if sel { theme::semibold(14.0) } else { theme::regular(14.0) }, if sel { t.accent_soft_text } else { t.text });
            if resp.clicked() {
                self.backstage = Some(p);
            }
            y += 40.0;
        }

        let content = Rect::from_min_max(pos2(column.max.x + 40.0, rect.min.y + 28.0), pos2(rect.max.x - 40.0, rect.max.y - 20.0));
        let mut u = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(egui::Layout::top_down(egui::Align::Min)));
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut u, |ui| {
            ui.set_max_width(content.width().min(760.0));
            ui.spacing_mut().item_spacing.y = 8.0;
            match page {
                Backstage::Info => self.info_page(ui),
                Backstage::Options => self.options_page(ui),
                Backstage::About => self.about_page(ui),
            }
        });
    }

    fn section(ui: &mut Ui, t: &theme::Tokens, title: &str) {
        ui.add_space(10.0);
        ui.label(egui::RichText::new(title).font(theme::semibold(15.0)).color(t.text));
        let w = ui.available_width();
        let (r, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
        ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, t.rule));
    }

    fn info_page(&mut self, ui: &mut Ui) {
        let t = self.t;
        ui.label(egui::RichText::new("Account Information").font(theme::semibold(24.0)).color(t.text));
        Self::section(ui, &t, "Your phone");
        let (state, col) = match &self.conn {
            Conn::Connected => ("Connected".to_string(), t.green),
            Conn::Connecting | Conn::Starting => ("Connecting…".into(), t.orange),
            Conn::Reconnecting(d) => (format!("Reconnecting: {d}"), t.orange),
            Conn::PhoneOffline(d) => (d.clone(), t.red),
            Conn::OtherSession => ("Google Messages is open in another window or browser. Only one can be active at a time.".into(), t.orange),
            Conn::Error(e) => (format!("Not connected: {e}"), t.red),
            Conn::HelperMissing(e) => (e.clone(), t.red),
            Conn::NoSession | Conn::Pairing => ("Not paired yet".into(), t.text_dim),
        };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
            ui.painter().rect_filled(r, 8.0, t.accent_soft);
            icons::paint(ui.painter(), r.shrink(10.0), Icon::Phone, t.accent_soft_text, t.accent);
            ui.vertical(|ui| {
                let phone = self.phone.clone().filter(|p| !p.is_empty()).unwrap_or_else(|| "Android phone with Google Messages".into());
                ui.label(egui::RichText::new(phone).font(theme::semibold(14.0)).color(t.text));
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, col);
                    ui.add(egui::Label::new(egui::RichText::new(state).font(theme::regular(12.5)).color(t.text_dim)).wrap());
                });
            });
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if self.conn.paired_view() {
                if widgets::secondary_button(ui, &t, "Reconnect").clicked() {
                    self.conn = Conn::Connecting;
                    self.bridge.send(serde_json::json!({"cmd": "reconnect"}));
                }
                if widgets::secondary_button(ui, &t, "Sync now").clicked() {
                    self.sync_now();
                }
                if widgets::secondary_button(ui, &t, "Unpair this computer…").clicked() {
                    self.confirm_unpair = true;
                }
            } else if widgets::primary_button(ui, &t, "Sign in with Google to pair", true).clicked() {
                self.backstage = None;
                self.sign_in_with_google();
            }
        });

        Self::section(ui, &t, "Pairing");
        for line in [
            "MessageCraft works like Google Messages for web: your phone does the sending, and this computer shows your conversations.",
            "To pair: choose Sign in with Google and sign in with the Google account your phone's Google Messages uses, then tap the emoji MessageCraft shows on your phone. (Older phones whose Device pairing screen has a QR code scanner can pair with a QR code instead.)",
            "Keep your phone on and connected to the internet. If you open Google Messages for web in a browser, it takes over; use Reconnect here to switch back.",
            "Unpairing removes this computer from your phone's paired devices and deletes the copy of your conversations kept here.",
        ] {
            ui.add(egui::Label::new(egui::RichText::new(line).font(theme::regular(13.0)).color(t.text_dim)).wrap());
        }

        Self::section(ui, &t, "On this computer");
        ui.add(egui::Label::new(egui::RichText::new(format!("Your pairing, settings and a copy of recent conversations are kept in {}. The pairing is encrypted for your Windows account.", self.bridge.data_dir.display())).font(theme::regular(13.0)).color(t.text_dim)).wrap());
        if widgets::secondary_button(ui, &t, "Open data folder").clicked() {
            crate::app::open_path(&self.bridge.data_dir);
        }
    }

    fn options_page(&mut self, ui: &mut Ui) {
        let t = self.t;
        ui.label(egui::RichText::new("Options").font(theme::semibold(24.0)).color(t.text));
        Self::section(ui, &t, "Appearance");
        ui.horizontal(|ui| {
            for (a, label) in [(Appearance::System, "Use system setting"), (Appearance::Light, "Light"), (Appearance::Dark, "Dark")] {
                if ui.radio(self.settings.appearance == a, egui::RichText::new(label).font(theme::regular(13.0))).clicked() {
                    self.settings.appearance = a;
                    self.apply_settings(ui.ctx());
                    self.save_settings();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Interface size").font(theme::regular(13.0)).color(t.text));
            let mut z = self.settings.zoom as f32;
            if ui.add(egui::Slider::new(&mut z, 80.0..=200.0).step_by(5.0).suffix("%")).drag_stopped() || ui.input(|i| i.pointer.any_released()) && z as u16 != self.settings.zoom {
                self.settings.zoom = z as u16;
                self.apply_settings(ui.ctx());
                self.save_settings();
            }
        });
        let mut tighter = self.settings.tighter;
        if ui.checkbox(&mut tighter, egui::RichText::new("Tighter spacing in the conversation list").font(theme::regular(13.0))).changed() {
            self.settings.tighter = tighter;
            self.save_settings();
        }
        let mut pane = self.settings.folder_pane;
        if ui.checkbox(&mut pane, egui::RichText::new("Show the folder pane").font(theme::regular(13.0))).changed() {
            self.settings.folder_pane = pane;
            self.save_settings();
        }
        Self::section(ui, &t, "Writing");
        let mut enter = self.settings.enter_sends;
        if ui.checkbox(&mut enter, egui::RichText::new("Press Enter to send (Shift+Enter for a new line)").font(theme::regular(13.0))).changed() {
            self.settings.enter_sends = enter;
            self.save_settings();
        }
        ui.add(egui::Label::new(egui::RichText::new("When this is off, Ctrl+Enter sends.").font(theme::regular(12.0)).color(t.text_dim)));
    }

    fn about_page(&mut self, ui: &mut Ui) {
        let t = self.t;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
            ui.painter().rect_filled(r, 10.0, t.accent);
            icons::paint(ui.painter(), r.shrink(9.0), Icon::Chat, egui::Color32::WHITE, egui::Color32::WHITE);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("MessageCraft").font(theme::semibold(24.0)).color(t.text));
                ui.label(egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).font(theme::regular(13.0)).color(t.text_dim));
            });
        });
        Self::section(ui, &t, "About");
        ui.add(egui::Label::new(egui::RichText::new("Your phone's text messages (SMS, MMS and RCS chats) on Windows, in the style of MailCraft. MessageCraft is not made by or affiliated with Google.").font(theme::regular(13.0)).color(t.text_dim)).wrap());
        Self::section(ui, &t, "Built with");
        for line in [
            "libgm from mautrix-gmessages (AGPL-3.0): talks to Google Messages for web.",
            "MailCraft's palette, type scale and layout (MIT or Apache-2.0).",
            "egui and eframe (MIT or Apache-2.0), the Inter typeface (SIL Open Font License).",
        ] {
            ui.add(egui::Label::new(egui::RichText::new(format!("• {line}")).font(theme::regular(13.0)).color(t.text_dim)).wrap());
        }
    }

    // -----------------------------------------------------------------------------------------

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        let t = self.t;
        if self.new_chat.is_some() {
            let mut close = false;
            let mut start = false;
            let modal = egui::Modal::new(egui::Id::unique("new_chat")).frame(dialog_frame(&t)).show(ctx, |ui| {
                ui.set_width(420.0);
                let nc = self.new_chat.as_mut().expect("checked");
                ui.label(egui::RichText::new("New conversation").font(theme::semibold(17.0)).color(t.text));
                ui.add_space(4.0);
                ui.label(egui::RichText::new("To: one phone number, or several separated by commas for a group").font(theme::regular(12.5)).color(t.text_dim));
                let r = ui.add(egui::TextEdit::singleline(&mut nc.numbers).hint_text("+1 555 123 4567").desired_width(f32::INFINITY).font(theme::regular(14.0)).margin(Margin::symmetric(8, 6)));
                if nc.focus {
                    r.request_focus();
                    nc.focus = false;
                }
                let group = nc.numbers.contains(',') || nc.numbers.contains(';');
                if group {
                    ui.label(egui::RichText::new("Group name (optional, for chat groups)").font(theme::regular(12.5)).color(t.text_dim));
                    ui.add(egui::TextEdit::singleline(&mut nc.group_name).desired_width(f32::INFINITY).font(theme::regular(14.0)).margin(Margin::symmetric(8, 6)));
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !nc.numbers.trim().is_empty() {
                    start = true;
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let busy = nc.busy;
                    let ok = !nc.numbers.trim().is_empty() && !busy;
                    if widgets::primary_button(ui, &t, if busy { "Starting…" } else { "Start" }, ok).clicked() {
                        start = true;
                    }
                    if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if modal.should_close() || close {
                self.new_chat = None;
            } else if start {
                self.start_new_chat();
            }
        }
        if self.confirm_unpair {
            let mut close = false;
            let mut go = false;
            let modal = egui::Modal::new(egui::Id::unique("unpair")).frame(dialog_frame(&t)).show(ctx, |ui| {
                ui.set_width(420.0);
                ui.label(egui::RichText::new("Unpair this computer?").font(theme::semibold(17.0)).color(t.text));
                ui.add_space(4.0);
                ui.add(egui::Label::new(egui::RichText::new("MessageCraft will stop showing your messages, your phone will forget this computer, and the copy of your conversations kept here will be deleted. Your messages stay on your phone.").font(theme::regular(13.0)).color(t.text_dim)).wrap());
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, &t, "Unpair", true).clicked() {
                        go = true;
                    }
                    if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                        close = true;
                    }
                });
            });
            if modal.should_close() || close {
                self.confirm_unpair = false;
            }
            if go {
                self.confirm_unpair = false;
                self.backstage = None;
                self.unpair();
            }
        }
    }
}

fn dialog_frame(t: &theme::Tokens) -> Frame {
    Frame::new()
        .fill(t.menu)
        .stroke(Stroke::new(1.0, t.border_strong))
        .corner_radius(8)
        .shadow(egui::Shadow { offset: [0, 8], blur: 28, spread: 0, color: t.shadow_window })
        .inner_margin(Margin::same(20))
}

#[allow(dead_code)]
fn unused(_: StrokeKind) {}
