use crate::shortcuts::ToolbarAction;
use serde::{Deserialize, Serialize};

const SESSION_FILE: &str = "session.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub file_path: Option<String>,
    pub play_position: f32,
    pub zoom: f32,
    pub scroll_offset: f32,
    pub loop_a_secs: Option<f32>,
    pub loop_b_secs: Option<f32>,
    pub pitch_semitones: f32,
    pub tempo: f32,
    pub volume: f32,
    pub channel_mode: String,
    /// Arranger parse tekstveld, bewaard tussen sessies.
    #[serde(default)]
    pub arr_parse_buf: String,
    /// Laatst gebruikte directory voor file dialog.
    #[serde(default)]
    pub last_directory: Option<String>,
    /// Drempelwaarde voor BPM beat-detectie (0.0-1.0).
    #[serde(default)]
    pub bpm_threshold: f32,
    /// Latency compensatie (ms) voor marker plaatsen tijdens afspelen.
    #[serde(default)]
    pub playback_latency_ms: f32,
    /// Correctie (ms) voor auto-beat detectie offset (+ = later, - = vroeger).
    #[serde(default)]
    pub beat_offset_ms: f32,
    /// User-definable toolbar knoppen.
    #[serde(default)]
    pub toolbar_buttons: Option<Vec<ToolbarAction>>,
}

impl SessionState {
    pub fn save(
        file_path: Option<&str>,
        play_position: f32,
        zoom: f32,
        scroll_offset: f32,
        loop_a_secs: Option<f32>,
        loop_b_secs: Option<f32>,
        pitch_semitones: f32,
        tempo: f32,
        volume: f32,
        channel_mode: &str,
        arr_parse_buf: &str,
        last_directory: Option<&str>,
        bpm_threshold: f32,
        playback_latency_ms: f32,
        beat_offset_ms: f32,
        toolbar_buttons: &[ToolbarAction],
    ) {
        let state = SessionState {
            file_path: file_path.map(|s| s.to_string()),
            play_position,
            zoom,
            scroll_offset,
            loop_a_secs,
            loop_b_secs,
            pitch_semitones,
            tempo,
            volume,
            channel_mode: channel_mode.to_string(),
            arr_parse_buf: arr_parse_buf.to_string(),
            last_directory: last_directory.map(|s| s.to_string()),
            bpm_threshold,
            playback_latency_ms,
            beat_offset_ms,
            toolbar_buttons: Some(toolbar_buttons.to_vec()),
        };
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(SESSION_FILE, json);
        }
    }

    pub fn load() -> Option<SessionState> {
        match std::fs::read_to_string(SESSION_FILE) {
            Ok(json) => serde_json::from_str(&json).ok(),
            Err(_) => None,
        }
    }
}
