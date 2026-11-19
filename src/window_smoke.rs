//! Opt-in development-only native smoke test. Requires an isolated APPDATA
//! fixture and --demo; never reads or drives another application's window.
use egui::{Event, Key, Modifiers, PointerButton, ViewportCommand as Cmd, pos2};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::*,
    UI::WindowsAndMessaging::*,
};

struct SmokeApp {
    app: crate::app::App,
    input: Arc<Mutex<Vec<Event>>>,
    root: PathBuf,
}

pub fn wrap(cc: &eframe::CreationContext<'_>, app: crate::app::App) -> Box<dyn eframe::App> {
    let Some(root) = std::env::var_os("MESSAGESOLUTIONS_WINDOW_SMOKE") else {
        return Box::new(app);
    };
    let root = PathBuf::from(root);
    assert!(
        app.demo && std::env::var_os("APPDATA").map(PathBuf::from) == Some(root.join("data")),
        "Smoke tests require --demo and isolated APPDATA=<evidence>/data"
    );
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(root.join("data/MessageSolutions")).unwrap();
    let input = Arc::new(Mutex::new(Vec::new()));
    let queued = input.clone();
    let ctx = cc.egui_ctx.clone();
    let window = cc.winit_window().unwrap().clone();
    let RawWindowHandle::Win32(handle) = cc.window_handle().unwrap().as_raw() else {
        panic!("Expected Windows")
    };
    let handle = handle.hwnd.get();
    let output = root.clone();
    std::thread::spawn(move || {
        let pause = || std::thread::sleep(Duration::from_millis(700));
        let inject = |events: Vec<Event>| {
            queued.lock().unwrap().extend(events);
            ctx.request_repaint();
            pause();
        };
        let click = |x, y| {
            let pos = pos2(x, y);
            inject(vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ]);
            inject(vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }]);
        };
        let screenshot = |name: &str| {
            let file = output.join(format!("{name}.png"));
            let (tx, rx) = std::sync::mpsc::channel();
            ctx.request_screenshot(move |image| {
                let bytes: Vec<u8> = image
                    .pixels
                    .iter()
                    .flat_map(|pixel| pixel.to_array())
                    .collect();
                let result = image::save_buffer(
                    file,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                );
                let _ = tx.send(result.is_ok());
            });
            rx.recv_timeout(Duration::from_secs(10)).unwrap_or(false)
        };
        pause();
        let mut checks = Vec::new();
        checks.push(json!({"check":"undecorated resizable native window", "passed":!window.is_decorated() && window.is_resizable(), "scale":window.scale_factor()}));
        checks.push(json!({"check":"dark rendered frame", "passed":screenshot("dark")}));
        let width = ctx.content_rect().width();
        click(width - 63.0, 24.0);
        checks.push(json!({"check":"toolbar maximize via injected egui pointer", "passed":window.is_maximized()}));
        unsafe {
            let hwnd = handle as HWND;
            let mut rect: RECT = std::mem::zeroed();
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            let ok = GetWindowRect(hwnd, &mut rect) != 0
                && GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut info)
                    != 0;
            let inner = window.inner_position().unwrap();
            let size = window.inner_size();
            checks.push(json!({"check":"maximized client fits monitor work area", "passed":ok && inner.x>=info.rcWork.left && inner.y>=info.rcWork.top && inner.x+size.width as i32<=info.rcWork.right && inner.y+size.height as i32<=info.rcWork.bottom}));
        }
        checks.push(json!({"check":"maximized rendered frame", "passed":screenshot("maximized")}));
        click(ctx.content_rect().width() - 63.0, 24.0);
        checks.push(json!({"check":"toolbar restore via injected egui pointer", "passed":!window.is_maximized()}));
        // Test the actual installed Win32 subclass, including signed coordinates.
        unsafe {
            let hwnd = handle as HWND;
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect);
            let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
            let mut hits = Vec::new();
            for (x, y, expected) in [
                (1, 1, 13),
                (w - 2, 1, 14),
                (1, h - 2, 16),
                (w - 2, h - 2, 17),
                (1, h / 2, 10),
                (w - 2, h / 2, 11),
                (w / 2, 1, 12),
                (w / 2, h - 2, 15),
            ] {
                let lp = (((rect.top + y) as u16 as u32) << 16) | ((rect.left + x) as u16 as u32);
                hits.push(SendMessageW(hwnd, WM_NCHITTEST, 0, lp as isize) == expected);
            }
            checks.push(json!({"check":"eight native edge/corner hit tests", "passed":hits.iter().all(|hit|*hit),"hits":hits}));
            checks.push(json!({"check":"native system menu available", "passed":!GetSystemMenu(hwnd,0).is_null()}));
        }
        click(ctx.content_rect().width() - 105.0, 24.0);
        checks.push(json!({"check":"toolbar minimize via injected egui pointer", "passed":window.is_minimized()==Some(true)}));
        ctx.send_viewport_cmd(Cmd::Minimized(false));
        pause();
        ctx.send_viewport_cmd(Cmd::InnerSize(egui::vec2(900.0, 560.0)));
        pause();
        checks.push(json!({"check":"native resize to minimum", "passed":window.inner_size().width >= (900.0*window.scale_factor()) as u32}));
        ctx.set_theme(egui::ThemePreference::Light);
        pause();
        checks.push(
            json!({"check":"light minimum rendered frame", "passed":screenshot("light-minimum")}),
        );
        // Ctrl+F must still focus the existing search, with no window movement.
        inject(vec![Event::Key {
            key: Key::F,
            physical_key: Some(Key::F),
            pressed: true,
            repeat: false,
            modifiers: Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            },
        }]);
        checks.push(json!({"check":"Ctrl+F preserves search focus", "passed":ctx.memory(|m|m.focused())==Some(egui::Id::unique("search_box"))}));
        let key = |key| {
            inject(vec![
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                },
            ])
        };
        let maximize_id = egui::Id::unique(("window_caption", 1usize));
        for _ in 0..16 {
            if ctx.memory(|m| m.focused()) == Some(maximize_id) {
                break;
            }
            key(Key::Tab);
        }
        checks.push(json!({"check":"Tab reaches maximize button", "passed":ctx.memory(|m|m.focused())==Some(maximize_id)}));
        key(Key::Enter);
        checks.push(
            json!({"check":"Enter maximizes focused button", "passed":window.is_maximized()}),
        );
        key(Key::Space);
        checks.push(
            json!({"check":"Space restores focused button", "passed":!window.is_maximized()}),
        );
        // Double-click the first blank region using the real egui input path.
        let pos = pos2(180.0, 24.0);
        let mut double_click = vec![Event::PointerMoved(pos)];
        for pressed in [true, false, true, false] {
            double_click.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        inject(double_click);
        checks.push(json!({"check":"toolbar double-click via injected egui pointer", "passed":window.is_maximized()}));
        ctx.send_viewport_cmd(Cmd::Maximized(false));
        pause();
        ctx.set_zoom_factor(2.0);
        pause();
        checks.push(
            json!({"check":"200 percent zoom rendered frame", "passed":screenshot("zoom-200")}),
        );
        ctx.set_zoom_factor(1.0);
        pause();
        let passed = checks.iter().all(|check| check["passed"] == true);
        std::fs::write(output.join("native-results.json"), serde_json::to_vec_pretty(&json!({"passed":passed,"checks":checks,"unverified":["physical mouse dragging, edge resizing and double-click","interactive system menu and taskbar restore","screen reader announcement","physical mixed-DPI and multi-monitor transitions"]})).unwrap()).unwrap();
        click(ctx.content_rect().width() - 21.0, 24.0);
    });
    Box::new(SmokeApp { app, input, root })
}

impl eframe::App for SmokeApp {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        input.events.extend(self.input.lock().unwrap().drain(..));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
    fn on_exit(&mut self) {
        // Only this isolated test fixture enables saving the synthetic demo
        // records at shutdown. No bridge was started and no real data loaded.
        self.app.demo = false;
        self.app.on_exit();
        let dir = self.root.join("data/MessageSolutions");
        let settings: crate::model::Settings =
            serde_json::from_slice(&std::fs::read(dir.join("ui.json")).unwrap()).unwrap();
        let cache: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("cache.json")).unwrap()).unwrap();
        let passed = settings.zoom == self.app.settings.zoom
            && settings.appearance == self.app.settings.appearance
            && cache["conversations"]
                .as_array()
                .is_some_and(|items| items.len() == self.app.convs.len());
        std::fs::write(self.root.join("close-observed.json"), serde_json::to_vec_pretty(&json!({"passed":passed,"check":"Toolbar close reached App::on_exit and round-tripped settings and synthetic conversation cache in isolated fixture"})).unwrap()).unwrap();
    }
}
