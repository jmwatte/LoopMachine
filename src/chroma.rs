use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::{Mutex, OnceLock};

static FFT_PLANNER: OnceLock<Mutex<FftPlanner<f32>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaMode {
    /// Volledig frequentiespectrum
    Full,
    /// Alleen basfrequenties (40–250 Hz) — vangt lage E/F op basgitaar
    Bass,
}

/// Chroma resultaat: 12 waarden (C, C#, D, ..., B) van 0.0 tot 1.0
#[derive(Debug, Clone, Copy)]
pub struct Chroma(pub [f32; 12]);

impl Chroma {
    pub fn note_name(index: usize) -> &'static str {
        [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ][index % 12]
    }

    pub fn note_name_nl(index: usize) -> &'static str {
        [
            "Do", "Do#", "Re", "Re#", "Mi", "Fa", "Fa#", "Sol", "Sol#", "La", "La#", "Si",
        ][index % 12]
    }

    /// Hoogste noot (index 0-11)
    pub fn peak(&self) -> (usize, f32) {
        let mut best = (0, self.0[0]);
        for (i, &v) in self.0.iter().enumerate() {
            if v > best.1 {
                best = (i, v);
            }
        }
        best
    }

    /// Genereer een compacte weergave: alleen noten > drempel
    #[allow(dead_code)]
    pub fn compact(&self, threshold: f32) -> Vec<(usize, f32)> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, &v)| v > threshold)
            .map(|(i, &v)| (i, v))
            .collect()
    }
    /// Detecteer de meest waarschijnlijke toonsoort met Krumhansl-Schmuckler.
    /// Geeft (root_index, is_minor, confidence) terug.
    pub fn detect_key(&self) -> (usize, bool, f32) {
        // Krumhansl-Schmuckler key profiles (Krumhansl, 1990)
        let major_profile: [f32; 12] = [
            6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
        ];
        let minor_profile: [f32; 12] = [
            6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
        ];

        let mut best_corr = -1.0f32;
        let mut best_root = 0usize;
        let mut best_is_minor = false;

        for root in 0..12 {
            // Major
            let mut corr = 0.0;
            for i in 0..12 {
                let profile_idx = (i + 12 - root) % 12;
                corr += self.0[i] * major_profile[profile_idx];
            }
            if corr > best_corr {
                best_corr = corr;
                best_root = root;
                best_is_minor = false;
            }

            // Minor
            corr = 0.0;
            for i in 0..12 {
                let profile_idx = (i + 12 - root) % 12;
                corr += self.0[i] * minor_profile[profile_idx];
            }
            if corr > best_corr {
                best_corr = corr;
                best_root = root;
                best_is_minor = true;
            }
        }

        (best_root, best_is_minor, best_corr)
    }

    /// Toon de toonsoort als leesbare string.
    pub fn key_name(&self) -> String {
        let (root, is_minor, _conf) = self.detect_key();
        let note = Self::note_name(root);
        let suffix = if is_minor { "m" } else { "" };
        format!("{} {}", note, suffix)
    }
}
/// Bereken chroma voor een slice audio.
/// `samples` = mono f32 samples, `sample_rate` = sample rate in Hz.
/// `start_sec` / `end_sec` = tijdbereik in seconden.
/// `mode` = `ChromaMode::Full` (volledig spectrum) of `ChromaMode::Bass` (alleen 60–250 Hz).
pub fn detect_chroma(
    samples: &[f32],
    sample_rate: u32,
    start_sec: Option<f32>,
    end_sec: Option<f32>,
    mode: ChromaMode,
) -> Chroma {
    let fft_size = 8192;
    let hop_size = 4096;

    let start_sample = start_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(0)
        .min(samples.len().saturating_sub(fft_size));
    let end_sample = end_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(samples.len())
        .min(samples.len());

    if end_sample <= start_sample + fft_size {
        return Chroma([0.0; 12]);
    }

    let planner = FFT_PLANNER.get_or_init(|| Mutex::new(FftPlanner::new()));
    let fft = planner.lock().unwrap().plan_fft_forward(fft_size);

    // Bereken frequentiebereik voor bass-modus
    let freq_per_bin = sample_rate as f32 / fft_size as f32;
    let (bin_start, bin_end) = if mode == ChromaMode::Bass {
        let start = (40.0 / freq_per_bin) as usize; // ~40 Hz (vangt lage E/F op bas)
        let end = (250.0 / freq_per_bin) as usize; // ~250 Hz
        (start.max(1), end.min(fft_size / 2))
    } else {
        (1, fft_size / 2)
    };

    let mut chroma_sum = [0.0_f64; 12];
    let mut _frame_count = 0_u32;

    // Hann window
    let window: Vec<f32> = (0..fft_size)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos())
        })
        .collect();

    let mut frame = start_sample;
    while frame + fft_size <= end_sample {
        // Venster toepassen + FFT
        let mut buffer: Vec<Complex<f32>> = (0..fft_size)
            .map(|i| Complex::new(samples[frame + i] * window[i], 0.0))
            .collect();

        fft.process(&mut buffer);

        // Energie per FFT bin (alleen relevante bins)
        for bin in bin_start..bin_end {
            let energy = buffer[bin].norm_sqr();
            if energy < 1e-12 {
                continue;
            }

            // Frequentie van deze bin
            let freq = bin as f32 * sample_rate as f32 / fft_size as f32;

            // Chroma: log2(freq / 440) * 12 + 9  (C = 0, A = 9)
            let chroma_raw = (freq / 440.0).log2() * 12.0 + 9.0;
            let note_f = chroma_raw.rem_euclid(12.0);
            // ✅ Fix: afronden i.p.v. afkappen — voorkomt dat een licht ongestemde A (439 Hz)
            //    als G# wordt geïnterpreteerd.
            let note_idx = (note_f + 0.5) as usize % 12;

            chroma_sum[note_idx] += energy as f64;
        }

        _frame_count += 1;
        frame += hop_size;
    }

    // Normaliseer
    let max = chroma_sum.iter().cloned().fold(0.0_f64, f64::max);
    let mut result = [0.0_f32; 12];
    if max > 1e-12 {
        for i in 0..12 {
            result[i] = (chroma_sum[i] / max) as f32;
        }
    }

    Chroma(result)
}
