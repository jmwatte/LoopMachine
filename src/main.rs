#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod arrangement;
mod chroma;
mod loops;
mod session;
mod shortcuts;
mod timestretch;
pub mod video_player;
mod waveform;
mod waveform_player;

fn main() -> Result<(), eframe::Error> {
    // ── Logger: schrijf naar bestand (console is onzichtbaar met windows_subsystem) ──
    let log_path = crate::session::data_dir().join("loopmachine.log");
    let _ = std::fs::remove_file(&log_path);
    if let Ok(log_file) = std::fs::File::create(&log_path) {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
            .format_timestamp_millis()
            .target(env_logger::Target::Pipe(Box::new(log_file)))
            .init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
            .format_timestamp_millis()
            .init();
    }

    log::info!("LoopMachine gestart — logbestand: {}", log_path.display());

    // Migreer data van oude locatie (working directory) naar %APPDATA%/LoopMachine/
    crate::loops::migrate_if_needed();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 600.0])
            .with_resizable(true)
            .with_decorations(true)
            .with_transparent(false),
        ..Default::default()
    };

    eframe::run_native(
        "Waveform Loop Editor",
        options,
        Box::new(|cc| {
            // ── Font setup ──
            let mut fonts = eframe::egui::FontDefinitions::default();

            fonts.font_data.insert(
                "DejaVuSansMono".to_string(),
                eframe::egui::FontData::from_static(include_bytes!(
                    "../vendor/fonts/DejaVuSansMono.ttf"
                )),
            );

            if let Some(monospace) = fonts.families.get_mut(&eframe::egui::FontFamily::Monospace) {
                monospace.insert(0, "DejaVuSansMono".to_string());
            }

            if let Some(proportional) = fonts
                .families
                .get_mut(&eframe::egui::FontFamily::Proportional)
            {
                proportional.push("DejaVuSansMono".to_string());
            }

            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(app::LoopEditorApp::new()))
        }),
    )
}
