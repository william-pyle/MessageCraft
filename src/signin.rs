//! "Sign in with Google": `MessageCraft.exe --google-signin` opens a small browser window (Edge
//! WebView2) at Google's sign-in page. Once the person is signed in and the page reaches Google
//! Messages, it reads the Google session cookies that Google Messages for web uses, prints them as
//! one line of JSON for the main window, and closes. The main window passes them to the
//! connection helper, which asks the phone to confirm with an emoji.
//!
//! It runs as its own process because a window can only have one event loop, and the main window's
//! belongs to egui.

use std::collections::HashMap;
use std::io::Write;
use std::time::{Duration, Instant};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, WindowBuilder};
use wry::{PageLoadEvent, WebContext, WebViewBuilder};

const START: &str = "https://accounts.google.com/ServiceLogin?continue=https%3A%2F%2Fmessages.google.com%2Fweb%2Fconfig";
const CONFIG: &str = "https://messages.google.com/web/config";
/// The cookies Google Messages for web signs its requests with (as libgm expects them).
const REQUIRED: [&str; 6] = ["SID", "HSID", "SSID", "APISID", "SAPISID", "OSID"];
const OPTIONAL: [&str; 3] = ["__Secure-1PSIDTS", "__Secure-1PSID", "__Secure-3PSID"];

enum Ev {
    Loaded(String),
}

fn say(v: serde_json::Value) {
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

pub fn run(icon: egui::IconData) -> ! {
    let event_loop = EventLoopBuilder::<Ev>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Sign in to Google - MessageCraft")
        .with_inner_size(tao::dpi::LogicalSize::new(520.0, 760.0))
        .with_window_icon(Icon::from_rgba(icon.rgba, icon.width, icon.height).ok())
        .build(&event_loop)
        .unwrap_or_else(|e| {
            say(serde_json::json!({"error": format!("couldn't open the sign-in window: {e}")}));
            std::process::exit(1)
        });

    // A profile of its own, emptied when the sign-in is done: the session it needs is handed to
    // the connection helper, which keeps it encrypted.
    let profile = crate::bridge::data_dir().join("signin-profile");
    let mut context = WebContext::new(Some(profile.clone()));
    let webview = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(START)
        .with_on_page_load_handler(move |ev, url| {
            if matches!(ev, PageLoadEvent::Finished) {
                let _ = proxy.send_event(Ev::Loaded(url));
            }
        })
        .build(&window);
    let webview = match webview {
        Ok(w) => w,
        Err(e) => {
            say(serde_json::json!({"error": format!("Microsoft Edge WebView2 isn't available: {e}")}));
            std::process::exit(1)
        }
    };

    let mut on_messages_since: Option<Instant> = None;
    let mut opened_web = false;
    let mut done = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(700));
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                if !done {
                    say(serde_json::json!({"cancelled": true}));
                }
                let _ = std::fs::remove_dir_all(&profile);
                *control_flow = ControlFlow::Exit;
                return;
            }
            Event::UserEvent(Ev::Loaded(url)) => {
                if url.starts_with("https://messages.google.com") && on_messages_since.is_none() {
                    on_messages_since = Some(Instant::now());
                }
            }
            _ => {}
        }
        if done {
            return;
        }
        let Some(since) = on_messages_since else { return };
        // Signed in and on Google Messages: read the session.
        let Ok(cookies) = webview.cookies_for_url(CONFIG) else { return };
        let mut found: HashMap<String, String> = HashMap::new();
        for c in cookies {
            let name = c.name().to_string();
            if REQUIRED.contains(&name.as_str()) || OPTIONAL.contains(&name.as_str()) {
                found.entry(name).or_insert_with(|| c.value().to_string());
            }
        }
        let missing: Vec<&str> = REQUIRED.iter().copied().filter(|n| !found.contains_key(*n)).collect();
        if missing.is_empty() || since.elapsed() > Duration::from_secs(40) {
            done = true;
            say(serde_json::json!({"cookies": found, "missing": missing}));
            let _ = webview.clear_all_browsing_data();
            *control_flow = ControlFlow::Exit;
        } else if missing == ["OSID"] && !opened_web && since.elapsed() > Duration::from_secs(5) {
            // Google sets OSID for Messages on its web page; visit it once.
            opened_web = true;
            let _ = webview.load_url("https://messages.google.com/web/");
        }
    })
}
