use eframe::egui;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

use crate::commands;
use crate::settings::{HotkeyConfig, Settings, SettingsWindow};

// Tray menu action codes
const TRAY_NONE: u8 = 0;
const TRAY_TOGGLE: u8 = 1;
const TRAY_SETTINGS: u8 = 2;
const TRAY_QUIT: u8 = 3;

pub struct LauncherApp {
    hotkey_manager: GlobalHotKeyManager,
    toggle_hotkey_id: u32,
    command_input: String,
    is_visible: Arc<AtomicBool>,
    text_edit_rect: Option<egui::Rect>,
    settings: Settings,
    settings_window: SettingsWindow,
    was_focused: bool,
    _tray: tray_icon::TrayIcon,
    toggle_signal: Arc<AtomicBool>,
    tray_action: Arc<AtomicU8>,
    x11_window_id: Arc<AtomicU32>,
    x11_search_attempts: u32,
    dragging: bool,
    pending_w_magic_word: Option<commands::MagicWord>,
    w_dialog_input: String,
    shared_pos_x: Arc<AtomicU32>,
    shared_pos_y: Arc<AtomicU32>,
    last_known_pos: Option<(f32, f32)>,
    last_pos_query: std::time::Instant,
    needs_initial_move: bool,
}

fn toggle_pipe_path() -> std::path::PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(runtime_dir).join("slickrun-toggle")
    } else {
        std::path::PathBuf::from("/tmp/slickrun-toggle")
    }
}

fn start_toggle_pipe_listener(
    ctx: egui::Context,
    signal: Arc<AtomicBool>,
) {
    let pipe_path = toggle_pipe_path();
    let _ = std::fs::remove_file(&pipe_path);

    unsafe {
        let c_path = std::ffi::CString::new(pipe_path.to_str().unwrap()).unwrap();
        let ret = libc::mkfifo(c_path.as_ptr(), 0o644);
        if ret != 0 {
            eprintln!("[SlickRun] Failed to create FIFO at {:?}: errno={}", pipe_path, *libc::__errno_location());
        } else {
            eprintln!("[SlickRun] FIFO pipe created at {:?}", pipe_path);
        }
    }

    std::thread::spawn(move || loop {
        match std::fs::read_to_string(&pipe_path) {
            Ok(data) => {
                eprintln!("[SlickRun] FIFO received: {:?}", data.trim());
                signal.store(true, Ordering::SeqCst);
                ctx.request_repaint();
            }
            Err(e) => {
                eprintln!("[SlickRun] FIFO read error: {e}");
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    });
}


/// Find our own X11 window by walking up from the input-focused window.
/// On the first frame our app has focus, so get_input_focus gives us a window
/// in our hierarchy. Walk up to find the client top-level.
fn find_own_x11_window() -> Option<u32> {
    match find_own_x11_window_inner() {
        Ok(Some(id)) => Some(id),
        Ok(None) => None,
        Err(e) => {
            eprintln!("[SlickRun] X11 window search error: {e}");
            None
        }
    }
}

fn find_own_x11_window_inner() -> Result<Option<u32>, Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // On first frame, our window has input focus. Walk up to find the client window.
    let focus_reply = conn.get_input_focus()?.reply()?;
    let focus = focus_reply.focus;
    eprintln!("[SlickRun] X11: get_input_focus=0x{:x} root=0x{:x}", focus, root);

    if focus <= 1 || focus == root {
        return Ok(None);
    }

    // Walk up from focused window to root.
    // In a reparenting WM: root -> WM_frame -> client_window -> sub_windows -> focused
    // We want the client_window (child of WM frame = grandchild of root).
    let mut win = focus;
    let mut client_window = focus;
    loop {
        let tree = conn.query_tree(win)?.reply()?;
        eprintln!("[SlickRun]   walk: 0x{:x} -> parent 0x{:x}", win, tree.parent);
        if tree.parent == root || tree.parent == 0 {
            break;
        }
        client_window = win;
        win = tree.parent;
    }

    if win == focus {
        // Focused window is a direct child of root (no reparenting)
        eprintln!("[SlickRun] Window 0x{:x} is direct child of root", win);
        return Ok(Some(win));
    }

    eprintln!("[SlickRun] Client window: 0x{:x}, WM frame: 0x{:x}", client_window, win);
    Ok(Some(client_window))
}

/// Activate an X11 window by its stored ID.
/// Maps the window (un-minimizes) and sends _NET_ACTIVE_WINDOW to the WM.
fn activate_x11_window_by_id(window_id: u32) {
    if let Err(e) = activate_x11_window_by_id_inner(window_id) {
        eprintln!("[SlickRun] X11 activation by ID failed: {e}");
    }
}

fn activate_x11_window_by_id_inner(window_id: u32) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Map the window — un-minimizes at X11 level. Harmless if already mapped.
    conn.map_window(window_id)?;

    // Raise the window
    let values = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
    conn.configure_window(window_id, &values)?;

    // Send _NET_ACTIVE_WINDOW to the window manager for proper focus/raise
    let net_active = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW")?.reply()?.atom;
    let data = ClientMessageData::from([
        1u32, // source indication: 1 = application
        0,    // timestamp
        0,    // requestor's currently active window
        0, 0,
    ]);
    let event = ClientMessageEvent {
        response_type: CLIENT_MESSAGE_EVENT,
        format: 32,
        sequence: 0,
        window: window_id,
        type_: net_active,
        data,
    };
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    conn.send_event(false, root, mask, event)?;

    // Also directly set input focus as a fallback
    conn.set_input_focus(InputFocus::PARENT, window_id, x11rb::CURRENT_TIME)?;

    conn.flush()?;
    eprintln!("[SlickRun] Activated window 0x{:x} (map+raise+activate+focus)", window_id);
    Ok(())
}

/// Query Mutter for the SlickRun window position via the GNOME Shell extension.
fn get_mutter_window_position() -> Option<(i32, i32)> {
    let output = std::process::Command::new("gdbus")
        .args([
            "call", "--session",
            "--dest", "org.gnome.Shell",
            "--object-path", "/com/slickrun/Toggle",
            "--method", "com.slickrun.Toggle.GetPosition",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // Output format: "(x, y)\n"
    let s = String::from_utf8_lossy(&output.stdout);
    let s = s.trim().trim_start_matches('(').trim_end_matches(')');
    let mut parts = s.split(',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    if x >= 0 && y >= 0 { Some((x, y)) } else { None }
}

/// Move the SlickRun window via the GNOME Shell extension (Mutter level).
/// This is the only way to position a window on GNOME Wayland.
fn move_window_via_mutter(x: i32, y: i32) {
    let _ = std::process::Command::new("gdbus")
        .args([
            "call", "--session",
            "--dest", "org.gnome.Shell",
            "--object-path", "/com/slickrun/Toggle",
            "--method", "com.slickrun.Toggle.MoveWindow",
            &x.to_string(),
            &y.to_string(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    eprintln!("[SlickRun] Moving window via Mutter to ({}, {})", x, y);
}

/// Set _NET_WM_STATE_ABOVE on an X11 window for always-on-top.
fn set_x11_always_on_top(window_id: u32, enable: bool) {
    if let Err(e) = set_x11_always_on_top_inner(window_id, enable) {
        eprintln!("[SlickRun] X11 always-on-top failed: {e}");
    }
}

fn set_x11_always_on_top_inner(window_id: u32, enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::*;

    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
    let net_wm_state_above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;

    let action = if enable { 1u32 } else { 0u32 }; // 1 = _NET_WM_STATE_ADD, 0 = _NET_WM_STATE_REMOVE
    let data = ClientMessageData::from([action, net_wm_state_above, 0, 1, 0]);
    let event = ClientMessageEvent {
        response_type: CLIENT_MESSAGE_EVENT,
        format: 32,
        sequence: 0,
        window: window_id,
        type_: net_wm_state,
        data,
    };
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    conn.send_event(false, root, mask, event)?;
    conn.flush()?;
    Ok(())
}

/// Convert HotkeyConfig to GNOME keybinding format, e.g. "<Control>q"
fn hotkey_to_gnome_binding(hotkey: &HotkeyConfig) -> String {
    let mut s = String::new();
    if hotkey.super_key {
        s.push_str("<Super>");
    }
    if hotkey.ctrl {
        s.push_str("<Control>");
    }
    if hotkey.shift {
        s.push_str("<Shift>");
    }
    if hotkey.alt {
        s.push_str("<Alt>");
    }
    let key = &hotkey.key;
    if key.len() == 1 {
        s.push_str(&key.to_lowercase());
    } else {
        s.push_str(key);
    }
    s
}

/// Install a minimal GNOME Shell extension that exposes a D-Bus method
/// to activate (focus) the SlickRun window at Mutter level.
/// This is the ONLY reliable way to focus a window on GNOME Wayland.
fn install_gnome_shell_extension() {
    let ext_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
        .join("gnome-shell/extensions/slickrun-toggle@slickrun");

    // Always write the extension files (in case of version updates)
    let _ = std::fs::create_dir_all(&ext_dir);

    let metadata = r#"{
    "uuid": "slickrun-toggle@slickrun",
    "name": "SlickRun Toggle",
    "description": "Activate SlickRun launcher window via D-Bus",
    "shell-version": ["45", "46", "47", "48"],
    "version": 4
}"#;

    let extension_js = r#"import Gio from 'gi://Gio';

const SlickRunIface = `<node>
  <interface name="com.slickrun.Toggle">
    <method name="Activate"/>
    <method name="GetPosition">
      <arg type="i" direction="out" name="x"/>
      <arg type="i" direction="out" name="y"/>
    </method>
    <method name="MoveWindow">
      <arg type="i" direction="in" name="x"/>
      <arg type="i" direction="in" name="y"/>
    </method>
  </interface>
</node>`;

export default class SlickRunToggleExtension {
    _findWindow() {
        const actor = global.get_window_actors().find(a => {
            const m = a.meta_window;
            return m && m.get_title() === 'SlickRun';
        });
        return actor ? actor.meta_window : null;
    }

    enable() {
        this._impl = {
            Activate: () => {
                const win = this._findWindow();
                if (win) win.activate(global.get_current_time());
            },
            GetPosition: () => {
                const win = this._findWindow();
                if (win) {
                    const rect = win.get_frame_rect();
                    return [rect.x, rect.y];
                }
                return [-1, -1];
            },
            MoveWindow: (x, y) => {
                const win = this._findWindow();
                if (win) win.move_frame(true, x, y);
            }
        };
        this._dbus = Gio.DBusExportedObject.wrapJSObject(SlickRunIface, this._impl);
        this._dbus.export(Gio.DBus.session, '/com/slickrun/Toggle');
    }

    disable() {
        if (this._dbus) {
            this._dbus.unexport();
            this._dbus = null;
        }
    }
}
"#;

    let _ = std::fs::write(ext_dir.join("metadata.json"), metadata);
    let _ = std::fs::write(ext_dir.join("extension.js"), extension_js);
    eprintln!("[SlickRun] GNOME Shell extension installed at {:?}", ext_dir);

    // Add to enabled-extensions via gsettings (persists across re-login).
    // This is more reliable than `gnome-extensions enable` which requires
    // GNOME Shell to already know about the extension.
    let uuid = "slickrun-toggle@slickrun";
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.shell", "enabled-extensions"])
        .output()
    {
        let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !current.contains(uuid) {
            // Parse current list and add our UUID
            let mut exts: Vec<String> = if current == "@as []" || current.is_empty() {
                vec![]
            } else {
                current
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .split(',')
                    .map(|s| s.trim().trim_matches('\'').to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            };
            exts.push(uuid.to_string());
            let new_val = format!(
                "[{}]",
                exts.iter()
                    .map(|e| format!("'{}'", e))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let _ = std::process::Command::new("gsettings")
                .args(["set", "org.gnome.shell", "enabled-extensions", &new_val])
                .output();
            eprintln!("[SlickRun] Added extension to enabled-extensions");
        }
    }

    // Check if the extension's D-Bus interface is already active
    let check = std::process::Command::new("gdbus")
        .args([
            "call", "--session",
            "--dest", "org.gnome.Shell",
            "--object-path", "/com/slickrun/Toggle",
            "--method", "com.slickrun.Toggle.Activate",
        ])
        .output();
    match check {
        Ok(o) if o.status.success() => {
            eprintln!("[SlickRun] Extension D-Bus interface is active");
        }
        _ => {
            eprintln!("[SlickRun] *** FIRST RUN: Please log out and back in to activate the GNOME Shell extension ***");
            eprintln!("[SlickRun] *** After re-login, Ctrl+Q will properly focus the window ***");
        }
    }
}

/// Register a GNOME custom keyboard shortcut.
/// The shortcut calls our D-Bus extension (for window activation at Mutter level)
/// and also writes to the FIFO (for app state sync).
fn register_gnome_shortcut(hotkey: &HotkeyConfig) {
    if std::process::Command::new("gsettings")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("gsettings not found — cannot register GNOME shortcut");
        return;
    }

    let binding = hotkey_to_gnome_binding(hotkey);
    let pipe_path = toggle_pipe_path();
    // Call the GNOME Shell extension's Activate method (for Mutter-level focus),
    // then notify the app via FIFO (for visibility toggle via off-screen positioning).
    let command = format!(
        "bash -c 'gdbus call --session --dest org.gnome.Shell --object-path /com/slickrun/Toggle --method com.slickrun.Toggle.Activate 2>/dev/null; echo t > {}'",
        pipe_path.display()
    );

    let slickrun_path =
        "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/slickrun/";

    let output = std::process::Command::new("gsettings")
        .args([
            "get",
            "org.gnome.settings-daemon.plugins.media-keys",
            "custom-keybindings",
        ])
        .output();

    let current = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => return,
    };

    if !current.contains("slickrun") {
        let mut paths: Vec<String> = if current == "@as []" || current.is_empty() {
            vec![]
        } else {
            current
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        paths.push(slickrun_path.to_string());

        let paths_str = paths
            .iter()
            .map(|p| format!("'{}'", p))
            .collect::<Vec<_>>()
            .join(", ");

        let _ = std::process::Command::new("gsettings")
            .args([
                "set",
                "org.gnome.settings-daemon.plugins.media-keys",
                "custom-keybindings",
                &format!("[{}]", paths_str),
            ])
            .output();
    }

    let schema_path = format!(
        "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{}",
        slickrun_path
    );

    let _ = std::process::Command::new("gsettings")
        .args(["set", &schema_path, "name", "SlickRun Toggle"])
        .output();
    let _ = std::process::Command::new("gsettings")
        .args(["set", &schema_path, "command", &command])
        .output();
    let _ = std::process::Command::new("gsettings")
        .args(["set", &schema_path, "binding", &binding])
        .output();

    eprintln!("[SlickRun] Registered GNOME shortcut: {} -> D-Bus + FIFO", binding);
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>, tray: tray_icon::TrayIcon) -> Self {
        let settings = Settings::load();
        let manager = GlobalHotKeyManager::new().unwrap();

        let hotkey = settings.hotkey.to_hotkey().expect("Invalid hotkey config");
        match manager.register(hotkey) {
            Ok(()) | Err(global_hotkey::Error::AlreadyRegistered(_)) => {}
            Err(e) => eprintln!("[SlickRun] Failed to register X11 hotkey: {e}"),
        }

        // Install GNOME Shell extension for Mutter-level window activation
        install_gnome_shell_extension();

        // Register GNOME custom shortcut (works on Wayland)
        register_gnome_shortcut(&settings.hotkey);

        // FIFO pipe for toggle
        let needs_initial_move = settings.window_x.is_some() && settings.window_y.is_some();
        let toggle_signal = Arc::new(AtomicBool::new(false));
        let is_visible = Arc::new(AtomicBool::new(true));
        let x11_window_id = Arc::new(AtomicU32::new(0));
        start_toggle_pipe_listener(cc.egui_ctx.clone(), toggle_signal.clone());

        // Shared window position — updated in update(), read by tray handler on quit
        let shared_pos_x = Arc::new(AtomicU32::new(0));
        let shared_pos_y = Arc::new(AtomicU32::new(0));

        // Tray menu events via set_event_handler
        let tray_action = Arc::new(AtomicU8::new(TRAY_NONE));
        let tray_action_for_handler = tray_action.clone();
        let ctx_for_tray = cc.egui_ctx.clone();
        let x11_id_for_tray = x11_window_id.clone();
        let pipe_for_tray = toggle_pipe_path();
        let pos_x_for_tray = shared_pos_x.clone();
        let pos_y_for_tray = shared_pos_y.clone();
        tray_icon::menu::MenuEvent::set_event_handler(Some(move |event: tray_icon::menu::MenuEvent| {
            eprintln!("[SlickRun] Tray menu event: {:?}", event.id().0);
            match event.id().0.as_str() {
                "quit" => {
                    eprintln!("[SlickRun] Tray quit — saving position and exiting");
                    // Save window position before exit
                    let x = f32::from_bits(pos_x_for_tray.load(Ordering::SeqCst));
                    let y = f32::from_bits(pos_y_for_tray.load(Ordering::SeqCst));
                    if x != 0.0 || y != 0.0 {
                        let mut s = Settings::load();
                        s.window_x = Some(x);
                        s.window_y = Some(y);
                        s.save();
                    }
                    let _ = std::fs::remove_file(&pipe_for_tray);
                    std::process::exit(0);
                }
                other => {
                    let action = match other {
                        "show_hide" => TRAY_TOGGLE,
                        "settings" => TRAY_SETTINGS,
                        _ => TRAY_NONE,
                    };
                    // Unminimize window via X11 so update() runs
                    let wid = x11_id_for_tray.load(Ordering::SeqCst);
                    if wid != 0 {
                        activate_x11_window_by_id(wid);
                    }
                    tray_action_for_handler.store(action, Ordering::SeqCst);
                    ctx_for_tray.request_repaint();
                }
            }
        }));

        // Background GLib event pump — blocks waiting for D-Bus events (tray menu)
        // so they're processed even when window is minimized and update() isn't running.
        std::thread::spawn(|| {
            let glib_ctx = gtk::glib::MainContext::default();
            loop {
                glib_ctx.iteration(true); // true = block until event available
            }
        });

        eprintln!("=== SlickRun ===");
        eprintln!("Hotkey: {}", settings.hotkey.display_string());
        eprintln!("Built-in commands: setup, quit");
        eprintln!("================");

        let settings_window = SettingsWindow::new(&settings);

        Self {
            hotkey_manager: manager,
            toggle_hotkey_id: hotkey.id(),
            command_input: String::new(),
            is_visible,
            text_edit_rect: None,
            settings,
            settings_window,
            was_focused: false,
            _tray: tray,
            toggle_signal,
            tray_action,
            x11_window_id,
            x11_search_attempts: 0,
            dragging: false,
            pending_w_magic_word: None,
            w_dialog_input: String::new(),
            shared_pos_x,
            shared_pos_y,
            last_known_pos: None,
            last_pos_query: std::time::Instant::now(),
            needs_initial_move,
        }
    }

    fn toggle_visibility(&mut self, ctx: &egui::Context) {
        if self.is_visible.load(Ordering::SeqCst) {
            self.hide(ctx);
        } else {
            self.show(ctx);
        }
    }

    fn save_position(&mut self, _ctx: &egui::Context) {
        // Try cached position first, fall back to querying Mutter directly
        let pos = self.last_known_pos.or_else(|| {
            get_mutter_window_position().map(|(x, y)| (x as f32, y as f32))
        });
        if let Some((x, y)) = pos {
            eprintln!("[SlickRun] Saving position: ({}, {})", x, y);
            self.settings.window_x = Some(x);
            self.settings.window_y = Some(y);
            self.shared_pos_x.store(x.to_bits(), Ordering::SeqCst);
            self.shared_pos_y.store(y.to_bits(), Ordering::SeqCst);
            self.settings.save();
        } else {
            eprintln!("[SlickRun] save_position: no known position yet");
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.save_position(ctx);
        let pipe_path = toggle_pipe_path();
        let _ = std::fs::remove_file(&pipe_path);
        // Close gracefully so destructors run and tray icon is unregistered
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn hide(&mut self, ctx: &egui::Context) {
        self.save_position(ctx);
        self.is_visible.store(false, Ordering::SeqCst);
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }

    fn show(&mut self, ctx: &egui::Context) {
        self.is_visible.store(true, Ordering::SeqCst);
        // Unminimize
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        // Restore saved position, or center on screen
        let pos = if self.settings_window.open {
            let win_w = 650.0;
            let win_h = 550.0;
            ctx.input(|i| i.viewport().monitor_size)
                .map(|ms| egui::pos2((ms.x - win_w) / 2.0, (ms.y - win_h) / 2.0))
                .unwrap_or(egui::pos2(400.0, 300.0))
        } else if let (Some(x), Some(y)) = (self.settings.window_x, self.settings.window_y) {
            egui::pos2(x, y)
        } else {
            let win_w = self.settings.window_width;
            let win_h = self.settings.window_height;
            ctx.input(|i| i.viewport().monitor_size)
                .map(|ms| egui::pos2((ms.x - win_w) / 2.0, (ms.y - win_h) / 2.0))
                .unwrap_or(egui::pos2(400.0, 300.0))
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        // Move via Mutter (OuterPosition doesn't work on Wayland)
        move_window_via_mutter(pos.x as i32, pos.y as i32);

        // Re-assert always-on-top after moving back on-screen
        if self.settings.stay_on_top {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            let wid = self.x11_window_id.load(Ordering::SeqCst);
            if wid != 0 {
                set_x11_always_on_top(wid, true);
            }
        }

        // Reset focus tracking so auto-hide doesn't fire immediately
        // (Mutter focus takes a frame or two to propagate)
        self.was_focused = false;

        // Request Mutter-level focus via GNOME Shell extension.
        // This is the ONLY way to get proper focus on GNOME Wayland.
        // When triggered from the keyboard shortcut, the shortcut command
        // already called Activate before the FIFO. This call covers
        // other show paths (tray menu, etc.) and is harmless if called twice.
        let _ = std::process::Command::new("gdbus")
            .args([
                "call", "--session",
                "--dest", "org.gnome.Shell",
                "--object-path", "/com/slickrun/Toggle",
                "--method", "com.slickrun.Toggle.Activate",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();

        // X11 activation as fallback (works on native X11, not on Wayland)
        let wid = self.x11_window_id.load(Ordering::SeqCst);
        if wid != 0 {
            activate_x11_window_by_id(wid);
        }
    }

    fn re_register_hotkey(&mut self) {
        if let Some(new_hk) = self.settings.hotkey.to_hotkey() {
            match self.hotkey_manager.register(new_hk) {
                Ok(()) | Err(global_hotkey::Error::AlreadyRegistered(_)) => {
                    self.toggle_hotkey_id = new_hk.id();
                }
                Err(e) => eprintln!("Failed to register hotkey: {e}"),
            }
        }
        register_gnome_shortcut(&self.settings.hotkey);
    }

    fn apply_settings(&mut self, new_settings: Settings, ctx: &egui::Context) {
        let hotkey_changed = {
            let old = &self.settings.hotkey;
            let new = &new_settings.hotkey;
            old.key != new.key
                || old.super_key != new.super_key
                || old.ctrl != new.ctrl
                || old.shift != new.shift
                || old.alt != new.alt
        };

        if hotkey_changed {
            if let Some(old_hk) = self.settings.hotkey.to_hotkey() {
                let _ = self.hotkey_manager.unregister(old_hk);
            }
        }

        self.settings = new_settings;
        self.settings.save();

        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
            self.settings.window_width,
            self.settings.window_height,
        )));

        let level = if self.settings.stay_on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));

        self.settings.update_autostart();

        if hotkey_changed {
            self.re_register_hotkey();
        }
    }

    fn best_autocomplete(&self, input: &str) -> Option<String> {
        if input.is_empty() {
            return None;
        }
        let first_word = input.split_whitespace().next().unwrap_or("");
        if first_word.is_empty() {
            return None;
        }
        let lower = first_word.to_lowercase();
        let mut names: Vec<String> = vec!["setup".into(), "quit".into()];
        for mw in &self.settings.magic_words {
            names.push(mw.keyword.clone());
        }
        names
            .into_iter()
            .filter(|name| name.to_lowercase().starts_with(&lower))
            .min_by_key(|name| name.len())
    }

    fn execute_command(&mut self, ctx: &egui::Context) {
        let input = self.command_input.trim().to_string();
        if input.is_empty() {
            return;
        }

        let (first_word, user_args) = match input.find(char::is_whitespace) {
            Some(pos) => (&input[..pos], input[pos..].trim()),
            None => (input.as_str(), ""),
        };

        if first_word.eq_ignore_ascii_case("setup") {
            self.command_input.clear();
            self.settings_window.open(&self.settings);
            return;
        }

        if first_word.eq_ignore_ascii_case("quit") {
            self.quit(ctx);
            return;
        }

        if let Some(mw) = commands::find_by_keyword(&self.settings.magic_words, first_word) {
            if user_args.is_empty() && mw.needs_w_input() {
                // $W$ magic word with no args — show input dialog
                self.pending_w_magic_word = Some(mw.clone());
                self.w_dialog_input.clear();
                self.command_input.clear();
                return;
            }
            mw.execute(user_args);
            self.command_input.clear();
            self.hide(ctx);
            return;
        }

        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(&input)
            .spawn();

        self.command_input.clear();
        self.hide(ctx);
    }
}

impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Store our X11 window ID on the first frames (while we still have focus)
        if self.x11_window_id.load(Ordering::SeqCst) == 0 && self.x11_search_attempts < 10 {
            self.x11_search_attempts += 1;
            if let Some(id) = find_own_x11_window() {
                self.x11_window_id.store(id, Ordering::SeqCst);
                eprintln!("[SlickRun] Stored X11 window ID: 0x{:x}", id);
                if self.settings.stay_on_top {
                    set_x11_always_on_top(id, true);
                }
            }
        }

        // Track window position via Mutter (only way to get real position on XWayland).
        // Query every ~1 second while visible to avoid spawning gdbus too often.
        if self.is_visible.load(Ordering::SeqCst) {
            let should_query = self.last_known_pos.is_none()
                || self.last_pos_query.elapsed() > std::time::Duration::from_secs(1);
            if should_query {
                self.last_pos_query = std::time::Instant::now();
                if let Some((x, y)) = get_mutter_window_position() {
                    self.last_known_pos = Some((x as f32, y as f32));
                    self.shared_pos_x.store((x as f32).to_bits(), Ordering::SeqCst);
                    self.shared_pos_y.store((y as f32).to_bits(), Ordering::SeqCst);
                }
            }
        }

        // Pump GTK events for tray icon — MUST do at least one iteration
        // to poll D-Bus file descriptors (events_pending alone doesn't poll I/O)
        gtk::main_iteration_do(false);
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }

        // Toggle pipe (FIFO from GNOME keyboard shortcut)
        if self.toggle_signal.swap(false, Ordering::SeqCst) {
            eprintln!("[SlickRun] Toggle signal received via FIFO");
            self.toggle_visibility(ctx);
        }

        // X11 global hotkey (fallback, works on XWayland)
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id() == self.toggle_hotkey_id {
                eprintln!("[SlickRun] X11 hotkey toggle");
                self.toggle_visibility(ctx);
            }
        }

        // Tray menu events (via set_event_handler → AtomicU8)
        match self.tray_action.swap(TRAY_NONE, Ordering::SeqCst) {
            TRAY_TOGGLE => {
                eprintln!("[SlickRun] Tray: toggle visibility");
                self.toggle_visibility(ctx);
            }
            TRAY_SETTINGS => {
                eprintln!("[SlickRun] Tray: open settings");
                self.show(ctx);
                self.settings_window.open(&self.settings);
            }
            TRAY_QUIT => {
                eprintln!("[SlickRun] Tray: quit");
                self.show(ctx); // unminimize so Close command processes cleanly
                self.quit(ctx);
            }
            _ => {}
        }

        // Always track focus state so transitions are detected correctly
        let focused = ctx.input(|i| i.focused);

        // On first focus, move window to saved position via Mutter (blocking).
        // Must wait until window has focus — move_frame doesn't work on unfocused windows.
        if self.needs_initial_move && focused {
            self.needs_initial_move = false;
            if let (Some(x), Some(y)) = (self.settings.window_x, self.settings.window_y) {
                eprintln!("[SlickRun] Initial move to saved position ({}, {})", x, y);
                let _ = std::process::Command::new("gdbus")
                    .args([
                        "call", "--session",
                        "--dest", "org.gnome.Shell",
                        "--object-path", "/com/slickrun/Toggle",
                        "--method", "com.slickrun.Toggle.MoveWindow",
                        &(x as i32).to_string(),
                        &(y as i32).to_string(),
                    ])
                    .output(); // blocking — wait for move to complete
            }
        }

        // Drag end detection: focus returns after being lost while dragging.
        // When drag ends, re-request focus so auto-hide doesn't fire immediately.
        if self.dragging && !self.was_focused && focused {
            self.dragging = false;
            eprintln!("[SlickRun] Drag ended — re-focusing");
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            self.was_focused = true; // skip the transition check below
        }

        // Auto-hide on focus loss (suppressed while dragging or $W$ dialog open)
        if self.settings.hide_on_inactive && self.is_visible.load(Ordering::SeqCst) && !self.settings_window.open && !self.dragging && self.pending_w_magic_word.is_none() {
            if self.was_focused && !focused {
                eprintln!("[SlickRun] Focus lost — auto-hiding");
                self.hide(ctx);
            }
        }

        if focused != self.was_focused {
            eprintln!("[SlickRun] Focus changed: {} -> {}", self.was_focused, focused);
        }
        self.was_focused = focused;

        // Settings mode: resize window and render settings UI inline
        if self.settings_window.open {
            // ESC closes settings
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.settings_window.open = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    self.settings.window_width,
                    self.settings.window_height,
                )));
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }

            // Ensure window is large enough for settings
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(650.0, 550.0)));
            ctx.set_visuals(egui::Visuals::dark());

            let saved = self.settings_window.show_in_viewport(ctx);
            if let Some(new_settings) = saved {
                self.apply_settings(new_settings, ctx);
            }
            if !self.settings_window.open {
                // Settings just closed — restore command bar size
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    self.settings.window_width,
                    self.settings.window_height,
                )));
            }

            // Drag support in settings mode — only the very top edge (above tab bar)
            if ctx.input(|i| i.pointer.primary_pressed()) {
                if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                    if pos.y < 4.0 {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                }
            }

            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        }

        // $W$ parameter input dialog
        if self.pending_w_magic_word.is_some() {
            // Expand window to fit the label + input (scales with font size)
            let w_dialog_height = 12.0 + 14.0 + 6.0 + self.settings.font_size + 16.0 + 12.0 + 20.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                self.settings.window_width,
                w_dialog_height,
            )));

            let w_enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            let w_escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));

            ctx.set_visuals(egui::Visuals::dark());

            let keyword = self
                .pending_w_magic_word
                .as_ref()
                .map(|mw| mw.keyword.clone())
                .unwrap_or_default();

            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(40, 40, 40))
                        .inner_margin(12.0),
                )
                .show(ctx, |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} — enter parameters:", keyword))
                            .color(egui::Color32::from_rgb(200, 200, 200))
                            .size(14.0),
                    );
                    ui.add_space(6.0);

                    let w_input_id = egui::Id::new("w_dialog_input");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.w_dialog_input)
                            .id(w_input_id)
                            .font(egui::FontId::monospace(self.settings.font_size))
                            .desired_width(f32::INFINITY)
                            .hint_text("Type or paste text, then press Enter"),
                    );
                    response.request_focus();
                });

            if w_enter && !self.w_dialog_input.is_empty() {
                let mw = self.pending_w_magic_word.take().unwrap();
                let args = self.w_dialog_input.clone();
                self.w_dialog_input.clear();
                // Restore original window size before hiding
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    self.settings.window_width,
                    self.settings.window_height,
                )));
                mw.execute(&args);
                self.hide(ctx);
            } else if w_escape {
                self.pending_w_magic_word = None;
                self.w_dialog_input.clear();
                // Restore original window size
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                    self.settings.window_width,
                    self.settings.window_height,
                )));
            }

            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            return;
        }

        // Read key events BEFORE widgets consume them
        let enter_pressed = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let tab_pressed = ctx.input(|i| i.key_pressed(egui::Key::Tab));
        let escape_pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let alpha = (self.settings.opacity_percent as f32 / 100.0 * 255.0) as u8;
        let bg = egui::Color32::from_rgba_unmultiplied(30, 30, 30, alpha);

        // Autocomplete hint
        let hint_suffix = if !self.command_input.is_empty() {
            self.best_autocomplete(&self.command_input)
                .and_then(|hint| {
                    let first_word = self.command_input.split_whitespace().next().unwrap_or("");
                    if hint.len() > first_word.len()
                        && hint
                            .to_lowercase()
                            .starts_with(&first_word.to_lowercase())
                    {
                        Some(hint[first_word.len()..].to_string())
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        let font_size = self.settings.font_size;

        // Transparent visuals for the command bar
        {
            let mut visuals = egui::Visuals::dark();
            visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
            visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.hovered.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.active.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.noninteractive.bg_fill = egui::Color32::TRANSPARENT;
            visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::TRANSPARENT;
            visuals.selection.bg_fill =
                egui::Color32::from_rgba_unmultiplied(60, 60, 120, alpha);
            visuals.panel_fill = egui::Color32::TRANSPARENT;
            ctx.set_visuals(visuals);
        }

        // Main command bar
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(bg).inner_margin(8.0))
            .show(ctx, |ui| {
                let font_color = self.settings.font_color32();
                let text_edit_id = egui::Id::new("main_command_input");

                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(">")
                                .color(font_color)
                                .monospace()
                                .size(font_size + 4.0),
                        )
                        .selectable(false),
                    );

                    let input_widget = egui::TextEdit::singleline(&mut self.command_input)
                        .id(text_edit_id)
                        .font(egui::FontId::monospace(font_size))
                        .desired_width(f32::INFINITY)
                        .text_color(font_color);

                    let response = ui.add(input_widget);
                    self.text_edit_rect = Some(response.rect);

                    // Inline autocomplete ghost text
                    if let Some(ref suffix) = hint_suffix {
                        let galley = ui.painter().layout_no_wrap(
                            self.command_input.clone(),
                            egui::FontId::monospace(font_size),
                            egui::Color32::TRANSPARENT,
                        );
                        let text_end_x = response.rect.left() + 4.0 + galley.size().x;
                        let text_y = response.rect.top()
                            + (response.rect.height() - galley.size().y) / 2.0;
                        ui.painter().text(
                            egui::pos2(text_end_x, text_y),
                            egui::Align2::LEFT_TOP,
                            suffix,
                            egui::FontId::monospace(font_size),
                            egui::Color32::from_rgba_unmultiplied(120, 120, 120, 160),
                        );
                    }

                    response.request_focus();

                    if tab_pressed {
                        if let Some(match_name) = self.best_autocomplete(&self.command_input) {
                            self.command_input = match_name;
                            if let Some(mut state) =
                                egui::TextEdit::load_state(ctx, text_edit_id)
                            {
                                let ccursor =
                                    egui::text::CCursor::new(self.command_input.len());
                                state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::one(ccursor),
                                ));
                                state.store(ctx, text_edit_id);
                            }
                        }
                    }

                    if enter_pressed {
                        if let Some(completed) = self.best_autocomplete(&self.command_input) {
                            let first_word =
                                self.command_input.split_whitespace().next().unwrap_or("");
                            if completed.to_lowercase() != first_word.to_lowercase() {
                                let rest = self.command_input[first_word.len()..].to_string();
                                self.command_input = format!("{}{}", completed, rest);
                            }
                        }
                        self.execute_command(ctx);
                    }

                    if escape_pressed {
                        self.command_input.clear();
                        self.hide(ctx);
                    }
                });

                // Drag — click anywhere outside text edit starts drag
                let pressed = ctx.input(|i| i.pointer.primary_pressed());
                if pressed {
                    if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                        let in_text_edit = self
                            .text_edit_rect
                            .is_some_and(|r: egui::Rect| r.contains(pos));
                        if !in_text_edit {
                            self.dragging = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }
                }
            });

        // Keep event loop alive for polling
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}
