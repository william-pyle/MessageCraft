//! MessageCraft: your phone's text messages on Windows, in the style of MailCraft.

#![windows_subsystem = "windows"]

mod app;
mod backstage;
mod bridge;
mod chrome;
mod demo;
mod icons;
mod lists;
mod contactcraft;
mod people;
mod model;
mod reading;
mod signin;
mod theme;
mod timefmt;
mod update;
mod widgets;

/// The window icon: MailCraft's blue tile with a white speech bubble, drawn here.
fn app_icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    let blue = [0x0F, 0x68, 0xF0];
    let inside_round = |x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32| -> bool {
        let cx = x.clamp(x0 + r, x1 - r);
        let cy = y.clamp(y0 + r, y1 - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    for py in 0..N {
        for px in 0..N {
            let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
            let i = (py * N + px) * 4;
            if !inside_round(x, y, 2.0, 2.0, 62.0, 62.0, 14.0) {
                continue;
            }
            let mut c = [blue[0], blue[1], blue[2]];
            let in_bubble = inside_round(x, y, 13.0, 15.0, 51.0, 43.0, 9.0);
            // the tail
            let in_tail = y >= 41.0 && y <= 52.0 && x >= 18.0 && x <= 30.0 && (x - 18.0) >= (y - 41.0) * 0.2 && (30.0 - x) >= (y - 41.0) * 1.05;
            if in_bubble || in_tail {
                c = [255, 255, 255];
                for dx in [23.0f32, 32.0, 41.0] {
                    if (x - dx).powi(2) + (y - 29.0).powi(2) <= 12.0 {
                        c = blue;
                    }
                }
            }
            rgba[i] = c[0];
            rgba[i + 1] = c[1];
            rgba[i + 2] = c[2];
            rgba[i + 3] = 255;
        }
    }
    egui::IconData { rgba, width: N as u32, height: N as u32 }
}

fn main() -> eframe::Result {
    if std::env::args().any(|a| a == "--google-signin") {
        signin::run(app_icon());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MessageCraft")
            .with_app_id("MessageCraft")
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native("MessageCraft", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
