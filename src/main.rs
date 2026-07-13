#![windows_subsystem = "windows"]

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
    // ── Logger initialisatie (zie RUST_LOG omgevingsvariabele) ──
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp_millis()
        .init();

    log::info!("LoopMachine gestart");

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
            // DejaVu Sans Mono heeft uitstekende Unicode-dekking (pijlen, symbolen, etc.)
            // We gebruiken het als primair monospace font + als fallback voor proportional.
            let mut fonts = eframe::egui::FontDefinitions::default();

            fonts.font_data.insert(
                "DejaVuSansMono".to_string(),
                eframe::egui::FontData::from_static(include_bytes!(
                    "../vendor/fonts/DejaVuSansMono.ttf"
                )),
            );

            // Monospace: DejaVu bovenaan (voorrang boven Hack)
            if let Some(monospace) = fonts.families.get_mut(&eframe::egui::FontFamily::Monospace) {
                monospace.insert(0, "DejaVuSansMono".to_string());
            }

            // Proportional: DejaVu achteraan (laatste fallback voor missende tekens)
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
