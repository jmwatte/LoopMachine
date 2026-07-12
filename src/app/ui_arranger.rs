use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use eframe::egui::{self, Color32, RichText};

use crate::app::LoopEditorApp;
use crate::arrangement::Arrangement;
use crate::waveform_player::WaveformCommand;

impl LoopEditorApp {
    pub fn show_arranger_ui(&mut self, ctx: &egui::Context) {
        if !self.show_arranger {
            return;
        }

        let mut needs_save = false;
        let mut play_requested = false;
        let mut stop_requested = false;
        let playback_idx = self.active_arrangement;

        egui::Window::new("Arranger")
            .id(egui::Id::new("arranger_window"))
            .default_size([550.0, 400.0])
            .show(ctx, |ui| {
                // ── Bovenste balk ──
                ui.horizontal(|ui| {
                    if ui.button("🔙").clicked() {
                        self.show_arranger = false;
                        return;
                    }

                    let sel_name = self
                        .active_arrangement
                        .and_then(|i| self.arrangements.get(i))
                        .map(|a| a.name.clone())
                        .unwrap_or_default();

                    egui::ComboBox::from_id_source("arrangement_select")
                        .selected_text(sel_name)
                        .show_ui(ui, |ui| {
                            for (i, arr) in self.arrangements.iter().enumerate() {
                                if ui
                                    .selectable_label(self.active_arrangement == Some(i), &arr.name)
                                    .clicked()
                                {
                                    self.active_arrangement = Some(i);
                                    self.arr_current_step = None;
                                }
                            }
                        });

                    if ui.button("➕ Nieuw").clicked() {
                        let count = self.arrangements.len() + 1;
                        self.arrangements.push(Arrangement {
                            name: format!("Arrangement {}", count),
                            steps: Vec::new(),
                        });
                        self.active_arrangement = Some(self.arrangements.len() - 1);
                        needs_save = true;
                    }

                    if self.active_arrangement.is_some() {
                        if ui.button("❌").clicked() {
                            if let Some(idx) = self.active_arrangement {
                                self.arrangements.remove(idx);
                                self.active_arrangement = None;
                                self.arr_current_step = None;
                                needs_save = true;
                            }
                        }
                    }
                });

                ui.separator();

                // ── Inhoud ──
                if let Some(a_idx) = self.active_arrangement {
                    // Naam + Play/Stop
                    {
                        let arr = &mut self.arrangements[a_idx];
                        ui.horizontal(|ui| {
                            ui.label("Naam:");
                            ui.add(egui::TextEdit::singleline(&mut arr.name).desired_width(200.0));
                            if ui.button("▶ Play").clicked() {
                                play_requested = true;
                            }
                            if ui.button("⏹ Stop").clicked() {
                                stop_requested = true;
                            }
                            if let Some(step) = self.arr_current_step {
                                ui.label(
                                    RichText::new(format!("Stap {}/{}", step + 1, arr.steps.len()))
                                        .color(Color32::YELLOW)
                                        .size(14.0),
                                );
                            }
                        });
                    }

                    // Play via app method — kan niet in closure met get_mut
                    // Dus: pak de arr_idx voor gebruik na closure
                    let play_arr_idx = self.active_arrangement;

                    ui.separator();

                    // Step lijst: clone data voor weergave, pas later mutaties toe
                    let steps_data: Vec<_> = self.arrangements[a_idx]
                        .steps
                        .iter()
                        .map(|s| {
                            (
                                s.loop_id.clone(),
                                s.track_path.clone(),
                                s.color,
                                s.repeats,
                                s.pitch_semitones,
                                s.tempo,
                            )
                        })
                        .collect();

                    let mut remove_idx: Option<usize> = None;
                    let mut move_up_idx: Option<usize> = None;
                    let mut move_down_idx: Option<usize> = None;
                    let mut changes: Vec<(usize, u32)> = Vec::new();

                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .id_source("arr_steps")
                        .show(ui, |ui| {
                            for (i, (id, tpath, color, repeats, pitch, tempo)) in
                                steps_data.iter().enumerate()
                            {
                                let is_current = self.arr_current_step == Some(i);
                                let bg = if is_current {
                                    egui::Color32::from_rgba_premultiplied(60, 60, 80, 255)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                let c = egui::Color32::from_rgb(color[0], color[1], color[2]);

                                let step_id = id.clone();
                                let step_tpath = tpath.clone();
                                let step_pitch = *pitch;
                                let step_tempo = *tempo;

                                egui::Frame::none().fill(bg).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let _ = egui::Frame::none().fill(c).show(ui, |ui| {
                                            ui.set_min_size(egui::vec2(12.0, 12.0));
                                        });

                                        // Label
                                        let label = self
                                            .library
                                            .tracks
                                            .iter()
                                            .find(|t| t.track_path == step_tpath)
                                            .and_then(|t| {
                                                t.loops.iter().find(|l| {
                                                    l.short_id.as_deref() == Some(&step_id)
                                                })
                                            })
                                            .map(|l| format!("({}) {}", step_id, l.label))
                                            .unwrap_or_else(|| format!("({})", step_id));
                                        ui.label(label);

                                        // Preview
                                        if ui.small_button(">").clicked() {
                                            if let Some(ref path) = self.waveform_state.path {
                                                if *path == step_tpath {
                                                    for track in &self.library.tracks {
                                                        if track.track_path == step_tpath {
                                                            if let Some(saved) =
                                                                track.loops.iter().find(|l| {
                                                                    l.short_id.as_deref()
                                                                        == Some(&step_id)
                                                                })
                                                            {
                                                                let sr =
                                                                    self.waveform_state.sample_rate;
                                                                let a = (saved.loop_a_secs
                                                                    * sr as f32)
                                                                    as usize;
                                                                let b = (saved.loop_b_secs
                                                                    * sr as f32)
                                                                    as usize;
                                                                let _ = self.waveform_cmd_tx.send(
                                                                    WaveformCommand::Play {
                                                                        samples: self
                                                                            .waveform_state
                                                                            .samples
                                                                            .clone(),
                                                                        sample_rate: sr,
                                                                        start_sample: a,
                                                                        segment_start_sec: 0.0,
                                                                        a_sample: a,
                                                                        b_sample: b,
                                                                        pitch_semitones: Arc::new(
                                                                            AtomicU32::new(
                                                                                f32::to_bits(
                                                                                    step_pitch,
                                                                                ),
                                                                            ),
                                                                        ),
                                                                        tempo: Arc::new(
                                                                            AtomicU32::new(
                                                                                f32::to_bits(
                                                                                    step_tempo,
                                                                                ),
                                                                            ),
                                                                        ),
                                                                        click_positions: self
                                                                            .click_positions
                                                                            .clone(),
                                                                        click_enabled: self
                                                                            .click_enabled
                                                                            .clone(),
                                                                    },
                                                                );
                                                                self.arr_current_step = None;
                                                            }
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // Herhalingen
                                        ui.add_space(10.0);
                                        let mut r = *repeats;
                                        if ui.small_button("−").clicked() && r > 1 {
                                            r -= 1;
                                            changes.push((i, r));
                                        }
                                        ui.label(format!("x{}", r));
                                        if ui.small_button("+").clicked() {
                                            r += 1;
                                            changes.push((i, r));
                                        }

                                        if ui.small_button("X").clicked() {
                                            remove_idx = Some(i);
                                        }
                                        if i > 0 && ui.small_button("^").clicked() {
                                            move_up_idx = Some(i);
                                        }
                                        if i + 1 < steps_data.len()
                                            && ui.small_button("v").clicked()
                                        {
                                            move_down_idx = Some(i);
                                        }
                                    });
                                });
                            }
                        });

                    // Mutaties toepassen na closure
                    for (idx, new_r) in changes {
                        if idx < self.arrangements[a_idx].steps.len() {
                            self.arrangements[a_idx].steps[idx].repeats = new_r;
                            needs_save = true;
                        }
                    }
                    if let Some(idx) = remove_idx {
                        self.arrangements[a_idx].steps.remove(idx);
                        needs_save = true;
                    }
                    if let Some(idx) = move_up_idx {
                        if idx > 0 {
                            self.arrangements[a_idx].steps.swap(idx, idx - 1);
                            needs_save = true;
                        }
                    }
                    if let Some(idx) = move_down_idx {
                        if idx + 1 < self.arrangements[a_idx].steps.len() {
                            self.arrangements[a_idx].steps.swap(idx, idx + 1);
                            needs_save = true;
                        }
                    }

                    ui.separator();

                    // ── Parse ──
                    let mut do_parse = false;
                    ui.horizontal(|ui| {
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.arr_parse_buf)
                                .hint_text("2b3A5C")
                                .desired_width(120.0),
                        );
                        if resp.lost_focus() && !self.arr_parse_buf.is_empty() {
                            do_parse = true;
                            self.save_session();
                        }
                        if ui.button("Parse").clicked() && !self.arr_parse_buf.is_empty() {
                            do_parse = true;
                        }
                    });

                    if do_parse {
                        let buf = self.arr_parse_buf.clone();
                        if let Ok(parsed) = crate::arrangement::parse_arranger_string(&buf) {
                            for (pid, prepeats) in parsed {
                                for track in &self.library.tracks {
                                    if let Some(ld) = track
                                        .loops
                                        .iter()
                                        .find(|l| l.short_id.as_deref() == Some(&pid))
                                    {
                                        let color = crate::arrangement::color_for_arranger(
                                            &pid,
                                            &track.track_path,
                                        );
                                        self.arrangements[a_idx].steps.push(
                                            crate::arrangement::ArrStep {
                                                loop_id: pid,
                                                track_path: track.track_path.clone(),
                                                repeats: prepeats,
                                                pitch_semitones: ld.pitch_semitones,
                                                tempo: ld.tempo,
                                                color,
                                            },
                                        );
                                        needs_save = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // ── Voeg toe ──
                    ui.label("Toevoegen:");
                    egui::ScrollArea::vertical()
                        .id_source("arr_add_loops")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for track in &self.library.tracks {
                                    for loop_data in &track.loops {
                                        let sid = loop_data
                                            .short_id
                                            .clone()
                                            .unwrap_or_else(|| "?".to_string());
                                        let lbl = std::path::Path::new(&track.track_path)
                                            .file_stem()
                                            .map(|s| {
                                                format!(
                                                    "({}) {} - {}",
                                                    sid,
                                                    s.to_string_lossy(),
                                                    loop_data.label
                                                )
                                            })
                                            .unwrap_or_else(|| {
                                                format!("({}) {}", sid, loop_data.label)
                                            });
                                        if ui.small_button(&lbl).clicked() {
                                            let color = crate::arrangement::color_for_arranger(
                                                &sid,
                                                &track.track_path,
                                            );
                                            self.arrangements[a_idx].steps.push(
                                                crate::arrangement::ArrStep {
                                                    loop_id: sid.clone(),
                                                    track_path: track.track_path.clone(),
                                                    repeats: 1,
                                                    pitch_semitones: loop_data.pitch_semitones,
                                                    tempo: loop_data.tempo,
                                                    color,
                                                },
                                            );
                                            needs_save = true;
                                        }
                                    }
                                }
                            });
                        });

                    // Play (na de closures zodat er geen borrow-conflicten zijn)
                    if play_arr_idx.is_some() && false {
                        // handled below
                    }
                } else if self.arrangements.is_empty() {
                    ui.label("Geen arrangementen. Klik '➕ Nieuw' om te beginnen.");
                } else {
                    ui.label("Selecteer een arrangement.");
                }
            });

        // Play/Stop na window (buiten borrow-conflicten)
        if play_requested {
            if let Some(idx) = playback_idx {
                self.play_arrangement(idx);
            }
        }
        if stop_requested {
            let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
            self.arr_current_step = None;
        }
        if needs_save {
            crate::arrangement::save_arrangements(&self.arrangements);
        }
    }
}
