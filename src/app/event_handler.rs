use crate::app::{LoopEditorApp, UndoState};
use crate::shortcuts::{KeyBinding, ShortcutAction, ToolbarAction};
use crate::waveform_player::{WaveformCommand, WaveformEvent};
use eframe::egui;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

// ───────────────────────────────────────────────
// Hulpmethodes voor update() — event-loop, shortcuts, en CentralPanel
// ───────────────────────────────────────────────

impl LoopEditorApp {
    /// Verwerk alle binnenkomende events van de audio-thread.
    pub(crate) fn handle_waveform_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.waveform_event_rx.try_recv() {
            match event {
                WaveformEvent::Playing => {
                    self.waveform_is_playing = true;
                    self.waveform_has_content = true;
                    self.sync_video_playback();
                    ctx.request_repaint();
                }
                WaveformEvent::Stopped => {
                    self.waveform_is_playing = false;
                    self.waveform_has_content = false;
                    self.sync_video_playback();
                    ctx.request_repaint();
                }
                WaveformEvent::Paused => {
                    self.waveform_is_playing = false;
                    self.sync_video_playback();
                    ctx.request_repaint();
                }
                WaveformEvent::Resumed => {
                    self.waveform_is_playing = true;
                    self.sync_video_playback();
                    ctx.request_repaint();
                }
                WaveformEvent::Error(msg) => {
                    self.waveform_is_playing = false;
                    self.status_message = format!("Waveform fout: {}", msg);
                    ctx.request_repaint();
                }
                WaveformEvent::Position(pos, dur) => {
                    self.waveform_play_duration = dur;

                    let mut seek_completed = false;
                    // ✅ Check of de audio-thread de seek heeft voltooid
                    if self.waveform_state.seek_pending.is_some() {
                        if let Some(target) = self.waveform_state.seek_pending {
                            if (pos - target).abs() < 0.05 {
                                self.waveform_state.seek_pending = None;
                                seek_completed = true;
                            }
                        }
                    }

                    // ✅ Accepteer de positie ALLEEN als:
                    // 1. Er geen seek pending is (de audio is gearriveerd)
                    // 2. We niet aan het slepen zijn
                    let prev_pos = self.waveform_play_position;
                    if self.waveform_state.seek_pending.is_none()
                        && !self.waveform_state.dragging_playhead
                    {
                        self.waveform_play_position = pos;

                        // Sync video positie ALLEEN bij voltooide seek, niet bij elke frame
                        if seek_completed && self.video_player.is_some() {
                            self.sync_video_position();
                        }

                        // 🔁 Loop-wrap detectie: als de audio-thread de positie heeft
                        // teruggewrapt (modulo), seek dan ook mpv naar de start van de loop.
                        // Dit houdt de video gesynchroniseerd met audio bij elke wrap.
                        if self.video_player.is_some() && pos < prev_pos && !seek_completed {
                            if let (Some(a), Some(b)) = (
                                self.waveform_state.loop_a_secs,
                                self.waveform_state.loop_b_secs,
                            ) {
                                let loop_dur = b - a;
                                if loop_dur > 0.0
                                    && (prev_pos - pos).abs() > loop_dur * 0.3
                                    && prev_pos >= b - loop_dur * 0.2
                                {
                                    log::debug!("loop-wrap: seek video naar {:.3}s", pos);
                                    self.sync_video_position();
                                }
                            }
                        }
                    }

                    // Loop-herhaal detectie: als de positie van B terugspringt
                    // naar A (wrap), tel dan een iteratie.
                    // We gebruiken prev_pos (oude waarde) omdat play_position
                    // hierboven al is bijgewerkt naar de nieuwe positie.
                    if self.loop_repeat_count > 0 {
                        if let (Some(a), Some(b)) = (
                            self.waveform_state.loop_a_secs,
                            self.waveform_state.loop_b_secs,
                        ) {
                            let loop_dur = b - a;
                            if loop_dur > 0.0
                                && pos < prev_pos
                                && (prev_pos - pos).abs() > loop_dur * 0.5
                                // Alleen tellen als prev_pos dicht bij B was (echte wrap)
                                && prev_pos >= b - loop_dur * 0.1
                            {
                                self.loop_iteration_count += 1;
                                // Stop pas als de teller boven loop_repeat_count uitkomt.
                                // Bij 2 wil de gebruiker 2× horen: 1/2 en 2/2, dus stoppen bij 3.
                                if self.loop_iteration_count > self.loop_repeat_count {
                                    self.send_cmd(WaveformCommand::Stop);
                                    self.waveform_is_playing = false;
                                    self.status_message = format!(
                                        "Loop {}/{} — gestopt",
                                        self.loop_repeat_count, self.loop_repeat_count
                                    );
                                    self.status_message_timer = 3 * 60;
                                }
                            }
                        }
                    }

                    ctx.request_repaint();
                }
                WaveformEvent::StepChanged(idx) => {
                    self.arr_current_step = Some(idx);
                    self.waveform_is_playing = true;
                    ctx.request_repaint();
                }
                WaveformEvent::StepRepeated(idx) => {
                    self.arr_current_step = Some(idx);
                    ctx.request_repaint();
                }
                WaveformEvent::ArrangementFinished => {
                    self.arr_current_step = None;
                    self.waveform_is_playing = false;
                    self.waveform_has_content = false;
                    ctx.request_repaint();
                }
            }
        }
    }

    /// Verval statusmelding, kalibratie-flits, en repaint-aanvragen.
    pub(crate) fn housekeeping(&mut self, ctx: &egui::Context) {
        // Verval statusmelding na 5 seconden
        if self.status_message_timer > 0 {
            self.status_message_timer -= 1;
            if self.status_message_timer == 0 {
                self.status_message.clear();
            }
        }

        // ── Kalibratie flits bewaking ──
        self.check_calibration_flash();

        // 🔥 CRITICAL: Force continuous repaints while playing so the playhead moves smoothly
        if self.waveform_is_playing || self.calibration_active {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Verwerk toetsenbord shortcuts (behalve als tekstveld focus heeft).
    pub(crate) fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // ── Toetsenbord shortcuts ──
        let is_text_focused = ctx.memory(|mem| mem.focused().is_some());
        if let Some(action) = self.listening_for_action {
            if let Some(key_event) = ctx.input(|i| i.keys_down.iter().next().copied()) {
                let mods = ctx.input(|i| i.modifiers);
                let binding = KeyBinding {
                    key: key_event.into(),
                    ctrl: mods.ctrl,
                    shift: mods.shift,
                    alt: mods.alt,
                };
                // Check op conflicts
                if let Some(conflict) = self.shortcuts.find_conflict(&binding, action) {
                    self.status_message = format!(
                        "⚠ Conflict: '{}' is al gebruikt voor '{}'",
                        binding.display(),
                        conflict.display_name()
                    );
                    self.status_message_timer = 5 * 60;
                } else {
                    if let Err(e) = self.shortcuts.set_binding(action, binding) {
                        self.status_message = format!("Fout bij opslaan: {}", e);
                    } else {
                        self.status_message = format!(
                            "✓ '{}' nu gekoppeld aan '{}'",
                            binding.display(),
                            action.display_name()
                        );
                        self.status_message_timer = 3 * 60;
                    }
                }
                self.listening_for_action = None;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.listening_for_action = None;
            }
        } else if !is_text_focused {
            if self
                .shortcuts
                .is_pressed(ShortcutAction::PlayPause, &ctx.input(|i| i.clone()))
            {
                if self.waveform_has_content {
                    // Audio is geladen (speelt of gepauzeerd) → toggle
                    self.send_cmd(WaveformCommand::TogglePause);
                } else if let Some(ref _path) = self.waveform_state.path {
                    // Nog niks geladen in audio-thread → start nieuwe playback
                    let (decode_start, play_start, decode_end) = match (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        (Some(a), Some(b)) if b > a => {
                            // If looping, decode the whole loop (A to B), but start playing at the current playhead
                            let start = self.waveform_play_position.clamp(a, b);
                            (a, start, b)
                        }
                        _ => {
                            // Geen loop: decode alleen vanaf playhead, stuur a == b zodat de
                            // audio-thread weet dat er géén looping is.
                            let start = self.waveform_play_position;
                            (start, start, start) // decode_end == play_start → a_sample == b_sample
                        }
                    };

                    let sr = self.waveform_state.sample_rate as f32;
                    let start_sample = (play_start * sr) as usize;
                    let a_sample = (decode_start * sr) as usize;
                    let b_sample = (decode_end * sr) as usize;

                    self.send_cmd(WaveformCommand::Play {
                        samples: self.waveform_state.samples.clone(),
                        sample_rate: self.waveform_state.sample_rate,
                        start_sample,
                        segment_start_sec: 0.0, // ✅ De buffer begint nu bij 0.0s van de track
                        a_sample,
                        b_sample,
                        pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                            self.waveform_state.pitch_semitones,
                        ))),
                        tempo: Arc::new(AtomicU32::new(f32::to_bits(self.waveform_state.tempo))),
                        click_positions: self.click_positions.clone(),
                        click_enabled: self.click_enabled.clone(),
                    });

                    self.waveform_is_playing = true;
                    self.loop_iteration_count = 1; // 1e play-through
                }
            }

            // ── Marker shortcuts (1-9), Backspace (verwijder dichtstbijzijnde), [ ] (A-B) ──
            if self.waveform_state.path.is_some() {
                // ── Marker shortcuts: S (Section), M (Measure), B (Beat) ──
                // Alle drie werken met toggle: druk nogmaals op dezelfde plek om te verwijderen.
                let tolerance = 0.05_f32;
                // Compenseer voor audio-uitvoerlatentie tijdens afspelen
                let mut pos = self.waveform_play_position;
                if self.waveform_is_playing && self.playback_latency_ms > 0.0 {
                    pos = (pos - self.playback_latency_ms / 1000.0).max(0.0);
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddSectionMarker, &ctx.input(|i| i.clone()))
                {
                    let existing = self.waveform_state.markers.iter().position(|m| {
                        m.kind == crate::waveform::MarkerKind::Section
                            && (m.position_secs - pos).abs() < tolerance
                    });
                    if let Some(idx) = existing {
                        self.waveform_state.markers.remove(idx);
                        self.status_message = format!("Section marker verwijderd op {:.1}s", pos);
                    } else {
                        let count = self
                            .waveform_state
                            .markers
                            .iter()
                            .filter(|m| m.kind == crate::waveform::MarkerKind::Section)
                            .count()
                            + 1;
                        self.waveform_state.markers.push(crate::waveform::Marker {
                            name: format!("S{}", count),
                            position_secs: pos,
                            kind: crate::waveform::MarkerKind::Section,
                        });
                        self.status_message = format!("Section marker op {:.1}s", pos);
                    }
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message_timer = 3 * 60;
                    // Ververs click-posities voor audit
                    if !self.click_on_bpm {
                        self.update_click_positions();
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddMeasureMarker, &ctx.input(|i| i.clone()))
                {
                    let existing = self.waveform_state.markers.iter().position(|m| {
                        m.kind == crate::waveform::MarkerKind::Measure
                            && (m.position_secs - pos).abs() < tolerance
                    });
                    if let Some(idx) = existing {
                        self.waveform_state.markers.remove(idx);
                        self.status_message = format!("Measure marker verwijderd op {:.1}s", pos);
                    } else {
                        let count = self
                            .waveform_state
                            .markers
                            .iter()
                            .filter(|m| m.kind == crate::waveform::MarkerKind::Measure)
                            .count()
                            + 1;
                        self.waveform_state.markers.push(crate::waveform::Marker {
                            name: format!("M{}", count),
                            position_secs: pos,
                            kind: crate::waveform::MarkerKind::Measure,
                        });
                        self.status_message = format!("Measure marker op {:.1}s", pos);
                    }
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message_timer = 3 * 60;
                    // Ververs click-posities voor audit
                    if !self.click_on_bpm {
                        self.update_click_positions();
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddBeatMarker, &ctx.input(|i| i.clone()))
                {
                    let existing = self.waveform_state.markers.iter().position(|m| {
                        m.kind == crate::waveform::MarkerKind::Beat
                            && (m.position_secs - pos).abs() < tolerance
                    });
                    if let Some(idx) = existing {
                        self.waveform_state.markers.remove(idx);
                        self.status_message = format!("Beat marker verwijderd op {:.1}s", pos);
                    } else {
                        self.waveform_state.markers.push(crate::waveform::Marker {
                            name: "B".to_string(),
                            position_secs: pos,
                            kind: crate::waveform::MarkerKind::Beat,
                        });
                        self.status_message = format!("Beat marker op {:.1}s", pos);
                    }
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message_timer = 3 * 60;
                    // Update BPM uit markers
                    self.status_message = format!(
                        "{}  |  {}",
                        self.status_message,
                        Self::bpm_from_markers(&self.waveform_state.markers)
                    );
                    // Ververs click-posities voor audit
                    if !self.click_on_bpm {
                        self.update_click_positions();
                    }
                }

                if self.shortcuts.is_pressed(
                    ShortcutAction::DeleteNearestMarker,
                    &ctx.input(|i| i.clone()),
                ) {
                    // Check eerst of er markers geselecteerd zijn (via Shift+drag)
                    if let Some((sel_a, sel_b)) = self.waveform_state.selected_marker_range {
                        let (lo, hi) = if sel_a < sel_b {
                            (sel_a, sel_b)
                        } else {
                            (sel_b, sel_a)
                        };
                        let before = self.waveform_state.markers.len();
                        self.waveform_state
                            .markers
                            .retain(|m| m.position_secs < lo || m.position_secs > hi);
                        let removed = before - self.waveform_state.markers.len();
                        if removed > 0 {
                            self.waveform_state.selected_marker_range = None;
                            self.push_undo();
                            self.sync_markers_to_library();
                            self.status_message = format!("{} markers verwijderd", removed);
                            self.status_message_timer = 3 * 60;
                        }
                    } else {
                        // Geen selectie: verwijder dichtstbijzijnde marker bij playhead
                        let pos = self.waveform_play_position;
                        let mut best_idx: Option<usize> = None;
                        let mut best_dist = 2.0_f32;
                        for (i, m) in self.waveform_state.markers.iter().enumerate() {
                            let dist = (m.position_secs - pos).abs();
                            if dist < best_dist {
                                best_dist = dist;
                                best_idx = Some(i);
                            }
                        }
                        if let Some(idx) = best_idx {
                            let removed = self.waveform_state.markers.remove(idx);
                            self.push_undo();
                            self.sync_markers_to_library();
                            self.status_message = format!("Marker '{}' verwijderd", removed.name);
                            self.status_message_timer = 3 * 60;
                            // Ververs click-posities voor audit
                            if !self.click_on_bpm {
                                self.update_click_positions();
                            }
                        }
                    }
                }

                // ── MarkerPrev/MarkerNext — playhead naar vorige/volgende marker ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::MarkerPrev, &ctx.input(|i| i.clone()))
                {
                    let pos = self.waveform_play_position;
                    let target = self
                        .waveform_state
                        .markers
                        .iter()
                        .map(|m| m.position_secs)
                        .filter(|&p| p < pos - 0.01)
                        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    if let Some(target) = target {
                        self.waveform_play_position = target;
                        self.waveform_state.seek_pending = Some(target);
                        self.send_cmd(WaveformCommand::Seek { pos_secs: target });
                        self.waveform_state.playhead_frames_after_drag = 15;
                        self.status_message = format!("Playhead naar marker op {:.1}s", target);
                        self.status_message_timer = 2 * 60;
                    } else {
                        self.status_message = "Geen marker links van playhead".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::MarkerNext, &ctx.input(|i| i.clone()))
                {
                    let pos = self.waveform_play_position;
                    let target = self
                        .waveform_state
                        .markers
                        .iter()
                        .map(|m| m.position_secs)
                        .filter(|&p| p > pos + 0.01)
                        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    if let Some(target) = target {
                        self.waveform_play_position = target;
                        self.waveform_state.seek_pending = Some(target);
                        self.send_cmd(WaveformCommand::Seek { pos_secs: target });
                        self.waveform_state.playhead_frames_after_drag = 15;
                        self.status_message = format!("Playhead naar marker op {:.1}s", target);
                        self.status_message_timer = 2 * 60;
                    } else {
                        self.status_message = "Geen marker rechts van playhead".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SetLoopA, &ctx.input(|i| i.clone()))
                {
                    self.waveform_state.loop_a_secs = Some(self.waveform_play_position);
                    self.push_undo();
                    self.send_cmd(WaveformCommand::SetLoopBounds {
                        a_secs: self.waveform_play_position,
                        b_secs: self
                            .waveform_state
                            .loop_b_secs
                            .unwrap_or(self.waveform_state.duration_secs),
                    });
                    self.status_message =
                        format!("Loop A gezet op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SetLoopB, &ctx.input(|i| i.clone()))
                {
                    self.waveform_state.loop_b_secs = Some(self.waveform_play_position);
                    self.push_undo();
                    self.send_cmd(WaveformCommand::SetLoopBounds {
                        a_secs: self.waveform_state.loop_a_secs.unwrap_or(0.0),
                        b_secs: self.waveform_play_position,
                    });
                    self.status_message =
                        format!("Loop B gezet op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeLoopLeft, &ctx.input(|i| i.clone()))
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        let len = b - a;
                        let new_a = (a - len).max(0.0);
                        let new_b = new_a + len;
                        self.waveform_state.loop_a_secs = Some(new_a);
                        self.waveform_state.loop_b_secs = Some(new_b);

                        self.waveform_play_position = new_a;
                        self.waveform_state.seek_pending = Some(new_a);
                        self.waveform_state.playhead_frames_after_drag = 15;

                        self.send_cmd(WaveformCommand::SetLoopBounds {
                            a_secs: new_a,
                            b_secs: new_b,
                        });
                        if self.waveform_has_content {
                            self.send_cmd(WaveformCommand::Seek { pos_secs: new_a });
                        }
                        self.status_message =
                            format!("Loop genudget ← naar {:.1}s–{:.1}s", new_a, new_b);
                        self.status_message_timer = 3 * 60;
                        self.center_view_on_loop(self.last_panel_width);
                    } else {
                        self.status_message = "Geen A-B loop ingesteld om te nudgen".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeLoopRight, &ctx.input(|i| i.clone()))
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        let len = b - a;
                        let dur = self.waveform_state.duration_secs;
                        let new_b = (b + len).min(dur);
                        let new_a = new_b - len;
                        self.waveform_state.loop_a_secs = Some(new_a);
                        self.waveform_state.loop_b_secs = Some(new_b);

                        self.waveform_play_position = new_a;
                        self.waveform_state.seek_pending = Some(new_a);
                        self.waveform_state.playhead_frames_after_drag = 15;

                        self.send_cmd(WaveformCommand::SetLoopBounds {
                            a_secs: new_a,
                            b_secs: new_b,
                        });
                        if self.waveform_has_content {
                            self.send_cmd(WaveformCommand::Seek { pos_secs: new_a });
                        }
                        self.status_message =
                            format!("Loop genudget → naar {:.1}s–{:.1}s", new_a, new_b);
                        self.status_message_timer = 3 * 60;
                        self.center_view_on_loop(self.last_panel_width);
                    } else {
                        self.status_message = "Geen A-B loop ingesteld om te nudgen".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                // ── DoubleLoopLength (Ctrl+D) ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::DoubleLoopLength, &ctx.input(|i| i.clone()))
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        if b > a {
                            let len = b - a;
                            let new_a = a;
                            let new_b = (a + len * 2.0).min(self.waveform_state.duration_secs);
                            self.waveform_state.loop_a_secs = Some(new_a);
                            self.waveform_state.loop_b_secs = Some(new_b);
                            self.send_cmd(WaveformCommand::SetLoopBounds {
                                a_secs: new_a,
                                b_secs: new_b,
                            });
                            self.status_message = format!(
                                "Loop verdubbeld naar {:.1}s–{:.1}s ({:.1}s)",
                                new_a,
                                new_b,
                                new_b - new_a
                            );
                            self.status_message_timer = 3 * 60;
                        }
                    } else {
                        self.status_message = "Geen A-B loop om te verdubbelen".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                // ── HalveLoopLength (Ctrl+Shift+D) ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::HalveLoopLength, &ctx.input(|i| i.clone()))
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        if b > a {
                            let len = b - a;
                            let new_a = a;
                            let new_b = a + len / 2.0;
                            if new_b > new_a {
                                self.waveform_state.loop_a_secs = Some(new_a);
                                self.waveform_state.loop_b_secs = Some(new_b);
                                self.send_cmd(WaveformCommand::SetLoopBounds {
                                    a_secs: new_a,
                                    b_secs: new_b,
                                });
                                self.status_message = format!(
                                    "Loop gehalveerd naar {:.1}s–{:.1}s ({:.1}s)",
                                    new_a,
                                    new_b,
                                    new_b - new_a
                                );
                                self.status_message_timer = 3 * 60;
                            }
                        }
                    } else {
                        self.status_message = "Geen A-B loop om te halveren".to_string();
                        self.status_message_timer = 2 * 60;
                    }
                }

                // ── SnapLoopLeft (Q) — snap A naar dichtstbijzijnde marker links ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SnapLoopLeft, &ctx.input(|i| i.clone()))
                {
                    if let Some(a) = self.waveform_state.loop_a_secs {
                        let nearest_left = self
                            .waveform_state
                            .markers
                            .iter()
                            .map(|m| m.position_secs)
                            .filter(|&pos| pos < a)
                            .max_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                        if let Some(target) = nearest_left {
                            let delta = a - target;
                            self.waveform_state.loop_a_secs = Some(target);
                            if let Some(b) = self.waveform_state.loop_b_secs {
                                self.waveform_state.loop_b_secs = Some((b - delta).max(0.0));
                            }
                            self.sync_loop_bounds();
                            self.status_message =
                                format!("Loop gesnapt naar marker op {:.1}s", target);
                            self.status_message_timer = 3 * 60;
                        } else {
                            self.status_message = "Geen marker links van de loop".to_string();
                            self.status_message_timer = 2 * 60;
                        }
                    }
                }

                // ── SnapLoopRight (W) — snap A naar dichtstbijzijnde marker rechts ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SnapLoopRight, &ctx.input(|i| i.clone()))
                {
                    if let Some(a) = self.waveform_state.loop_a_secs {
                        let nearest_right = self
                            .waveform_state
                            .markers
                            .iter()
                            .map(|m| m.position_secs)
                            .filter(|&pos| pos > a)
                            .min_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                        if let Some(target) = nearest_right {
                            let delta = target - a;
                            self.waveform_state.loop_a_secs = Some(target);
                            if let Some(b) = self.waveform_state.loop_b_secs {
                                self.waveform_state.loop_b_secs =
                                    Some((b + delta).min(self.waveform_state.duration_secs));
                            }
                            self.sync_loop_bounds();
                            self.status_message =
                                format!("Loop gesnapt naar marker op {:.1}s", target);
                            self.status_message_timer = 3 * 60;
                        } else {
                            self.status_message = "Geen marker rechts van de loop".to_string();
                            self.status_message_timer = 2 * 60;
                        }
                    }
                }
            }

            // ── Nudge marker A links/rechts (J / Shift+J) ──
            if self.waveform_state.path.is_some() {
                let step = 0.05;
                let mut changed = false;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeALeft, &ctx.input(|i| i.clone()))
                {
                    if let Some(a) = self.waveform_state.loop_a_secs.as_mut() {
                        *a = (*a - step).max(0.0);
                        if let Some(b) = self.waveform_state.loop_b_secs {
                            if *a >= b {
                                *a = (b - step).max(0.0);
                            }
                        }
                        changed = true;
                    }
                }
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeARight, &ctx.input(|i| i.clone()))
                {
                    if let Some(a) = self.waveform_state.loop_a_secs.as_mut() {
                        *a = (*a + step).min(self.waveform_state.duration_secs);
                        if let Some(b) = self.waveform_state.loop_b_secs {
                            if *a >= b {
                                *a = (b - step).max(0.0);
                            }
                        }
                        changed = true;
                    }
                }
                if changed {
                    self.sync_loop_bounds();
                }
            }

            // ── Nudge marker B links/rechts (L / Shift+L) ──
            if self.waveform_state.path.is_some() {
                let step = 0.05;
                let mut changed = false;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeBLeft, &ctx.input(|i| i.clone()))
                {
                    if let Some(b) = self.waveform_state.loop_b_secs.as_mut() {
                        *b = (*b - step).max(0.0);
                        if let Some(a) = self.waveform_state.loop_a_secs {
                            if *b <= a {
                                *b = (a + step).min(self.waveform_state.duration_secs);
                            }
                        }
                        changed = true;
                    }
                }
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgeBRight, &ctx.input(|i| i.clone()))
                {
                    if let Some(b) = self.waveform_state.loop_b_secs.as_mut() {
                        *b = (*b + step).min(self.waveform_state.duration_secs);
                        if let Some(a) = self.waveform_state.loop_a_secs {
                            if *b <= a {
                                *b = (a + step).min(self.waveform_state.duration_secs);
                            }
                        }
                        changed = true;
                    }
                }
                if changed {
                    self.sync_loop_bounds();
                }
            }

            // ── ←/→ Playhead nudgen (0.20s) ──
            if self.waveform_state.path.is_some() {
                let step = 0.20;
                let mut new_pos: Option<f32> = None;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::NudgePlayheadLeft, &ctx.input(|i| i.clone()))
                {
                    new_pos = Some((self.waveform_play_position - step).max(0.0));
                }
                if self.shortcuts.is_pressed(
                    ShortcutAction::NudgePlayheadRight,
                    &ctx.input(|i| i.clone()),
                ) {
                    new_pos = Some(
                        (self.waveform_play_position + step).min(self.waveform_state.duration_secs),
                    );
                }
                if let Some(pos) = new_pos {
                    self.waveform_play_position = pos;
                    self.waveform_state.seek_pending = Some(pos);
                    self.send_cmd(WaveformCommand::Seek { pos_secs: pos });
                    self.waveform_state.playhead_frames_after_drag = 15;
                }
            }

            // ── Center loop in viewport ──
            if self.waveform_state.path.is_some() {
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::CenterLoop, &ctx.input(|i| i.clone()))
                {
                    self.center_view_on_loop(self.last_panel_width);
                    self.status_message = "Weergave gecentreerd".to_string();
                    self.status_message_timer = 2 * 60;
                }
            }

            // ── ↑/↓ Rewind/Forward 2 seconden ──
            if self.waveform_state.path.is_some() {
                let mut seek_delta: Option<f32> = None;
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SeekBackward, &ctx.input(|i| i.clone()))
                {
                    seek_delta = Some(-2.0);
                }
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SeekForward, &ctx.input(|i| i.clone()))
                {
                    seek_delta = Some(2.0);
                }

                if let Some(delta) = seek_delta {
                    let new_pos = (self.waveform_play_position + delta)
                        .clamp(0.0, self.waveform_state.duration_secs);
                    self.waveform_play_position = new_pos;
                    self.waveform_state.seek_pending = Some(new_pos); // ✅ NIEUW: Markeer als pending

                    // if self.waveform_has_content {
                    self.send_cmd(WaveformCommand::Seek { pos_secs: new_pos });
                    // ✅ FIX: Negeer oude Position events voor ~250ms
                    self.waveform_state.playhead_frames_after_drag = 15;
                    //   }
                }
            }

            // Stop
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Stop, &ctx.input(|i| i.clone()))
            {
                self.send_cmd(WaveformCommand::Stop);
                self.waveform_is_playing = false;
                self.waveform_has_content = false;
            }

            // ClearLoop
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ClearLoop, &ctx.input(|i| i.clone()))
            {
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

            // ToggleLoopBypass
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ToggleLoopBypass, &ctx.input(|i| i.clone()))
            {
                self.loop_bypassed = !self.loop_bypassed;
                self.send_cmd(WaveformCommand::SetLoopEnabled(!self.loop_bypassed));
                self.status_message = if self.loop_bypassed {
                    "Loop bypassed — speelt door naar einde".to_string()
                } else {
                    "Loop hervat".to_string()
                };
                self.status_message_timer = 2 * 60;
            }

            // SaveLoop
            if self
                .shortcuts
                .is_pressed(ShortcutAction::SaveLoop, &ctx.input(|i| i.clone()))
            {
                self.save_current_loop();
            }

            // ToggleLoopPoint — 1 toets A-B
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ToggleLoopPoint, &ctx.input(|i| i.clone()))
            {
                let pos = self.waveform_play_position;
                if let Some(pending) = self.pending_loop_point {
                    let (a, b) = if pos > pending {
                        (pending, pos)
                    } else {
                        (pos, pending)
                    };
                    self.waveform_state.loop_a_secs = Some(a);
                    self.waveform_state.loop_b_secs = Some(b);
                    self.pending_loop_point = None;
                    self.push_undo();
                    self.send_cmd(WaveformCommand::SetLoopBounds {
                        a_secs: a,
                        b_secs: b,
                    });
                    self.status_message = format!("Loop A-B gezet: {:.1}s → {:.1}s", a, b);
                    self.status_message_timer = 3 * 60;
                } else {
                    self.pending_loop_point = Some(pos);
                    self.status_message = format!("Loop punt 1 op {:.1}s — druk nogmaals", pos);
                    self.status_message_timer = 3 * 60;
                }
            }

            // ZoomIn / ZoomOut / ResetZoom
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ZoomIn, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).max(5000.0);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ZoomOut, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.zoom = (self.waveform_state.zoom / 1.3).max(5.0);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ResetZoom, &ctx.input(|i| i.clone()))
            {
                self.waveform_state.zoom = 50.0;
                self.waveform_state.scroll_offset = 0.0;
            }

            // ScrollForward / ScrollBackward — pagineren op huidig zoomniveau
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ScrollForward, &ctx.input(|i| i.clone()))
            {
                let panel_width = self.last_panel_width.max(100.0);
                let page_secs = if self.waveform_state.zoom > 0.0 {
                    panel_width / self.waveform_state.zoom
                } else {
                    10.0
                };
                self.waveform_state.scroll_offset = (self.waveform_state.scroll_offset + page_secs)
                    .min((self.waveform_state.duration_secs - page_secs).max(0.0));
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ScrollBackward, &ctx.input(|i| i.clone()))
            {
                let panel_width = self.last_panel_width.max(100.0);
                let page_secs = if self.waveform_state.zoom > 0.0 {
                    panel_width / self.waveform_state.zoom
                } else {
                    10.0
                };
                self.waveform_state.scroll_offset =
                    (self.waveform_state.scroll_offset - page_secs).max(0.0);
            }

            // OpenFile
            if self
                .shortcuts
                .is_pressed(ShortcutAction::OpenFile, &ctx.input(|i| i.clone()))
            {
                self.file_dialog.select_file();
            }

            // Undo
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Undo, &ctx.input(|i| i.clone()))
            {
                if let Some(state) = self.undo_stack.pop() {
                    self.redo_stack.push(UndoState::snapshot_from(self));
                    self.restore_undo(state);
                }
            }

            // Redo
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Redo, &ctx.input(|i| i.clone()))
            {
                if let Some(state) = self.redo_stack.pop() {
                    self.undo_stack.push(UndoState::snapshot_from(self));
                    self.restore_undo(state);
                }
            }

            // View
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ShowShortcuts, &ctx.input(|i| i.clone()))
            {
                self.show_shortcuts = !self.show_shortcuts;
            }

            // RestartLoop — seek naar A (of begin file) en start playback
            if self
                .shortcuts
                .is_pressed(ShortcutAction::RestartLoop, &ctx.input(|i| i.clone()))
            {
                if self.waveform_state.path.is_some() {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        if b > a {
                            // Loop ingesteld: speel vanaf A met looping
                            self.waveform_play_position = a;
                            self.waveform_state.seek_pending = Some(a);
                            self.waveform_state.playhead_frames_after_drag = 15;
                            self.send_cmd(WaveformCommand::Play {
                                samples: self.waveform_state.samples.clone(),
                                sample_rate: self.waveform_state.sample_rate,
                                start_sample: (a * self.waveform_state.sample_rate as f32) as usize,
                                segment_start_sec: 0.0,
                                a_sample: (a * self.waveform_state.sample_rate as f32) as usize,
                                b_sample: (b * self.waveform_state.sample_rate as f32) as usize,
                                pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                                    self.waveform_state.pitch_semitones,
                                ))),
                                tempo: Arc::new(AtomicU32::new(f32::to_bits(
                                    self.waveform_state.tempo,
                                ))),
                                click_positions: self.click_positions.clone(),
                                click_enabled: self.click_enabled.clone(),
                            });
                            self.waveform_is_playing = true;
                            self.waveform_has_content = true;
                            self.status_message = format!("Loop herstart vanaf {:.1}s", a);
                            self.status_message_timer = 3 * 60;
                        }
                    } else {
                        // Geen loop: speel vanaf begin van de file
                        let dur = self.waveform_state.duration_secs;
                        self.waveform_play_position = 0.0;
                        self.waveform_state.seek_pending = Some(0.0);
                        self.waveform_state.playhead_frames_after_drag = 15;
                        self.send_cmd(WaveformCommand::Play {
                            samples: self.waveform_state.samples.clone(),
                            sample_rate: self.waveform_state.sample_rate,
                            start_sample: 0,
                            segment_start_sec: 0.0,
                            a_sample: 0,
                            b_sample: 0, // a == b → geen looping
                            pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                                self.waveform_state.pitch_semitones,
                            ))),
                            tempo: Arc::new(AtomicU32::new(f32::to_bits(
                                self.waveform_state.tempo,
                            ))),
                            click_positions: self.click_positions.clone(),
                            click_enabled: self.click_enabled.clone(),
                        });
                        self.waveform_is_playing = true;
                        self.waveform_has_content = true;
                        self.status_message = format!("Speel vanaf begin ({:.1}s)", dur);
                        self.status_message_timer = 3 * 60;
                    }
                }
            }

            // Tools — hergebruik execute_toolbar_action logica
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Detect, &ctx.input(|i| i.clone()))
            {
                self.run_detection();
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ExtendBeats, &ctx.input(|i| i.clone()))
            {
                self.extend_beat_markers();
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::PlaceBeats, &ctx.input(|i| i.clone()))
            {
                self.execute_toolbar_action(ToolbarAction::PlaceBeats);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ToggleArranger, &ctx.input(|i| i.clone()))
            {
                self.show_arranger ^= true;
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::Setup, &ctx.input(|i| i.clone()))
            {
                self.show_setup ^= true;
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ToggleAudit, &ctx.input(|i| i.clone()))
            {
                self.toggle_beat_audit();
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::TempoDown, &ctx.input(|i| i.clone()))
            {
                self.execute_toolbar_action(ToolbarAction::TempoDown);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::TempoUp, &ctx.input(|i| i.clone()))
            {
                self.execute_toolbar_action(ToolbarAction::TempoUp);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::PitchDown, &ctx.input(|i| i.clone()))
            {
                self.execute_toolbar_action(ToolbarAction::PitchDown);
            }
            if self
                .shortcuts
                .is_pressed(ShortcutAction::PitchUp, &ctx.input(|i| i.clone()))
            {
                self.execute_toolbar_action(ToolbarAction::PitchUp);
            }

            // ExportLoops — open export window
            if self
                .shortcuts
                .is_pressed(ShortcutAction::ExportLoops, &ctx.input(|i| i.clone()))
            {
                if self.waveform_state.path.is_some() {
                    let track_path = self.waveform_state.path.as_ref().unwrap();
                    let track = self.library.track_for_path(track_path);
                    if track.loops.is_empty() {
                        self.status_message = "Geen opgeslagen loops voor deze track".to_string();
                        self.status_message_timer = 3 * 60;
                    } else {
                        self.open_export_window();
                    }
                } else {
                    self.status_message = "Geen audiobestand geladen".to_string();
                    self.status_message_timer = 3 * 60;
                }
            }
        }
    }

    /// Verwerk drag-and-drop van audiobestanden.
    pub(crate) fn handle_drag_drop(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty() {
            if let Some(path) = dropped
                .first()
                .and_then(|f| f.path.as_ref())
                .and_then(|p| p.to_str())
            {
                self.file_path = path.to_string();
                self.load_file(path);
            }
        }
    }
}
