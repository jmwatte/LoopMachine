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
    // ── Zorg dat data-directory bestaat ──
    let data_dir = crate::session::data_dir();
    let _ = std::fs::create_dir_all(&data_dir);

    // ── Logger: schrijf naar bestand ──
    let log_path = data_dir.join("loopmachine.log");
    let _ = std::fs::remove_file(&log_path);
    match std::fs::File::create(&log_path) {
        Ok(log_file) => {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
                .format_timestamp_millis()
                .target(env_logger::Target::Pipe(Box::new(log_file)))
                .init();
        }
        Err(e) => {
            // Fallback: stderr (werkt alleen als console zichtbaar is)
            eprintln!(
                "Kon logbestand niet aanmaken '{}': {}",
                log_path.display(),
                e
            );
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
                .format_timestamp_millis()
                .init();
        }
    }

    log::info!("LoopMachine gestart — log: {}", log_path.display());

    // Migreer data van oude locatie naar %APPDATA%/LoopMachine/
    crate::session::data_dir(); // forceer aanmaken
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
