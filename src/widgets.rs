//! Small painted widgets in MailCraft's manner: icon buttons, ribbon buttons, pills, avatars.

use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

/// A square button with only an icon, a hover wash and a tooltip.
pub fn icon_button(ui: &mut Ui, t: &Tokens, icon: Icon, size: f32, tip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tip));
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, 4.0, t.pressed);
    } else if resp.hovered() {
        p.rect_filled(rect, 4.0, t.hover);
    }
    let r = Rect::from_center_size(rect.center(), Vec2::splat((size * 0.62).min(20.0)));
    icons::paint(p, r, icon, t.icon, t.accent);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// A ribbon button: icon and label, 32 high, with MailCraft's checked state (soft fill, accent edge).
pub fn ribbon_button(ui: &mut Ui, t: &Tokens, icon: Icon, label: &str, checked: bool, enabled: bool, dropdown: bool) -> Response {
    const H: f32 = 32.0;
    const PAD: f32 = 8.0;
    const ICON: f32 = 20.0;
    const GAP: f32 = 8.0;
    let font = theme::regular(13.333);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text);
    let w = PAD + ICON + GAP + galley.size().x + PAD + if dropdown { 14.0 } else { 0.0 };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, H), sense);
    resp.widget_info(|| {
        if checked { egui::WidgetInfo::selected(egui::Role::Button, enabled, true, label) } else { egui::WidgetInfo::labeled(egui::Role::Button, enabled, label) }
    });
    let p = ui.painter();
    if checked {
        p.rect_filled(rect, 4.0, t.checked);
        p.rect_stroke(rect, 4.0, Stroke::new(1.0, t.accent.linear_multiply(0.8)), StrokeKind::Inside);
    } else if enabled && resp.is_pointer_button_down_on() {
        p.rect_filled(rect, 4.0, t.pressed);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, 4.0, t.hover);
    }
    let ink = if enabled { t.icon } else { t.text_disabled };
    let accent = if enabled { t.accent } else { t.text_disabled };
    let ir = Rect::from_min_size(pos2(rect.min.x + PAD, rect.center().y - ICON / 2.0), Vec2::splat(ICON));
    icons::paint(p, ir, icon, ink, accent);
    let text_col = if checked { t.accent_soft_text } else if enabled { t.text } else { t.text_disabled };
    p.text(pos2(ir.max.x + GAP, rect.center().y), Align2::LEFT_CENTER, label, font, text_col);
    if dropdown {
        let dr = Rect::from_center_size(pos2(rect.max.x - PAD - 4.0, rect.center().y), Vec2::splat(10.0));
        icons::paint(p, dr, Icon::ChevronDown, t.text_dim, t.text_dim);
    }
    resp
}

/// The thin rule between ribbon groups.
pub fn ribbon_rule(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(vec2(19.0, 32.0), Sense::hover());
    let x = rect.center().x.round() + 0.5;
    ui.painter().line_segment([pos2(x, rect.min.y + 4.0), pos2(x, rect.max.y - 4.0)], Stroke::new(1.0, t.border));
}

/// A filled accent button (Send, Pair, Start).
pub fn primary_button(ui: &mut Ui, t: &Tokens, label: &str, enabled: bool) -> Response {
    let font = theme::semibold(13.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.on_accent);
    let size = vec2(galley.size().x + 32.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let fill = if !enabled {
        t.pill_quiet
    } else if resp.is_pointer_button_down_on() {
        t.accent.linear_multiply(0.8)
    } else if resp.hovered() {
        t.accent.linear_multiply(0.9)
    } else {
        t.accent
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, if enabled { t.on_accent } else { t.text_disabled });
    resp
}

/// An outlined button (Unpair, Cancel, Get a new code).
pub fn secondary_button(ui: &mut Ui, t: &Tokens, label: &str) -> Response {
    let font = theme::regular(13.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text);
    let size = vec2(galley.size().x + 28.0, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let fill = if resp.is_pointer_button_down_on() {
        t.pressed
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, t.border_strong), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, t.text);
    resp
}

/// A small outlined chip with an icon, like MailCraft's "Report a Problem" in the title bar.
pub fn chip(ui: &mut Ui, t: &Tokens, icon: Icon, label: &str, dot: Option<Color32>) -> Response {
    let font = theme::medium(12.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text);
    let extra = if dot.is_some() { 12.0 } else { 0.0 };
    let size = vec2(8.0 + 16.0 + 6.0 + galley.size().x + 10.0 + extra, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 4.0, t.hover);
    }
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let mut x = rect.min.x + 8.0;
    if let Some(c) = dot {
        p.circle_filled(pos2(x + 3.0, rect.center().y), 3.5, c);
        x += 12.0;
    }
    icons::paint(p, Rect::from_min_size(pos2(x, rect.center().y - 8.0), Vec2::splat(16.0)), icon, t.icon, t.accent);
    p.text(pos2(x + 22.0, rect.center().y), Align2::LEFT_CENTER, label, font, t.text);
    resp
}

/// A round avatar with initials, or a person mark when there are none.
pub fn avatar(p: &egui::Painter, t: &Tokens, center: egui::Pos2, radius: f32, name: &str, key: &str, phone_hex: &str, group: bool) {
    let col = theme::avatar_color(key, phone_hex);
    p.circle_filled(center, radius, col);
    let ini = theme::initials(name);
    if group {
        let r = Rect::from_center_size(center, Vec2::splat(radius * 1.2));
        icons::paint(p, r, Icon::People, Color32::WHITE, Color32::WHITE);
    } else if ini.is_empty() {
        let r = Rect::from_center_size(center, Vec2::splat(radius * 1.15));
        icons::paint(p, r, Icon::Person, Color32::WHITE, Color32::WHITE);
    } else {
        let _ = t;
        p.text(center, Align2::CENTER_CENTER, ini, theme::semibold(radius * 0.78), Color32::WHITE);
    }
}

/// A count pill on the right of a folder row.
pub fn count_pill(p: &egui::Painter, t: &Tokens, right_center: egui::Pos2, n: usize, strong: bool) {
    let text = n.to_string();
    let font = theme::bold(11.0);
    let galley = p.layout_no_wrap(text.clone(), font.clone(), t.text);
    let w = (galley.size().x + 12.0).max(22.0);
    let rect = Rect::from_center_size(pos2(right_center.x - w / 2.0, right_center.y), vec2(w, 17.0));
    let (fill, ink) = if strong { (t.accent_soft, t.accent_soft_text) } else { (t.pill_quiet, t.text_dim) };
    p.rect_filled(rect, CornerRadius::same(8), fill);
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, ink);
}

/// One line of text cut to `width` with an ellipsis, left edge at `pos`, vertically centred.
pub fn clipped(p: &egui::Painter, pos: egui::Pos2, width: f32, text: &str, font: egui::FontId, color: Color32) -> f32 {
    if width <= 4.0 || text.is_empty() {
        return 0.0;
    }
    let first_line: String = text.lines().next().unwrap_or("").chars().take(400).collect();
    let mut job = egui::text::LayoutJob::single_section(first_line, egui::text::TextFormat { font_id: font, color, ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: width, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = p.layout_job(job);
    let size = galley.size();
    p.galley(pos2(pos.x, pos.y - size.y / 2.0), galley, color);
    size.x
}
