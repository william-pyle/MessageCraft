//! The reading pane (a conversation, its messages and the box to reply in), and the pairing page.

use egui::{Align2, Color32, Frame, Margin, Rect, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::app::{App, Conn, MediaSlot, PairStep};
use crate::icons::{self, Icon};
use crate::model::{Conversation, Media, Message};
use crate::theme;
use crate::contactcraft;
use crate::timefmt;
use crate::widgets::{self, icon_button};

const BUBBLE_PAD_X: f32 = 12.0;
const BUBBLE_PAD_Y: f32 = 8.0;

impl App {
    pub fn reading_pane(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
        let Some(id) = self.selected.clone() else {
            self.empty_reading(ui, rect);
            return;
        };
        let Some(conv) = self.convs.get(&id).cloned() else {
            self.selected = None;
            return;
        };

        // Header: the conversation's name, the people in it, and actions.
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), 76.0));
        let p = ui.painter().clone();
        let mut title = conv.title();
        if !conv.group {
            if let Some(c) = self.people.contact_for(conv.contact_number()) {
                if !c.theme.emoji.is_empty() { title.push_str(&format!("  {}", c.theme.emoji)); }
                if let Some(accent) = contactcraft::color(&c.theme.accent) {
                    p.rect_filled(Rect::from_min_size(header.min, vec2(header.width(), 3.0)), 0.0, accent);
                }
            }
        }
        widgets::clipped(&p, pos2(header.min.x + 20.0, header.min.y + 22.0), header.width() - 150.0, &title, theme::semibold(17.0), t.text);
        let key = if conv.contact_number().is_empty() { conv.id.clone() } else { conv.contact_number().to_string() };
        let hex = if conv.color.is_empty() { conv.others().next().map(|p| p.color.clone()).unwrap_or_default() } else { conv.color.clone() };
        self.people.avatar(ui, pos2(header.min.x + 36.0, header.min.y + 54.0), 16.0, &title, &key, &hex, conv.group, &t);
        let who = if conv.group {
            let names: Vec<String> = conv.others().map(|p| if p.name.is_empty() { p.number.clone() } else { p.name.clone() }).collect();
            names.join(", ")
        } else {
            conv.others().next().map(|p| if p.name.is_empty() || p.name == p.number { p.number.clone() } else { format!("{}  <{}>", p.name, p.number) }).unwrap_or_default()
        };
        widgets::clipped(&p, pos2(header.min.x + 60.0, header.min.y + 47.0), header.width() - 200.0, &who, theme::semibold(12.5), t.text);
        let kind = match conv.kind.as_str() {
            "rcs" => "Chat message (RCS)",
            _ => "Text message (SMS/MMS)",
        };
        widgets::clipped(&p, pos2(header.min.x + 60.0, header.min.y + 63.0), header.width() - 200.0, kind, theme::regular(11.5), t.text_dim);
        if let Some(last) = self.threads.get(&id).and_then(|th| th.messages.last()) {
            p.text(pos2(header.max.x - 18.0, header.min.y + 50.0), Align2::RIGHT_CENTER, timefmt::full(last.ts), theme::regular(11.5), t.text_dim);
        }
        let actions = Rect::from_min_size(pos2(header.max.x - 130.0, header.min.y + 8.0), vec2(114.0, 26.0));
        let mut a = ui.new_child(egui::UiBuilder::new().max_rect(actions).layout(egui::Layout::right_to_left(egui::Align::Center)));
        a.spacing_mut().item_spacing.x = 2.0;
        // The person in a one-to-one conversation: open or create in ContactCraft.
        if !conv.group && self.people.state == crate::people::ContactsState::Ready {
            if let Some(other) = conv.others().next().cloned() {
                let existing = self.people.contact_for(&other.number).map(|c| c.id.clone());
                let tip = if existing.is_some() { "Open in ContactCraft contacts" } else { "Add to ContactCraft contacts" };
                if icon_button(&mut a, &t, Icon::Person, 26.0, tip).clicked() {
                    match existing {
                        Some(cid) => {
                            self.people.open = true;
                            self.people.selected = Some(cid);
                        }
                        None => self.add_contact_from(&other.name, &other.number),
                    }
                }
            }
        }
        if icon_button(&mut a, &t, Icon::Sync, 26.0, "Reload this conversation").clicked() {
            self.load_thread(&id, true);
        }
        if icon_button(&mut a, &t, Icon::Attach, 26.0, "Attach files").clicked() {
            if let Some(files) = rfd::FileDialog::new().set_title("Attach files").pick_files() {
                self.drafts.entry(id.clone()).or_default().files.extend(files);
                self.focus_composer = true;
            }
        }
        if icon_button(&mut a, &t, Icon::Reply, 26.0, "Reply").clicked() {
            self.focus_composer = true;
        }
        p.line_segment([pos2(rect.min.x + 12.0, header.max.y + 0.5), pos2(rect.max.x - 12.0, header.max.y + 0.5)], Stroke::new(1.0, t.rule));

        // The reply box at the bottom.
        let files_h = if self.drafts.get(&id).is_some_and(|d| !d.files.is_empty()) { 34.0 } else { 0.0 };
        let composer_h = 64.0 + files_h;
        let composer = Rect::from_min_max(pos2(rect.min.x + 14.0, rect.max.y - composer_h - 10.0), pos2(rect.max.x - 14.0, rect.max.y - 12.0));
        let messages_rect = Rect::from_min_max(pos2(rect.min.x + 2.0, header.max.y + 2.0), pos2(rect.max.x - 2.0, composer.min.y - 6.0));
        if !conv.group {
            if let Some(c) = self.people.contact_for(conv.contact_number()) {
                    if let Some(col) = contactcraft::color(&c.theme.background) { ui.painter().rect_filled(messages_rect, 0.0, col); }
                    if let Some(tex) = self.people.background_for(conv.contact_number()) {
                        let opacity = c.theme.background_opacity;
                        ui.painter().image(tex.id(), messages_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE.linear_multiply(opacity as f32 / 100.0));
                    }
            }
        }

        let mut mu = ui.new_child(egui::UiBuilder::new().max_rect(messages_rect).layout(egui::Layout::top_down(egui::Align::Min)));
        self.messages_view(&mut mu, &conv);

        let mut cu = ui.new_child(egui::UiBuilder::new().max_rect(composer).layout(egui::Layout::top_down(egui::Align::Min)));
        self.composer(&mut cu, &conv);
    }

    fn empty_reading(&self, ui: &mut Ui, rect: Rect) {
        let t = self.t;
        let p = ui.painter();
        let c = rect.center();
        icons::paint(p, Rect::from_center_size(pos2(c.x, c.y - 40.0), Vec2::splat(56.0)), Icon::Chat, t.text_disabled, t.accent);
        p.text(pos2(c.x, c.y + 8.0), Align2::CENTER_TOP, "Select a conversation to read it", theme::medium(15.0), t.text);
        p.text(pos2(c.x, c.y + 32.0), Align2::CENTER_TOP, "or start a new one with New Conversation (Ctrl+N).", theme::regular(12.5), t.text_dim);
    }

    fn messages_view(&mut self, ui: &mut Ui, conv: &Conversation) {
        let t = self.t;
        let id = conv.id.clone();
        let (messages, loading, failed, has_more, loading_older, scroll_end) = match self.threads.get_mut(&id) {
            Some(th) => {
                let s = th.scroll_to_end;
                th.scroll_to_end = false;
                (th.messages.clone(), th.loading && !th.loaded, th.failed, th.has_more, th.loading_older, s)
            }
            None => (Vec::new(), true, false, false, false, false),
        };
        if messages.is_empty() {
            let r = ui.max_rect();
            let text = if failed {
                "Couldn't load this conversation from your phone. Use Reload to try again."
            } else if loading {
                "Loading messages from your phone…"
            } else {
                "No messages yet. Say hello below."
            };
            ui.painter().text(r.center(), Align2::CENTER_CENTER, text, theme::regular(13.0), t.text_dim);
            return;
        }
        let width = ui.available_width();
        let bubble_max = (width * 0.68).clamp(220.0, 560.0);
        let mut load_older = false;
        let mut to_open: Option<Media> = None;
        let mut to_request: Vec<(String, String, String)> = Vec::new();
        let mut retry: Option<Message> = None;
        let mut delete: Option<String> = None;
        let typing = self.typing.contains_key(&id);

        let area = ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).id_salt(("thread", id.clone()));
        area.show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.add_space(10.0);
            if has_more {
                ui.vertical_centered(|ui| {
                    let label = if loading_older { "Loading…" } else { "Load older messages" };
                    if widgets::secondary_button(ui, &t, label).clicked() && !loading_older {
                        load_older = true;
                    }
                });
                ui.add_space(8.0);
            }
            let last_mine = messages.iter().rposition(|m| m.from_me);
            let mut prev: Option<&Message> = None;
            for (i, m) in messages.iter().enumerate() {
                let new_day = prev.is_none_or(|p| !timefmt::same_day(p.ts, m.ts));
                if new_day {
                    ui.add_space(10.0);
                    let (r, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
                    let label = timefmt::day(m.ts);
                    let g = ui.painter().layout_no_wrap(label.clone(), theme::medium(11.5), t.text_dim);
                    let half = g.size().x / 2.0 + 12.0;
                    let y = r.center().y;
                    ui.painter().line_segment([pos2(r.min.x + 24.0, y), pos2(r.center().x - half, y)], Stroke::new(1.0, t.rule));
                    ui.painter().line_segment([pos2(r.center().x + half, y), pos2(r.max.x - 24.0, y)], Stroke::new(1.0, t.rule));
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, label, theme::medium(11.5), t.text_dim);
                    ui.add_space(6.0);
                }
                let same_sender = prev.is_some_and(|p| p.from_me == m.from_me && p.participant_id == m.participant_id && (m.ts - p.ts) < 5 * 60 * 1000) && !new_day;
                ui.add_space(if same_sender { 3.0 } else { 10.0 });

                // In a group, name the sender above their first message in a run.
                if conv.group && !m.from_me && !same_sender {
                    let name = if !m.sender_name.is_empty() {
                        m.sender_name.clone()
                    } else {
                        conv.participant(&m.participant_id).map(|p| p.label().to_string()).unwrap_or_default()
                    };
                    if !name.is_empty() {
                        let (r, _) = ui.allocate_exact_size(vec2(width, 16.0), Sense::hover());
                        ui.painter().text(pos2(r.min.x + 62.0, r.center().y), Align2::LEFT_CENTER, name, theme::medium(11.5), t.text_dim);
                    }
                }

                let show_avatar = !m.from_me && !messages.get(i + 1).is_some_and(|n| !n.from_me && n.participant_id == m.participant_id && (n.ts - m.ts) < 5 * 60 * 1000 && timefmt::same_day(n.ts, m.ts));
                let resp = self.bubble(ui, conv, m, width, bubble_max, show_avatar, &mut to_open, &mut to_request);
                let msg = m.clone();
                resp.context_menu(|ui| {
                    if !msg.text.is_empty() && ui.button("Copy text").clicked() {
                        ui.ctx().copy_text(msg.text.clone());
                        ui.close();
                    }
                    if msg.failed && msg.local && ui.button("Try sending again").clicked() {
                        retry = Some(msg.clone());
                        ui.close();
                    }
                    if !msg.local && ui.button("Delete for me").clicked() {
                        delete = Some(msg.id.clone());
                        ui.close();
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(timefmt::full(msg.ts)).color(t.text_dim).size(11.0));
                });
                resp.on_hover_text_at_pointer(timefmt::full(m.ts));

                // Status under our latest message, and any failure under its own.
                if m.from_me && (Some(i) == last_mine || m.failed) {
                    let (r, _) = ui.allocate_exact_size(vec2(width, 16.0), Sense::hover());
                    let (text, col) = status_line(m, &t);
                    ui.painter().text(pos2(r.max.x - 24.0, r.center().y + 2.0), Align2::RIGHT_CENTER, text, theme::regular(11.0), col);
                }
                prev = Some(m);
            }
            if typing {
                ui.add_space(8.0);
                let (r, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
                let br = Rect::from_min_size(pos2(r.min.x + 62.0, r.min.y), vec2(52.0, 28.0));
                ui.painter().rect_filled(br, 14.0, t.bubble_in);
                let phase = (ui.input(|i| i.time) * 3.0) as f32;
                for k in 0..3 {
                    let a = 0.45 + 0.55 * ((phase - k as f32 * 0.6).sin() * 0.5 + 0.5);
                    ui.painter().circle_filled(pos2(br.min.x + 15.0 + k as f32 * 11.0, br.center().y), 3.0, t.text_dim.linear_multiply(a));
                }
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(60));
            }
            ui.add_space(14.0);
            if scroll_end {
                ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
            }
        });

        for (mid, key, mime) in to_request {
            self.request_media(&mid, &key, &mime);
        }
        if let Some(m) = to_open {
            self.open_media(&m.id, &m.key, &m.mime);
        }
        if load_older {
            self.load_older(&id);
        }
        if let Some(m) = retry {
            if let Some(th) = self.threads.get_mut(&id) {
                th.messages.retain(|x| x.tmp_id != m.tmp_id);
            }
            self.drafts.entry(id.clone()).or_default().text = m.text.clone();
            self.send_draft(&id);
        }
        if let Some(mid) = delete {
            self.bridge.send(serde_json::json!({"cmd": "delete_message", "message_id": mid}));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn bubble(
        &mut self,
        ui: &mut Ui,
        conv: &Conversation,
        m: &Message,
        width: f32,
        bubble_max: f32,
        show_avatar: bool,
        to_open: &mut Option<Media>,
        to_request: &mut Vec<(String, String, String)>,
    ) -> egui::Response {
        let t = self.t;
        let contact = if conv.group {
            conv.participant(&m.participant_id).and_then(|p| self.people.contact_for(&p.number))
        } else {
            self.people.contact_for(conv.contact_number())
        };
        let (mut fill, mut ink) = if m.from_me {
            (if m.failed { t.red.linear_multiply(0.35) } else { t.bubble_out }, t.bubble_out_text)
        } else {
            (t.bubble_in, t.text)
        };
        let mut radius = 16.0;
        let mut font_size = 13.5;
        if let Some(c) = contact {
            let th = &c.theme;
            fill = contactcraft::color(if m.from_me { &th.bubble_out } else { &th.bubble_in }).unwrap_or(fill);
            ink = contactcraft::color(if m.from_me { &th.bubble_out_text } else { &th.bubble_in_text }).unwrap_or(ink);
            if th.bubble_radius > 0 { radius = th.bubble_radius as f32; }
            if th.text_size > 0.0 { font_size = th.text_size.clamp(9.0, 30.0); }
            if !m.from_me { ink = contactcraft::color(&th.text_color).unwrap_or(ink); }
        }
        let font = theme::regular(font_size);
        let text_galley = if m.text.is_empty() {
            None
        } else {
            Some(ui.painter().layout(m.text.clone(), font, ink, bubble_max - 2.0 * BUBBLE_PAD_X))
        };

        // Attachments first, then the text, all in one column.
        let mut parts_h = 0.0;
        let mut parts_w: f32 = 0.0;
        let mut media_sizes: Vec<Vec2> = Vec::new();
        for md in m.media() {
            let s = if md.is_image() {
                let (w, h) = (md.w.max(1) as f32, md.h.max(1) as f32);
                let max_w = (bubble_max).min(320.0);
                let scale = (max_w / w).min(260.0 / h).min(1.0);
                let s = if md.w > 0 && md.h > 0 { vec2(w * scale, h * scale) } else { vec2(220.0, 160.0) };
                vec2(s.x.max(80.0), s.y.max(60.0))
            } else {
                vec2(240.0_f32.min(bubble_max), 44.0)
            };
            parts_h += s.y + 4.0;
            parts_w = parts_w.max(s.x);
            media_sizes.push(s);
        }
        let text_size = text_galley.as_ref().map(|g| g.size()).unwrap_or(Vec2::ZERO);
        let bubble_w = if text_galley.is_some() { (text_size.x + 2.0 * BUBBLE_PAD_X).max(parts_w) } else { parts_w };
        let bubble_h = parts_h + if text_galley.is_some() { text_size.y + 2.0 * BUBBLE_PAD_Y } else { 0.0 };
        let (row, resp) = ui.allocate_exact_size(vec2(width, bubble_h.max(20.0)), Sense::click());
        let x = if m.from_me { row.max.x - 24.0 - bubble_w } else { row.min.x + 62.0 };
        let mut y = row.min.y;
        let p = ui.painter().clone();

        if show_avatar && !m.from_me {
            let participant = conv.participant(&m.participant_id);
            let name = participant.map(|p| p.label().to_string()).unwrap_or_else(|| conv.title());
            let key = participant.map(|p| p.number.clone()).unwrap_or_else(|| conv.id.clone());
            let hex = participant.map(|p| p.color.clone()).unwrap_or_default();
            self.people.avatar(ui, pos2(row.min.x + 38.0, row.max.y - 14.0), 14.0, &name, &key, &hex, false, &t);
        }

        for (md, size) in m.media().iter().zip(media_sizes) {
            let r = Rect::from_min_size(pos2(if m.from_me { x + bubble_w - size.x } else { x }, y), size);
            let hit = ui.interact(r, egui::Id::unique(("media", &m.id, &md.id, &md.name)), Sense::click());
            if md.is_image() && !md.id.is_empty() {
                let (show_id, show_key) = if !md.thumb_id.is_empty() { (md.thumb_id.clone(), md.thumb_key.clone()) } else { (md.id.clone(), md.key.clone()) };
                match self.media.get(&show_id) {
                    Some(MediaSlot::Ready(tex)) => {
                        p.rect_filled(r, 12.0, t.pill_quiet);
                        let img = egui::Image::new((tex.id(), r.size())).corner_radius(12);
                        img.paint_at(ui, r);
                    }
                    Some(MediaSlot::Failed(_)) => {
                        p.rect_filled(r, 12.0, t.pill_quiet);
                        icons::paint(&p, Rect::from_center_size(r.center(), Vec2::splat(28.0)), Icon::Image, t.text_disabled, t.text_disabled);
                        p.text(pos2(r.center().x, r.center().y + 26.0), Align2::CENTER_CENTER, "Click to open", theme::regular(11.0), t.text_dim);
                    }
                    other => {
                        if other.is_none() && ui.is_rect_visible(r) {
                            to_request.push((show_id, show_key, md.mime.clone()));
                        }
                        p.rect_filled(r, 12.0, t.pill_quiet);
                        icons::paint(&p, Rect::from_center_size(r.center(), Vec2::splat(28.0)), Icon::Image, t.text_dim, t.accent);
                    }
                }
                if hit.hovered() {
                    p.rect_stroke(r, 12.0, Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
                }
                if hit.on_hover_text("Open the full picture").clicked() {
                    *to_open = Some(md.clone());
                }
            } else {
                p.rect_filled(r, 10.0, if m.from_me { fill } else { t.pill_quiet });
                p.rect_stroke(r, 10.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
                let icon = if md.mime.starts_with("image/") { Icon::Image } else { Icon::Attach };
                icons::paint(&p, Rect::from_min_size(pos2(r.min.x + 10.0, r.center().y - 10.0), Vec2::splat(20.0)), icon, ink, t.accent);
                let name = if md.name.is_empty() { kind_name(&md.mime).to_string() } else { md.name.clone() };
                widgets::clipped(&p, pos2(r.min.x + 38.0, r.center().y - 7.0), r.width() - 46.0, &name, theme::medium(12.5), ink);
                let sub = if md.mime == "local/file" { "Sending…".to_string() } else if md.size > 0 { format!("{} · Click to open", human_size(md.size)) } else { "Click to open".into() };
                widgets::clipped(&p, pos2(r.min.x + 38.0, r.center().y + 9.0), r.width() - 46.0, &sub, theme::regular(11.0), if m.from_me { ink } else { t.text_dim });
                if md.mime != "local/file" && !md.id.is_empty() && hit.clicked() {
                    *to_open = Some(md.clone());
                }
            }
            y += size.y + 4.0;
        }

        if let Some(g) = text_galley {
            let br = Rect::from_min_size(pos2(x, y), vec2(bubble_w, text_size.y + 2.0 * BUBBLE_PAD_Y));
            p.rect_filled(br, radius, fill);
            if m.deleted {
                p.rect_stroke(br, radius, Stroke::new(1.0, t.border), StrokeKind::Inside);
            }
            let tx = if m.from_me { br.max.x - BUBBLE_PAD_X - text_size.x } else { br.min.x + BUBBLE_PAD_X };
            p.galley(pos2(tx, br.min.y + BUBBLE_PAD_Y), g, ink);
            // Reactions sit on the bubble's lower edge.
            let reactions = m.reactions();
            if !reactions.is_empty() {
                let label: String = reactions.iter().map(|r| if r.count > 1 { format!("{}{}", r.emoji, r.count) } else { r.emoji.clone() }).collect::<Vec<_>>().join(" ");
                let rg = p.layout_no_wrap(label, theme::regular(12.0), t.text);
                let rr = Rect::from_min_size(pos2(if m.from_me { br.min.x - 6.0 } else { br.max.x - rg.size().x - 8.0 }, br.max.y - 8.0), rg.size() + vec2(12.0, 4.0));
                p.rect_filled(rr, 10.0, t.surface);
                p.rect_stroke(rr, 10.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
                p.galley(rr.min + vec2(6.0, 2.0), rg, t.text);
            }
        }
        resp
    }

    fn composer(&mut self, ui: &mut Ui, conv: &Conversation) {
        let t = self.t;
        let id = conv.id.clone();
        let rect = ui.max_rect();
        let connected = matches!(self.conn, Conn::Connected);
        let mut send = false;
        let mut remove_file: Option<usize> = None;
        let mut attach = false;
        let enter_sends = self.settings.enter_sends;
        let focus = std::mem::take(&mut self.focus_composer);
        let draft = self.drafts.entry(id.clone()).or_default();

        let mut y = rect.min.y;
        if !draft.files.is_empty() {
            let mut fx = rect.min.x;
            for (i, f) in draft.files.iter().enumerate() {
                let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let g = ui.painter().layout_no_wrap(name.clone(), theme::regular(12.0), t.text);
                let w = (g.size().x + 54.0).min(240.0);
                let r = Rect::from_min_size(pos2(fx, y), vec2(w, 26.0));
                ui.painter().rect_filled(r, 13.0, t.accent_soft);
                icons::paint(ui.painter(), Rect::from_min_size(pos2(r.min.x + 8.0, r.center().y - 7.0), Vec2::splat(14.0)), Icon::Attach, t.accent_soft_text, t.accent_soft_text);
                widgets::clipped(ui.painter(), pos2(r.min.x + 26.0, r.center().y), w - 50.0, &name, theme::regular(12.0), t.accent_soft_text);
                let xr = Rect::from_center_size(pos2(r.max.x - 13.0, r.center().y), Vec2::splat(18.0));
                let mut c = ui.new_child(egui::UiBuilder::new().max_rect(xr));
                if icon_button(&mut c, &t, Icon::Close, 18.0, "Remove").clicked() {
                    remove_file = Some(i);
                }
                fx += w + 6.0;
            }
            y += 34.0;
        }

        let box_rect = Rect::from_min_max(pos2(rect.min.x, y), rect.max);
        let has_focus = ui.memory(|m| m.focused() == Some(egui::Id::unique(("composer", &id))));
        ui.painter().rect_filled(box_rect, 8.0, t.input);
        ui.painter().rect_stroke(box_rect, 8.0, Stroke::new(1.0, if has_focus { t.accent } else { t.border_strong }), StrokeKind::Inside);

        let attach_r = Rect::from_min_size(pos2(box_rect.min.x + 8.0, box_rect.center().y - 15.0), Vec2::splat(30.0));
        let mut ac = ui.new_child(egui::UiBuilder::new().max_rect(attach_r));
        if icon_button(&mut ac, &t, Icon::Attach, 30.0, "Attach files").clicked() {
            attach = true;
        }

        let can_send = connected && (!draft.text.trim().is_empty() || !draft.files.is_empty()) && !conv.read_only;
        let send_r = Rect::from_min_size(pos2(box_rect.max.x - 92.0, box_rect.center().y - 16.0), vec2(82.0, 32.0));
        let mut sc = ui.new_child(egui::UiBuilder::new().max_rect(send_r).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let sresp = {
            let (r, resp) = sc.allocate_exact_size(send_r.size(), if can_send { Sense::click() } else { Sense::hover() });
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, can_send, "Send"));
            let fill = if !can_send {
                t.pill_quiet
            } else if resp.hovered() {
                t.accent.linear_multiply(0.9)
            } else {
                t.accent
            };
            sc.painter().rect_filled(r, 6.0, fill);
            let ink = if can_send { t.on_accent } else { t.text_disabled };
            icons::paint(sc.painter(), Rect::from_min_size(pos2(r.min.x + 12.0, r.center().y - 8.0), Vec2::splat(16.0)), Icon::Send, ink, ink);
            sc.painter().text(pos2(r.min.x + 36.0, r.center().y), Align2::LEFT_CENTER, "Send", theme::semibold(13.0), ink);
            resp
        };
        let tip = if !connected {
            "Waiting for your phone"
        } else if conv.read_only {
            "You can't reply in this conversation"
        } else if enter_sends {
            "Send (Enter). Shift+Enter starts a new line."
        } else {
            "Send (Ctrl+Enter)"
        };
        if sresp.on_hover_text(tip).clicked() {
            send = true;
        }

        let edit_r = Rect::from_min_max(pos2(attach_r.max.x + 6.0, box_rect.min.y + 8.0), pos2(send_r.min.x - 10.0, box_rect.max.y - 8.0));
        let mut eu = ui.new_child(egui::UiBuilder::new().max_rect(edit_r).layout(egui::Layout::top_down(egui::Align::Min)));
        let hint = if conv.read_only {
            "This conversation can't be replied to".to_string()
        } else if conv.kind == "rcs" {
            "Chat message".to_string()
        } else {
            "Text message".to_string()
        };
        let edit_id = egui::Id::unique(("composer", &id));
        // Enter sends (unless Shift is held): take the key before the text box sees it.
        if has_focus && can_send {
            let pressed = eu.input_mut(|i| {
                if enter_sends {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                } else {
                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                }
            });
            if pressed {
                send = true;
            }
        }
        ScrollArea::vertical().max_height(edit_r.height()).auto_shrink([false, true]).id_salt(("composer_scroll", &id)).show(&mut eu, |ui| {
            let te = egui::TextEdit::multiline(&mut draft.text)
                .id(edit_id)
                .frame(Frame::NONE)
                .margin(Margin::symmetric(2, 6))
                .desired_rows(1)
                .desired_width(edit_r.width() - 6.0)
                .font(theme::regular(13.5))
                .text_color(t.text)
                .hint_text(egui::RichText::new(hint).color(t.text_dim))
                .interactive(!conv.read_only);
            let r = ui.add(te);
            if focus {
                r.request_focus();
            }
            if r.changed() && connected {
                // Let the other side see we're typing, at most every few seconds.
                let now = ui.input(|i| i.time);
                let last = ui.data(|d| d.get_temp::<f64>(egui::Id::unique(("typing_sent", &id))).unwrap_or(-10.0));
                if now - last > 4.0 {
                    ui.data_mut(|d| d.insert_temp(egui::Id::unique(("typing_sent", &id)), now));
                    self.bridge.send(serde_json::json!({"cmd": "typing", "conversation_id": id}));
                }
            }
        });

        if let Some(i) = remove_file {
            if let Some(d) = self.drafts.get_mut(&id) {
                if i < d.files.len() {
                    d.files.remove(i);
                }
            }
        }
        if attach {
            if let Some(files) = rfd::FileDialog::new().set_title("Attach files").pick_files() {
                self.drafts.entry(id.clone()).or_default().files.extend(files);
            }
            self.focus_composer = true;
        }
        if send {
            self.send_draft(&id);
            self.focus_composer = true;
        }
    }

    // -----------------------------------------------------------------------------------------
    // Pairing

    pub fn pairing_view(&mut self, ui: &mut Ui) {
        let t = self.t;
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, 8.0, t.surface);
        ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.0, t.border), StrokeKind::Inside);

        let card_w = (rect.width() - 80.0).clamp(320.0, 880.0);
        let card_h = 420.0_f32.min(rect.height() - 40.0);
        let card = Rect::from_center_size(rect.center(), vec2(card_w, card_h));
        let mut u = ui.new_child(egui::UiBuilder::new().max_rect(card).layout(egui::Layout::left_to_right(egui::Align::Min)));
        let wide = card_w > 640.0;
        let text_w = if wide { card_w - 330.0 } else { card_w };

        u.vertical(|ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y = 10.0;
            match &self.conn {
                Conn::HelperMissing(msg) => {
                    ui.label(egui::RichText::new("MessageCraft can't reach its connection helper").font(theme::semibold(22.0)).color(t.text));
                    ui.label(egui::RichText::new(msg.clone()).font(theme::regular(13.0)).color(t.text_dim));
                    ui.label(egui::RichText::new("The helper (messagecraft-bridge.exe) must sit in the same folder as MessageCraft.exe.").font(theme::regular(13.0)).color(t.text_dim));
                    ui.add_space(8.0);
                    if widgets::primary_button(ui, &t, "Try again", true).clicked() {
                        let ctx = ui.ctx().clone();
                        self.restart_helper(&ctx);
                    }
                    return;
                }
                Conn::Starting => {
                    ui.label(egui::RichText::new("Starting MessageCraft…").font(theme::semibold(22.0)).color(t.text));
                    return;
                }
                _ => {}
            }
            let qr_mode = matches!(self.pair_step, PairStep::Qr);
            ui.label(egui::RichText::new("Use MessageCraft with your phone").font(theme::semibold(22.0)).color(t.text));
            ui.label(
                egui::RichText::new("Read and send your phone's texts from this computer. MessageCraft pairs with Google Messages on your Android phone, the same way Google Messages for web does.")
                    .font(theme::regular(13.5))
                    .color(t.text_dim),
            );
            ui.add_space(6.0);
            let steps: [&str; 3] = if qr_mode {
                [
                    "Open Google Messages on your Android phone.",
                    "Tap your profile picture (or the ⋮ menu), then Device pairing.",
                    "Tap QR code scanner and point your phone at the code.",
                ]
            } else {
                [
                    "Click Sign in with Google and sign in with the Google account your phone's Google Messages uses.",
                    "Google finds your phone, and Google Messages on it shows a few emoji.",
                    "On your phone, tap the emoji MessageCraft shows. That's it.",
                ]
            };
            for (i, s) in steps.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 11.0, t.accent_soft);
                    ui.painter().text(r.center(), Align2::CENTER_CENTER, (i + 1).to_string(), theme::semibold(12.0), t.accent_soft_text);
                    ui.add(egui::Label::new(egui::RichText::new(*s).font(theme::regular(13.5)).color(t.text)).wrap());
                });
            }
            ui.add_space(6.0);
            let note = if qr_mode {
                "Newer versions of Google Messages pair with your Google account instead of a QR code. If your phone's Device pairing screen has no QR code scanner, go back and use Sign in with Google."
            } else {
                "Your phone's Device pairing screen should say \"sign in to the same Google Account\". Your phone stays the place your messages live: it needs to be on and online for MessageCraft to sync and send. You only pair once; the pairing is kept on this computer, locked to your Windows account."
            };
            ui.add(egui::Label::new(egui::RichText::new(note).font(theme::regular(12.0)).color(t.text_dim)).wrap());
            if !wide {
                ui.add_space(8.0);
                self.pair_block(ui);
            }
        });
        if wide {
            u.add_space(30.0);
            u.vertical(|ui| self.pair_block(ui));
        }
    }

    /// The right-hand side of the pairing page: the choice, the sign-in under way, the emoji to
    /// tap, or the QR code.
    fn pair_block(&mut self, ui: &mut Ui) {
        let t = self.t;
        let size = 260.0;
        ui.set_width(size + 20.0);
        ui.spacing_mut().item_spacing.y = 10.0;
        let error = self.pair_error.clone();
        enum Show {
            Choose,
            SigningIn,
            Contacting,
            Emoji(String, Option<egui::TextureId>),
            Qr,
        }
        let show = match &self.pair_step {
            PairStep::Choose => Show::Choose,
            PairStep::SigningIn => Show::SigningIn,
            PairStep::Contacting => Show::Contacting,
            PairStep::Emoji { emoji, image } => Show::Emoji(emoji.clone(), image.as_ref().map(|i| i.id())),
            PairStep::Qr => Show::Qr,
        };
        match show {
            Show::Choose => {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
                ui.painter().rect_filled(r, 10.0, t.pill_quiet);
                icons::paint(ui.painter(), Rect::from_center_size(r.center() - vec2(0.0, 20.0), Vec2::splat(84.0)), Icon::Phone, t.text_dim, t.accent);
                ui.painter().text(pos2(r.center().x, r.center().y + 50.0), Align2::CENTER_CENTER, "Ready to pair", theme::medium(14.0), t.text_dim);
                if let Some(e) = &error {
                    ui.add(egui::Label::new(egui::RichText::new(e).font(theme::regular(12.5)).color(t.red)).wrap());
                }
                if widgets::primary_button(ui, &t, "Sign in with Google", true).clicked() {
                    self.sign_in_with_google();
                }
                if widgets::secondary_button(ui, &t, "Use a QR code instead").clicked() {
                    self.pair();
                }
            }
            Show::SigningIn => {
                self.waiting_card(ui, size, "Sign in in the window that opened", "Use the Google account your phone uses.");
                if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                    self.cancel_pairing();
                }
            }
            Show::Contacting => {
                let sub = match &self.google_email {
                    Some(e) => format!("Signed in as {e}. Finding your phone…"),
                    None => "Finding your phone…".to_string(),
                };
                self.waiting_card(ui, size, "Contacting your phone", &sub);
                if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                    self.cancel_pairing();
                }
            }
            Show::Emoji(emoji, image) => {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
                ui.painter().rect_filled(r, 10.0, t.pill_quiet);
                let pic = Rect::from_center_size(r.center() - vec2(0.0, 18.0), Vec2::splat(150.0));
                match image {
                    Some(id) => {
                        egui::Image::new((id, pic.size())).paint_at(ui, pic);
                    }
                    None => {
                        ui.painter().text(pic.center(), Align2::CENTER_CENTER, &emoji, theme::regular(110.0), t.text);
                    }
                }
                ui.painter().text(pos2(r.center().x, r.max.y - 30.0), Align2::CENTER_CENTER, "Tap this emoji on your phone", theme::semibold(15.0), t.text);
                ui.add(egui::Label::new(egui::RichText::new("Google Messages on your phone is showing a few emoji. Tap the one above to finish pairing.").font(theme::regular(12.5)).color(t.text_dim)).wrap());
                if widgets::secondary_button(ui, &t, "Cancel").clicked() {
                    self.cancel_pairing();
                }
            }
            Show::Qr => {
                self.qr_block(ui);
                if widgets::secondary_button(ui, &t, "Back").clicked() {
                    self.cancel_pairing();
                }
            }
        }
    }

    fn waiting_card(&self, ui: &mut Ui, size: f32, title: &str, sub: &str) {
        let t = self.t;
        let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        ui.painter().rect_filled(r, 10.0, t.pill_quiet);
        let time = ui.input(|i| i.time) as f32;
        let c = r.center() - vec2(0.0, 24.0);
        for k in 0..8 {
            let a = k as f32 / 8.0 * std::f32::consts::TAU + time * 2.5;
            let fade = 0.25 + 0.75 * (k as f32 / 8.0);
            ui.painter().circle_filled(c + vec2(a.cos(), a.sin()) * 26.0, 4.5, t.accent.linear_multiply(fade));
        }
        ui.painter().text(pos2(r.center().x, r.center().y + 34.0), Align2::CENTER_CENTER, title, theme::semibold(14.0), t.text);
        widgets::clipped(ui.painter(), pos2(r.min.x + 14.0, r.center().y + 58.0), r.width() - 28.0, sub, theme::regular(12.0), t.text_dim);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }

    fn qr_block(&mut self, ui: &mut Ui) {
        let t = self.t;
        let size = 260.0;
        if let Some((_, modules, w, at)) = &self.qr {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            // A QR code is always dark on light, whatever the theme, so phones can read it.
            ui.painter().rect_filled(r, 10.0, Color32::WHITE);
            let quiet = 3.0;
            let n = *w as f32 + 2.0 * quiet;
            let cell = (size - 8.0) / n;
            let origin = r.min + vec2(4.0 + quiet * cell, 4.0 + quiet * cell);
            for (i, dark) in modules.iter().enumerate() {
                if *dark {
                    let x = (i % w) as f32;
                    let y = (i / w) as f32;
                    let cr = Rect::from_min_size(origin + vec2(x * cell, y * cell), Vec2::splat(cell + 0.3));
                    ui.painter().rect_filled(cr, 0.0, Color32::from_rgb(0x12, 0x16, 0x1D));
                }
            }
            let left = 30.0 - at.elapsed().as_secs_f32();
            ui.add_space(6.0);
            let text = if left > 0.0 { format!("This code refreshes in {} s", left.ceil() as i32) } else { "Getting a new code…".into() };
            ui.label(egui::RichText::new(text).font(theme::regular(12.0)).color(t.text_dim));
        } else {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            ui.painter().rect_filled(r, 10.0, t.pill_quiet);
            let msg = if matches!(self.conn, Conn::Pairing) { "Getting a code from Google…" } else if self.qr_expired { "The code expired." } else { "Ready to pair." };
            ui.painter().text(r.center(), Align2::CENTER_CENTER, msg, theme::regular(13.0), t.text_dim);
            ui.add_space(8.0);
            if self.qr_expired && widgets::primary_button(ui, &t, "Get a new code", true).clicked() {
                self.pair();
            }
        }
    }
}

fn status_line(m: &Message, t: &theme::Tokens) -> (String, Color32) {
    if m.failed {
        let why = if !m.local_error.is_empty() { m.local_error.clone() } else if !m.status_text.is_empty() { m.status_text.clone() } else { "Not sent".into() };
        return (format!("Not sent — {why}. Right-click to try again."), t.red);
    }
    let s = match m.status.as_str() {
        "sending" => "Sending…",
        "sent" => "Sent",
        "delivered" => "Delivered",
        "read" => "Read",
        "scheduled" => "Scheduled",
        "not_delivered" => "Not delivered yet",
        _ => "",
    };
    let s = if s.is_empty() { timefmt::clock(m.ts) } else { format!("{s} · {}", timefmt::clock(m.ts)) };
    (s, t.text_dim)
}

fn kind_name(mime: &str) -> &'static str {
    if mime.starts_with("video/") {
        "Video"
    } else if mime.starts_with("audio/") {
        "Audio"
    } else if mime.contains("vcard") {
        "Contact card"
    } else {
        "Attachment"
    }
}

fn human_size(b: i64) -> String {
    let b = b as f64;
    if b >= 1024.0 * 1024.0 {
        format!("{:.1} MB", b / 1024.0 / 1024.0)
    } else if b >= 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{b:.0} B")
    }
}
