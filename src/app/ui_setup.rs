use std::sync::Arc;

use eframe::egui::{self, Color32, RichText};

use crate::app::LoopEditorApp;
use crate::waveform_player::WaveformCommand;

impl LoopEditorApp {
    /// Toon het Setup/Kalibratie venster.
    pub fn show_setup_window(&mut self, ctx: &egui::Context) {
        if !self.show_setup {
            return;
        }

        egui::Window::new("\u{2699} Setup — Latency & Beat Audit")
            .id(egui::Id::new("setup_window"))
            .resizable(true)
            .default_size([500.0, 500.0])
            .default_pos([200.0, 100.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // ── KOP: Uitleg ──
                    ui.label(
                        RichText::new(
                            "Stel hier de latency-compensatie in en audit de beat-markers.\n\
                             De waarden worden automatisch opgeslagen in session.json.",
                        )
                        .size(11.0)
                        .color(Color32::GRAY),
                    );
                    ui.separator();

                    // ════════════════════════════════════════
                    // SECTIE 1: Latency Kalibratie
                    // ════════════════════════════════════════
                    ui.label(
                        RichText::new("\u{1F4A1} Latency Kalibratie")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(200, 200, 100)),
                    );
                    ui.label(
                        RichText::new(
                            "Als je tijdens het afspelen een marker zet (S/M/B), komt die te laat\n\
                             omdat de audio eerst door de SoundTouch-processor en de audiokaart-buffer\n\
                             moet. Deze vertraging verschilt per computer.\n\n\
                             Stel de schuif zo af dat wat je hoort en wat je ziet synchroon lopen.",
                        )
                        .size(11.0)
                        .color(Color32::GRAY),
                    );

                    ui.add_space(4.0);

                    // ── Latency slider ──
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Manual marker latency:")
                                .size(13.0)
                                .strong(),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.playback_latency_ms, 0.0..=200.0)
                                .text("ms")
                                .step_by(5.0),
                        );
                    });
                    ui.label(
                        RichText::new(format!(
                            "Bij handmatig plaatsen (B-toets) wordt marker {}ms VÓÓR de playhead gezet",
                            self.playback_latency_ms,
                        ))
                        .size(11.0)
                        .color(Color32::from_rgb(150, 200, 150)),
                    );
                    ui.label(
                        RichText::new(
                            "Als markers te VROEG staan → latency VERLAGEN\n\
                             Als markers te LAAT staan  → latency VERHOGEN"
                        )
                        .size(10.0)
                        .color(Color32::from_rgb(200, 150, 150)),
                    );

                    ui.add_space(6.0);

                    // ── Kalibratie test ──
                    ui.horizontal(|ui| {
                        if ui
                            .button("\u{1F514} Start kalibratie-test")
                            .on_hover_text(
                                "Speel een testclick + visuele flits af.\n\
                                 Pas de latency aan tot click en flits synchroon zijn.",
                            )
                            .clicked()
                        {
                            self.run_calibration_test();
                        }

                        // Visuele flits indicator
                        if self.calibration_flash > 0 {
                            let brightness = (self.calibration_flash as u8 * 17).min(200);
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(36.0, 36.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                rect,
                                4.0,
                                Color32::from_rgb(brightness, brightness, 255),
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "\u{26A1}",
                                egui::TextStyle::Body.resolve(ui.style()),
                                Color32::WHITE,
                            );
                        }
                    });

                    ui.label(
                        RichText::new(
                            "Er worden 8 clicks om de 1.5s gegenereerd. De UI flitst op\n\
                             exact elke click-positie. Jij hoort de click later door\n\
                             de audio-buffers. Pas de latency aan tot het flits-ritme\n\
                             en click-ritme synchroon lopen. Herhaal voor de zekerheid.\n\
                             Tip: begin bij 0ms en werk omhoog tot je de vertraging ziet.",
                        )
                        .size(10.0)
                        .color(Color32::GRAY),
                    );

                    ui.separator();

                    // ════════════════════════════════════════
                    // SECTIE 2: Beat Audit (kliktrack)
                    // ════════════════════════════════════════
                    ui.label(
                        RichText::new("\u{1F50A} Beat Audit — Kliktrack")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(100, 200, 220)),
                    );
                    ui.label(
                        RichText::new(
                            "Schakel de kliktrack in om te horen waar de beat-markers vallen.\n\
                             De click wordt sample-accuraat in de audio gemixed (geen sync-problemen).",
                        )
                        .size(11.0)
                        .color(Color32::GRAY),
                    );

                    ui.add_space(4.0);

                    // ── Audit toggle ──
                    let prev_enabled = self.click_enabled.load(std::sync::atomic::Ordering::Relaxed);
                    let mut audit_on = prev_enabled;
                    ui.checkbox(&mut audit_on, "Beat audit aan (hoorbare clicks op markers)");
                    if audit_on != prev_enabled {
                        self.click_enabled.store(audit_on, std::sync::atomic::Ordering::Relaxed);
                        if audit_on {
                            self.update_click_positions();
                        }
                    }

                    // ── Click source keuze ──
                    if audit_on {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label("Clicks op:");
                            ui.radio_value(&mut self.click_on_bpm, true, "Auto-BPM beats");
                            ui.radio_value(&mut self.click_on_bpm, false, "Handmatige markers");
                        });

                        if ui.button("\u{1F504} Ververs click-posities").clicked() {
                            self.update_click_positions();
                        }

                        // Toon statistieken
                        let pos_count = {
                            let positions = self.click_positions.lock().unwrap();
                            positions.len()
                        };
                        let source_label = if self.click_on_bpm { "BPM beats" } else { "markers" };
                        ui.label(
                            RichText::new(format!(
                                "{} clicks geladen uit {}",
                                pos_count, source_label
                            ))
                            .size(11.0)
                            .color(if pos_count > 0 {
                                Color32::from_rgb(150, 200, 150)
                            } else {
                                Color32::GRAY
                            }),
                        );
                    }

                    ui.add_space(6.0);

                    // ── Beat offset correctie (voor auto-BPM) ──
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Beat offset-correctie:")
                                .size(13.0)
                                .strong(),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.beat_offset_ms, -50.0..=50.0)
                                .text("ms")
                                .step_by(1.0),
                        );
                    });
                    ui.label(
                        RichText::new(
                            "(+ = beats later, - = beats vroeger). Alleen voor auto-BPM markers.",
                        )
                        .size(10.0)
                        .color(Color32::GRAY),
                    );

                    ui.separator();

                    // ════════════════════════════════════════
                    // SECTIE 3: BPM Detectie drempel
                    // ════════════════════════════════════════
                    ui.label(
                        RichText::new("\u{1F3B5} BPM Detectie")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(120, 200, 120)),
                    );

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Detectie-drempel (strength):")
                                .size(13.0)
                                .strong(),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.bpm_threshold, 0.0..=1.0)
                                .text("")
                                .step_by(0.05),
                        );
                        if ui
                            .button("\u{1F504} Herplaats BPM beats")
                            .on_hover_text("Herplaats beat-markers met de huidige drempel + offset")
                            .clicked()
                        {
                            self.place_bpm_markers();
                            if self.click_enabled.load(std::sync::atomic::Ordering::Relaxed)
                                && self.click_on_bpm
                            {
                                self.update_click_positions();
                            }
                        }
                    });
                    ui.label(
                        RichText::new(format!(
                            "Hoe hoger ({:.0}%), hoe strenger — alleen sterke beats worden marker.",
                            self.bpm_threshold * 100.0
                        ))
                        .size(10.0)
                        .color(Color32::GRAY),
                    );

                    ui.separator();

                    // ════════════════════════════════════════
                    // SECTIE 4: Snelle acties
                    // ════════════════════════════════════════
                    ui.label(
                        RichText::new("\u{2699} Snelacties")
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(200, 180, 120)),
                    );

                    ui.horizontal(|ui| {
                        if ui
                            .button("\u{1F504} Start auto-detectie")
                            .on_hover_text("Voer chroma + BPM + beat-detectie uit")
                            .clicked()
                        {
                            self.run_detection();
                            if self.click_enabled.load(std::sync::atomic::Ordering::Relaxed)
                                && self.click_on_bpm
                            {
                                self.update_click_positions();
                            }
                        }

                        if ui
                            .button("\u{1F5D1} Wis alle markers")
                            .on_hover_text("Verwijder alle markers")
                            .clicked()
                        {
                            self.clear_markers_by_kind(None);
                            if self.click_enabled.load(std::sync::atomic::Ordering::Relaxed) {
                                self.update_click_positions();
                            }
                        }
                    });

                    // ── Bulk opschuiven ──
                    ui.horizontal(|ui| {
                        ui.label("Schuif markers:");
                        ui.add(
                            egui::Slider::new(&mut self.bulk_shift_ms, -200..=200)
                                .text("ms")
                                .step_by(10.0),
                        );
                        if ui
                            .button("Toepassen")
                            .on_hover_text(
                                "Alle markers met dit aantal ms verschuiven.\n\
                                 Positief = later, Negatief = vroeger.",
                            )
                            .clicked()
                        {
                            let shift_ms = self.bulk_shift_ms;
                            if shift_ms != 0 && !self.waveform_state.markers.is_empty() {
                                let shift_secs = shift_ms as f32 / 1000.0;
                                let count = self.waveform_state.markers.len();
                                for m in self.waveform_state.markers.iter_mut() {
                                    m.position_secs = (m.position_secs + shift_secs)
                                        .max(0.0)
                                        .min(self.waveform_state.duration_secs);
                                }
                                self.push_undo();
                                self.sync_markers_to_library();
                                if !self.click_on_bpm {
                                    self.update_click_positions();
                                }
                                self.status_message = format!(
                                    "{} markers {}ms opgeschoven ({})",
                                    count,
                                    shift_ms,
                                    if shift_ms > 0 { "later" } else { "vroeger" }
                                );
                                self.status_message_timer = 5 * 60;
                            }
                        }
                        if ui.button("Reset").clicked() {
                            self.bulk_shift_ms = 0;
                        }
                    });

                        ui.add_space(8.0);

                    // ── Status ──
                    if !self.status_message.is_empty() {
                        ui.separator();
                        ui.label(
                            RichText::new(&self.status_message)
                                .size(12.0)
                                .color(Color32::from_rgb(100, 200, 100)),
                        );
                    }
                });
            });
    }

    /// Start de kalibratie-test.
    ///
    /// Genereert 8 clicks om de 1.5 seconden (12 clicks bij 76 BPM ≈ 4 maten).
    /// De UI flitst op exact elke click-positie. Jij hoort de click iets later
    /// door de audio-buffers. Pas `playback_latency_ms` aan tot het flits-ritme
    /// en het click-ritme synchroon lopen.
    fn run_calibration_test(&mut self) {
        if self.waveform_state.path.is_none() {
            self.status_message = "Eerst een audiobestand laden".to_string();
            self.status_message_timer = 3 * 60;
            return;
        }

        // Genereer 8 clicks om de 1.5 seconden
        let interval = 1.5_f32;
        let num_clicks = 8;

        let mut cal_positions: Vec<f32> = Vec::with_capacity(num_clicks);
        let mut pos = 0.5_f32; // start na 0.5s stilte
        for _ in 0..num_clicks {
            cal_positions.push(pos);
            pos += interval;
        }

        if cal_positions.is_empty() {
            return;
        }

        // Reset alle calibratie-state
        self.calibration_click_positions = cal_positions.clone();
        self.calibration_next_idx = 0;
        self.calibration_active = true;
        self.calibration_flash = 0;

        // Zet clicks op die posities (audio-thread genereert ze sample-accuraat)
        *self.click_positions.lock().unwrap() = cal_positions.clone();
        self.click_enabled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.click_on_bpm = false;

        // Reset seek_pending zodat Position events worden geaccepteerd
        self.waveform_state.seek_pending = None;

        // Reset playhead naar 0 zodat check_calibration_flash synchroon loopt
        self.waveform_play_position = 0.0;

        // Gebruik een stille buffer i.p.v. de muziek, zodat je alleen de clicks hoort
        let sr = self.waveform_state.sample_rate;
        let total_secs = cal_positions.last().copied().unwrap_or(8.0) + 1.0;
        let silent_samples = Arc::new(vec![0.0f32; (sr as f32 * total_secs) as usize]);

        let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
            samples: silent_samples,
            sample_rate: sr,
            start_sample: 0,
            segment_start_sec: 0.0,
            a_sample: 0,
            b_sample: 0,
            pitch_semitones: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(f32::to_bits(
                self.waveform_state.pitch_semitones,
            ))),
            tempo: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(f32::to_bits(
                self.waveform_state.tempo,
            ))),
            click_positions: self.click_positions.clone(),
            click_enabled: self.click_enabled.clone(),
        });
        self.waveform_is_playing = true;
        self.waveform_has_content = true;
        self.loop_iteration_count = 1;

        self.status_message = format!(
            "\u{26A1} Kalibratie: {} clicks om de {:.0}s — pas latency aan tot \
             flits-ritme en click-ritme synchroon zijn",
            cal_positions.len(),
            interval
        );
        self.status_message_timer = 20 * 60;
    }

    /// Aanroepen vanuit de update-loop: controleert of de playhead een
    /// calibratie-positie heeft bereikt en flitst dan.
    /// Dit geeft een ritmisch flitspatroon dat je kunt vergelijken met
    /// de hoorbare clicks.
    pub fn check_calibration_flash(&mut self) {
        if self.calibration_active {
            // Check alle nog niet-geflitste posities
            // Gebruik `calibration_active` i.p.v. `waveform_is_playing`
            // omdat de Playing event nog onderweg kan zijn.
            while self.calibration_next_idx < self.calibration_click_positions.len() {
                let pos = self.calibration_click_positions[self.calibration_next_idx];
                if self.waveform_play_position >= pos - 0.02 {
                    // Klein beetje marge (-20ms) zodat we de flits niet missen
                    self.calibration_flash = 15;
                    self.calibration_next_idx += 1;
                } else {
                    break;
                }
            }

            // Als alle posities geweest zijn, deactiveer
            if self.calibration_next_idx >= self.calibration_click_positions.len() {
                self.calibration_active = false;
                self.status_message =
                    "Kalibratie voltooid — pas latency aan en test opnieuw".to_string();
                self.status_message_timer = 5 * 60;
            }
        }

        // Verval de flits
        if self.calibration_flash > 0 {
            self.calibration_flash -= 1;
        }
    }

    /// Werk de click-posities bij op basis van de huidige instellingen.
    pub fn update_click_positions(&mut self) {
        let positions: Vec<f32> = if self.click_on_bpm {
            // Gebruik BPM beat posities (met offset correctie)
            self.bpm_beat_positions
                .as_ref()
                .map(|beats| {
                    beats
                        .iter()
                        .filter(|(_, strength)| *strength >= self.bpm_threshold)
                        .map(|(pos, _)| (pos + self.beat_offset_ms / 1000.0).max(0.0))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            // Gebruik handmatige markers (alleen beat markers, of alle markers)
            self.waveform_state
                .markers
                .iter()
                .map(|m| m.position_secs)
                .collect()
        };

        *self.click_positions.lock().unwrap() = positions;
    }

    /// Schakel de beat-audit modus aan/uit.
    pub fn toggle_beat_audit(&mut self) {
        let currently_on = self
            .click_enabled
            .load(std::sync::atomic::Ordering::Relaxed);
        let new_state = !currently_on;
        self.click_enabled
            .store(new_state, std::sync::atomic::Ordering::Relaxed);
        if new_state {
            self.update_click_positions();
            self.status_message = format!(
                "\u{1F50A} Beat audit aan — clicks op {}",
                if self.click_on_bpm {
                    "BPM beats"
                } else {
                    "markers"
                }
            );
        } else {
            self.status_message = "\u{1F507} Beat audit uit".to_string();
        }
        self.status_message_timer = 3 * 60;
    }
}
