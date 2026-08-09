// deskmate — a pixel desk pet for coding agents.
//
// The app does two things:
//   1. Shows a small transparent always-on-top window with the pet (see /ui).
//   2. Runs a tiny local HTTP server that any tool can POST events to.
//      Events are forwarded to the webview, which drives the animations.
//
// Protocol: docs/PROTOCOL.md. Default endpoint: http://127.0.0.1:8990/event

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::Read;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_PORT: u16 = 8990;
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Why the event server never came up, if it didn't. The webview asks for this
/// on load and bubbles it: a frameless window launched from a dock icon has
/// nowhere to show a message printed on stderr, so without this the app looks
/// alive while silently ignoring every event.
#[derive(Default)]
struct StartupError(Mutex<Option<String>>);

#[tauri::command]
fn startup_error(state: tauri::State<'_, StartupError>) -> Option<String> {
    state.0.lock().ok()?.clone()
}

/// Where the pet sits inside the window, in logical pixels, as measured and
/// reported by the webview. Everything outside it is made click-through.
#[derive(Default)]
struct HitRegion(Mutex<Option<(f64, f64, f64, f64)>>);

#[tauri::command]
fn set_hit_region(state: tauri::State<'_, HitRegion>, x: f64, y: f64, w: f64, h: f64) {
    if let Ok(mut slot) = state.0.lock() {
        *slot = Some((x, y, w, h));
    }
}

/// An always-on-top window swallows clicks across its whole rectangle, and this
/// one is 180x255 of almost entirely transparent space. Follow the cursor and
/// let clicks through everywhere except over the pet and its message panel.
fn run_cursor_watcher(app: AppHandle) {
    let mut ignoring: Option<bool> = None;

    loop {
        thread::sleep(Duration::from_millis(60));

        let Some(window) = app.get_webview_window("main") else {
            continue;
        };
        let (Ok(cursor), Ok(origin), Ok(scale)) = (
            app.cursor_position(),
            window.outer_position(),
            window.scale_factor(),
        ) else {
            continue;
        };

        let region = app
            .state::<HitRegion>()
            .0
            .lock()
            .ok()
            .and_then(|slot| *slot);
        let over_pet = match region {
            Some((x, y, w, h)) => {
                let local_x = (cursor.x - origin.x as f64) / scale;
                let local_y = (cursor.y - origin.y as f64) / scale;
                local_x >= x && local_x < x + w && local_y >= y && local_y < y + h
            }
            // Until the webview reports in, keep the window solid rather than
            // risk a pet that cannot be clicked or dragged at all.
            None => true,
        };

        if ignoring != Some(!over_pet) {
            let _ = window.set_ignore_cursor_events(!over_pet);
            ignoring = Some(!over_pet);
        }
    }
}

fn event_port() -> u16 {
    std::env::var("DESKMATE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT)
}

/// Minimal validation: must be a JSON object with a string `kind`.
/// Everything else is passed through so adapters can evolve freely.
fn validate(body: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let obj = value.as_object()?;
    obj.get("kind")?.as_str()?;
    Some(value)
}

fn run_event_server(app: AppHandle) {
    let port = event_port();
    let addr = format!("127.0.0.1:{port}");
    let server = match tiny_http::Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("deskmate: could not bind {addr}: {e}");
            eprintln!("deskmate: is another deskmate running? Set DESKMATE_PORT to change it.");
            if let Some(state) = app.try_state::<StartupError>() {
                if let Ok(mut slot) = state.0.lock() {
                    *slot = Some(format!(
                        "Nothing can reach the pet: port {port} is already taken. \
                         Another deskmate may be running — set DESKMATE_PORT to move it."
                    ));
                }
            }
            return;
        }
    };
    println!("deskmate: listening on http://{addr}");

    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
        let url = request.url().to_string();

        let (status, reply) = match (method.as_str(), url.as_str()) {
            ("GET", "/health") => (200, r#"{"ok":true,"app":"deskmate"}"#.to_string()),
            // Adapters are CLI tools and never send an Origin header. Browsers
            // always send one on a cross-origin POST, and a text/plain form post
            // is a "simple request" that skips preflight entirely — so without
            // this check any page you happen to visit could write on the pet.
            ("POST", "/event") if request.headers().iter().any(|h| h.field.equiv("Origin")) => (
                403,
                r#"{"ok":false,"error":"requests from a browser origin are not accepted"}"#
                    .to_string(),
            ),
            ("POST", "/event") => {
                let mut body = String::new();
                let ok = request
                    .as_reader()
                    .take(MAX_BODY_BYTES as u64)
                    .read_to_string(&mut body)
                    .is_ok();
                match (ok, validate(&body)) {
                    (true, Some(event)) => {
                        // Forward to every window (there is only one).
                        if let Err(e) = app.emit("deskmate:event", &event) {
                            eprintln!("deskmate: emit failed: {e}");
                        }
                        (200, r#"{"ok":true}"#.to_string())
                    }
                    _ => (
                        400,
                        r#"{"ok":false,"error":"body must be a JSON object with a string `kind`"}"#
                            .to_string(),
                    ),
                }
            }
            _ => (404, r#"{"ok":false,"error":"not found"}"#.to_string()),
        };

        let header =
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
        let response = tiny_http::Response::from_string(reply)
            .with_status_code(status)
            .with_header(header);
        let _ = request.respond(response);
    }
}

fn main() {
    tauri::Builder::default()
        .manage(StartupError::default())
        .manage(HitRegion::default())
        .invoke_handler(tauri::generate_handler![startup_error, set_hit_region])
        .setup(|app| {
            let handle = app.handle().clone();
            thread::spawn(move || run_event_server(handle));

            let handle = app.handle().clone();
            thread::spawn(move || run_cursor_watcher(handle));

            // A frameless, undecorated window with no taskbar entry has no
            // close button and no menu of its own, so the tray is the only way
            // out on Windows and Linux.
            let quit = MenuItem::with_id(app, "quit", "Quit deskmate", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("deskmate")
                .on_menu_event(|app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // Nudge the pet toward the bottom-right corner on first launch.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let screen = monitor.size();
                    // Window positions are global desktop coordinates, so offset
                    // by the monitor's own origin — that is only (0, 0) on the
                    // primary screen. Without it the pet lands on the wrong
                    // monitor, or off-screen entirely, in a multi-monitor setup.
                    let origin = monitor.position();
                    // Fallback matches the window size in tauri.conf.json.
                    let size = window.outer_size().unwrap_or(tauri::PhysicalSize {
                        width: 180,
                        height: 255,
                    });
                    let x = origin.x + screen.width.saturating_sub(size.width + 40) as i32;
                    let y = origin.y + screen.height.saturating_sub(size.height + 80) as i32;
                    let _ = window.set_position(tauri::PhysicalPosition { x, y });
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running deskmate");
}
