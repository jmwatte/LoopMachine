pub mod event_handler;
pub mod ui_arranger;
pub mod ui_export;
pub mod ui_library;
pub mod ui_setup;
pub mod ui_shortcuts;
pub mod ui_toolbar;
use self::ui_export::{ExportFormat, ExportMode, ExportParams, ExportState};
use crate::arrangement::{color_for_arranger, Arrangement};
use crate::chroma::{
    detect_beats, detect_bpm, detect_chroma, detect_key_via_cli, Chroma, ChromaMode,
};
use crate::loops::{Library, SavedLoop};
use crate::session::SessionState;
use crate::shortcuts::{ShortcutAction, ShortcutsConfig, ToolbarAction};
use crate::video_player::VideoPlayer;
use crate::waveform::{render_waveform, ChannelMode, WaveformState};
use crate::waveform_player::{start_waveform_thread, WaveformCommand, WaveformEvent};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, RichText};
use egui_file_dialog::FileDialog;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, Mutex};

// ───────────────────────────────────────────────
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
    /// Bass-only chroma (60–250 Hz) voor robuustere toonaarddetectie
    pub bass_chroma: Option<Chroma>,
    /// Keyfinder-cli resultaat: None=nog niet uitgevoerd, Some(Ok(key))=gelukt, Some(Err(e)))=fout
    pub keyfinder_cli_result: Option<Result<String, String>>,
    /// BPM detectie resultaat (SoundTouch)
    pub bpm_result: Option<f32>,
    /// Beat posities (seconden) met betrouwbaarheid (SoundTouch)
    pub bpm_beat_positions: Option<Vec<(f32, f32)>>,
    /// Drempelwaarde voor beat-detectie (0.0-1.0), hoe hoger hoe strenger
    pub bpm_threshold: f32,
    /// Latency compensatie (ms) voor marker plaatsen tijdens afspelen.
    pub playback_latency_ms: f32,
    /// Correctie (ms) voor auto-beat detectie offset (+ = later, - = vroeger)
    pub beat_offset_ms: f32,
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
    /// Laatst gebruikte directory voor file dialog
    pub file_dialog_last_dir: Option<String>,

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

    // Loop label rename
    editing_loop_label: Option<usize>,
    editing_loop_label_buf: String,

    // ── Setup / Kalibratie ──
    pub show_setup: bool,
    /// Gedeelde click-posities voor audit (seconden), thread-safe.
    pub click_positions: Arc<Mutex<Vec<f32>>>,
    /// Audit modus aan/uit
    pub click_enabled: Arc<AtomicBool>,
    /// True = clicks op BPM beats, false = clicks op markers
    pub click_on_bpm: bool,
    /// Kalibratie flits countdown (0 = geen flits)
    pub calibration_flash: u32,
    /// Alle click posities voor de kalibratie (wordt één voor één afgeflitst)
    pub calibration_click_positions: Vec<f32>,
    /// Index van de volgende calibratie-click om te flitsen
    pub calibration_next_idx: usize,
    /// Kalibratie is actief en wacht op de playhead
    pub calibration_active: bool,
    /// Bulk-shift waarde voor markers (ms), persistent in de UI
    pub bulk_shift_ms: i32,

    // ── User-definable toolbar ──
    /// Lijst van acties die als knoppen in de actie-werkbalk verschijnen.
    pub toolbar_buttons: Vec<ToolbarAction>,
    /// Toon het toolbar-editor venster
    pub show_toolbar_editor: bool,

    // ── Video (ffmpeg/mpv) ──
    /// Pad naar ffmpeg executable.
    pub ffmpeg_path: Option<String>,
    /// Pad naar mpv executable.
    pub mpv_path: Option<String>,
    /// Optionele video-player (mpv) voor video-bestanden.
    pub video_player: Option<crate::video_player::VideoPlayer>,
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
            bass_chroma: None,
            keyfinder_cli_result: None,
            bpm_result: None,
            bpm_beat_positions: None,
            bpm_threshold: 0.3,
            playback_latency_ms: 40.0,
            beat_offset_ms: 0.0,
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
            file_dialog: FileDialog::new()
                .add_file_filter(
                    "Audio / Video",
                    std::sync::Arc::new(|p: &std::path::Path| {
                        matches!(
                            p.extension().and_then(|s| s.to_str()),
                            Some(
                                "mp3"
                                    | "wav"
                                    | "flac"
                                    | "ogg"
                                    | "m4a"
                                    | "aac"
                                    | "wma"
                                    | "mp4"
                                    | "mov"
                                    | "avi"
                                    | "mkv"
                                    | "webm"
                            )
                        )
                    }),
                )
                .default_file_filter("Audio / Video"),
            file_dialog_last_dir: None,
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
            editing_loop_label: None,
            editing_loop_label_buf: String::new(),

            // ── Setup / Kalibratie ──
            show_setup: false,
            click_positions: Arc::new(Mutex::new(Vec::new())),
            click_enabled: Arc::new(AtomicBool::new(false)),
            click_on_bpm: true,
            calibration_flash: 0,
            calibration_click_positions: Vec::new(),
            calibration_next_idx: 0,
            calibration_active: false,
            bulk_shift_ms: 0,
            toolbar_buttons: ToolbarAction::default_toolbar(),
            show_toolbar_editor: false,
            ffmpeg_path: None,
            mpv_path: None,
            video_player: None,
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
            app.bpm_threshold = session.bpm_threshold;
            app.playback_latency_ms = session.playback_latency_ms;
            app.beat_offset_ms = session.beat_offset_ms;
            // Herstel video-paden uit sessie
            app.ffmpeg_path = session.ffmpeg_path.clone();
            app.mpv_path = session.mpv_path.clone();
            // Herstel toolbar buttons uit sessie
            if let Some(ref buttons) = session.toolbar_buttons {
                if !buttons.is_empty() {
                    app.toolbar_buttons = buttons.clone();
                }
            }
            // Herstel laatste directory voor file dialog
            if let Some(ref dir) = session.last_directory {
                if Path::new(dir).exists() {
                    app.file_dialog_last_dir = Some(dir.clone());
                    app.file_dialog.config_mut().initial_directory = std::path::PathBuf::from(dir);
                }
            }
        }

        app
    }

    /// Check of het geladen bestand een video is (op basis van extensie).
    pub fn is_video_file(&self) -> bool {
        self.waveform_state.path.as_deref().map_or(false, |p| {
            matches!(
                std::path::Path::new(p).extension().and_then(|s| s.to_str()),
                Some("mp4" | "mov" | "avi" | "mkv" | "webm")
            )
        })
    }

    /// Open video in mpv (als het een video-bestand is en mpv geconfigureerd is).
    pub fn open_video(&mut self) {
        if let (Some(ref path), Some(ref mpv_path)) =
            (self.waveform_state.path.clone(), self.mpv_path.clone())
        {
            if self.is_video_file() {
                let mut player = VideoPlayer::new(mpv_path);
                match player.open(path) {
                    Ok(()) => {
                        self.video_player = Some(player);
                        self.status_message = "🎬 Video-speler geopend".to_string();
                    }
                    Err(e) => {
                        self.status_message = format!("mpv fout: {}", e);
                    }
                }
                self.status_message_timer = 4 * 60;
            }
        }
    }

    /// Sync mpv met huidige play-state (positie, play/pause).
    pub fn sync_video(&self) {
        if let Some(ref player) = self.video_player {
            if self.waveform_is_playing {
                player.resume();
            } else {
                player.pause();
            }
            player.seek(self.waveform_play_position);
        }
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
                self.send_cmd(WaveformCommand::Stop);
                self.waveform_is_playing = false;
            }
            self.waveform_has_content = false;
        }

        match crate::waveform::decode_audio(path, self.waveform_state.channel_mode) {
            Ok((samples, sample_rate, duration_secs, warning)) => {
                self.waveform_state.path = Some(path.to_string());
                // Bouw waveform summary voor snelle weergave bij elke zoom
                let summary = crate::waveform::WaveformSummary::build(&samples);
                self.waveform_state.samples = Arc::new(samples);
                self.waveform_state.sample_rate = sample_rate;
                self.waveform_state.summary = Some(summary);
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
                self.bass_chroma = None;
                self.keyfinder_cli_result = None;
                self.bpm_result = None;
                self.bpm_beat_positions = None;
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

    /// Bereken BPM uit handmatig geplaatste beat markers (B-toets).
    /// Gebruikt de mediane interval tussen opeenvolgende markers.
    fn bpm_from_markers(markers: &[crate::waveform::Marker]) -> String {
        use crate::waveform::MarkerKind;

        // Verzamel alleen beat markers, gesorteerd
        let mut beats: Vec<f32> = markers
            .iter()
            .filter(|m| m.kind == MarkerKind::Beat)
            .map(|m| m.position_secs)
            .collect();
        beats.sort_by(|a, b| a.total_cmp(b));

        if beats.len() < 2 {
            return format!(
                "Marker-BPM: -- ({} beat{})",
                beats.len(),
                if beats.len() == 1 { "" } else { "s" }
            );
        }

        // Bereken intervallen
        let mut intervals: Vec<f32> = beats.windows(2).map(|w| w[1] - w[0]).collect();

        // Filter outliers: verwijder intervallen die >50% afwijken van mediaan
        intervals.sort_by(|a, b| a.total_cmp(b));
        let median = intervals[intervals.len() / 2];
        intervals.retain(|&i| (i - median).abs() / median.max(0.01) < 0.5);

        if intervals.is_empty() {
            return "Marker-BPM: -- (te variabel)".to_string();
        }

        // Herbereken mediaan van gefilterde set
        let mid = intervals.len() / 2;
        let filtered_median = if intervals.len() % 2 == 0 {
            (intervals[mid - 1] + intervals[mid]) / 2.0
        } else {
            intervals[mid]
        };

        let bpm = 60.0 / filtered_median;
        format!("Marker-BPM: {:.1} ({} beats)", bpm, beats.len())
    }

    /// Verwijder markers op basis van type.
    /// - `None`: verwijder alle markers
    /// - `Some(kind)`: verwijder alleen markers van dat type
    fn clear_markers_by_kind(&mut self, kind: Option<crate::waveform::MarkerKind>) {
        use crate::waveform::MarkerKind;
        let before = self.waveform_state.markers.len();
        self.waveform_state.markers.retain(|m| {
            if let Some(k) = kind {
                m.kind != k
            } else {
                false
            }
        });
        let removed = before - self.waveform_state.markers.len();
        if removed > 0 {
            self.push_undo();
            self.sync_markers_to_library();
            let label = match kind {
                None => "alle markers".to_string(),
                Some(MarkerKind::Section) => "sectie-markers".to_string(),
                Some(MarkerKind::Measure) => "maat-markers".to_string(),
                Some(MarkerKind::Beat) => "beat-markers".to_string(),
            };
            self.status_message = format!("{} {} verwijderd", removed, label);
            self.status_message_timer = 3 * 60;
        }
    }

    /// Zet BPM-beat posities om naar echte markers in de waveform.
    /// Zet BPM-beat posities om naar echte markers in de waveform.
    /// Gebruikt `self.bpm_threshold` om ruis weg te filteren.
    fn place_bpm_markers(&mut self) {
        use crate::waveform::MarkerKind;

        let Some(ref beats) = self.bpm_beat_positions else {
            self.status_message = "Geen BPM beat data beschikbaar".to_string();
            self.status_message_timer = 3 * 60;
            return;
        };

        // Verwijder oude BPM-gebaseerde beat markers (die uit een eerdere plaatsing)
        self.waveform_state
            .markers
            .retain(|m| m.kind != MarkerKind::Beat);

        // Plaats markers op elke beat (boven minimale strength)
        // Zelfs bij drempel 0.0 skippen we extreem zwakke beats (< 1% van max)
        let min_strength = self.bpm_threshold.max(0.01);
        let mut count = 0;
        for &(pos_secs, strength) in beats {
            if strength < min_strength {
                continue;
            }
            count += 1;
            let adjusted_pos = (pos_secs + self.beat_offset_ms / 1000.0).max(0.0);
            self.waveform_state.markers.push(crate::waveform::Marker {
                name: format!("B{}", count),
                position_secs: adjusted_pos,
                kind: MarkerKind::Beat,
            });
        }

        self.push_undo();
        self.sync_markers_to_library();
        self.status_message = format!(
            "{} BPM markers geplaatst (strength > {:.0}%)",
            count,
            min_strength * 100.0
        );
        self.status_message_timer = 5 * 60;
    }

    /// Voer volledige detectie uit: chroma (K-S/LK), keyfinder-cli, BPM.
    fn run_detection(&mut self) {
        let samples = &self.waveform_state.samples;
        let sr = self.waveform_state.sample_rate;
        let a = self.waveform_state.loop_a_secs;
        let b = self.waveform_state.loop_b_secs;
        if !samples.is_empty() && sr > 0 {
            self.chroma_result = Some(detect_chroma(samples, sr, a, b, ChromaMode::Full));
            self.bass_chroma = Some(detect_chroma(samples, sr, a, b, ChromaMode::Bass));
            if let Some(bass) = self.bass_chroma {
                let ks_top = bass.top_candidates(3);
                let lk_top = bass.top_candidates_lk(3);
                let ks_keys: String = ks_top
                    .iter()
                    .map(|(r, m, c)| {
                        format!("{} ({:.0}%)", Chroma::key_name_static(*r, *m), c * 100.0)
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                let lk_keys: String = lk_top
                    .iter()
                    .map(|(r, m, c)| {
                        format!("{} ({:.0}%)", Chroma::key_name_static(*r, *m), c * 100.0)
                    })
                    .collect::<Vec<_>>()
                    .join(" | ");
                self.status_message = format!("K-S: {}  /  LK: {}", ks_keys, lk_keys);
                self.status_message_timer = 5 * 60;
            }
            // Keyfinder-cli
            match detect_key_via_cli(samples, sr, a, b) {
                Ok(kf_key) => {
                    self.keyfinder_cli_result = Some(Ok(kf_key.clone()));
                    self.status_message = format!("{}  |  KF: {}", self.status_message, kf_key);
                }
                Err(e) => {
                    self.keyfinder_cli_result = Some(Err(e.clone()));
                    self.status_message = format!("{}  |  KF-fout: {}", self.status_message, e);
                    self.status_message_timer = 5 * 60;
                }
            }
            // BPM detectie
            self.bpm_result = detect_bpm(samples, sr, a, b);
            self.bpm_beat_positions = detect_beats(samples, sr, a, b);
            if let Some(bpm) = self.bpm_result {
                self.status_message = format!("{}  |  BPM: {:.1}", self.status_message, bpm);
            }
        }
    }

    /// Verleng handmatige beat markers over de hele audiofile.
    /// Berekent BPM uit bestaande markers en plaatst er nieuwe bij van 0s tot einde.
    fn extend_beat_markers(&mut self) {
        use crate::waveform::{Marker, MarkerKind};

        // Bepaal BPM en offset uit bestaande beat markers
        let mut beats: Vec<f32> = self
            .waveform_state
            .markers
            .iter()
            .filter(|m| m.kind == MarkerKind::Beat)
            .map(|m| m.position_secs)
            .collect();
        beats.sort_by(|a, b| a.total_cmp(b));

        if beats.len() < 2 {
            self.status_message = "Zet eerst minstens 2 beat markers (B-toets)".to_string();
            self.status_message_timer = 3 * 60;
            return;
        }

        let duration = self.waveform_state.duration_secs;
        if duration <= 0.0 {
            return;
        }

        // Bereken gemiddeld interval
        let intervals: Vec<f32> = beats.windows(2).map(|w| w[1] - w[0]).collect();
        let avg_interval: f32 = intervals.iter().sum::<f32>() / intervals.len() as f32;
        if avg_interval <= 0.0 {
            return;
        }

        // Bepaal offset = het gemiddelde verschil tussen marker posities en hun "grid"-plek
        // Eerste marker bepaalt de fase
        let first_beat = beats[0];
        // Zoek beste offset door alle markers te gebruiken: minimaliseer som van afwijkingen
        let mut best_offset = first_beat % avg_interval;
        // Zorg dat offset niet te dicht bij 0 of avg_interval zit
        if best_offset < 0.0 {
            best_offset += avg_interval;
        }

        // Verwijder oude beat markers
        self.waveform_state
            .markers
            .retain(|m| m.kind != MarkerKind::Beat);

        // Plaats nieuwe beat markers van begin tot einde
        let mut pos = best_offset;
        let mut count = 0;
        while pos < duration {
            count += 1;
            self.waveform_state.markers.push(Marker {
                name: format!("B{}", count),
                position_secs: pos,
                kind: MarkerKind::Beat,
            });
            pos += avg_interval;
        }

        let bpm = 60.0 / avg_interval;
        self.push_undo();
        self.sync_markers_to_library();
        self.status_message = format!(
            "Beat verlengd: {} markers geplaatst ({:.1} BPM)",
            count, bpm
        );
        self.status_message_timer = 5 * 60;
    }

    /// Stuur huidige A-B loop naar de audio-thread.
    fn sync_loop_bounds(&mut self) {
        let a = self.waveform_state.loop_a_secs.unwrap_or(0.0);
        let b = self.waveform_state.loop_b_secs.unwrap_or(0.0);
        self.send_cmd(WaveformCommand::SetLoopBounds {
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
                self.send_cmd(WaveformCommand::PlaySequence {
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
    /// Stuur een commando naar de audio-thread via het crossbeam-kanaal.
    /// Logt een error als het verzenden mislukt (audio-thread is dan gestopt).
    fn send_cmd(&self, cmd: WaveformCommand) {
        if let Err(e) = self.waveform_cmd_tx.send(cmd) {
            log::error!("Kon commando niet verzenden naar audio-thread: {}", e);
        }
    }

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
            self.file_dialog_last_dir.as_deref(),
            self.bpm_threshold,
            self.playback_latency_ms,
            self.beat_offset_ms,
            &self.toolbar_buttons,
            self.ffmpeg_path.as_deref(),
            self.mpv_path.as_deref(),
        );
    }

    /// Herstel een UndoState.
    fn restore_undo(&mut self, state: UndoState) {
        state.apply_to(self);
        self.sync_loop_bounds();
        self.status_message = "Undo/Redo".to_string();
        self.status_message_timer = 2 * 60;
    }

    /// Centreer de viewport op de A-B loop, of op de playhead als er geen loop is.
    fn center_view_on_loop(&mut self, viewport_width_px: f32) {
        if viewport_width_px <= 0.0 || self.waveform_state.duration_secs <= 0.0 {
            return;
        }

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
                return;
            }
        }

        // Geen geldige A-B loop → centreer op de playhead
        let target_zoom = (viewport_width_px * 0.6) / 10.0; // ~10 sec zichtbaar
        self.waveform_state.zoom = target_zoom.clamp(5.0, 5000.0);

        let visible_secs = viewport_width_px / self.waveform_state.zoom;
        let pos = self
            .waveform_play_position
            .clamp(0.0, self.waveform_state.duration_secs);
        let max_scroll = (self.waveform_state.duration_secs - visible_secs).max(0.0);
        self.waveform_state.scroll_offset = (pos - visible_secs / 2.0).clamp(0.0, max_scroll);
    }
}

impl eframe::App for LoopEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_waveform_events(ctx);
        self.housekeeping(ctx);
        self.handle_keyboard_shortcuts(ctx);
        self.handle_drag_drop(ctx);

        self.show_file_toolbar(ctx);
        self.show_action_toolbar(ctx);
        self.show_shortcuts_help(ctx);

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
                        self.send_cmd(WaveformCommand::SetLoopBounds {
                            a_secs: a,
                            b_secs: b,
                        });
                        // Als de loop was bypassed, heractiveer haar bij A/B-wijziging
                        if self.loop_bypassed {
                            self.loop_bypassed = false;
                            self.send_cmd(WaveformCommand::SetLoopEnabled(true));
                            self.status_message = "Loop geüpdatet en geactiveerd".to_string();
                            self.status_message_timer = 3 * 60;
                        }
                    }
                } else {
                    // Rechterklik: loop gewist → stuur 0/0 naar audio-thread + expliciet uitschakelen
                    self.send_cmd(WaveformCommand::SetLoopBounds {
                        a_secs: 0.0,
                        b_secs: 0.0,
                    });
                    self.send_cmd(WaveformCommand::SetLoopEnabled(false));
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
                self.send_cmd(WaveformCommand::Seek { pos_secs: seek_pos });
                //  }
            }

            // Toon bestandsinfo rechts
            if self.waveform_state.path.is_some() {
                ui.horizontal(|ui| {
                    // ── Markers op huidige playhead positie ──
                    let markers_at_pos: Vec<&str> = self
                        .waveform_state
                        .markers
                        .iter()
                        .filter(|m| (m.position_secs - self.waveform_play_position).abs() < 0.05)
                        .map(|m| m.name.as_str())
                        .collect();
                    if !markers_at_pos.is_empty() {
                        ui.label(
                            RichText::new(format!("📍 {}", markers_at_pos.join(", ")))
                                .size(11.0)
                                .color(Color32::from_rgb(180, 180, 220)),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut info = format!(
                            "{:.1}s  |  {} Hz  |  Zoom: {}x",
                            self.waveform_state.duration_secs,
                            self.waveform_state.sample_rate,
                            (self.waveform_state.zoom / 50.0 * 100.0) as u32
                        );
                        if let (Some(a), Some(b)) = (
                            self.waveform_state.loop_a_secs,
                            self.waveform_state.loop_b_secs,
                        ) {
                            if b > a {
                                let len_ms = (b - a) * 1000.0;
                                info = format!("A-B: {:.1}s ({:.0}ms)  |  {}", b - a, len_ms, info);
                            }
                        }
                        ui.label(RichText::new(info).size(11.0).color(Color32::GRAY));
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
                        self.send_cmd(WaveformCommand::SetPitch(pitch));
                    }
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.pitch_semitones = 0.0;
                    if self.waveform_is_playing {
                        self.send_cmd(WaveformCommand::SetPitch(0.0));
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
                        self.send_cmd(WaveformCommand::SetTempo(tempo));
                    }
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.tempo = 1.0;
                    if self.waveform_is_playing {
                        self.send_cmd(WaveformCommand::SetTempo(1.0));
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
                    self.send_cmd(WaveformCommand::SetVolume(vol));
                }
                if ui.button("⟲").clicked() {
                    self.waveform_state.volume = 1.0;
                    self.send_cmd(WaveformCommand::SetVolume(1.0));
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
                                self.send_cmd(WaveformCommand::Stop);
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
                                self.send_cmd(WaveformCommand::SetLoopEnabled(!self.loop_bypassed));
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

                                self.send_cmd(WaveformCommand::Play {
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
                                    click_positions: self.click_positions.clone(),
                                    click_enabled: self.click_enabled.clone(),
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
                    .on_hover_text("Centreer A-B loop of playhead in het venster")
                    .clicked()
                {
                    self.center_view_on_loop(panel_width);
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

            // ── Track Paneel (onder de knoppen) — scrollbaar als venster klein is ──
            egui::ScrollArea::vertical().show(ui, |ui| {
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
                        let mut rename_op: Option<(usize, String)> = None;

                        for (i, saved) in track.loops.iter().enumerate() {
                            ui.horizontal(|ui| {
                                // Knoppen vooraan (links)
                                if ui.small_button("▶").clicked() {
                                    load_idx = Some(i);
                                }
                                if ui.small_button("❌").clicked() {
                                    delete_idx = Some(i);
                                }

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
                                // Loop naam — inline editable via dubbelklik
                                let is_editing = self.editing_loop_label == Some(i);
                                if is_editing {
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(
                                            &mut self.editing_loop_label_buf,
                                        )
                                        .desired_width(200.0),
                                    );
                                    if resp.lost_focus()
                                        || ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))
                                    {
                                        if !self.editing_loop_label_buf.is_empty() {
                                            rename_op =
                                                Some((i, self.editing_loop_label_buf.clone()));
                                        }
                                        self.editing_loop_label = None;
                                        self.editing_loop_label_buf.clear();
                                    }
                                } else {
                                    let label_resp = ui.label(
                                        RichText::new(format!("{}{}", id_str, saved.label))
                                            .size(13.0)
                                            .strong(),
                                    );
                                    if label_resp.double_clicked() {
                                        self.editing_loop_label = Some(i);
                                        self.editing_loop_label_buf = saved.label.clone();
                                    }
                                }
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
                            });
                        }

                        if let Some((idx, new_name)) = rename_op {
                            if let Some(t) = self
                                .library
                                .tracks
                                .iter_mut()
                                .find(|t| t.track_path == track_path)
                            {
                                if idx < t.loops.len() {
                                    t.loops[idx].label = new_name;
                                    crate::loops::save_library(&self.library);
                                }
                            }
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

                                self.send_cmd(WaveformCommand::SetLoopBounds {
                                    a_secs: saved.loop_a_secs,
                                    b_secs: saved.loop_b_secs,
                                });
                                if self.waveform_has_content {
                                    self.send_cmd(WaveformCommand::Seek {
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
                        self.run_detection();
                    }

                    // ── Chroma visualisatie ──
                    if let Some(chroma) = self.chroma_result {
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Toonhoogtes (chroma)").size(12.0).strong());
                            if ui.small_button("❌").clicked() {
                                self.chroma_result = None;
                                self.bass_chroma = None;
                            }
                        });
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
                                0 | 2 | 4 | 5 | 7 | 9 | 11 => (220, 180, 50),
                                _ => (100, 100, 100),
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
                        // Gebruik bas-chroma voor toonaarddetectie (beter voor blues/pop)
                        let key_chroma = self.bass_chroma.unwrap_or(chroma);
                        let (peak_note, peak_conf) = chroma.peak();
                        let peak_name = Chroma::note_name(peak_note);
                        // Krumhansl-Schmuckler (klassiek)
                        let ks_cand = key_chroma.top_candidates(4);
                        let ks_text: String = ks_cand
                            .iter()
                            .map(|(r, m, c)| {
                                format!("{} ({:.0}%)", Chroma::key_name_static(*r, *m), c * 100.0)
                            })
                            .collect::<Vec<_>>()
                            .join(" | ");
                        ui.label(
                            RichText::new(format!("K-S:  {}  ", ks_text))
                                .size(14.0)
                                .strong()
                                .color(Color32::from_rgb(100, 200, 100)),
                        );

                        // libkeyfinder (moderner, beter voor blues/pop)
                        let lk_cand = key_chroma.top_candidates_lk(4);
                        let lk_text: String = lk_cand
                            .iter()
                            .map(|(r, m, c)| {
                                format!("{} ({:.0}%)", Chroma::key_name_static(*r, *m), c * 100.0)
                            })
                            .collect::<Vec<_>>()
                            .join(" | ");
                        ui.label(
                            RichText::new(format!("LK: {}  ", lk_text))
                                .size(14.0)
                                .strong()
                                .color(Color32::from_rgb(220, 180, 80)),
                        );

                        // Keyfinder-cli (externe tool)
                        match &self.keyfinder_cli_result {
                            Some(Ok(key)) => {
                                ui.label(
                                    RichText::new(format!("KF:  {}  ", key))
                                        .size(14.0)
                                        .strong()
                                        .color(Color32::from_rgb(180, 140, 220)),
                                );
                            }
                            Some(Err(e)) => {
                                ui.label(
                                    RichText::new(format!("KF:  ❌ {}", e))
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::from_rgb(220, 80, 80)),
                                );
                            }
                            None => {
                                ui.label(
                                    RichText::new("KF:  (niet beschikbaar)")
                                        .size(13.0)
                                        .color(Color32::GRAY),
                                );
                            }
                        }

                        // BPM (SoundTouch)
                        match self.bpm_result {
                            Some(bpm) => {
                                ui.label(
                                    RichText::new(format!("BPM: {:.1}", bpm))
                                        .size(14.0)
                                        .strong()
                                        .color(Color32::from_rgb(120, 200, 220)),
                                );
                                // Drempel slider + knop om BPM markers te plaatsen
                                if self.bpm_beat_positions.is_some() {
                                    ui.label("↓");
                                    ui.add(
                                        egui::Slider::new(&mut self.bpm_threshold, 0.0..=1.0)
                                            .text("dr")
                                            .step_by(0.05),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut self.beat_offset_ms, -50.0..=50.0)
                                            .text("off ms")
                                            .step_by(1.0),
                                    );
                                    if ui.small_button("📌 Beats").clicked() {
                                        self.place_bpm_markers();
                                    }
                                }
                            }
                            None => {
                                ui.label(
                                    RichText::new("BPM: (niet beschikbaar)")
                                        .size(13.0)
                                        .color(Color32::GRAY),
                                );
                            }
                        }

                        // Marker-BPM (uit handmatige beat markers) + extend knop
                        ui.horizontal(|ui| {
                            let marker_bpm_label =
                                Self::bpm_from_markers(&self.waveform_state.markers);
                            let has_marker_bpm = self
                                .waveform_state
                                .markers
                                .iter()
                                .filter(|m| m.kind == crate::waveform::MarkerKind::Beat)
                                .count()
                                >= 2;
                            ui.label(
                                RichText::new(&marker_bpm_label)
                                    .size(13.0)
                                    .color(Color32::from_rgb(160, 180, 200)),
                            );
                            if has_marker_bpm
                                && ui
                                    .small_button("↗ Verleng")
                                    .on_hover_text("Verspreid beat markers over de hele audio")
                                    .clicked()
                            {
                                self.extend_beat_markers();
                            }
                        });

                        // Sterkste noot
                        ui.label(
                            RichText::new(format!(
                                "Peak: {} ({:.0}%)",
                                peak_name,
                                peak_conf * 100.0
                            ))
                            .size(12.0)
                            .color(Color32::GRAY),
                        );
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
            }); // end ScrollArea
        }); // end CentralPanel.show()

        self.show_library_window(ctx);
        self.show_confirm_delete(ctx);
        self.show_shortcut_editor(ctx);
        self.show_setup_window(ctx);
        // ── File dialog (egui-native, geen Windows COM issues) ──
        self.file_dialog.update(ctx);

        if let Some(path) = self.file_dialog.take_selected() {
            let raw = path.to_string_lossy();
            // Strip \? prefix that Windows file dialogs sometimes add
            let prefix = "\\?\\\\";
            let path_str = if raw.starts_with(prefix) {
                raw[prefix.len()..].to_string()
            } else {
                raw.to_string()
            };
            self.file_path = path_str.clone();
            self.load_file(&path_str);
            // Onthoud de directory voor volgende keer
            if let Some(parent) = path.parent() {
                let dir = parent.to_string_lossy().to_string();
                self.file_dialog_last_dir = Some(dir.clone());
                self.file_dialog.config_mut().initial_directory = parent.to_path_buf();
                self.save_session();
            }
        }

        // ── Export venster ──
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

            egui::Window::new("📤 Export Loops")
                .id(egui::Id::new("export_window"))
                .resizable(true)
                .default_size([500.0, 420.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("🎵 {}", track_label))
                                .size(14.0)
                                .strong(),
                        );
                        ui.separator();
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
