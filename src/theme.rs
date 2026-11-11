//! MessageCraft's look: the MailCraft palette, type scale and widget style.
//!
//! The colour values, type sizes and widget metrics follow MailCraft's own theme
//! (github.com/UCS-Care-Group/mailcraft, crates/ui-egui/src/theme.rs, MIT OR Apache-2.0), so the
//! two programs sit side by side as one family. See NOTICE.md.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Visuals};

fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Every colour the window uses; widgets never hard-code colours.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct Tokens {
    pub dark: bool,
    pub title_bar: Color32,
    pub ribbon: Color32,
    pub tab_strip: Color32,
    pub nav: Color32,
    pub canvas: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub text_disabled: Color32,
    pub icon: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub checked: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub on_accent: Color32,
    pub input: Color32,
    pub input_border: Color32,
    pub group_label: Color32,
    pub status_bar: Color32,
    pub selection: Color32,
    pub selection_edge: Color32,
    pub accent_soft: Color32,
    pub accent_soft_text: Color32,
    pub pill_quiet: Color32,
    pub rule: Color32,
    pub border_faint: Color32,
    pub hover_subtle: Color32,
    pub menu: Color32,
    pub red: Color32,
    pub green: Color32,
    pub orange: Color32,
    pub icon_red: Color32,
    pub icon_yellow: Color32,
    pub icon_green: Color32,
    pub scrim: Color32,
    pub shadow_popup: Color32,
    pub shadow_window: Color32,
    pub text_selection: Color32,
    pub ribbon_edge: Stroke,
    pub field_edge: Color32,
    pub backstage_column: Color32,
    /// A message from someone else, and one of ours.
    pub bubble_in: Color32,
    pub bubble_out: Color32,
    pub bubble_out_text: Color32,
}

impl Tokens {
    /// The light palette: a cool grey window, white and near-white sheets, a blue accent.
    pub fn light() -> Self {
        let window = c(0xF2, 0xF4, 0xF7);
        let panel = c(0xF8, 0xF9, 0xFB);
        let surface = Color32::WHITE;
        let hover = c(0xE8, 0xEC, 0xF2);
        let accent = c(0x0F, 0x68, 0xF0);
        let soft = c(0xE3, 0xED, 0xFF);
        let input_border = c(0x7F, 0x8A, 0x9E);
        Tokens {
            dark: false,
            title_bar: window,
            ribbon: Color32::WHITE,
            tab_strip: window,
            nav: window,
            canvas: window,
            panel,
            surface,
            border: c(0xDD, 0xE2, 0xEA),
            border_strong: c(0xB8, 0xC0, 0xCD),
            text: c(0x1D, 0x23, 0x30),
            text_dim: c(0x5A, 0x62, 0x73),
            text_disabled: c(0x7F, 0x88, 0x99),
            icon: c(0x3A, 0x42, 0x52),
            hover,
            pressed: c(0xD6, 0xDC, 0xE6),
            checked: soft,
            accent,
            accent_text: c(0x06, 0x34, 0xC4),
            on_accent: Color32::WHITE,
            input: Color32::WHITE,
            input_border,
            group_label: c(0x5A, 0x62, 0x73),
            status_bar: window,
            selection: c(0xEE, 0xF4, 0xFF),
            selection_edge: accent,
            accent_soft: soft,
            accent_soft_text: c(0x06, 0x34, 0xC4),
            pill_quiet: c(0xEC, 0xEF, 0xF4),
            rule: c(0xE6, 0xE9, 0xEF),
            border_faint: c(0xEC, 0xEF, 0xF4),
            hover_subtle: hover.linear_multiply(0.6),
            menu: Color32::WHITE,
            red: c(0xD1, 0x3B, 0x3B),
            green: c(0x1F, 0x7A, 0x4D),
            orange: c(0xB8, 0x5C, 0x08),
            icon_red: c(0xC4, 0x31, 0x4B),
            icon_yellow: c(0x9C, 0x67, 0x00),
            icon_green: c(0x2B, 0x7A, 0x4B),
            scrim: Color32::from_black_alpha(60),
            shadow_popup: Color32::from_black_alpha(40),
            shadow_window: Color32::from_black_alpha(50),
            text_selection: accent.linear_multiply(0.30),
            ribbon_edge: Stroke::new(1.0, c(0xE3, 0xE7, 0xEE)),
            field_edge: input_border,
            backstage_column: panel,
            bubble_in: c(0xEC, 0xEF, 0xF4),
            bubble_out: accent,
            bubble_out_text: Color32::WHITE,
        }
    }

    /// The dark palette: one cool ramp, window to folder pane to list to ribbon, and a lighter
    /// blue accent.
    pub fn dark() -> Self {
        let window = c(0x12, 0x16, 0x1D);
        let panel = c(0x16, 0x1B, 0x23);
        let surface = c(0x1B, 0x21, 0x2B);
        let hover = c(0x2A, 0x32, 0x40);
        let accent = c(0x5A, 0xA2, 0xFF);
        let soft = c(0x1F, 0x33, 0x58);
        let input_border = c(0x7D, 0x88, 0x9C);
        Tokens {
            dark: true,
            title_bar: window,
            ribbon: c(0x20, 0x27, 0x33),
            tab_strip: window,
            nav: window,
            canvas: window,
            panel,
            surface,
            border: c(0x2C, 0x34, 0x42),
            border_strong: c(0x4A, 0x55, 0x68),
            text: c(0xF1, 0xF4, 0xF9),
            text_dim: c(0xB4, 0xBC, 0xCB),
            text_disabled: c(0x6E, 0x78, 0x8C),
            icon: c(0xDF, 0xE4, 0xEC),
            hover,
            pressed: c(0x36, 0x40, 0x52),
            checked: soft,
            accent,
            accent_text: c(0x8F, 0xBF, 0xFF),
            on_accent: c(0x08, 0x13, 0x2B),
            input: surface,
            input_border,
            group_label: c(0xB4, 0xBC, 0xCB),
            status_bar: window,
            selection: c(0x1F, 0x2B, 0x40),
            selection_edge: accent,
            accent_soft: soft,
            accent_soft_text: c(0xCF, 0xE0, 0xFF),
            pill_quiet: c(0x26, 0x2E, 0x3B),
            rule: c(0x25, 0x2C, 0x38),
            border_faint: c(0x22, 0x29, 0x34),
            hover_subtle: hover.linear_multiply(0.6),
            menu: c(0x20, 0x27, 0x33),
            red: c(0xEE, 0x6A, 0x6A),
            green: c(0x5B, 0xC0, 0x8A),
            orange: c(0xF0, 0xA0, 0x50),
            icon_red: c(0xF0, 0x7A, 0x7A),
            icon_yellow: c(0xE8, 0xB7, 0x4A),
            icon_green: c(0x5B, 0xC0, 0x8A),
            scrim: Color32::from_black_alpha(110),
            shadow_popup: Color32::from_black_alpha(120),
            shadow_window: Color32::from_black_alpha(140),
            text_selection: accent.linear_multiply(0.45),
            ribbon_edge: Stroke::new(1.0, c(0x2A, 0x32, 0x40)),
            field_edge: input_border,
            backstage_column: panel,
            bubble_in: c(0x26, 0x2E, 0x3B),
            bubble_out: c(0x1F, 0x4F, 0x9A),
            bubble_out_text: c(0xF1, 0xF4, 0xF9),
        }
    }

    pub fn for_dark(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }
}

// ---------------------------------------------------------------------------------------------
// Fonts: Segoe UI from Windows in front (as MailCraft does), the bundled Inter behind it.

fn system_face(name: &str) -> Option<Vec<u8>> {
    let dir = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    std::fs::read(dir.join("Fonts").join(name)).ok()
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, data: FontData| {
        fonts.font_data.insert(name.into(), Arc::new(data));
    };
    add(&mut fonts, "Inter", FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf")));
    add(&mut fonts, "InterMedium", FontData::from_static(include_bytes!("../assets/fonts/Inter-Medium.ttf")));
    add(&mut fonts, "InterSemiBold", FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")));

    let sys = |file: &str, name: &str, fonts: &mut FontDefinitions| -> Option<String> {
        let bytes = system_face(file)?;
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(bytes)));
        Some(name.to_string())
    };
    let regular = sys("segoeui.ttf", "SegoeUI", &mut fonts);
    let semibold = if regular.is_some() { sys("seguisb.ttf", "SegoeUISemibold", &mut fonts) } else { None };
    let bold = if regular.is_some() { sys("segoeuib.ttf", "SegoeUIBold", &mut fonts) } else { None };
    let emoji = sys("seguiemj.ttf", "SegoeEmoji", &mut fonts);
    let symbol = sys("seguisym.ttf", "SegoeSymbol", &mut fonts);

    let mut fallback: Vec<String> = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some(e) = emoji {
        fallback.insert(0, e);
    }
    if let Some(s) = symbol {
        fallback.push(s);
    }
    let chain = |faces: &[Option<&str>]| -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for n in faces.iter().flatten().map(|s| s.to_string()).chain(fallback.iter().cloned()) {
            if !names.contains(&n) {
                names.push(n);
            }
        }
        names
    };
    let (r, s, b) = (regular.as_deref(), semibold.as_deref(), bold.as_deref());
    fonts.families.insert(FontFamily::Proportional, chain(&[r, Some("Inter")]));
    fonts.families.insert(FontFamily::Name("medium".into()), chain(&[s, Some("InterMedium"), r, Some("Inter")]));
    fonts.families.insert(FontFamily::Name("semibold".into()), chain(&[s, Some("InterSemiBold"), r, Some("Inter")]));
    fonts.families.insert(FontFamily::Name("bold".into()), chain(&[b, s, Some("InterSemiBold"), r, Some("Inter")]));
    ctx.set_fonts(fonts);
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("medium".into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Apply both palettes to egui and MailCraft's spacing.
pub fn apply(ctx: &egui::Context) {
    ctx.set_visuals_of(egui::Theme::Light, visuals(&Tokens::light()));
    ctx.set_visuals_of(egui::Theme::Dark, visuals(&Tokens::dark()));
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.interact_size = egui::vec2(24.0, 22.0);
        s.spacing.menu_margin = egui::Margin::same(6);
        s.text_styles.insert(egui::TextStyle::Body, regular(12.5));
        s.text_styles.insert(egui::TextStyle::Button, regular(12.5));
        s.text_styles.insert(egui::TextStyle::Small, regular(10.5));
        s.text_styles.insert(egui::TextStyle::Heading, semibold(17.0));
        s.interaction.tooltip_delay = 0.45;
    });
}

fn visuals(t: &Tokens) -> Visuals {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = t.canvas;
    v.window_fill = t.menu;
    v.extreme_bg_color = t.input;
    v.faint_bg_color = t.panel;
    v.window_stroke = Stroke::new(1.0, t.border_strong);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(6);
    v.selection.bg_fill = t.text_selection;
    v.selection.stroke = Stroke::new(1.0, t.accent);
    v.hyperlink_color = t.accent_text;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.inactive.bg_fill = t.input;
    v.widgets.inactive.weak_bg_fill = t.input;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.input_border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.inactive.corner_radius = CornerRadius::same(6);
    v.widgets.hovered.bg_fill = t.hover;
    v.widgets.hovered.weak_bg_fill = t.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.text_dim);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.hovered.corner_radius = CornerRadius::same(6);
    v.widgets.active.bg_fill = t.pressed;
    v.widgets.active.weak_bg_fill = t.pressed;
    v.widgets.active.fg_stroke = Stroke::new(1.0, t.text);
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.text_dim);
    v.widgets.active.corner_radius = CornerRadius::same(6);
    v.widgets.open.bg_fill = t.hover;
    v.widgets.open.weak_bg_fill = t.hover;
    v.widgets.open.corner_radius = CornerRadius::same(6);
    v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: t.shadow_popup };
    v.window_shadow = egui::Shadow { offset: [0, 8], blur: 28, spread: 0, color: t.shadow_window };
    v
}

/// A stable colour for a person's avatar (MailCraft's avatar palette), unless the phone gave one.
pub fn avatar_color(key: &str, phone_hex: &str) -> Color32 {
    if let Some(c) = parse_hex(phone_hex) {
        return c;
    }
    const PALETTE: [(u8, u8, u8); 8] = [
        (0x0B, 0x7F, 0xAB),
        (0x2E, 0x8B, 0x57),
        (0xB3, 0x5C, 0x1E),
        (0x7E, 0x57, 0xC2),
        (0xB0, 0x3A, 0x5B),
        (0x3F, 0x6F, 0xB5),
        (0x6B, 0x7A, 0x1F),
        (0x8A, 0x5A, 0x44),
    ];
    let hash = key.to_lowercase().bytes().fold(0x811C_9DC5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let (r, g, b) = PALETTE[hash as usize % PALETTE.len()];
    Color32::from_rgb(r, g, b)
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    let col = Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    // Very light colours don't carry white initials: darken them.
    let lum = 0.299 * col.r() as f32 + 0.587 * col.g() as f32 + 0.114 * col.b() as f32;
    if lum > 190.0 { Some(col.linear_multiply(0.7)) } else { Some(col) }
}

pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().filter(|w| w.chars().next().is_some_and(|c| c.is_alphanumeric())).collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => {
            let first = one.chars().next().unwrap_or(' ');
            if first.is_ascii_digit() || first == '+' { String::new() } else { first.to_uppercase().collect() }
        }
        [first, .., last] => {
            let a = first.chars().next().unwrap_or(' ');
            let b = last.chars().next().unwrap_or(' ');
            if a.is_ascii_digit() { String::new() } else { format!("{}{}", a.to_uppercase(), b.to_uppercase()) }
        }
    }
}
