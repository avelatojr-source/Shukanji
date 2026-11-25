// Hide the console window in release builds; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tiktok_stream_tool::app::App;

/// The ShuKanji window/taskbar icon (decoded from the bundled PNG).
fn app_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(include_bytes!("../assets/app-icon.png")).ok()?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    // File logging under the per-user config directory.
    let log_dir = tiktok_stream_tool::persistence::paths::log_dir();
    let _log_guard = tiktok_stream_tool::diagnostics::logging::init(log_dir.as_deref());

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([1000.0, 640.0])
        .with_title("ShuKanji");
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        // Do NOT restore the previous window geometry. If the process is killed
        // while the window is minimized, the saved geometry is a 0x0 window
        // parked at (-32000, -32000) — restoring it makes the app open
        // invisible. Always start from the configured size instead.
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "ShuKanji",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
