use rustfft::{num_complex::Complex, FftPlanner};
use soundtouch::BPMDetect;
use std::path::PathBuf;
use std::process::Command;
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
    /// Krumhansl-Schmuckler profielen (klassiek, westers)
    const KS_MAJOR: [f32; 12] = [
        6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
    ];
    const KS_MINOR: [f32; 12] = [
        6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
    ];

    /// libkeyfinder profielen (gebaseerd op grotere dataset, beter voor blues/pop)
    const LK_MAJOR: [f32; 12] = [
        7.239, 3.504, 3.584, 2.845, 5.819, 4.559, 2.448, 6.995, 3.391, 4.556, 4.074, 4.459,
    ];
    const LK_MINOR: [f32; 12] = [
        7.003, 3.144, 4.359, 5.404, 3.672, 4.090, 3.908, 6.200, 3.634, 2.872, 5.355, 3.832,
    ];

    /// Bereken correlaties met gegeven profielen voor alle 24 toonaarden.
    fn correlations_for(
        &self,
        major_profile: &[f32; 12],
        minor_profile: &[f32; 12],
    ) -> Vec<(usize, bool, f32)> {
        let total_major: f32 = major_profile.iter().sum();
        let total_minor: f32 = minor_profile.iter().sum();

        let mut results = Vec::with_capacity(24);
        for root in 0..12 {
            let mut corr_maj = 0.0;
            for i in 0..12 {
                let profile_idx = (i + 12 - root) % 12;
                corr_maj += self.0[i] * major_profile[profile_idx];
            }
            results.push((root, false, corr_maj / total_major));

            let mut corr_min = 0.0;
            for i in 0..12 {
                let profile_idx = (i + 12 - root) % 12;
                corr_min += self.0[i] * minor_profile[profile_idx];
            }
            results.push((root, true, corr_min / total_minor));
        }

        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// Top N kandidaten met Krumhansl-Schmuckler profielen.
    pub fn top_candidates(&self, n: usize) -> Vec<(usize, bool, f32)> {
        self.correlations_for(&Self::KS_MAJOR, &Self::KS_MINOR)
            .into_iter()
            .take(n)
            .collect()
    }

    /// Top N kandidaten met libkeyfinder profielen (beter voor blues/pop).
    pub fn top_candidates_lk(&self, n: usize) -> Vec<(usize, bool, f32)> {
        self.correlations_for(&Self::LK_MAJOR, &Self::LK_MINOR)
            .into_iter()
            .take(n)
            .collect()
    }

    /// Toon een toonsoort als leesbare string.
    pub fn key_name_static(root: usize, is_minor: bool) -> String {
        let note = Self::note_name(root);
        let suffix = if is_minor { "m" } else { "" };
        format!("{} {}", note, suffix)
    }

    /// Toon de beste toonsoort als leesbare string (Krumhansl-Schmuckler).
    pub fn key_name(&self) -> String {
        let best = self.top_candidates(1);
        if let Some(&(r, m, _)) = best.first() {
            Self::key_name_static(r, m)
        } else {
            "? ?".to_string()
        }
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

/// Zoek het pad naar keyfinder-cli.exe.
/// Eerst naast de huidige executable, dan fallback naar working directory.
pub fn find_keyfinder_cli() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    // Relatief t.o.v. de executable (target/debug/ of target/release/)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if let Some(gp) = parent.parent() {
                if let Some(ggp) = gp.parent() {
                    candidates.push(ggp.join("keyfinder").join("keyfinder-cli.exe"));
                }
            }
            // Naast de executable zelf
            candidates.push(parent.join("keyfinder-cli.exe"));
        }
    }
    // Relatief t.o.v. working directory
    candidates.push(PathBuf::from("keyfinder/keyfinder-cli.exe"));
    // Probeer ook keyfinder.dll in dezelfde map als de exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("keyfinder").join("keyfinder-cli.exe"));
        }
    }
    for path in &candidates {
        if path.exists() {
            return Some(path.clone());
        }
    }
    None
}

/// Schrijf mono f32 samples naar een tijdelijke WAV-file en retourneer het pad.
fn write_temp_wav(samples: &[f32], sample_rate: u32) -> Result<std::path::PathBuf, String> {
    use hound::{SampleFormat, WavSpec, WavWriter};
    use std::fs;

    let tmp_dir = std::env::temp_dir();
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("Kan temp map niet aanmaken: {}", e))?;

    let path = tmp_dir.join("loopmachine_kf_temp.wav");

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer =
        WavWriter::create(&path, spec).map_err(|e| format!("Kan temp WAV niet maken: {}", e))?;

    for &s in samples {
        writer
            .write_sample(s)
            .map_err(|e| format!("Fout bij schrijven WAV: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Fout bij finalizen WAV: {}", e))?;
    Ok(path)
}

/// Roep keyfinder-cli.exe aan voor toonaarddetectie.
/// Geeft Ok(toonaard) bij succes (bv. "A", "Bbm", "C#"), of Err(foutmelding) bij probleem.
/// Als `start_sec` en `end_sec` zijn ingesteld, wordt alleen dat stuk geanalyseerd.
pub fn detect_key_via_cli(
    samples: &[f32],
    sample_rate: u32,
    start_sec: Option<f32>,
    end_sec: Option<f32>,
) -> Result<String, String> {
    if samples.is_empty() {
        return Err("Geen audiogesamples beschikbaar".to_string());
    }

    let cli_path = find_keyfinder_cli().ok_or_else(|| {
        format!(
            "keyfinder-cli.exe niet gevonden (gezocht in exe-pad en '{}')",
            std::env::current_dir()
                .map(|p| p
                    .join("keyfinder")
                    .join("keyfinder-cli.exe")
                    .to_string_lossy()
                    .to_string())
                .unwrap_or_default()
        )
    })?;

    // Bepaal welk stuk samples we naar WAV schrijven (A-B selectie of hele file)
    let start_sample = start_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(0)
        .min(samples.len().saturating_sub(1));
    let end_sample = end_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(samples.len())
        .min(samples.len());
    let slice = if end_sample > start_sample {
        &samples[start_sample..end_sample]
    } else {
        samples
    };

    // Schrijf samples naar tijdelijke WAV
    let wav_path = write_temp_wav(slice, sample_rate)?;

    // Zoek de map van de CLI (voor DLLs) en stel die in als working directory
    let cli_dir = cli_path.parent().map(|p| p.to_path_buf());
    let mut cmd = Command::new(&cli_path);
    cmd.arg("-n").arg("standard").arg(&wav_path);
    if let Some(ref dir) = cli_dir {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Kan keyfinder-cli niet starten: {}", e))?;

    // Opruimen: verwijder temp WAV
    let _ = std::fs::remove_file(&wav_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "keyfinder-cli gaf foutcode {}{}",
            output
                .status
                .code()
                .map(|c| format!(" ({})", c))
                .unwrap_or_default(),
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        ));
    }
    // Parse output: negeer "Samples loaded" regel en andere info-regels
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);
    let key = combined
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.starts_with("Samples loaded")
                && !t.starts_with("Samplerate")
                && !t.starts_with("Channels")
                && !t.starts_with("Duration")
        })
        .last()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if key.is_empty() {
        Err(format!(
            "keyfinder-cli gaf geen herkenbare output: '{}'",
            combined.trim()
        ))
    } else {
        Ok(key)
    }
}

/// Detecteer BPM (tempo) in audiogesamples.
/// Gebruikt de SoundTouch BPMDetect (autocorrelatie-based).
/// Als `start_sec` en `end_sec` zijn ingesteld, wordt alleen dat stuk geanalyseerd.
pub fn detect_bpm(
    samples: &[f32],
    sample_rate: u32,
    start_sec: Option<f32>,
    end_sec: Option<f32>,
) -> Option<f32> {
    if samples.is_empty() || sample_rate == 0 {
        return None;
    }

    // Bepaal welk stuk samples we analyseren (A-B selectie of hele file)
    let start_sample = start_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(0)
        .min(samples.len().saturating_sub(1));
    let end_sample = end_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(samples.len())
        .min(samples.len());

    if end_sample <= start_sample {
        return None;
    }

    let slice = &samples[start_sample..end_sample];

    let mut detector = BPMDetect::new(1, sample_rate);
    detector.input_samples(slice);
    let bpm = detector.get_bpm();

    if bpm > 0.0 && bpm < 500.0 {
        Some(bpm)
    } else {
        None
    }
}

/// Detecteer individuele beat-posities in audiogesamples.
/// Gebruikt de SoundTouch BPMDetect voor beat-tracking.
/// Retourneert een Vec van `(position_secs, strength)` voor elke gedetecteerde beat.
/// De `strength` is een maat voor betrouwbaarheid (0.0–1.0).
pub fn detect_beats(
    samples: &[f32],
    sample_rate: u32,
    start_sec: Option<f32>,
    end_sec: Option<f32>,
) -> Option<Vec<(f32, f32)>> {
    if samples.is_empty() || sample_rate == 0 {
        return None;
    }

    // Bepaal welk stuk samples we analyseren (A-B selectie of hele file)
    let start_sample = start_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(0)
        .min(samples.len().saturating_sub(1));
    let end_sample = end_sec
        .map(|s| (s * sample_rate as f32) as usize)
        .unwrap_or(samples.len())
        .min(samples.len());

    if end_sample <= start_sample {
        return None;
    }

    let slice = &samples[start_sample..end_sample];

    let mut detector = BPMDetect::new(1, sample_rate);
    detector.input_samples(slice);

    // Vraag de beat posities op
    let max = detector.query_size(10000);
    if max <= 0 {
        return None;
    }
    let mut positions = vec![0.0f32; max as usize];
    let mut strengths = vec![0.0f32; max as usize];
    let count = detector.get_beats(&mut positions, &mut strengths, max);

    if count > 0 {
        let beats: Vec<(f32, f32)> = positions[..count as usize]
            .iter()
            .zip(strengths[..count as usize].iter())
            .map(|(&pos, &str)| (pos, str))
            .collect();
        Some(beats)
    } else {
        None
    }
}
