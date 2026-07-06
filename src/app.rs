use crate::arrangement::{color_for_arranger, Arrangement};
use crate::chroma::{detect_chroma, Chroma};
use crate::loops::{Library, SavedLoop};
use crate::session::SessionState;
use crate::shortcuts::{KeyBinding, ShortcutAction, ShortcutsConfig};
use crate::waveform::{render_waveform, ChannelMode, WaveformState};
use crate::waveform_player::{start_waveform_thread, WaveformCommand, WaveformEvent};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, RichText};
use egui_file_dialog::FileDialog;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

// ───────────────────────────────────────────────
// Export types
// ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportMode {
    Separate,
    Combined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Wav,
}

/// State collected when user clicks "Export" — held while the file dialog is open.
#[derive(Clone)]
struct ExportParams {
    loops: Vec<crate::loops::SavedLoop>,
    base_name: String,
    mode: ExportMode,
    #[allow(dead_code)]
    format: ExportFormat,
    sample_rate: u32,
    samples: std::sync::Arc<Vec<f32>>,
}

#[derive(Clone)]
struct ExportState {
    show_window: bool,
    selected: Vec<bool>,
    base_name: String,
    mode: ExportMode,
    format: ExportFormat,
    /// Cache of (label, a_secs, b_secs) populated once when window opens,
    /// avoids cloning the full SavedLoop vector every frame.
    cached_loop_info: Vec<(String, f32, f32)>,
}

pub struct LoopEditorApp {
    // Waveform state
    pub waveform_state: WaveformState,
    pub waveform_cmd_tx: Sender<WaveformCommand>,
    pub waveform_event_rx: Receiver<WaveformEvent>,
    pub waveform_is_playing: bool,
    pub waveform_play_position: f32,
    pub waveform_play_duration: f32,
    pub waveform_has_content: bool,

    // Library (tracks, loops, markers)
    pub library: Library,
    pub show_loop_library: bool,
    pub active_loop_idx: Option<usize>,

    // Loop point pending (voor 1-toets A-B zetten)
    pub pending_loop_point: Option<f32>,

    // Chroma detectie
    pub chroma_result: Option<Chroma>,

    // File path input
    pub file_path: String,
    pub status_message: String,
    pub status_message_timer: u32,

    // Help / shortcuts
    pub show_shortcuts: bool,

    // Looping bypass
    pub loop_bypassed: bool,

    // Loop herhaal-teller
    pub loop_repeat_count: u32,    // 0 = oneindig
    pub loop_iteration_count: u32, // interne teller, reset bij elke Play

    // Shortcuts
    pub shortcuts: ShortcutsConfig,
    pub show_shortcut_editor: bool,
    pub listening_for_action: Option<ShortcutAction>,

    // Undo/Redo
    pub undo_stack: Vec<UndoState>,
    pub redo_stack: Vec<UndoState>,

    // Paneel breedte (voor center_view_on_loop)
    pub last_panel_width: f32,

    // File dialog (egui-native)
    pub file_dialog: FileDialog,

    // Arranger
    pub show_arranger: bool,
    pub active_arrangement: Option<usize>,
    pub arrangements: Vec<Arrangement>,
    pub arr_current_step: Option<usize>,
    pub arr_parse_buf: String,

    // Export
    confirm_delete_track: Option<(usize, String)>,
    export_state: ExportState,
    export_dialog: FileDialog,
    export_pending: Option<ExportParams>,
}

/// Momentopname van de muteerbare editor state (voor undo/redo).
#[derive(Clone)]
pub struct UndoState {
    pub play_position: f32,
    pub loop_a_secs: Option<f32>,
    pub loop_b_secs: Option<f32>,
    pub pitch_semitones: f32,
    pub tempo: f32,
    pub volume: f32,
    pub zoom: f32,
    pub scroll_offset: f32,
    pub markers: Vec<crate::waveform::Marker>,
    pub loop_bypassed: bool,
}

impl UndoState {
    pub fn snapshot_from(app: &LoopEditorApp) -> Self {
        Self {
            play_position: app.waveform_play_position,
            loop_a_secs: app.waveform_state.loop_a_secs,
            loop_b_secs: app.waveform_state.loop_b_secs,
            pitch_semitones: app.waveform_state.pitch_semitones,
            tempo: app.waveform_state.tempo,
            volume: app.waveform_state.volume,
            zoom: app.waveform_state.zoom,
            scroll_offset: app.waveform_state.scroll_offset,
            markers: app.waveform_state.markers.clone(),
            loop_bypassed: app.loop_bypassed,
        }
    }

    pub fn apply_to(self, app: &mut LoopEditorApp) {
        app.waveform_play_position = self.play_position;
        app.waveform_state.loop_a_secs = self.loop_a_secs;
        app.waveform_state.loop_b_secs = self.loop_b_secs;
        app.waveform_state.pitch_semitones = self.pitch_semitones;
        app.waveform_state.tempo = self.tempo;
        app.waveform_state.volume = self.volume;
        app.waveform_state.zoom = self.zoom;
        app.waveform_state.scroll_offset = self.scroll_offset;
        app.waveform_state.markers = self.markers;
        app.loop_bypassed = self.loop_bypassed;
    }
}

impl LoopEditorApp {
    pub fn new() -> Self {
        let (waveform_cmd_tx, waveform_event_rx) = start_waveform_thread();
        let library = crate::loops::load_library();
        let shortcuts = ShortcutsConfig::load();

        let mut app = Self {
            waveform_state: WaveformState::default(),
            waveform_cmd_tx,
            waveform_event_rx,
            waveform_is_playing: false,
            waveform_play_position: 0.0,
            waveform_play_duration: 0.0,
            waveform_has_content: false,
            library,
            show_loop_library: false,
            active_loop_idx: None,
            pending_loop_point: None,
            chroma_result: None,
            file_path: String::new(),
            status_message: String::new(),
            status_message_timer: 0,
            show_shortcuts: false,
            loop_bypassed: false,
            loop_repeat_count: 0,
            loop_iteration_count: 0,
            shortcuts,
            show_shortcut_editor: false,
            listening_for_action: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_panel_width: 800.0,
            file_dialog: FileDialog::new().add_file_filter(
                "Audio",
                std::sync::Arc::new(|p: &std::path::Path| {
                    matches!(
                        p.extension().and_then(|s| s.to_str()),
                        Some("mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma")
                    )
                }),
            ),
            show_arranger: false,
            active_arrangement: None,
            arrangements: crate::arrangement::load_arrangements(),
            arr_current_step: None,
            arr_parse_buf: String::new(),
            export_state: ExportState {
                show_window: false,
                selected: Vec::new(),
                base_name: "audiotrack_loops".to_string(),
                mode: ExportMode::Combined,
                format: ExportFormat::Wav,
                cached_loop_info: Vec::new(),
            },
            export_dialog: FileDialog::new()
                .add_file_filter(
                    "WAV Audio (*.wav)",
                    std::sync::Arc::new(|p: &std::path::Path| {
                        p.extension().and_then(|s| s.to_str()) == Some("wav")
                    }),
                )
                .title("Exporteer loops"),
            export_pending: None,
            confirm_delete_track: None,
        };

        // Laad sessie (vorige file, positie, etc.)
        if let Some(session) = SessionState::load() {
            app.waveform_state.zoom = session.zoom;
            app.waveform_state.scroll_offset = session.scroll_offset;
            app.waveform_state.loop_a_secs = session.loop_a_secs;
            app.waveform_state.loop_b_secs = session.loop_b_secs;
            app.waveform_state.pitch_semitones = session.pitch_semitones;
            app.waveform_state.tempo = session.tempo;
            app.waveform_state.volume = session.volume;
            app.waveform_play_position = session.play_position;
            app.waveform_state.channel_mode =
                serde_json::from_str(&format!("\"{}\"", session.channel_mode))
                    .unwrap_or(ChannelMode::Mono);

            if let Some(ref path) = session.file_path {
                if Path::new(path).exists() {
                    app.file_path = path.clone();
                    app.load_file(path);
                }
            }
            app.arr_parse_buf = session.arr_parse_buf;
        }

        app
    }

    pub fn load_file(&mut self, path: &str) {
        // Controleer of bestand bestaat — voor duidelijke foutmelding
        if !std::path::Path::new(path).exists() {
            let msg = format!("Bestand niet gevonden: {}", path);
            self.waveform_state.error = Some(msg.clone());
            self.status_message = msg;
            self.status_message_timer = 10 * 60;
            return;
        }

        // Stop huidige playback als er een ander bestand wordt geladen
        if self.waveform_state.path.as_deref() != Some(path) {
            if self.waveform_is_playing {
                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
                self.waveform_is_playing = false;
            }
            self.waveform_has_content = false;
        }

        match crate::waveform::decode_audio(path, self.waveform_state.channel_mode) {
            Ok((samples, sample_rate, duration_secs, warning)) => {
                self.waveform_state.path = Some(path.to_string());
                self.waveform_state.samples = Arc::new(samples);
                self.waveform_state.sample_rate = sample_rate;
                self.waveform_state.duration_secs = duration_secs;
                self.waveform_state.zoom = 50.0;
                self.waveform_state.scroll_offset = 0.0;
                self.waveform_state.loop_a_secs = None;
                self.waveform_state.loop_b_secs = None;
                self.waveform_state.error = None;
                self.waveform_play_position = 0.0;
                self.waveform_play_duration = duration_secs;

                // Herstel markers uit de bibliotheek
                let track = self.library.track_for_path(path);
                self.waveform_state.markers = track.markers.clone();
                self.active_loop_idx = None;
                self.pending_loop_point = None;
                self.chroma_result = None;
                self.save_session();

                let mut msg = format!(
                    "Geladen: {} ({:.1}s, {} Hz)",
                    Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default(),
                    duration_secs,
                    sample_rate,
                );
                if let Some(warn) = warning {
                    msg.push_str(&format!("  |  {}", warn));
                }
                self.status_message = msg;
                self.status_message_timer = 5 * 60;
            }
            Err(e) => {
                self.waveform_state.error = Some(e.clone());
                self.status_message = format!("Fout bij laden: {}", e);
                self.status_message_timer = 10 * 60;
            }
        }
    }

    /// Synchroniseer markers van waveform_state naar library en sla op.
    fn sync_markers_to_library(&mut self) {
        if let Some(ref path) = self.waveform_state.path.clone() {
            let track = self.library.track_for_path(path);
            track.markers = self.waveform_state.markers.clone();
            crate::loops::save_library(&self.library);
        }
    }

    /// Stuur huidige A-B loop naar de audio-thread.
    fn sync_loop_bounds(&mut self) {
        let a = self.waveform_state.loop_a_secs.unwrap_or(0.0);
        let b = self.waveform_state.loop_b_secs.unwrap_or(0.0);
        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
            a_secs: a,
            b_secs: b,
        });
    }

    /// Speel een heel arrangement af (fire & forget naar audio-thread).
    fn play_arrangement(&mut self, arr_idx: usize) {
        if let Some(arr) = self.arrangements.get(arr_idx) {
            let mut seq_steps = Vec::new();

            for step in &arr.steps {
                // Alleen stappen toevoegen van de huidig geladen track
                if let Some(ref path) = self.waveform_state.path {
                    if *path != step.track_path {
                        continue;
                    }

                    let track = self.library.track_for_path(path);
                    if let Some(saved) = track
                        .loops
                        .iter()
                        .find(|l| l.short_id.as_deref() == Some(&step.loop_id))
                    {
                        let sr = self.waveform_state.sample_rate;
                        let a = (saved.loop_a_secs * sr as f32) as usize;
                        let b = (saved.loop_b_secs * sr as f32) as usize;
                        if b > a {
                            seq_steps.push(crate::arrangement::SequenceStep {
                                samples: self.waveform_state.samples.clone(),
                                sample_rate: sr,
                                start_sample: a,
                                end_sample: b,
                                repeats: step.repeats,
                            });
                        }
                    }
                }
            }

            if !seq_steps.is_empty() {
                let _ = self.waveform_cmd_tx.send(WaveformCommand::PlaySequence {
                    sequence_steps: seq_steps,
                    pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                        self.waveform_state.pitch_semitones,
                    ))),
                    tempo: Arc::new(AtomicU32::new(f32::to_bits(self.waveform_state.tempo))),
                });
                self.arr_current_step = Some(0);
            } else {
                self.status_message =
                    "Open eerst het juiste audiobestand voor dit arrangement.".to_string();
                self.status_message_timer = 5 * 60;
            }
        }
    }

    /// Sla de huidige A-B selectie op als een nieuwe loop in de bibliotheek.
    fn save_current_loop(&mut self) -> bool {
        if let (Some(a), Some(b)) = (
            self.waveform_state.loop_a_secs,
            self.waveform_state.loop_b_secs,
        ) {
            if b > a {
                if let Some(ref path) = self.waveform_state.path.clone() {
                    let label = self.library.generate_label(path);
                    let saved = SavedLoop {
                        label,
                        short_id: None,
                        loop_a_secs: a,
                        loop_b_secs: b,
                        pitch_semitones: self.waveform_state.pitch_semitones,
                        tempo: self.waveform_state.tempo,
                        notes: String::new(),
                    };
                    self.library.add_loop(path, saved);
                    let total = self
                        .library
                        .tracks
                        .iter()
                        .filter(|t| t.track_path == *path)
                        .flat_map(|t| &t.loops)
                        .count();
                    crate::loops::save_library(&self.library);
                    self.status_message = format!("Loop opgeslagen! ({} totaal)", total);
                    self.status_message_timer = 3 * 60;
                    return true;
                }
            }
        }
        self.status_message = "Geen A-B loop om op te slaan".to_string();
        self.status_message_timer = 2 * 60;
        false
    }

    /// Sla huidige editor state op voor undo.
    fn push_undo(&mut self) {
        const MAX_UNDO: usize = 50;
        self.undo_stack.push(UndoState::snapshot_from(self));
        if self.undo_stack.len() > MAX_UNDO {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Sla huidige state op in session.json (voor herstart).
    fn save_session(&self) {
        let mode_str = format!("{:?}", self.waveform_state.channel_mode);
        SessionState::save(
            self.waveform_state.path.as_deref(),
            self.waveform_play_position,
            self.waveform_state.zoom,
            self.waveform_state.scroll_offset,
            self.waveform_state.loop_a_secs,
            self.waveform_state.loop_b_secs,
            self.waveform_state.pitch_semitones,
            self.waveform_state.tempo,
            self.waveform_state.volume,
            &mode_str,
            &self.arr_parse_buf,
        );
    }

    /// Herstel een UndoState.
    fn restore_undo(&mut self, state: UndoState) {
        state.apply_to(self);
        self.sync_loop_bounds();
        self.status_message = "Undo/Redo".to_string();
        self.status_message_timer = 2 * 60;
    }

    /// Centreer de viewport op de huidige A-B loop.
    fn center_view_on_loop(&mut self, viewport_width_px: f32) {
        if let (Some(a), Some(b)) = (
            self.waveform_state.loop_a_secs,
            self.waveform_state.loop_b_secs,
        ) {
            if b > a {
                let loop_width = b - a;
                let target_zoom = (viewport_width_px * 0.6) / loop_width;
                self.waveform_state.zoom = target_zoom.clamp(5.0, 5000.0);

                let visible_secs = viewport_width_px / self.waveform_state.zoom;
                let mid = (a + b) / 2.0;
                let max_scroll = (self.waveform_state.duration_secs - visible_secs).max(0.0);
                self.waveform_state.scroll_offset =
                    (mid - visible_secs / 2.0).clamp(0.0, max_scroll);
            }
        }
    }

    // ───────────────────────────────────────────────
    // Export logic
    // ───────────────────────────────────────────────

    fn open_export_window(&mut self) {
        let track_path = match self.waveform_state.path.clone() {
            Some(p) => p,
            None => return,
        };
        let track = self.library.track_for_path(&track_path);
        let count = track.loops.len();

        self.export_state.selected = vec![false; count];
        self.export_state.base_name = "audiotrack_loops".to_string();
        self.export_state.cached_loop_info = track
            .loops
            .iter()
            .map(|l| (l.label.clone(), l.loop_a_secs, l.loop_b_secs))
            .collect();
        self.export_state.show_window = true;
    }

    fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
        if samples.is_empty() {
            return Err("Geen samples om te schrijven".to_string());
        }
        let samples_i16: Vec<i16> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let file = File::create(path)
            .map_err(|e| format!("Kon bestand niet aanmaken '{}': {}", path.display(), e))?;
        let mut writer = hound::WavWriter::new(BufWriter::new(file), spec)
            .map_err(|e| format!("Fout bij initialiseren WAV: {}", e))?;

        for &sample in &samples_i16 {
            writer
                .write_sample(sample)
                .map_err(|e| format!("Fout bij schrijven sample: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Fout bij afsluiten WAV: {}", e))?;
        Ok(())
    }

    fn execute_export(
        &self,
        params: &ExportParams,
        target: &std::path::Path,
    ) -> Result<String, String> {
        match params.mode {
            ExportMode::Combined => self.export_combined(params, target),
            ExportMode::Separate => self.export_separate(params, target),
        }
    }

    fn export_combined(
        &self,
        params: &ExportParams,
        path: &std::path::Path,
    ) -> Result<String, String> {
        let mut all_samples: Vec<f32> = Vec::new();
        let sr = params.sample_rate as f32;

        let sample_len = params.samples.len();
        for saved in &params.loops {
            let from = (saved.loop_a_secs * sr) as usize;
            let to = (saved.loop_b_secs * sr) as usize;
            if to > from && from < sample_len {
                let safe_to = to.min(sample_len);
                all_samples.extend_from_slice(&params.samples[from..safe_to]);
            }
        }

        if all_samples.is_empty() {
            return Err("Geen samples om te exporteren (ongeldige loop ranges?)".to_string());
        }

        Self::write_wav(path, &all_samples, params.sample_rate)?;

        let total_secs = all_samples.len() as f32 / params.sample_rate as f32;
        Ok(format!(
            "✅ {} loops gecombineerd \u{2192} '{}' ({:.1}s)",
            params.loops.len(),
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default(),
            total_secs,
        ))
    }

    fn export_separate(
        &self,
        params: &ExportParams,
        dir: &std::path::Path,
    ) -> Result<String, String> {
        let sr = params.sample_rate as f32;
        let sample_len = params.samples.len();
        let mut count = 0usize;

        for saved in &params.loops {
            // Build a filesystem-safe slug from the loop label
            let label_slug: String = saved
                .label
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            // Edge case: slug is all underscores or empty
            let final_slug = if label_slug.trim_matches('_').is_empty() {
                format!("loop_{}", count + 1)
            } else {
                label_slug
            };

            // Prevent overwriting existing files — auto-append counter
            let mut file_name = format!("{}_{}.wav", params.base_name, final_slug);
            let mut path = dir.join(&file_name);
            let mut counter = 1usize;
            while path.exists() {
                file_name = format!("{}_{}_{:03}.wav", params.base_name, final_slug, counter);
                path = dir.join(&file_name);
                counter += 1;
            }

            let from = (saved.loop_a_secs * sr) as usize;
            let to = (saved.loop_b_secs * sr) as usize;
            if to > from && from < sample_len {
                let safe_to = to.min(sample_len);
                Self::write_wav(&path, &params.samples[from..safe_to], params.sample_rate)?;
                count += 1;
            }
        }

        Ok(format!(
            "✅ {} loops geëxporteerd naar '{}'",
            count,
            dir.display(),
        ))
    }

    /// Toon de arranger window UI.
    fn show_arranger_ui(&mut self, ctx: &egui::Context) {
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

impl eframe::App for LoopEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Verwerk waveform audio events ──
        while let Ok(event) = self.waveform_event_rx.try_recv() {
            match event {
                WaveformEvent::Playing => {
                    self.waveform_is_playing = true;
                    self.waveform_has_content = true;
                    ctx.request_repaint();
                }
                WaveformEvent::Stopped => {
                    self.waveform_is_playing = false;
                    self.waveform_has_content = false;
                    ctx.request_repaint();
                }
                WaveformEvent::Paused => {
                    self.waveform_is_playing = false;
                    ctx.request_repaint();
                }
                WaveformEvent::Resumed => {
                    self.waveform_is_playing = true;
                    ctx.request_repaint();
                }
                WaveformEvent::Error(msg) => {
                    self.waveform_is_playing = false;
                    self.status_message = format!("Waveform fout: {}", msg);
                    ctx.request_repaint();
                }
                WaveformEvent::Position(pos, dur) => {
                    self.waveform_play_duration = dur;

                    // ✅ Check of de audio-thread de seek heeft voltooid
                    if let Some(target) = self.waveform_state.seek_pending {
                        // Als de audio-thread binnen 50ms (0.05s) van de target positie is,
                        // beschouwen we de seek als geslaagd.
                        if (pos - target).abs() < 0.05 {
                            self.waveform_state.seek_pending = None;
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
                                    let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
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

        // Verval statusmelding na 5 seconden
        if self.status_message_timer > 0 {
            self.status_message_timer -= 1;
            if self.status_message_timer == 0 {
                self.status_message.clear();
            }
        }

        // 🔥 CRITICAL: Force continuous repaints while playing so the playhead moves smoothly
        if self.waveform_is_playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        // ── Keyboard Shortcuts ──
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
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::TogglePause);
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

                    let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
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
                    });

                    self.waveform_is_playing = true;
                    self.loop_iteration_count = 1; // 1e play-through
                }
            }

            // ── Marker shortcuts (1-9), Backspace (verwijder dichtstbijzijnde), [ ] (A-B) ──
            if self.waveform_state.path.is_some() {
                // ── Marker shortcuts: S (Section), M (Measure), B (Beat) ──
                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddSectionMarker, &ctx.input(|i| i.clone()))
                {
                    let count = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| m.kind == crate::waveform::MarkerKind::Section)
                        .count()
                        + 1;
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: format!("S{}", count),
                        position_secs: self.waveform_play_position,
                        kind: crate::waveform::MarkerKind::Section,
                    });
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message =
                        format!("Section marker op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddMeasureMarker, &ctx.input(|i| i.clone()))
                {
                    let count = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| m.kind == crate::waveform::MarkerKind::Measure)
                        .count()
                        + 1;
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: format!("M{}", count),
                        position_secs: self.waveform_play_position,
                        kind: crate::waveform::MarkerKind::Measure,
                    });
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message =
                        format!("Measure marker op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::AddBeatMarker, &ctx.input(|i| i.clone()))
                {
                    let count = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| m.kind == crate::waveform::MarkerKind::Beat)
                        .count()
                        + 1;
                    self.waveform_state.markers.push(crate::waveform::Marker {
                        name: format!("B{}", count),
                        position_secs: self.waveform_play_position,
                        kind: crate::waveform::MarkerKind::Beat,
                    });
                    self.push_undo();
                    self.sync_markers_to_library();
                    self.status_message =
                        format!("Beat marker op {:.1}s", self.waveform_play_position);
                    self.status_message_timer = 3 * 60;
                }

                if self.shortcuts.is_pressed(
                    ShortcutAction::DeleteNearestMarker,
                    &ctx.input(|i| i.clone()),
                ) {
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
                    }
                }

                if self
                    .shortcuts
                    .is_pressed(ShortcutAction::SetLoopA, &ctx.input(|i| i.clone()))
                {
                    self.waveform_state.loop_a_secs = Some(self.waveform_play_position);
                    self.push_undo();
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
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
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
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

                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                            a_secs: new_a,
                            b_secs: new_b,
                        });
                        if self.waveform_has_content {
                            let _ = self
                                .waveform_cmd_tx
                                .send(WaveformCommand::Seek { pos_secs: new_a });
                        }
                        self.status_message =
                            format!("Loop genudget ← naar {:.1}s–{:.1}s", new_a, new_b);
                        self.status_message_timer = 3 * 60;
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

                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                            a_secs: new_a,
                            b_secs: new_b,
                        });
                        if self.waveform_has_content {
                            let _ = self
                                .waveform_cmd_tx
                                .send(WaveformCommand::Seek { pos_secs: new_a });
                        }
                        self.status_message =
                            format!("Loop genudget → naar {:.1}s–{:.1}s", new_a, new_b);
                        self.status_message_timer = 3 * 60;
                    } else {
                        self.status_message = "Geen A-B loop ingesteld om te nudgen".to_string();
                        self.status_message_timer = 2 * 60;
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
                    let _ = self
                        .waveform_cmd_tx
                        .send(WaveformCommand::Seek { pos_secs: pos });
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
                    self.status_message = "Loop gecentreerd in viewport".to_string();
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
                    let _ = self
                        .waveform_cmd_tx
                        .send(WaveformCommand::Seek { pos_secs: new_pos });
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
                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
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
                let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
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
                let _ = self
                    .waveform_cmd_tx
                    .send(WaveformCommand::SetLoopEnabled(!self.loop_bypassed));
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
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
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
                self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).min(5000.0);
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

            // RestartLoop — seek naar A en start playback
            if self
                .shortcuts
                .is_pressed(ShortcutAction::RestartLoop, &ctx.input(|i| i.clone()))
            {
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        self.waveform_play_position = a;
                        self.waveform_state.seek_pending = Some(a);
                        self.waveform_state.playhead_frames_after_drag = 15;
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
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
                        });
                        self.waveform_is_playing = true;
                        self.waveform_has_content = true;
                        self.status_message = format!("Loop herstart vanaf {:.1}s", a);
                        self.status_message_timer = 3 * 60;
                    }
                }
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
        // ── Drag & drop bestanden ──
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

        // ── Top paneel met bestand openen ──
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
                    if let Some(ref path) = self.waveform_state.path.clone() {
                        self.load_file(path);
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

                // Status rechts uitlijnen
                if !self.status_message.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(&self.status_message)
                                .size(12.0)
                                .color(Color32::from_rgb(100, 200, 100)),
                        );
                    });
                }

                // ── Export button ──
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("ARR").clicked() {
                        self.show_arranger ^= true;
                    }
                });
            });
            ui.add_space(4.0);
        });

        // ── Shortcuts help overlay (dynamisch uit shortcuts data) ──
        if self.show_shortcuts {
            egui::Window::new("⌨ Toetsenbord Shortcuts")
                .id(egui::Id::new("shortcuts_window"))
                .resizable(true)
                .default_size([400.0, 500.0])
                .default_pos([200.0, 150.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical(|ui| {
                            // ── Dynamische shortcuts uit ShortcutAction::all() ──
                            use crate::shortcuts::ShortcutAction;
                            let categories =
                                ["Playback", "Loop", "Markers", "View", "File", "Edit"];
                            for category in categories {
                                ui.label(
                                    RichText::new(category)
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::from_rgb(180, 180, 220)),
                                );
                                for action in ShortcutAction::all()
                                    .iter()
                                    .filter(|a| a.category() == category)
                                {
                                    let key_text = self
                                        .shortcuts
                                        .binding_for(*action)
                                        .map(|b| b.display())
                                        .unwrap_or_else(|| "—".to_string());
                                    shortcut_row(ui, &key_text, action.display_name());
                                }
                                ui.separator();
                            }

                            // ── Extra muis-acties (geen shortcuts) ──
                            ui.label(
                                RichText::new("Mouse / Interactie")
                                    .size(13.0)
                                    .strong()
                                    .color(Color32::from_rgb(180, 180, 220)),
                            );
                            shortcut_row(ui, "Ctrl+Sleep", "A-B selectie maken");
                            shortcut_row(ui, "Dubbelklik", "Zet A-marker");
                            shortcut_row(ui, "Shift+Dubbelklik", "Zet B-marker");
                            shortcut_row(ui, "Rechterklik", "Wis A-B selectie");
                            shortcut_row(ui, "Scroll", "Zoom in/uit");
                            shortcut_row(ui, "Sleep (geen Ctrl)", "Horizontaal scrollen");
                            ui.separator();

                            if ui.button("⚙ Edit Shortcuts").clicked() {
                                self.show_shortcut_editor = !self.show_shortcut_editor;
                            }
                            ui.label(
                                RichText::new("Druk op F1 om te sluiten")
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        });
                    });
                });
        }

        // ── Hoofdpaneel ──
        egui::CentralPanel::default().show(ctx, |ui| {
            let panel_width = ui.available_width().max(100.0);
            self.last_panel_width = panel_width;
            ui.separator();

            // ── Foutmelding ──
            if let Some(ref err) = self.waveform_state.error {
                ui.label(
                    RichText::new(format!("⚠ {}", err))
                        .size(13.0)
                        .color(Color32::from_rgb(255, 100, 100)),
                );
            }

            // ── Waveform ──
            let play_position = if self.waveform_state.path.is_some() {
                Some(self.waveform_play_position)
            } else {
                None
            };

            let (loop_changed, seek_to, drag_ended) =
                render_waveform(ui, &mut self.waveform_state, play_position);

            // 🔥 Loop-grenzen tijdens playback: stuur SetLoopBounds
            //    → audio-thread past ze direct toe zonder de source te herstarten
            // Stuur loop-verandering altijd naar audio-thread, ook als de
            // audio stilstaat. Anders blijft de audio-thread een oude loop
            // onthouden, die bij een volgende Play onzichtbaar wordt hervat.
            if loop_changed {
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                            a_secs: a,
                            b_secs: b,
                        });
                        // Als de loop was bypassed, heractiveer haar bij A/B-wijziging
                        if self.loop_bypassed {
                            self.loop_bypassed = false;
                            let _ = self
                                .waveform_cmd_tx
                                .send(WaveformCommand::SetLoopEnabled(true));
                            self.status_message = "Loop geüpdatet en geactiveerd".to_string();
                            self.status_message_timer = 3 * 60;
                        }
                    }
                } else {
                    // Rechterklik: loop gewist → stuur 0/0 naar audio-thread + expliciet uitschakelen
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                        a_secs: 0.0,
                        b_secs: 0.0,
                    });
                    let _ = self
                        .waveform_cmd_tx
                        .send(WaveformCommand::SetLoopEnabled(false));
                    self.pending_loop_point = None;
                    self.status_message = "Loop gewist".to_string();
                    self.status_message_timer = 2 * 60;
                }
            }

            // Als een A/B marker drag zojuist is losgelaten, sla de staat op voor undo
            if drag_ended {
                self.push_undo();
            }

            // Click of drag-release: update playhead position, seek audio-thread if playing
            if let Some(seek_pos) = seek_to {
                self.waveform_play_position = seek_pos;
                self.waveform_state.seek_pending = Some(seek_pos); // ✅ NIEUW: Markeer als pending
                                                                   //    if self.waveform_has_content {
                let _ = self
                    .waveform_cmd_tx
                    .send(WaveformCommand::Seek { pos_secs: seek_pos });
                //  }
            }

            // Toon bestandsinfo rechts
            if self.waveform_state.path.is_some() {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{:.1}s  |  {} Hz  |  Zoom: {}x",
                                self.waveform_state.duration_secs,
                                self.waveform_state.sample_rate,
                                (self.waveform_state.zoom / 50.0 * 100.0) as u32
                            ))
                            .size(11.0)
                            .color(Color32::GRAY),
                        );
                    });
                });
            }

            ui.separator();

            // ── Pitch / Tempo controls ──
            ui.horizontal(|ui| {
                ui.label("Pitch:");
                let old_pitch = self.waveform_state.pitch_semitones;
                let mut pitch = old_pitch;
                ui.add(
                    egui::Slider::new(&mut pitch, -12.0..=12.0)
                        .text("semitones")
                        .step_by(0.5),
                );
                if (pitch - old_pitch).abs() > 0.01 {
                    self.waveform_state.pitch_semitones = pitch;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetPitch(pitch));
                    }
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.pitch_semitones = 0.0;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetPitch(0.0));
                    }
                }

                ui.separator();

                ui.label("Tempo:");
                let old_tempo = self.waveform_state.tempo;
                let mut tempo = old_tempo;
                ui.add(
                    egui::Slider::new(&mut tempo, 0.25..=2.0)
                        .text("x")
                        .step_by(0.05),
                );
                if (tempo - old_tempo).abs() > 0.005 {
                    self.waveform_state.tempo = tempo;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetTempo(tempo));
                    }
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.tempo = 1.0;
                    if self.waveform_is_playing {
                        let _ = self.waveform_cmd_tx.send(WaveformCommand::SetTempo(1.0));
                    }
                }

                ui.separator();

                ui.label("Vol:");
                let old_vol = self.waveform_state.volume;
                let mut vol = old_vol;
                ui.add(
                    egui::Slider::new(&mut vol, 0.0..=2.0)
                        .text("x")
                        .step_by(0.05),
                );
                if (vol - old_vol).abs() > 0.01 {
                    self.waveform_state.volume = vol;
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetVolume(vol));
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.volume = 1.0;
                    let _ = self.waveform_cmd_tx.send(WaveformCommand::SetVolume(1.0));
                }

                // Playback status
                if self.waveform_is_playing {
                    let p = self.waveform_play_position;
                    let d = self.waveform_play_duration;
                    ui.label(
                        RichText::new(format!(
                            "▶ {:02}:{:02} / {:02}:{:02}",
                            (p / 60.0) as u32,
                            p as u32 % 60,
                            (d / 60.0) as u32,
                            d as u32 % 60,
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(100, 200, 100)),
                    );
                }
            });

            ui.separator();

            // ── Loop controls + zoom ──
            ui.horizontal(|ui| {
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        if self.waveform_is_playing {
                            if ui.button("⏹ Stop").clicked() {
                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
                                self.waveform_is_playing = false;
                                ctx.request_repaint();
                            }

                            // Bypass loop toggle
                            let (btn_label, btn_color) = if self.loop_bypassed {
                                ("🔁 Loop: OFF", Color32::from_rgb(200, 100, 100))
                            } else {
                                ("🔁 Loop: ON", Color32::from_rgb(80, 200, 80))
                            };
                            if ui
                                .button(RichText::new(btn_label).color(btn_color))
                                .clicked()
                            {
                                self.loop_bypassed = !self.loop_bypassed;
                                let _ = self
                                    .waveform_cmd_tx
                                    .send(WaveformCommand::SetLoopEnabled(!self.loop_bypassed));
                                self.status_message = if self.loop_bypassed {
                                    "Loop bypassed — speelt door naar einde".to_string()
                                } else {
                                    "Loop hervat".to_string()
                                };
                                self.status_message_timer = 3 * 60;
                            }
                        } else if ui.button("▶ Play Loop").clicked() {
                            if let Some(ref _path) = self.waveform_state.path {
                                let sr = self.waveform_state.sample_rate as f32;
                                let a_sample = (a * sr) as usize;
                                let b_sample = (b * sr) as usize;

                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Play {
                                    samples: self.waveform_state.samples.clone(),
                                    sample_rate: self.waveform_state.sample_rate,
                                    start_sample: a_sample, // Start met spelen op A
                                    segment_start_sec: 0.0, // ✅ De buffer begint nu bij 0.0s van de track
                                    a_sample,
                                    b_sample,
                                    pitch_semitones: Arc::new(AtomicU32::new(f32::to_bits(
                                        self.waveform_state.pitch_semitones,
                                    ))),
                                    tempo: Arc::new(AtomicU32::new(f32::to_bits(
                                        self.waveform_state.tempo,
                                    ))),
                                });
                                self.waveform_is_playing = true;
                                self.loop_iteration_count = 1; // 1e play-through
                                ctx.request_repaint();
                            }
                        }
                    }
                }

                // ── Loop herhaal-teller ──
                if let (Some(a), Some(b)) = (
                    self.waveform_state.loop_a_secs,
                    self.waveform_state.loop_b_secs,
                ) {
                    if b > a {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Herhaal:");
                            let resp = ui.add(
                                egui::DragValue::new(&mut self.loop_repeat_count)
                                    .range(0..=999)
                                    .speed(1.0)
                                    .prefix("× "),
                            );
                            if resp.changed() && self.loop_repeat_count == 0 {
                                self.status_message = "0 = oneindig herhalen".to_string();
                                self.status_message_timer = 2 * 60;
                            }
                            if self.loop_repeat_count > 0 && self.waveform_is_playing {
                                let display = self.loop_iteration_count.min(self.loop_repeat_count);
                                ui.label(format!("({}/{})", display, self.loop_repeat_count));
                            }
                        });
                    }
                }

                // Save Loop
                if self.waveform_state.loop_a_secs.is_some()
                    && self.waveform_state.loop_b_secs.is_some()
                {
                    if ui.button("💾 Save Loop").clicked() {
                        self.save_current_loop();
                    }
                }

                ui.separator();

                // Loop bibliotheek toggle
                if ui.button("📚 Alle Tracks").clicked() {
                    self.show_loop_library = !self.show_loop_library;
                }

                ui.separator();

                // Center loop in viewport
                if ui
                    .button("🎯 Center Loop")
                    .on_hover_text("Centreer de A-B loop in het venster")
                    .clicked()
                {
                    if let (Some(a), Some(b)) = (
                        self.waveform_state.loop_a_secs,
                        self.waveform_state.loop_b_secs,
                    ) {
                        if b > a {
                            self.center_view_on_loop(panel_width);
                        }
                    }
                }

                ui.separator();

                // Zoom
                if ui.button("🔍−").clicked() {
                    self.waveform_state.zoom = (self.waveform_state.zoom / 1.3).max(5.0);
                }
                if ui.button("🔍+").clicked() {
                    self.waveform_state.zoom = (self.waveform_state.zoom * 1.3).min(5000.0);
                }
                if ui.button("⟲ Reset zoom/scroll").clicked() {
                    self.waveform_state.zoom = 50.0;
                    self.waveform_state.scroll_offset = 0.0;
                }
            });

            // ── Track Paneel (onder de knoppen) ──
            if let Some(ref path) = self.waveform_state.path.clone() {
                let track_path = path.clone();
                ui.separator();

                // ── Opgeslagen Loops (altijd zichtbaar) ──
                ui.strong("Opgeslagen Loops");
                let track = self.library.track_for_path(&track_path);
                if track.loops.is_empty() {
                    ui.label(
                        "Nog geen loops opgeslagen. Maak een A-B selectie en klik 'Save Loop'.",
                    );
                } else {
                    let mut delete_idx: Option<usize> = None;
                    let mut load_idx: Option<usize> = None;

                    for (i, saved) in track.loops.iter().enumerate() {
                        ui.horizontal(|ui| {
                            // Toon short_id met gekleurd blokje
                            let id_str = saved
                                .short_id
                                .as_deref()
                                .map(|id| format!("({}) ", id))
                                .unwrap_or_default();
                            let col = saved
                                .short_id
                                .as_deref()
                                .map(|id| color_for_arranger(id, &track_path));
                            if let Some([r, g, b]) = col {
                                let color = Color32::from_rgb(r, g, b);
                                egui::Frame::default()
                                    .fill(color)
                                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(80)))
                                    .show(ui, |ui| {
                                        ui.set_min_size(egui::vec2(10.0, 10.0));
                                    });
                            }
                            ui.label(
                                RichText::new(format!("{}{}", id_str, saved.label))
                                    .size(13.0)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "  {:02}:{:02} → {:02}:{:02}  |  Pitch: {:+.1}  Tempo: {:.2}x",
                                    (saved.loop_a_secs / 60.0) as u32,
                                    saved.loop_a_secs as u32 % 60,
                                    (saved.loop_b_secs / 60.0) as u32,
                                    saved.loop_b_secs as u32 % 60,
                                    saved.pitch_semitones,
                                    saved.tempo,
                                ))
                                .size(11.0)
                                .color(Color32::GRAY),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("❌").clicked() {
                                        delete_idx = Some(i);
                                    }
                                    if ui.small_button("▶").clicked() {
                                        load_idx = Some(i);
                                    }
                                },
                            );
                        });
                    }

                    if let Some(idx) = delete_idx {
                        let track = self.library.track_for_path(&track_path);
                        if idx < track.loops.len() {
                            track.loops.remove(idx);
                            crate::loops::save_library(&self.library);
                        }
                    }

                    if let Some(idx) = load_idx {
                        let saved = {
                            let track = self.library.track_for_path(&track_path);
                            track.loops.get(idx).cloned()
                        };
                        if let Some(saved) = saved {
                            self.waveform_state.loop_a_secs = Some(saved.loop_a_secs);
                            self.waveform_state.loop_b_secs = Some(saved.loop_b_secs);
                            self.waveform_state.pitch_semitones = saved.pitch_semitones;
                            self.waveform_state.tempo = saved.tempo;
                            self.waveform_play_position = saved.loop_a_secs;
                            self.waveform_state.seek_pending = Some(saved.loop_a_secs);
                            self.waveform_state.playhead_frames_after_drag = 15;

                            let _ = self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                                a_secs: saved.loop_a_secs,
                                b_secs: saved.loop_b_secs,
                            });
                            if self.waveform_has_content {
                                let _ = self.waveform_cmd_tx.send(WaveformCommand::Seek {
                                    pos_secs: saved.loop_a_secs,
                                });
                            }
                            self.status_message = format!("Loop '{}' geladen", saved.label);
                            self.status_message_timer = 3 * 60;
                            self.active_loop_idx = Some(idx);
                            self.center_view_on_loop(panel_width);
                        }
                    }
                }

                // ── Chroma detectie knop ──
                if ui
                    .button(if self.chroma_result.is_some() {
                        "🔄 Opnieuw detecteren"
                    } else {
                        "🔍 Detecteer noten"
                    })
                    .on_hover_text("Analyseer de A-B selectie op toonhoogtes")
                    .clicked()
                {
                    let samples = &self.waveform_state.samples;
                    let sr = self.waveform_state.sample_rate;
                    let a = self.waveform_state.loop_a_secs;
                    let b = self.waveform_state.loop_b_secs;
                    if !samples.is_empty() && sr > 0 {
                        self.chroma_result = Some(detect_chroma(samples, sr, a, b));
                        if let Some(chroma) = self.chroma_result {
                            let (note, conf) = chroma.peak();
                            let name = Chroma::note_name(note);
                            self.status_message = format!(
                                "🔍 Meest waarschijnlijke noot: {} ({:.0}% zeker)",
                                name,
                                conf * 100.0
                            );
                            self.status_message_timer = 5 * 60;
                        }
                    }
                }

                // ── Chroma visualisatie ──
                if let Some(chroma) = self.chroma_result {
                    ui.separator();
                    ui.label(RichText::new("Toonhoogtes (chroma)").size(12.0).strong());
                    let bar_max_width = ui.available_width().min(300.0);
                    for i in 0..12 {
                        let val = chroma.0[i];
                        if val < 0.01 {
                            continue;
                        }
                        let bar_width = bar_max_width * val;
                        let name = Chroma::note_name(i);
                        let name_nl = Chroma::note_name_nl(i);
                        let (r, g, b) = match i % 12 {
                            0 | 2 | 4 | 5 | 7 | 9 | 11 => (220, 180, 50), // witte toetsen
                            _ => (100, 100, 100),                         // zwarte toetsen
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:>3} ({})", name, name_nl))
                                    .size(13.0)
                                    .color(Color32::from_rgb(r, g, b)),
                            );
                            let _ = egui::Frame::none().fill(Color32::from_rgb(r, g, b)).show(
                                ui,
                                |ui| {
                                    ui.set_min_size(egui::vec2(bar_width.max(2.0), 12.0));
                                },
                            );
                        });
                    }
                    if let Some((note, conf)) = chroma.compact(0.2).first().copied() {
                        ui.label(
                            RichText::new(format!(
                                "→ Meest waarschijnlijk: {} ({:.0}%)",
                                Chroma::note_name(note),
                                conf * 100.0
                            ))
                            .size(14.0)
                            .strong()
                            .color(Color32::from_rgb(100, 200, 100)),
                        );
                    }
                }

                // ── Notities voor de actieve loop ──
                if let Some(idx) = self.active_loop_idx {
                    let track = self.library.track_for_path(&track_path);
                    if idx < track.loops.len() {
                        let label = track.loops[idx].label.clone();
                        let notes = track.loops[idx].notes.clone();
                        let mut current_notes = notes.clone();

                        ui.separator();
                        ui.label(
                            RichText::new(format!("📝 Notities: {}", label))
                                .size(12.0)
                                .strong(),
                        );
                        let resp = ui.add_sized(
                            egui::vec2(ui.available_width(), 100.0),
                            egui::TextEdit::multiline(&mut current_notes)
                                .hint_text("Akkoorden, noten, transcripties..."),
                        );
                        if resp.lost_focus() || resp.changed() {
                            if let Some(track) = self
                                .library
                                .tracks
                                .iter_mut()
                                .find(|t| t.track_path == track_path)
                            {
                                if idx < track.loops.len() {
                                    track.loops[idx].notes = current_notes;
                                    crate::loops::save_library(&self.library);
                                }
                            }
                        }
                    }
                }
            } // end if let Some(path)
        }); // end CentralPanel.show()

        // ── Alle Tracks bibliotheek (popup) ──
        if self.show_loop_library {
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
                                        ui.label(
                                            RichText::new("📝").size(14.0).color(Color32::GRAY),
                                        );
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.small_button("❌").clicked() {
                                                self.confirm_delete_track = Some((ti, track.label.clone()));
                                            }
                                            if ui.small_button("▶").clicked() {
                                                load_loop_op = Some((ti, 0)); // load track, eerste loop
                                            }
                                        },
                                    );
                                });

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
                                                    RichText::new(format!(
                                                        "{}{}",
                                                        id_str, saved.label
                                                    ))
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
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
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
                                    let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);
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
                                    let _ = self
                                        .waveform_cmd_tx
                                        .send(WaveformCommand::SetPitch(saved.pitch_semitones));
                                    let _ = self
                                        .waveform_cmd_tx
                                        .send(WaveformCommand::SetTempo(saved.tempo));
                                    let _ =
                                        self.waveform_cmd_tx.send(WaveformCommand::SetLoopBounds {
                                            a_secs: saved.loop_a_secs,
                                            b_secs: saved.loop_b_secs,
                                        });
                                    let _ = self.waveform_cmd_tx.send(WaveformCommand::Seek {
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
                    }
                });
        }

        // ── Confirm track delete ──
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

        // ── Shortcut Editor Window ──
        if self.show_shortcut_editor {
            egui::Window::new("⌨ Shortcut Editor")
                .id(egui::Id::new("shortcut_editor_window"))
                .resizable(true)
                .default_size([550.0, 600.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Klik op een actie en druk op een nieuwe toets om te wijzigen.");
                        if ui.button("🔄 Reset alles naar defaults").clicked() {
                            if let Err(e) = self.shortcuts.reset_all() {
                                self.status_message = format!("Fout: {}", e);
                            } else {
                                self.status_message =
                                    "Alle shortcuts gereset naar defaults".to_string();
                            }
                            self.status_message_timer = 3 * 60;
                        }
                    });
                    ui.separator();

                    // Groepeer per categorie
                    let categories = ["Playback", "Loop", "Markers", "View", "File"];
                    for category in categories {
                        ui.heading(category);
                        for action in ShortcutAction::all()
                            .iter()
                            .filter(|a| a.category() == category)
                        {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(action.display_name()).size(13.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("⟲").clicked() {
                                            let _ = self.shortcuts.reset_action(*action);
                                        }

                                        let binding = self
                                            .shortcuts
                                            .binding_for(*action)
                                            .map(|b| b.display())
                                            .unwrap_or_else(|| "—".to_string());

                                        let is_listening =
                                            self.listening_for_action == Some(*action);
                                        let btn_text = if is_listening {
                                            RichText::new("... druk toets ...")
                                                .color(Color32::YELLOW)
                                        } else {
                                            RichText::new(binding)
                                                .color(Color32::from_rgb(200, 200, 60))
                                        };

                                        if ui.button(btn_text).clicked() {
                                            self.listening_for_action = Some(*action);
                                        }
                                    },
                                );
                            });
                        }
                        ui.separator();
                    }
                });
        }

        // ── File dialog (egui-native, geen Windows COM issues) ──
        self.file_dialog.update(ctx);

        if let Some(path) = self.file_dialog.take_selected() {
            let raw = path.to_string_lossy();
            // Strip \\?\ prefix that Windows file dialogs sometimes add
            let prefix = "\\\\?\\";
            let path_str = if raw.starts_with(prefix) {
                raw[prefix.len()..].to_string()
            } else {
                raw.to_string()
            };
            self.file_path = path_str.clone();
            self.load_file(&path_str);
        }

        // ── Export Window ──
        if self.export_state.show_window {
            let track_path = match self.waveform_state.path.clone() {
                Some(p) => p,
                None => {
                    self.export_state.show_window = false;
                    return;
                }
            };
            let track = self.library.track_for_path(&track_path);
            let total = track.loops.len();
            let track_label = track.label.clone();
            let track_loops: Vec<crate::loops::SavedLoop> = track.loops.clone();

            // Keep selected vector in sync with track.loops
            if self.export_state.selected.len() != total {
                self.export_state.selected = vec![false; total];
            }

            // Flag to open the file dialog after the window closure
            let mut will_export = false;
            let mut will_export_mode = ExportMode::Combined;

            egui::Window::new("\u{1F4E4} Export Loops")
                .id(egui::Id::new("export_window"))
                .resizable(true)
                .default_size([500.0, 420.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("\u{1F3B5} {}", track_label))
                                .size(14.0)
                                .strong(),
                        );
                        ui.separator();

                        // ── Select / Deselect All ──
                        ui.horizontal(|ui| {
                            if ui.button("Select All").clicked() {
                                for s in &mut self.export_state.selected {
                                    *s = true;
                                }
                            }
                            if ui.button("Deselect All").clicked() {
                                for s in &mut self.export_state.selected {
                                    *s = false;
                                }
                            }
                        });
                        ui.separator();

                        // ── Loop list with checkboxes ──
                        ui.label(RichText::new("Selecteer loops:").size(13.0).strong());
                        for (i, (label, a, b)) in
                            self.export_state.cached_loop_info.iter().enumerate()
                        {
                            if i >= self.export_state.selected.len() {
                                break;
                            }
                            let checked = &mut self.export_state.selected[i];
                            let time_str = format!(
                                "{:02}:{:02} \u{2192} {:02}:{:02}",
                                (*a / 60.0) as u32,
                                *a as u32 % 60,
                                (*b / 60.0) as u32,
                                *b as u32 % 60,
                            );
                            ui.checkbox(checked, format!("{}  ({})", label, time_str));
                        }

                        ui.separator();

                        // ── Settings ──
                        ui.label(RichText::new("Instellingen:").size(13.0).strong());

                        ui.horizontal(|ui| {
                            ui.label("Basis naam:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.export_state.base_name)
                                    .desired_width(250.0),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label("Formaat:");
                            let fmt = &mut self.export_state.format;
                            egui::ComboBox::from_id_source("export_format")
                                .selected_text("WAV (.wav)")
                                .show_ui(ui, |ui| {
                                    if ui
                                        .selectable_label(*fmt == ExportFormat::Wav, "WAV (.wav)")
                                        .clicked()
                                    {
                                        *fmt = ExportFormat::Wav;
                                    }
                                });
                        });

                        ui.horizontal(|ui| {
                            ui.label("Modus:");
                            let mode = &mut self.export_state.mode;
                            ui.radio_value(mode, ExportMode::Combined, "Gecombineerd bestand");
                            ui.radio_value(mode, ExportMode::Separate, "Aparte bestanden");
                        });

                        ui.separator();

                        // ── Export button ──
                        let selected_count =
                            self.export_state.selected.iter().filter(|&&s| s).count();
                        let can_export = selected_count > 0;

                        ui.horizontal(|ui| {
                            if ui.button("Annuleren").clicked() {
                                self.export_state.show_window = false;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let btn_text = format!(
                                        "\u{1F4E4} Export ({} loop{})",
                                        selected_count,
                                        if selected_count != 1 { "s" } else { "" }
                                    );
                                    let btn = egui::Button::new(RichText::new(btn_text).size(14.0));
                                    let resp = ui.add_enabled(can_export, btn);
                                    if can_export && resp.clicked() {
                                        // Collect selected loops
                                        let selected_loops: Vec<crate::loops::SavedLoop> = self
                                            .export_state
                                            .selected
                                            .iter()
                                            .zip(track_loops.iter())
                                            .filter(|(sel, _)| **sel)
                                            .map(|(_, l)| l.clone())
                                            .collect();

                                        let params = ExportParams {
                                            loops: selected_loops,
                                            base_name: self.export_state.base_name.clone(),
                                            mode: self.export_state.mode,
                                            format: self.export_state.format,
                                            sample_rate: self.waveform_state.sample_rate,
                                            samples: self.waveform_state.samples.clone(),
                                        };
                                        self.export_pending = Some(params);
                                        self.export_state.show_window = false;
                                        will_export = true;
                                        will_export_mode = self.export_state.mode;
                                    }
                                },
                            );
                        });
                    });
                });

            // Open file dialog after the window closure (avoids borrow conflicts)
            if will_export {
                match will_export_mode {
                    ExportMode::Combined => {
                        self.export_dialog.save_file();
                    }
                    ExportMode::Separate => {
                        self.export_dialog.select_directory();
                    }
                }
            }
        }

        // ── Export dialog processing ──
        self.export_dialog.update(ctx);

        if self.export_pending.is_some() {
            let mode = self.export_pending.as_ref().unwrap().mode;
            let path_opt = match mode {
                ExportMode::Combined => self.export_dialog.take_selected(),
                ExportMode::Separate => self.export_dialog.take_selected(),
            };

            if path_opt.is_some() {
                // User confirmed — handled below
            } else if self.export_dialog.state() != egui_file_dialog::DialogState::Open {
                // Dialog was closed without selection
                self.export_pending = None;
            }

            if let Some(path) = path_opt {
                let params = self.export_pending.take().unwrap();
                let result = self.execute_export(&params, &path);
                match result {
                    Ok(msg) => {
                        self.status_message = msg;
                        self.status_message_timer = 6 * 60;
                    }
                    Err(e) => {
                        self.status_message = format!("\u{26A0} Export mislukt: {}", e);
                        self.status_message_timer = 6 * 60;
                    }
                }
            }
        }

        // ── Arranger window ──
        self.show_arranger_ui(ctx);
    }
}

/// Helper om een shortcut-key/uitleg regel te tekenen.
fn shortcut_row(ui: &mut egui::Ui, key: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(key)
                .size(14.0)
                .strong()
                .color(Color32::from_rgb(200, 200, 60)),
        );
        ui.label(
            RichText::new(description)
                .size(13.0)
                .color(Color32::LIGHT_GRAY),
        );
    });
}
