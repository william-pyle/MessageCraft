//! Windows frame integration. Keep winit's sizing, work-area, DPI and close paths;
//! only supply the edge hit tests it lacks for undecorated windows.
use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

#[derive(Default)]
pub struct NativeWindow {
    hwnd: Option<isize>,
    pub error: Option<String>,
}

impl NativeWindow {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(windows)]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = cc.window_handle()
                && let RawWindowHandle::Win32(handle) = handle.as_raw()
            {
                let hwnd = handle.hwnd.get();
                // Called on the window's event-loop thread. No owned pointer is
                // passed to the subclass, which removes itself at destruction.
                if unsafe {
                    windows_sys::Win32::UI::Shell::SetWindowSubclass(
                        hwnd as _,
                        Some(edge_proc),
                        1,
                        0,
                    )
                } != 0
                {
                    return Self {
                        hwnd: Some(hwnd),
                        error: None,
                    };
                }
            }
            // Keep a usable native frame if Windows cannot install hit tests.
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Decorations(true));
            return Self {
                hwnd: None,
                error: Some("Custom window controls unavailable; using the Windows frame.".into()),
            };
        }
        #[cfg(not(windows))]
        {
            let _ = cc;
            Self::default()
        }
    }

    pub fn available(&self) -> bool {
        self.hwnd.is_some()
    }

    pub fn system_menu(&self, pixels_per_point: f32) -> Result<(), String> {
        #[cfg(windows)]
        {
            use windows_sys::Win32::{
                Foundation::POINT, Graphics::Gdi::ClientToScreen, UI::WindowsAndMessaging::*,
            };
            let hwnd = self.hwnd.ok_or("Window handle unavailable")? as _;
            // Same OS-owned menu and WM_SYSCOMMAND path as Alpha, with the anchor
            // converted from egui points to physical screen coordinates.
            unsafe {
                let menu = GetSystemMenu(hwnd, 0);
                if menu.is_null() {
                    return Err("Windows system menu is unavailable".into());
                }
                let maximized = IsZoomed(hwnd) != 0;
                for (command, enabled) in [
                    (SC_RESTORE, maximized),
                    (SC_MAXIMIZE, !maximized),
                    (SC_SIZE, !maximized),
                    (SC_MOVE, !maximized),
                ] {
                    EnableMenuItem(
                        menu,
                        command,
                        MF_BYCOMMAND | if enabled { MF_ENABLED } else { MF_GRAYED },
                    );
                }
                let mut point = POINT {
                    x: (8.0 * pixels_per_point) as i32,
                    y: (crate::chrome::TITLE_BAR * pixels_per_point) as i32,
                };
                if ClientToScreen(hwnd, &mut point) == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let command = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    0,
                    hwnd,
                    std::ptr::null(),
                );
                if command != 0 && PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0) == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
unsafe extern "system" fn edge_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    id: usize,
    _: usize,
) -> isize {
    use windows_sys::Win32::{
        Foundation::RECT,
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{DefSubclassProc, RemoveWindowSubclass},
            WindowsAndMessaging::*,
        },
    };
    // Signed extraction is essential for monitors to the left/above primary.
    unsafe {
        if msg == WM_NCHITTEST && IsZoomed(hwnd) == 0 && IsIconic(hwnd) == 0 {
            let mut rect: RECT = std::mem::zeroed();
            if GetWindowRect(hwnd, &mut rect) != 0 {
                let x = lp as i16 as i32 - rect.left;
                let y = (lp >> 16) as i16 as i32 - rect.top;
                let border = (6 * GetDpiForWindow(hwnd).max(96) / 96) as i32;
                if let Some(hit) =
                    edge_hit(x, y, rect.right - rect.left, rect.bottom - rect.top, border)
                {
                    return hit as isize;
                }
            }
        }
        if msg == WM_NCDESTROY {
            RemoveWindowSubclass(hwnd, Some(edge_proc), id);
        }
        DefSubclassProc(hwnd, msg, wp, lp)
    }
}

#[cfg(any(windows, test))]
fn edge_hit(x: i32, y: i32, width: i32, height: i32, border: i32) -> Option<u32> {
    // HTLEFT..HTBOTTOMRIGHT are stable Win32 hit-test constants (10..17).
    if x < 0 || y < 0 || x >= width || y >= height {
        return None;
    }
    let left = x < border;
    let right = x >= width - border;
    let top = y < border;
    let bottom = y >= height - border;
    let near_left = x < border * 2;
    let near_right = x >= width - border * 2;
    let near_top = y < border * 2;
    let near_bottom = y >= height - border * 2;
    match (left, right, top, bottom) {
        _ if (top && near_left) || (left && near_top) => Some(13),
        _ if (top && near_right) || (right && near_top) => Some(14),
        _ if (bottom && near_left) || (left && near_bottom) => Some(16),
        _ if (bottom && near_right) || (right && near_bottom) => Some(17),
        (true, _, _, _) => Some(10),
        (_, true, _, _) => Some(11),
        (_, _, true, _) => Some(12),
        (_, _, _, true) => Some(15),
        _ => None,
    }
}

pub const CONTROLS_WIDTH: f32 = 126.0;

/// The search and account controls keep their own hit regions, even at UI zoom.
pub fn search_rect(rect: Rect) -> Rect {
    let account = if rect.width() >= 600.0 { 200.0 } else { 42.0 };
    let width = (rect.width() * 0.36)
        .clamp(100.0, 540.0)
        .min((rect.width() - 140.0 - account - 24.0).max(60.0));
    let center = rect
        .center()
        .x
        .max(rect.left() + 140.0 + width / 2.0)
        .min(rect.right() - account - 24.0 - width / 2.0);
    Rect::from_center_size(pos2(center, rect.center().y), vec2(width, 34.0))
}

pub fn caption_buttons(ui: &mut Ui, rect: Rect, t: &crate::theme::Tokens) {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    for (index, label) in [
        "Minimize window",
        if maximized {
            "Restore window"
        } else {
            "Maximize window"
        },
        "Close window (Alt+F4)",
    ]
    .iter()
    .enumerate()
    {
        let r = Rect::from_min_size(
            pos2(
                rect.right() - CONTROLS_WIDTH + index as f32 * 42.0,
                rect.top(),
            ),
            vec2(42.0, rect.height()),
        );
        let response = ui.interact(
            r,
            egui::Id::unique(("window_caption", index)),
            Sense::click(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, *label));
        let p = ui.painter();
        let hovered = response.hovered() || response.is_pointer_button_down_on();
        if hovered {
            p.rect_filled(
                r,
                0.0,
                if index == 2 {
                    Color32::from_rgb(196, 43, 59)
                } else {
                    t.hover
                },
            );
        }
        if response.has_focus() {
            p.rect_stroke(
                r.shrink(3.0),
                2.0,
                Stroke::new(2.0, t.accent),
                StrokeKind::Inside,
            );
        }
        let ink = Stroke::new(
            1.2,
            if hovered && index == 2 {
                Color32::WHITE
            } else {
                t.text
            },
        );
        let icon = Rect::from_center_size(r.center(), vec2(10.0, 10.0));
        match index {
            0 => {
                p.line_segment(
                    [
                        pos2(icon.left(), icon.center().y),
                        pos2(icon.right(), icon.center().y),
                    ],
                    ink,
                );
            }
            1 => {
                if maximized {
                    p.rect_stroke(
                        icon.translate(vec2(2.0, -2.0)),
                        0.0,
                        ink,
                        StrokeKind::Inside,
                    );
                    p.rect_filled(icon, 0.0, if hovered { t.hover } else { t.title_bar });
                }
                p.rect_stroke(icon, 0.0, ink, StrokeKind::Inside);
            }
            _ => {
                p.line_segment([icon.left_top(), icon.right_bottom()], ink);
                p.line_segment([icon.right_top(), icon.left_bottom()], ink);
            }
        }
        if response.on_hover_text(*label).clicked() {
            ui.ctx().send_viewport_cmd(match index {
                0 => egui::ViewportCommand::Minimized(true),
                1 => egui::ViewportCommand::Maximized(!maximized),
                _ => egui::ViewportCommand::Close,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_edges_corners_and_interior_at_multiple_dpi() {
        for scale in [1, 2, 3] {
            let (w, h, b) = (900 * scale, 560 * scale, 6 * scale);
            for (x, y, hit) in [
                (0, 0, 13),
                (w - 1, 0, 14),
                (0, h - 1, 16),
                (w - 1, h - 1, 17),
                (0, h / 2, 10),
                (w - 1, h / 2, 11),
                (w / 2, 0, 12),
                (w / 2, h - 1, 15),
            ] {
                assert_eq!(edge_hit(x, y, w, h, b), Some(hit));
            }
            assert_eq!(edge_hit(w / 2, h / 2, w, h, b), None);
            assert_eq!(edge_hit(-1, 0, w, h, b), None);
            assert_eq!(edge_hit(w, h, w, h, b), None);
        }
    }
    #[test]
    fn toolbar_keeps_search_between_quick_actions_and_account_at_zoom() {
        for width in [450.0, 600.0, 900.0, 1440.0, 1920.0] {
            let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(width - CONTROLS_WIDTH, 48.0));
            let search = search_rect(rect);
            assert!(search.left() >= 130.0, "width {width}");
            assert!(search.right() <= rect.right() - 54.0);
            assert!(search.width() >= 60.0);
        }
    }
}
