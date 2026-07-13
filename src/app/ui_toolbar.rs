use std::collections::HashSet;

use crate::app::LoopEditorApp;
use crate::app::UndoState;
use crate::shortcuts::ToolbarAction;
use crate::waveform_player::WaveformCommand;
use eframe::egui::{self, Color32, RichText};

impl LoopEditorApp {
    // ───────────────────────────────────────────────
    // Export logic
    // ───────────────────────────────────────────────

    /// Render een toolbar-knop voor de gegeven actie en voer deze uit bij klik.
    pub(crate) fn toolbar_button(&mut self, ui: &mut egui::Ui, action: ToolbarAction) {
        // Bepaal of de actie beschikbaar is in de huidige context
        let available = match action {
            ToolbarAction::ExtendBeats => {
                let beat_count = self
                    .waveform_state
                    .markers
                    .iter()
                    .filter(|m| m.kind == crate::waveform::MarkerKind::Beat)
                    .count();
                beat_count >= 2
            }
            ToolbarAction::ClearLoop => self.waveform_state.loop_a_secs.is_some(),
            ToolbarAction::Undo => !self.undo_stack.is_empty(),
            ToolbarAction::Redo => !self.redo_stack.is_empty(),
            ToolbarAction::SaveLoop => {
                self.waveform_state.loop_a_secs.is_some() && self.waveform_state.path.is_some()
            }
            ToolbarAction::CenterLoop => self.waveform_state.path.is_some(),
            ToolbarAction::ZoomIn | ToolbarAction::ZoomOut | ToolbarAction::ResetZoom => {
                self.waveform_state.path.is_some()
            }
            ToolbarAction::PlaceBeats
            | ToolbarAction::TempoDown
            | ToolbarAction::TempoUp
            | ToolbarAction::PitchDown
            | ToolbarAction::PitchUp => self.waveform_state.path.is_some(),
            ToolbarAction::Detect => {
                self.waveform_state.path.is_some() && self.waveform_state.loop_a_secs.is_some()
            }
            ToolbarAction::Export => {
                self.waveform_state.path.is_some()
                    && !self
                        .library
                        .track_for_path(self.waveform_state.path.as_ref().unwrap())
                        .loops
                        .is_empty()
            }
            _ => true, // ToggleArranger, Setup, ToggleAudit altijd beschikbaar
        };

        let label = if action == ToolbarAction::ToggleAudit {
            let audit_on = self
                .click_enabled
                .load(std::sync::atomic::Ordering::Relaxed);
            if audit_on {
                format!("{} Audit: AAN", action.icon())
            } else {
                format!("{} Audit", action.icon())
            }
        } else if action == ToolbarAction::ToggleArranger {
            "ARR".to_string()
        } else if action == ToolbarAction::Detect {
            format!("{} Detecteer", action.icon())
        } else if action == ToolbarAction::ExtendBeats {
            format!("{} Verleng beats", action.icon())
        } else if action == ToolbarAction::ClearLoop {
            format!("{} Wis loop", action.icon())
        } else if action == ToolbarAction::Undo {
            "\u{21A9} Undo".to_string()
        } else if action == ToolbarAction::Redo {
            "\u{21AA} Redo".to_string()
        } else if action == ToolbarAction::Setup {
            format!("{} Setup", action.icon())
        } else {
            format!("{} {}", action.icon(), action.display_name())
        };

        // Bouw hover-text met eventuele sneltoets
        let hover = if let Some(sa) = action.shortcut_action() {
            if let Some(binding) = self.shortcuts.binding_for(sa) {
                format!("{}  ({})", action.hover_text(), binding.display())
            } else {
                action.hover_text().to_string()
            }
        } else {
            action.hover_text().to_string()
        };

        let resp = ui
            .add_enabled(available, egui::Button::new(label))
            .on_hover_text(hover);

        if resp.clicked() {
            self.execute_toolbar_action(action);
        }
    }

    /// Voer een toolbar-actie uit.
    pub(crate) fn execute_toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::Detect => self.run_detection(),
            ToolbarAction::ExtendBeats => self.extend_beat_markers(),
            ToolbarAction::ClearLoop => {
                self.waveform_state.loop_a_secs = None;
                self.waveform_state.loop_b_secs = None;
                self.pending_loop_point = None;
                self.push_undo();
                self.send_cmd(WaveformCommand::SetLoopBounds {
                    a_secs: 0.0,
                    b_secs: 0.0,
                });
                self.status_message = "Loop gewist".to_string();
                self.status_message_timer = 2 * 60;
            }
            ToolbarAction::Undo => {
                if let Some(state) = self.undo_stack.pop() {
                    self.redo_stack.push(UndoState::snapshot_from(self));
                    self.restore_undo(state);
                }
            }
            ToolbarAction::Redo => {
                if let Some(state) = self.redo_stack.pop() {
                    self.undo_stack.push(UndoState::snapshot_from(self));
                    self.restore_undo(state);
                }
            }
            ToolbarAction::SaveLoop => {
                self.save_current_loop();
            }
            ToolbarAction::CenterLoop => {
                let vp = self.last_panel_width.max(100.0);
                self.center_view_on_loop(vp);
                self.status_message = "Weergave gecentreerd op loop".to_string();
                self.status_message_timer = 2 * 60;
            }
            ToolbarAction::ZoomIn => {
                self.push_undo();
                self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).min(5000.0);
            }
            ToolbarAction::ZoomOut => {
                self.push_undo();
                self.waveform_state.zoom = (self.waveform_state.zoom / 1.3).max(5.0);
            }
            ToolbarAction::ResetZoom => {
                self.push_undo();
                self.waveform_state.zoom = 100.0;
                self.waveform_state.scroll_offset = 0.0;
            }
            ToolbarAction::TempoDown => {
                self.push_undo();
                self.waveform_state.tempo = (self.waveform_state.tempo / 1.1).max(0.1);
                if self.waveform_is_playing {
                    self.send_cmd(WaveformCommand::SetTempo(self.waveform_state.tempo));
                }
                self.status_message = format!("Tempo: {:.0}%", self.waveform_state.tempo * 100.0);
                self.status_message_timer = 2 * 60;
            }
            ToolbarAction::TempoUp => {
                self.push_undo();
                self.waveform_state.tempo = (self.waveform_state.tempo * 1.1).min(3.0);
                if self.waveform_is_playing {
                    self.send_cmd(WaveformCommand::SetTempo(self.waveform_state.tempo));
                }
                self.status_message = format!("Tempo: {:.0}%", self.waveform_state.tempo * 100.0);
                self.status_message_timer = 2 * 60;
            }
            ToolbarAction::PitchDown => {
                self.push_undo();
                self.waveform_state.pitch_semitones =
                    (self.waveform_state.pitch_semitones - 1.0).max(-12.0);
                if self.waveform_is_playing {
                    self.send_cmd(WaveformCommand::SetPitch(
                        self.waveform_state.pitch_semitones,
                    ));
                }
                self.status_message = format!(
                    "Pitch: {:+.0} semitones",
                    self.waveform_state.pitch_semitones
                );
                self.status_message_timer = 2 * 60;
            }
            ToolbarAction::PitchUp => {
                self.push_undo();
                self.waveform_state.pitch_semitones =
                    (self.waveform_state.pitch_semitones + 1.0).min(12.0);
                if self.waveform_is_playing {
                    self.send_cmd(WaveformCommand::SetPitch(
                        self.waveform_state.pitch_semitones,
                    ));
                }
                self.status_message = format!(
                    "Pitch: {:+.0} semitones",
                    self.waveform_state.pitch_semitones
                );
                self.status_message_timer = 2 * 60;
            }
            ToolbarAction::PlaceBeats => {
                let pos = self.waveform_play_position;
                let kind = crate::waveform::MarkerKind::Beat;
                let existing = self
                    .waveform_state
                    .markers
                    .iter()
                    .position(|m| m.kind == kind && (m.position_secs - pos).abs() < 0.05);
                if let Some(idx) = existing {
                    self.waveform_state.markers.remove(idx);
                } else {
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: "B".to_string(),
                        position_secs: pos,
                        kind,
                    });
                    self.waveform_state
                        .markers
                        .sort_by(|a, b| a.position_secs.partial_cmp(&b.position_secs).unwrap());
                }
            }
            ToolbarAction::ToggleArranger => {
                self.show_arranger ^= true;
            }
            ToolbarAction::Export => {
                self.open_export_window();
            }
            ToolbarAction::Setup => {
                self.show_setup ^= true;
            }
            ToolbarAction::ToggleAudit => {
                self.toggle_beat_audit();
            }
        }
    }

    /// Toon het toolbar-editor venster voor het aanpassen van knoppen.
    pub(crate) fn show_toolbar_editor_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_toolbar_editor;
        let mut close = false;
        {
            // Neem een clone zodat we de borrow op `self` later vrijgeven
            let mut scratch = self.toolbar_buttons.clone();
            let mut changed = false;

            egui::Window::new("Toolbar aanpassen")
                .id(egui::Id::new("toolbar_editor"))
                .open(&mut open)
                .default_size([420.0, 400.0])
                .resizable(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Gebruik pijltjes om de toolbar-knoppen te ordenen")
                                .size(12.0)
                                .color(Color32::GRAY),
                        );
                    });
                    ui.separator();

                    let mut remove_idx: Option<usize> = None;

                    egui::ScrollArea::vertical()
                        .id_source("toolbar_editor_scroll")
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Actieve knoppen (geordend):")
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.add_space(4.0);

                            let mut i = 0;
                            while i < scratch.len() {
                                let action = scratch[i];
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    if i > 0 {
                                        if ui
                                            .button("\u{25B2}")
                                            .on_hover_text("Naar boven")
                                            .clicked()
                                        {
                                            scratch.swap(i, i - 1);
                                            changed = true;
                                        }
                                    } else {
                                        ui.add_enabled(false, egui::Button::new("\u{25B2}"));
                                    }
                                    if i + 1 < scratch.len() {
                                        if ui
                                            .button("\u{25BC}")
                                            .on_hover_text("Naar beneden")
                                            .clicked()
                                        {
                                            scratch.swap(i, i + 1);
                                            changed = true;
                                        }
                                    } else {
                                        ui.add_enabled(false, egui::Button::new("\u{25BC}"));
                                    }
                                    if ui
                                        .button("\u{2716}")
                                        .on_hover_text("Verwijder uit toolbar")
                                        .clicked()
                                    {
                                        remove_idx = Some(i);
                                    }
                                    ui.label(format!(
                                        "{} {}",
                                        action.icon(),
                                        action.display_name()
                                    ));
                                });
                                i += 1;
                            }

                            if let Some(idx) = remove_idx {
                                scratch.remove(idx);
                                changed = true;
                            }

                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(8.0);

                            ui.label(
                                RichText::new("Beschikbare acties (klik om toe te voegen):")
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.add_space(4.0);

                            let in_toolbar: HashSet<ToolbarAction> =
                                scratch.iter().copied().collect();

                            for action in ToolbarAction::all() {
                                if in_toolbar.contains(action) {
                                    continue;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(format!(
                                            "{}  {}  — {}",
                                            action.icon(),
                                            action.display_name(),
                                            action.hover_text()
                                        ))
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    scratch.push(*action);
                                    changed = true;
                                }
                            }
                        });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Standaard instellen").clicked() {
                            scratch = ToolbarAction::default_toolbar();
                            changed = true;
                        }
                        if ui.button("Alles wissen").clicked() {
                            scratch.clear();
                            changed = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Sluiten").clicked() {
                                close = true;
                            }
                        });
                    });
                });

            if changed {
                self.toolbar_buttons = scratch;
                self.save_session();
            }
        }

        if !open || close {
            self.show_toolbar_editor = false;
        }
    }

    /// Render de bovenste werkbalk (bestand openen, kanaalmodus, etc.)
    pub(crate) fn show_file_toolbar(&mut self, ctx: &egui::Context) {
        use crate::waveform::ChannelMode;

        egui::TopBottomPanel::top("file_toolbar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("\u{1F4C2} Open bestand").clicked() {
                    self.file_dialog.select_file();
                }

                // Kanaal modus dropdown
                let old_mode = self.waveform_state.channel_mode;
                egui::ComboBox::from_id_source("channel_mode")
                    .selected_text(old_mode.display())
                    .show_ui(ui, |ui| {
                        for &mode in &[
                            ChannelMode::Mono,
                            ChannelMode::Left,
                            ChannelMode::Right,
                            ChannelMode::Mid,
                            ChannelMode::Side,
                        ] {
                            if ui
                                .selectable_label(
                                    self.waveform_state.channel_mode == mode,
                                    mode.display(),
                                )
                                .clicked()
                            {
                                self.waveform_state.channel_mode = mode;
                            }
                        }
                    });
                if self.waveform_state.channel_mode != old_mode {
                    // Bewaar loop bounds vóór herladen (load_file wist ze)
                    let saved_a = self.waveform_state.loop_a_secs;
                    let saved_b = self.waveform_state.loop_b_secs;
                    if let Some(ref path) = self.waveform_state.path.clone() {
                        self.load_file(path);
                    }
                    // Herstel loop bounds zodat A-B markers zichtbaar blijven
                    self.waveform_state.loop_a_secs = saved_a;
                    self.waveform_state.loop_b_secs = saved_b;
                    // Stuur loop bounds opnieuw naar audio-thread
                    if saved_a.is_some() || saved_b.is_some() {
                        self.sync_loop_bounds();
                    }
                    self.save_session();
                }

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut self.file_path)
                        .hint_text("Pad naar audiobestand...")
                        .desired_width(500.0),
                );

                // Ook laden als Enter wordt ingedrukt in het tekstveld
                if resp.has_focus() {
                    let enter = ui
                        .ctx()
                        .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    if enter {
                        let path = self.file_path.trim().to_string();
                        if !path.is_empty() {
                            self.load_file(&path);
                        }
                    }
                }

                ui.label(
                    RichText::new("(of sleep een bestand in het venster)")
                        .size(11.0)
                        .color(Color32::GRAY),
                );

                // ── Rechterkant knoppen + status ──
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("ARR").clicked() {
                        self.show_arranger ^= true;
                    }
                    if self.waveform_state.path.is_some() {
                        let track = self
                            .library
                            .track_for_path(self.waveform_state.path.as_ref().unwrap());
                        if !track.loops.is_empty() {
                            if ui
                                .button("\u{1F4E4} Export")
                                .on_hover_text("Exporteer loops naar WAV (Ctrl+E)")
                                .clicked()
                            {
                                self.open_export_window();
                            }
                        }
                    }
                    if !self.status_message.is_empty() {
                        ui.label(
                            RichText::new(&self.status_message)
                                .size(12.0)
                                .color(Color32::from_rgb(100, 200, 100)),
                        );
                    }
                });
            });
            ui.add_space(4.0);
        });
    }

    /// Render de actie-werkbalk (onderste toolbar met knoppen).
    pub(crate) fn show_action_toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("action_toolbar").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                // Dynamische toolbar: loop over de geconfigureerde knoppen
                let actions = self.toolbar_buttons.clone();
                for action in &actions {
                    self.toolbar_button(ui, *action);
                }

                // Wis markers (dropdown) — staat altijd rechts van de toolbar
                let has_markers = !self.waveform_state.markers.is_empty();
                if has_markers {
                    ui.add_space(8.0);
                    ui.menu_button("✕ Wis markers", |ui| {
                        if ui.button("Alle markers").clicked() {
                            self.clear_markers_by_kind(None);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Beat-markers (B)").clicked() {
                            self.clear_markers_by_kind(Some(crate::waveform::MarkerKind::Beat));
                            ui.close_menu();
                        }
                        if ui.button("Maat-markers (M)").clicked() {
                            self.clear_markers_by_kind(Some(crate::waveform::MarkerKind::Measure));
                            ui.close_menu();
                        }
                        if ui.button("Sectie-markers (S)").clicked() {
                            self.clear_markers_by_kind(Some(crate::waveform::MarkerKind::Section));
                            ui.close_menu();
                        }
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Kebab-menu voor toolbar aanpassen
                    let kebab_resp = ui
                        .add(
                            egui::Button::new("\u{22EE}").frame(false), // ⋮ vertical ellipsis
                        )
                        .on_hover_text("Toolbar aanpassen...");
                    if kebab_resp.clicked() {
                        self.show_toolbar_editor = true;
                    }

                    if !self.status_message.is_empty() {
                        ui.label(
                            RichText::new(&self.status_message)
                                .size(12.0)
                                .color(Color32::from_rgb(100, 200, 100)),
                        );
                    }
                });
            });
            ui.add_space(2.0);
        });

        // ── Toolbar editor venster ──
        if self.show_toolbar_editor {
            self.show_toolbar_editor_window(ctx);
        }
    }
}
