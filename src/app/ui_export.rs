use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

use crate::app::LoopEditorApp;
use crate::loops::SavedLoop;

// ───────────────────────────────────────────────
// Export types
// ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportMode {
    Separate,
    Combined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Wav,
}

/// State collected when user clicks "Export" — held while the file dialog is open.
#[derive(Clone)]
pub struct ExportParams {
    pub loops: Vec<SavedLoop>,
    pub base_name: String,
    pub mode: ExportMode,
    #[allow(dead_code)]
    pub format: ExportFormat,
    pub sample_rate: u32,
    pub samples: Arc<Vec<f32>>,
}

#[derive(Clone)]
pub struct ExportState {
    pub show_window: bool,
    pub selected: Vec<bool>,
    pub base_name: String,
    pub mode: ExportMode,
    pub format: ExportFormat,
    /// Cache of (label, a_secs, b_secs) populated once when window opens,
    /// avoids cloning the full SavedLoop vector every frame.
    pub cached_loop_info: Vec<(String, f32, f32)>,
}

// ───────────────────────────────────────────────
// Export methods on LoopEditorApp
// ───────────────────────────────────────────────

impl LoopEditorApp {
    pub fn open_export_window(&mut self) {
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

    fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
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

    pub fn execute_export(&self, params: &ExportParams, target: &Path) -> Result<String, String> {
        match params.mode {
            ExportMode::Combined => self.export_combined(params, target),
            ExportMode::Separate => self.export_separate(params, target),
        }
    }

    fn export_combined(&self, params: &ExportParams, path: &Path) -> Result<String, String> {
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

    fn export_separate(&self, params: &ExportParams, dir: &Path) -> Result<String, String> {
        let sr = params.sample_rate as f32;
        let sample_len = params.samples.len();
        let mut count = 0usize;

        for saved in &params.loops {
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
            let final_slug = if label_slug.trim_matches('_').is_empty() {
                format!("loop_{}", count + 1)
            } else {
                label_slug
            };

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
}
