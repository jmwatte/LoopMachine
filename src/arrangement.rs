use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::loops::Library;

// ───────────────────────────────────────────────
// Data-model
// ───────────────────────────────────────────────

/// Een arrangement: een benoemde lijst van stappen (loops) in een bepaalde volgorde.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arrangement {
    pub name: String,
    pub steps: Vec<ArrStep>,
}

/// Eén stap in een arrangement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrStep {
    /// Korte identifier van de loop (bv. "a", "b", "aa")
    pub loop_id: String,

    /// Pad naar het audiobestand (welke track)
    pub track_path: String,

    /// Hoe vaak deze stap herhaald moet worden (0 = oneindig)
    pub repeats: u32,

    /// Optionele overschrijving van pitch (halve tonen)
    #[serde(default)]
    pub pitch_semitones: f32,

    /// Optionele overschrijving van tempo (1.0 = normaal)
    #[serde(default = "default_tempo")]
    pub tempo: f32,

    /// Kleur voor visuele weergave (automatisch gegenereerd, maar overschrijfbaar)
    #[serde(default = "default_color")]
    pub color: [u8; 3],
}

fn default_tempo() -> f32 {
    1.0
}

fn default_color() -> [u8; 3] {
    [128; 3]
}

// ───────────────────────────────────────────────
// Kleur hashen — voor consistente kleuren per loop
// ───────────────────────────────────────────────

pub fn color_for_arranger(loop_id: &str, track_path: &str) -> [u8; 3] {
    let mut hasher = DefaultHasher::new();
    format!("{}:{}", track_path, loop_id).hash(&mut hasher);
    let hash = hasher.finish();

    let hue = (hash % 360) as u16; // 0-359
    let saturation: u8 = 180; // 70%
    let value: u8 = 230; // 90%
    hsv_to_rgb(hue, saturation, value)
}

/// Converteer HSV naar RGB.
/// h: 0-360, s: 0-255, v: 0-255
fn hsv_to_rgb(h: u16, s: u8, v: u8) -> [u8; 3] {
    if s == 0 {
        return [v, v, v];
    }

    let region = h / 60;
    let fpart = (h % 60) as f32 / 60.0;
    let p = ((v as u32) * (255 - s as u32) / 255) as u8;
    let q = ((v as u32) * (255 - ((s as u32) * (fpart * 255.0) as u32 / 255)) / 255) as u8;
    let t = ((v as u32) * (255 - ((s as u32) * ((1.0 - fpart) * 255.0) as u32 / 255)) / 255) as u8;

    match region {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

// ───────────────────────────────────────────────
// ID generatie
// ───────────────────────────────────────────────

/// Genereer een nieuwe unieke korte ID.
/// Volgorde: a, b, ..., z, aa, ab, ..., az, ba, bb, ..., zz
pub fn generate_short_id(existing_ids: &[String]) -> String {
    for id in all_possible_ids() {
        if !existing_ids.contains(&id) {
            return id;
        }
    }
    // Zou niet mogen gebeuren (702+ IDs per track)
    "??".to_string()
}

fn all_possible_ids() -> Vec<String> {
    let mut ids = Vec::new();

    // a-z
    for c in b'a'..=b'z' {
        ids.push(String::from_utf8(vec![c]).unwrap());
    }

    // aa-zz
    for c1 in b'a'..=b'z' {
        for c2 in b'a'..=b'z' {
            ids.push(String::from_utf8(vec![c1, c2]).unwrap());
        }
    }

    ids
}

// ───────────────────────────────────────────────
// Parser — tekstuele notatie naar stappen
// ───────────────────────────────────────────────

/// Parse een notatie string naar een lijst van (loop_id, repeats) pairs.
///
/// Voorbeelden:
/// - "ABC"  → [(a,1), (b,1), (c,1)]
/// - "2b3A" → [(b,2), (a,3)]
/// - "(aa)b" → [(aa,1), (b,1)]
pub fn parse_arranger_string(input: &str) -> Result<Vec<(String, u32)>, String> {
    let s: Vec<char> = input.to_lowercase().chars().collect();
    let mut i = 0;
    let mut result = Vec::new();

    while i < s.len() {
        // Spaties overslaan
        if s[i].is_whitespace() {
            i += 1;
            continue;
        }

        // Haakjes openen
        if s[i] == '(' {
            i += 1;
            // Optioneel getal
            let mut repeats = 1u32;
            if i < s.len() && s[i].is_ascii_digit() {
                let mut n = 0u32;
                while i < s.len() && s[i].is_ascii_digit() {
                    n = n * 10 + (s[i] as u8 - b'0') as u32;
                    i += 1;
                }
                repeats = n;
            }
            // Binnen haakjes: één ID (1 of 2 letters)
            let id = read_id(&s, &mut i)?;
            // Sluit-haakje
            if i >= s.len() || s[i] != ')' {
                return Err(format!("Missend haakje op positie {}", i));
            }
            i += 1; // sluit-haakje overslaan
            result.push((id, repeats));
            continue;
        }

        // Cijfer → herhalingen
        let mut repeats = 1u32;
        if s[i].is_ascii_digit() {
            repeats = 0;
            while i < s.len() && s[i].is_ascii_digit() {
                repeats = repeats * 10 + (s[i] as u8 - b'0') as u32;
                i += 1;
            }
        }

        // Letter → ID
        if i < s.len() && s[i].is_ascii_lowercase() {
            let id = read_id(&s, &mut i)?;
            result.push((id, repeats));
        } else if i < s.len() {
            return Err(format!("Ongeldig teken '{}' op positie {}", s[i], i));
        }
    }

    Ok(result)
}

/// Lees één ID van 1 of 2 letters op de huidige positie.
fn read_id(s: &[char], i: &mut usize) -> Result<String, String> {
    if *i >= s.len() || !s[*i].is_ascii_lowercase() {
        return Err(format!("Verwacht een letter op positie {}", *i));
    }

    let mut id = String::new();
    id.push(s[*i]);
    *i += 1;

    // Tweede letter? Alleen als het geen scheidingsteken volgt
    if *i < s.len() && s[*i].is_ascii_lowercase() && !is_delimiter_after(s, *i) {
        id.push(s[*i]);
        *i += 1;
    }

    Ok(id)
}

/// Check of het teken op positie i een delimiter is (cijfer, spatie, haakje, of einde-van-groep).
fn is_delimiter_after(s: &[char], i: usize) -> bool {
    if i + 1 >= s.len() {
        return true;
    }
    let next = s[i + 1];
    next.is_ascii_digit() || next.is_whitespace() || next == ')' || next == '('
}

// ───────────────────────────────────────────────
// SequenceStep — voor audio-thread communicatie
// ───────────────────────────────────────────────

/// Één stap in een sequentie voor de audio-thread.
/// Dit is de "gecompileerde" versie van ArrStep, met samples geladen.
#[derive(Debug, Clone)]
pub struct SequenceStep {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub start_sample: usize,
    pub end_sample: usize,
    pub repeats: u32,
}

// ───────────────────────────────────────────────
// Laden / Opslaan
// ───────────────────────────────────────────────

const ARRANGEMENTS_FILE: &str = "arrangements.json";

/// Laad arrangementen van schijf.
pub fn load_arrangements() -> Vec<Arrangement> {
    if let Ok(json) = std::fs::read_to_string(ARRANGEMENTS_FILE) {
        if let Ok(arr) = serde_json::from_str(&json) {
            return arr;
        }
    }
    Vec::new()
}

/// Sla arrangementen weg naar schijf.
pub fn save_arrangements(arrangements: &[Arrangement]) {
    if let Ok(json) = serde_json::to_string_pretty(arrangements) {
        let _ = std::fs::write(ARRANGEMENTS_FILE, json);
    }
}

// ───────────────────────────────────────────────
// Helper: build SequenceStep uit ArrStep + Library
// ───────────────────────────────────────────────

/// Converteer een ArrStep naar SequenceStep.
/// library_samples: map van track_path → (samples, sample_rate)
pub fn build_sequence_step(
    step: &ArrStep,
    samples_map: &std::collections::HashMap<String, (Arc<Vec<f32>>, u32)>,
    library: &mut Library,
) -> Option<SequenceStep> {
    let (samples, sample_rate) = samples_map.get(&step.track_path)?;
    let track = library.track_for_path(&step.track_path);
    let saved_loop = track
        .loops
        .iter()
        .find(|l| l.short_id.as_deref() == Some(&step.loop_id))?;

    let start_sample = (saved_loop.loop_a_secs * *sample_rate as f32) as usize;
    let end_sample = (saved_loop.loop_b_secs * *sample_rate as f32) as usize;

    Some(SequenceStep {
        samples: samples.clone(),
        sample_rate: *sample_rate,
        start_sample,
        end_sample,
        repeats: step.repeats,
    })
}
