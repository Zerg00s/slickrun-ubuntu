mod app;
mod commands;
mod settings;

use eframe::egui;
use tray_icon::menu::{Menu, MenuId, MenuItem};
use tray_icon::{Icon, TrayIconBuilder};

fn main() -> eframe::Result<()> {
    // Force X11 backend — global-hotkey uses XGrabKey which needs X11
    unsafe {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    }

    gtk::init().expect("Failed to init GTK");

    let tray = build_tray_icon();

    // Pump GTK events so libappindicator registers the icon via D-Bus
    for _ in 0..50 {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let settings = settings::Settings::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_decorations(false)
        .with_inner_size([settings.window_width, settings.window_height])
        .with_always_on_top()
        .with_transparent(true)
        .with_resizable(false);

    if let (Some(x), Some(y)) = (settings.window_x, settings.window_y) {
        viewport = viewport.with_position([x, y]);
    } else if let Some((sx, sy)) = screen_size() {
        let cx = (sx as f32 - settings.window_width) / 2.0;
        let cy = (sy as f32 - settings.window_height) / 2.0;
        viewport = viewport.with_position([cx, cy]);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SlickRun",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::LauncherApp::new(cc, tray)))),
    )
}

fn screen_size() -> Option<(u16, u16)> {
    use x11rb::connection::Connection;
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    Some((screen.width_in_pixels, screen.height_in_pixels))
}

fn build_tray_icon() -> tray_icon::TrayIcon {
    let menu = Menu::new();
    let show_hide = MenuItem::with_id(MenuId::new("show_hide"), "Show/Hide", true, None);
    let settings_item = MenuItem::with_id(MenuId::new("settings"), "Settings", true, None);
    let quit = MenuItem::with_id(MenuId::new("quit"), "Quit", true, None);
    menu.append(&show_hide).unwrap();
    menu.append(&settings_item).unwrap();
    menu.append(&quit).unwrap();

    let icon = create_icon();

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("SlickRun")
        .with_icon(icon)
        .build()
        .expect("Failed to build tray icon")
}

fn create_icon() -> Icon {
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let cx = (x as f32) - 15.5;
            let cy = (y as f32) - 15.5;
            if cx * cx + cy * cy <= 14.0 * 14.0 {
                rgba[idx] = 0;
                rgba[idx + 1] = 200;
                rgba[idx + 2] = 120;
                rgba[idx + 3] = 255;
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("Failed to create icon")
}
