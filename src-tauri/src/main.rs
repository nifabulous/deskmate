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
use std::thread;

use tauri::{AppHandle, Emitter, Manager};

const DEFAULT_PORT: u16 = 8990;
const MAX_BODY_BYTES: usize = 64 * 1024;

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
            return;
        }
    };
    println!("deskmate: listening on http://{addr}");

    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
        let url = request.url().to_string();

        let (status, reply) = match (method.as_str(), url.as_str()) {
            ("GET", "/health") => (200, r#"{"ok":true,"app":"deskmate"}"#.to_string()),
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
        .setup(|app| {
            let handle = app.handle().clone();
            thread::spawn(move || run_event_server(handle));

            // Nudge the pet toward the bottom-right corner on first launch.
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let screen = monitor.size();
                    let size = window.outer_size().unwrap_or(tauri::PhysicalSize {
                        width: 260,
                        height: 320,
                    });
                    let x = screen.width.saturating_sub(size.width + 40) as i32;
                    let y = screen.height.saturating_sub(size.height + 80) as i32;
                    let _ = window.set_position(tauri::PhysicalPosition { x, y });
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running deskmate");
}
