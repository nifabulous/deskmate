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
use std::path::Path;
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

/// Bring the Claude desktop app to the session a message came from.
///
/// `claude://resume?session=<id>` is the only externally reachable way in, but
/// it resolves `local_<id>` and *imports* whatever it finds. Hand it the id a
/// hook reports and it matches no live session, so it copies the transcript
/// into a second, untitled session — the "General coding session" you land in —
/// and strips thinking blocks from the original JSONL on the way through.
///
/// Handing it the desktop session's own id instead takes the app's
/// "already imported" path, which focuses the live session and touches nothing.
/// So resolve first, and fall back to the reported id only when there is no
/// desktop session to focus.
///
/// This is a private, undocumented interface found by reading the app bundle,
/// so treat failure as normal: any Claude update may change it.
///
/// The id arrives over HTTP from whatever posted the event, so both it and
/// anything resolved from it are checked against the UUID shape before going
/// anywhere near a URL.
#[tauri::command]
fn focus_session(session: String) -> Result<(), String> {
    if !is_session_id(&session) {
        return Err("not a session id".into());
    }
    let target = desktop_session_for(&session)
        .filter(|id| is_session_id(id))
        .unwrap_or(session);
    let url = format!("claude://resume?session={target}");

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        Err("only wired up for macOS so far".into())
    }
}

/// The id a hook reports is not the id the desktop app files the session under.
/// A stored record pairs them, and the two uuids differ:
///
/// ```json
/// {"sessionId": "local_bfb84bd4-…", "cliSessionId": "c247fe2e-…", "title": "…"}
/// ```
///
/// Given the `cliSessionId`, return the `sessionId` without its `local_` prefix,
/// which is the form the deep link wants. `None` for a session with no desktop
/// record — one run purely in a terminal — where importing is the only way in.
fn desktop_session_for(cli_session: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let claude = Path::new(&home).join("Library/Application Support/Claude");
    // Claude Code and Cowork keep separate stores of the same record shape.
    ["claude-code-sessions", "local-agent-mode-sessions"]
        .iter()
        .find_map(|store| find_session_record(&claude.join(store), cli_session, 2))
}

/// Records sit at `<store>/<account>/<workspace>/local_<uuid>.json`, so two
/// levels of directory is enough to reach them.
fn find_session_record(dir: &Path, cli_session: &str, depth: u8) -> Option<String> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                if let Some(found) = find_session_record(&path, cli_session, depth - 1) {
                    return Some(found);
                }
            }
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("local_") || !name.ends_with(".json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if record.get("cliSessionId").and_then(|v| v.as_str()) != Some(cli_session) {
            continue;
        }
        if let Some(id) = record.get("sessionId").and_then(|v| v.as_str()) {
            return Some(id.strip_prefix("local_").unwrap_or(id).to_string());
        }
    }
    None
}

/// 8-4-4-4-12 hex, the shape Claude Code session ids come in.
fn is_session_id(s: &str) -> bool {
    let groups = [8, 4, 4, 4, 12];
    let mut parts = s.split('-');
    for len in groups {
        match parts.next() {
            Some(p) if p.len() == len && p.bytes().all(|b| b.is_ascii_hexdigit()) => {}
            _ => return false,
        }
    }
    parts.next().is_none()
}

/// Remember where the pet was left. A desk pet gets put somewhere deliberately,
/// and dropping it back in the corner on every launch throws that away.
///
/// Written when a drag ends rather than on every move event: a drag emits
/// position updates continuously, and none of them are worth a file write.
#[tauri::command]
fn save_window_position(window: tauri::WebviewWindow) -> Result<(), String> {
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let path = position_file(window.app_handle()).ok_or("no config dir")?;
    std::fs::write(path, format!(r#"{{"x":{},"y":{}}}"#, pos.x, pos.y)).map_err(|e| e.to_string())
}

fn position_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("position.json"))
}

/// Parse a saved position. Separate from the file read so the shape can be
/// tested without a filesystem.
fn parse_position(text: &str) -> Option<(i32, i32)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let x = v.get("x")?.as_i64()?;
    let y = v.get("y")?.as_i64()?;
    // Anything outside i32 is not a screen coordinate, it is a corrupt file.
    Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

/// Would a window at this spot still be visible on one of the attached screens?
///
/// This matters more than it looks: the window has no taskbar entry and no
/// decorations, so a pet restored onto a monitor that has since been unplugged
/// is gone with no way to grab it back. Require a decent chunk of it to land on
/// some monitor, or fall back to the default corner.
fn is_on_screen(monitors: &[(i32, i32, u32, u32)], x: i32, y: i32, w: u32, h: u32) -> bool {
    let need_w = (w as i32 / 2).max(1);
    let need_h = (h as i32 / 2).max(1);
    monitors.iter().any(|&(mx, my, mw, mh)| {
        let overlap_w = (x + w as i32).min(mx + mw as i32) - x.max(mx);
        let overlap_h = (y + h as i32).min(my + mh as i32) - y.max(my);
        overlap_w >= need_w && overlap_h >= need_h
    })
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
/// one is 220x270 of almost entirely transparent space. Follow the cursor and
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

/// Fields the webview reads as strings. `session` is the sharp one: the UI
/// calls `.slice()` on it, so a number threw inside the event listener and the
/// message vanished — after this server had already answered 200. Anything the
/// UI would choke on has to be a 400 here, or the sender is told it worked.
const STRING_FIELDS: [&str; 4] = ["source", "session", "title", "detail"];

/// Minimal validation: a JSON object with a string `kind`, plus string types on
/// the optional fields above. Unknown keys still pass through untouched so
/// adapters can evolve freely.
fn validate(body: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let obj = value.as_object()?;
    obj.get("kind")?.as_str()?;
    for field in STRING_FIELDS {
        match obj.get(field) {
            // Absent or explicitly null is fine; the UI guards for both.
            None | Some(serde_json::Value::Null) => {}
            Some(v) if v.is_string() => {}
            Some(_) => return None,
        }
    }
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
                    // Forward to every window (there is only one). Answering 200
                    // after a failed emit tells the sender the pet heard it when
                    // nothing was delivered.
                    (true, Some(event)) => match app.emit("deskmate:event", &event) {
                        Ok(()) => (200, r#"{"ok":true}"#.to_string()),
                        Err(e) => {
                            eprintln!("deskmate: emit failed: {e}");
                            (
                                500,
                                r#"{"ok":false,"error":"deskmate could not deliver the event"}"#
                                    .to_string(),
                            )
                        }
                    },
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
        .invoke_handler(tauri::generate_handler![
            startup_error,
            set_hit_region,
            focus_session,
            save_window_position
        ])
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

            // Put the pet back where it was left, or nudge it toward the
            // bottom-right corner on a first launch.
            if let Some(window) = app.get_webview_window("main") {
                let size = window.outer_size().unwrap_or(tauri::PhysicalSize {
                    width: 220,
                    height: 270,
                });
                let saved = position_file(app.handle())
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|t| parse_position(&t));
                let monitors: Vec<(i32, i32, u32, u32)> = window
                    .available_monitors()
                    .unwrap_or_default()
                    .iter()
                    .map(|m| {
                        (
                            m.position().x,
                            m.position().y,
                            m.size().width,
                            m.size().height,
                        )
                    })
                    .collect();

                let restored = match saved {
                    Some((x, y)) if is_on_screen(&monitors, x, y, size.width, size.height) => {
                        window
                            .set_position(tauri::PhysicalPosition { x, y })
                            .is_ok()
                    }
                    _ => false,
                };

                if !restored {
                    if let Ok(Some(monitor)) = window.current_monitor() {
                        let screen = monitor.size();
                        // Window positions are global desktop coordinates, so
                        // offset by the monitor's own origin — that is only
                        // (0, 0) on the primary screen. Without it the pet lands
                        // on the wrong monitor, or off-screen entirely, in a
                        // multi-monitor setup.
                        let origin = monitor.position();
                        let x = origin.x + screen.width.saturating_sub(size.width + 40) as i32;
                        let y = origin.y + screen.height.saturating_sub(size.height + 80) as i32;
                        let _ = window.set_position(tauri::PhysicalPosition { x, y });
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running deskmate");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_real_session_id() {
        assert!(is_session_id("c247fe2e-aaa2-4084-98b1-ddc4acc461e0"));
        assert!(is_session_id("00000000-0000-4000-8000-000000000000"));
    }

    #[test]
    fn rejects_anything_that_is_not_one() {
        // Events arrive from any local process, so this is the gate that keeps a
        // hostile `session` from steering the URL somewhere else.
        for bad in [
            "",
            "not-a-session",
            "c247fe2e-aaa2-4084-98b1-ddc4acc461e0-extra",
            "c247fe2e-aaa2-4084-98b1-ddc4acc461e", // group too short
            "c247fe2eXaaa2X4084X98b1Xddc4acc461e0", // wrong separators
            "g247fe2e-aaa2-4084-98b1-ddc4acc461e0", // 'g' is not hex
            "../../etc/passwd",
            "x&open -a Calculator",
        ] {
            assert!(!is_session_id(bad), "should reject {bad:?}");
        }
    }

    /// Build a throwaway store laid out like the app's:
    /// `<store>/<account>/<workspace>/local_<uuid>.json`.
    fn fixture(name: &str, records: &[(&str, &str)]) -> std::path::PathBuf {
        let store = std::env::temp_dir().join(format!("deskmate-test-{name}"));
        let leaf = store.join("account-1").join("workspace-1");
        let _ = std::fs::remove_dir_all(&store);
        std::fs::create_dir_all(&leaf).unwrap();
        for (session_id, cli_session_id) in records {
            std::fs::write(
                leaf.join(format!("{session_id}.json")),
                format!(r#"{{"sessionId":"{session_id}","cliSessionId":"{cli_session_id}"}}"#),
            )
            .unwrap();
        }
        store
    }

    #[test]
    fn resolves_a_cli_id_to_the_desktop_session_that_owns_it() {
        // The pairing this whole thing turns on: the two ids are different
        // uuids, and only the desktop one focuses instead of importing a copy.
        let store = fixture(
            "resolve",
            &[
                (
                    "local_9b7595e0-f942-4438-9028-262e4dfb2c2e",
                    "a640dcf5-b8f7-4b79-bfeb-08334f9ad7e9",
                ),
                (
                    "local_bfb84bd4-b900-41b7-8c45-efdcfa7d653b",
                    "c247fe2e-aaa2-4084-98b1-ddc4acc461e0",
                ),
            ],
        );

        assert_eq!(
            find_session_record(&store, "c247fe2e-aaa2-4084-98b1-ddc4acc461e0", 2),
            Some("bfb84bd4-b900-41b7-8c45-efdcfa7d653b".to_string()),
            "should return the desktop id, without its local_ prefix"
        );
        // A terminal-only session has no record; the caller falls back to
        // importing, which is the only way into one of those.
        assert_eq!(
            find_session_record(&store, "00000000-0000-4000-8000-000000000000", 2),
            None
        );
        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn survives_junk_in_the_store() {
        // Real stores hold caches and settings next to session records.
        let store = fixture(
            "junk",
            &[(
                "local_bfb84bd4-b900-41b7-8c45-efdcfa7d653b",
                "c247fe2e-aaa2-4084-98b1-ddc4acc461e0",
            )],
        );
        let leaf = store.join("account-1").join("workspace-1");
        std::fs::write(leaf.join("local_broken.json"), "{not json").unwrap();
        std::fs::write(leaf.join("scheduled-tasks.json"), r#"{"a":1}"#).unwrap();
        std::fs::write(leaf.join("local_no-fields.json"), "{}").unwrap();

        assert_eq!(
            find_session_record(&store, "c247fe2e-aaa2-4084-98b1-ddc4acc461e0", 2),
            Some("bfb84bd4-b900-41b7-8c45-efdcfa7d653b".to_string())
        );
        let _ = std::fs::remove_dir_all(&store);
    }

    #[test]
    fn accepts_the_events_adapters_actually_send() {
        assert!(validate(r#"{"kind":"task_start"}"#).is_some());
        assert!(validate(
            r#"{"kind":"tool_use","source":"claude-code","session":"c247fe2e-aaa2-4084-98b1-ddc4acc461e0","title":"Editing","detail":"src/main.rs"}"#
        )
        .is_some());
        // Null and unknown keys are fine — the UI guards for absent fields, and
        // passing extra keys through is what lets adapters evolve.
        assert!(validate(r#"{"kind":"notify","session":null,"future_field":42}"#).is_some());
    }

    #[test]
    fn rejects_events_the_ui_would_choke_on() {
        // The bug this exists for: a numeric session threw `session.slice is not
        // a function` inside the UI listener, so the message never appeared —
        // and the sender had already been told 200.
        assert!(validate(r#"{"kind":"tool_use","session":12345}"#).is_none());
        assert!(validate(r#"{"kind":"tool_use","title":{"nested":"object"}}"#).is_none());
        assert!(validate(r#"{"kind":"tool_use","detail":["an","array"]}"#).is_none());
        assert!(validate(r#"{"kind":"tool_use","source":false}"#).is_none());
    }

    #[test]
    fn rejects_bodies_that_are_not_events() {
        assert!(validate("").is_none());
        assert!(validate("not json at all").is_none());
        assert!(validate("[1,2,3]").is_none());
        assert!(validate(r#""a bare string""#).is_none());
        assert!(validate(r#"{"no_kind":true}"#).is_none());
        assert!(validate(r#"{"kind":7}"#).is_none());
    }

    #[test]
    fn reads_back_a_saved_position() {
        assert_eq!(parse_position(r#"{"x":1720,"y":900}"#), Some((1720, 900)));
        // Negative coordinates are normal: a second monitor left of the primary.
        assert_eq!(
            parse_position(r#"{"x":-1280,"y":-200}"#),
            Some((-1280, -200))
        );
    }

    #[test]
    fn a_corrupt_position_file_falls_back_rather_than_panicking() {
        for bad in [
            "",
            "not json",
            "{}",
            r#"{"x":10}"#,
            r#"{"x":"left","y":"top"}"#,
            r#"{"x":99999999999999,"y":0}"#, // beyond i32: not a screen coordinate
        ] {
            assert_eq!(parse_position(bad), None, "should reject {bad:?}");
        }
    }

    #[test]
    fn refuses_to_restore_onto_a_monitor_that_is_gone() {
        // One 1920x1080 screen at the origin.
        let monitors = [(0, 0, 1920u32, 1080u32)];
        let (w, h) = (220u32, 270u32);

        assert!(
            is_on_screen(&monitors, 1660, 730, w, h),
            "bottom-right corner"
        );
        assert!(is_on_screen(&monitors, 0, 0, w, h), "top-left corner");
        // Half on, half off is still grabbable.
        assert!(
            is_on_screen(&monitors, 1810, 500, w, h),
            "hanging off the right"
        );

        // The case that matters: saved on a second monitor, now unplugged. The
        // window has no taskbar entry, so restoring here loses the pet for good.
        assert!(
            !is_on_screen(&monitors, 2400, 300, w, h),
            "off to the right"
        );
        assert!(
            !is_on_screen(&monitors, -1200, 100, w, h),
            "off to the left"
        );
        assert!(!is_on_screen(&monitors, 300, 1050, w, h), "mostly below");
        assert!(!is_on_screen(&[], 100, 100, w, h), "no monitors at all");
    }

    #[test]
    fn missing_store_is_not_an_error() {
        assert_eq!(
            find_session_record(Path::new("/nonexistent/deskmate"), "whatever", 2),
            None
        );
    }
}
