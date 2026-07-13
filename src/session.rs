use crate::shortcuts::ToolbarAction;
use log;
use serde::{Deserialize, Serialize};

/// Bepaal de data-directory: %APPDATA%/LoopMachine (Windows) of ~/.local/share/loopmachine (overig).
/// Alle data-bestanden worden hier opgeslagen, onafhankelijk van waar de executable staat.
pub fn data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let dir = std::path::PathBuf::from(appdata).join("LoopMachine");
            let _ = std::fs::create_dir_all(&dir);
            return dir;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let dir = std::path::PathBuf::from(home).join(".local/share/loopmachine");
            let _ = std::fs::create_dir_all(&dir);
            return dir;
        }
    }
    // Fallback: naast de executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.to_path_buf();
        }
    }
    std::path::PathBuf::from(".")
}

fn session_path() -> std::path::PathBuf {
    data_dir().join("session.json")
}

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
    /// Pad naar ffmpeg executable (optioneel, voor video).
    #[serde(default)]
    pub ffmpeg_path: Option<String>,
    /// Pad naar mpv executable (optioneel, voor video).
    #[serde(default)]
    pub mpv_path: Option<String>,
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
        ffmpeg_path: Option<&str>,
        mpv_path: Option<&str>,
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
            ffmpeg_path: ffmpeg_path.map(|s| s.to_string()),
            mpv_path: mpv_path.map(|s| s.to_string()),
        };
        match serde_json::to_string_pretty(&state) {
            Ok(json) => {
                let path = session_path();
                if let Err(e) = std::fs::write(&path, &json) {
                    log::error!("Kon sessie niet opslaan naar '{}': {}", path.display(), e);
                }
            }
            Err(e) => {
                log::error!("Kon sessie niet serialiseren: {}", e);
            }
        }
    }

    pub fn load() -> Option<SessionState> {
        let path = session_path();
        match std::fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).ok(),
            Err(_) => None,
        }
    }
}

// ───────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_serde_roundtrip() {
        let state = SessionState {
            file_path: Some("/test/song.wav".to_string()),
            play_position: 42.5,
            zoom: 150.0,
            scroll_offset: 10.0,
            loop_a_secs: Some(5.0),
            loop_b_secs: Some(30.0),
            pitch_semitones: 0.0,
            tempo: 1.0,
            volume: 0.8,
            channel_mode: "Mono".to_string(),
            arr_parse_buf: "ABC".to_string(),
            last_directory: Some("C:\\Music".to_string()),
            bpm_threshold: 0.3,
            playback_latency_ms: 40.0,
            beat_offset_ms: 0.0,
            toolbar_buttons: None,
            ffmpeg_path: None,
            mpv_path: None,
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        let restored: SessionState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.file_path, restored.file_path);
        assert_eq!(state.play_position, restored.play_position);
        assert_eq!(state.zoom, restored.zoom);
        assert_eq!(state.scroll_offset, restored.scroll_offset);
        assert_eq!(state.loop_a_secs, restored.loop_a_secs);
        assert_eq!(state.loop_b_secs, restored.loop_b_secs);
        assert_eq!(state.pitch_semitones, restored.pitch_semitones);
        assert_eq!(state.tempo, restored.tempo);
        assert_eq!(state.volume, restored.volume);
        assert_eq!(state.channel_mode, restored.channel_mode);
        assert_eq!(state.arr_parse_buf, restored.arr_parse_buf);
        assert_eq!(state.last_directory, restored.last_directory);
        assert_eq!(state.bpm_threshold, restored.bpm_threshold);
        assert_eq!(state.playback_latency_ms, restored.playback_latency_ms);
        assert_eq!(state.beat_offset_ms, restored.beat_offset_ms);
        assert_eq!(state.toolbar_buttons, restored.toolbar_buttons);
        assert_eq!(state.ffmpeg_path, restored.ffmpeg_path);
        assert_eq!(state.mpv_path, restored.mpv_path);
    }

    #[test]
    fn test_session_state_defaults_from_json() {
        // Alle #[serde(default)] velden ontbreken in JSON
        let json = r#"{
            "file_path": "/test.wav",
            "play_position": 0.0,
            "zoom": 100.0,
            "scroll_offset": 0.0,
            "channel_mode": "Mono",
            "loop_a_secs": null,
            "loop_b_secs": null,
            "pitch_semitones": 0.0,
            "tempo": 1.0,
            "volume": 1.0
        }"#;

        let state: SessionState = serde_json::from_str(json).unwrap();
        assert_eq!(state.arr_parse_buf, "");
        assert_eq!(state.last_directory, None);
        assert_eq!(state.bpm_threshold, 0.0); // default van f32 is 0.0
        assert_eq!(state.playback_latency_ms, 0.0);
        assert_eq!(state.beat_offset_ms, 0.0);
        assert_eq!(state.toolbar_buttons, None);
        assert_eq!(state.ffmpeg_path, None);
        assert_eq!(state.mpv_path, None);
    }

    #[test]
    fn test_session_state_with_toolbar() {
        let state = SessionState {
            file_path: None,
            play_position: 0.0,
            zoom: 100.0,
            scroll_offset: 0.0,
            loop_a_secs: None,
            loop_b_secs: None,
            pitch_semitones: 0.0,
            tempo: 1.0,
            volume: 1.0,
            channel_mode: "Stereo".to_string(),
            arr_parse_buf: String::new(),
            last_directory: None,
            bpm_threshold: 0.5,
            playback_latency_ms: 20.0,
            beat_offset_ms: 5.0,
            toolbar_buttons: Some(vec![
                ToolbarAction::Detect,
                ToolbarAction::Undo,
                ToolbarAction::SaveLoop,
            ]),
            ffmpeg_path: None,
            mpv_path: None,
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: SessionState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.bpm_threshold, 0.5);
        assert_eq!(restored.playback_latency_ms, 20.0);
        assert_eq!(restored.beat_offset_ms, 5.0);
        assert_eq!(
            restored.toolbar_buttons,
            Some(vec![
                ToolbarAction::Detect,
                ToolbarAction::Undo,
                ToolbarAction::SaveLoop,
            ])
        );
    }

    #[test]
    fn test_session_state_none_file_path() {
        let state = SessionState {
            file_path: None,
            play_position: 0.0,
            zoom: 100.0,
            scroll_offset: 0.0,
            loop_a_secs: None,
            loop_b_secs: None,
            pitch_semitones: 0.0,
            tempo: 1.0,
            volume: 1.0,
            channel_mode: "Mono".to_string(),
            arr_parse_buf: String::new(),
            last_directory: None,
            bpm_threshold: 0.0,
            playback_latency_ms: 0.0,
            beat_offset_ms: 0.0,
            toolbar_buttons: None,
            ffmpeg_path: None,
            mpv_path: None,
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.file_path, None);
    }
}
