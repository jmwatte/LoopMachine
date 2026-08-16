use eframe::egui::{self, Color32, RichText};
use std::path::Path;

use crate::app::LoopEditorApp;
use crate::arrangement::color_for_arranger;
use crate::waveform_player::WaveformCommand;

impl LoopEditorApp {
    pub fn show_library_window(&mut self, ctx: &egui::Context) {
        if !self.show_loop_library {
            return;
        }
        egui::Window::new("📚 Alle Tracks")
            .id(egui::Id::new("loop_library_window"))
            .resizable(true)
            .default_size([500.0, 400.0])
            .show(ctx, |ui| {
                if self.library.tracks.is_empty() {
                    ui.label("Nog geen tracks. Laad een audiobestand en maak loops.");
                } else {
                    let mut delete_loop_op: Option<(usize, usize)> = None;
                    let _delete_track_op: Option<usize> = None;
                    let mut load_loop_op: Option<(usize, usize)> = None;
                    let mut relink_op: Option<usize> = None;
                    let mut drive_swap_op: Option<(usize, String)> = None;

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (ti, track) in self.library.tracks.iter().enumerate() {
                            let has_notes = track.loops.iter().any(|l| !l.notes.is_empty());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("🎵 {}", track.label))
                                        .size(14.0)
                                        .strong(),
                                );
                                if has_notes {
                                    ui.label(RichText::new("📝").size(14.0).color(Color32::GRAY));
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("❌").clicked() {
                                            self.confirm_delete_track =
                                                Some((ti, track.label.clone()));
                                        }
                                        if ui.small_button("▶").clicked() {
                                            load_loop_op = Some((ti, 0)); // load track, eerste loop
                                        }
                                    },
                                );
                            });

                            // ── Missend bestand? Bied relink aan ──
                            if !Path::new(&track.track_path).exists() {
                                ui.indent(egui::Id::new(("relink", ti)), |ui| {
                                    ui.colored_label(
                                        Color32::from_rgb(255, 100, 100),
                                        "⛔ Bestand niet gevonden",
                                    );
                                    ui.label(
                                        RichText::new(&track.track_path)
                                            .size(10.0)
                                            .color(Color32::from_gray(140)),
                                    );
                                    ui.horizontal(|ui| {
                                        if ui
                                            .small_button("🔗 Relink…")
                                            .on_hover_text("Kies het nieuwe pad voor dit bestand")
                                            .clicked()
                                        {
                                            relink_op = Some(ti);
                                        }
                                        let cands =
                                            crate::loops::drive_swap_candidates(&track.track_path);
                                        if cands.len() == 1 {
                                            if let Some(cand) = cands.first() {
                                                if let Some((_, drive, _)) =
                                                    crate::loops::split_drive(cand)
                                                {
                                                    if ui
                                                        .small_button(format!(
                                                            "💡 Gebruik station {}",
                                                            drive
                                                        ))
                                                        .on_hover_text(cand.clone())
                                                        .clicked()
                                                    {
                                                        drive_swap_op = Some((ti, cand.clone()));
                                                    }
                                                }
                                            }
                                        }
                                    });
                                });
                            }

                            // Sub-lijst loops
                            if !track.loops.is_empty() {
                                for (li, saved) in track.loops.iter().enumerate() {
                                    ui.indent("loops", |ui| {
                                        ui.horizontal(|ui| {
                                            // Toon short_id met gekleurd blokje
                                            let id_str = saved
                                                .short_id
                                                .as_deref()
                                                .map(|id| format!("({}) ", id))
                                                .unwrap_or_default();
                                            let col = saved.short_id.as_deref().map(|id| {
                                                color_for_arranger(id, &track.track_path)
                                            });
                                            if let Some([r, g, b]) = col {
                                                let color = Color32::from_rgb(r, g, b);
                                                egui::Frame::default()
                                                    .fill(color)
                                                    .stroke(egui::Stroke::new(
                                                        1.0,
                                                        Color32::from_gray(80),
                                                    ))
                                                    .show(ui, |ui| {
                                                        ui.set_min_size(egui::vec2(10.0, 10.0));
                                                    });
                                            }
                                            ui.label(
                                                RichText::new(format!("{}{}", id_str, saved.label))
                                                    .size(12.0),
                                            );
                                            ui.label(
                                                RichText::new(format!(
                                                    "{:02}:{:02} → {:02}:{:02}",
                                                    (saved.loop_a_secs / 60.0) as u32,
                                                    saved.loop_a_secs as u32 % 60,
                                                    (saved.loop_b_secs / 60.0) as u32,
                                                    saved.loop_b_secs as u32 % 60,
                                                ))
                                                .size(11.0)
                                                .color(Color32::GRAY),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui.small_button("❌").clicked() {
                                                        delete_loop_op = Some((ti, li));
                                                    }
                                                    if ui.small_button("▶").clicked() {
                                                        load_loop_op = Some((ti, li));
                                                    }
                                                },
                                            );
                                        });
                                    });
                                }
                            } else {
                                ui.indent("loops", |ui| {
                                    ui.label(
                                        RichText::new("  (geen loops)")
                                            .size(11.0)
                                            .color(Color32::GRAY),
                                    );
                                });
                            }
                            ui.separator();
                        }
                    });

                    // ── Verwerk operaties buiten de iterator ──
                    if let Some((ti, li)) = delete_loop_op {
                        if ti < self.library.tracks.len() {
                            self.library.tracks[ti].loops.remove(li);
                            crate::loops::save_library(&self.library);
                        }
                    }

                    if let Some((ti, li)) = load_loop_op {
                        // Clone eerst alle data die we nodig hebben
                        let (track_path, saved) = {
                            let track = &self.library.tracks[ti];
                            let path = track.track_path.clone();
                            let saved = track.loops.get(li).cloned();
                            (path, saved)
                        };

                        // Laad de track altijd (ook als er geen loops zijn)
                        if self.waveform_state.path.as_deref() != Some(&track_path) {
                            if self.waveform_is_playing {
                                self.send_cmd(WaveformCommand::Stop);
                                self.waveform_is_playing = false;
                            }
                            self.load_file(&track_path);
                            self.waveform_has_content = false;
                        }

                        // Als er een specifieke loop geselecteerd is, laad die dan
                        if let Some(saved) = saved {
                            self.waveform_state.loop_a_secs = Some(saved.loop_a_secs);
                            self.waveform_state.loop_b_secs = Some(saved.loop_b_secs);
                            self.waveform_state.pitch_semitones = saved.pitch_semitones;
                            self.waveform_state.tempo = saved.tempo;
                            self.waveform_play_position = saved.loop_a_secs;
                            self.waveform_state.seek_pending = Some(saved.loop_a_secs);
                            self.waveform_state.playhead_frames_after_drag = 15;

                            self.center_view_on_loop(800.0);

                            if self.waveform_is_playing {
                                self.send_cmd(WaveformCommand::SetPitch(saved.pitch_semitones));
                                self.send_cmd(WaveformCommand::SetTempo(saved.tempo));
                                self.send_cmd(WaveformCommand::SetLoopBounds {
                                    a_secs: saved.loop_a_secs,
                                    b_secs: saved.loop_b_secs,
                                });
                                self.send_cmd(WaveformCommand::Seek {
                                    pos_secs: saved.loop_a_secs,
                                });
                            }

                            // Zet actieve loop voor notities (alleen als zelfde track)
                            if self.waveform_state.path.as_deref() == Some(&track_path) {
                                self.active_loop_idx = Some(li);
                            }

                            self.center_view_on_loop(800.0);

                            self.status_message = format!("Loop '{}' geladen", saved.label);
                            self.status_message_timer = 3 * 60;
                        } else {
                            self.waveform_state.loop_a_secs = None;
                            self.waveform_state.loop_b_secs = None;
                            self.status_message = "Track geladen".to_string();
                            self.status_message_timer = 3 * 60;
                        }
                    }

                    // ── Verwerk relink-operaties ──
                    if let Some(ti) = relink_op {
                        self.pending_relink_track = Some(ti);
                        self.relink_dialog.pick_file();
                    }
                    if let Some((ti, new_path)) = drive_swap_op {
                        self.apply_relink(ti, &new_path);
                    }
                }
            });
    }

    pub fn show_confirm_delete(&mut self, ctx: &egui::Context) {
        if let Some((ti, ref name)) = self.confirm_delete_track.clone() {
            egui::Window::new("⚠ Track verwijderen")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Weet je zeker dat je track \"{}\" en al zijn loops wilt verwijderen?",
                        name
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Ja").clicked() {
                            if ti < self.library.tracks.len() {
                                self.library.tracks.remove(ti);
                                crate::loops::save_library(&self.library);
                            }
                            self.confirm_delete_track = None;
                        }
                        if ui.button("Nee").clicked() {
                            self.confirm_delete_track = None;
                        }
                    });
                });
        }
    }
}
