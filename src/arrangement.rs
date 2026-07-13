use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

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
    for id in all_possible_ids().iter() {
        if !existing_ids.contains(id) {
            return id.clone();
        }
    }
    // Zou niet mogen gebeuren (702+ IDs per track)
    "??".to_string()
}

fn all_possible_ids() -> &'static Vec<String> {
    static CACHE: OnceLock<Vec<String>> = OnceLock::new();
    CACHE.get_or_init(|| {
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
    })
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

fn arrangements_path() -> std::path::PathBuf {
    crate::session::data_dir().join("arrangements.json")
}

/// Laad arrangementen van schijf.
/// Kleurt alle stappen die nog de default grijze kleur
/// hebben opnieuw in via de hash van track_path + loop_id.
pub fn load_arrangements() -> Vec<Arrangement> {
    if let Ok(json) = std::fs::read_to_string(arrangements_path()) {
        if let Ok(mut arr) = serde_json::from_str::<Vec<Arrangement>>(&json) {
            fixup_colors(&mut arr);
            return arr;
        }
    }
    Vec::new()
}

/// Vervang default grijze kleuren door gehashte kleuren op basis van track + loop_id.
fn fixup_colors(arrangements: &mut [Arrangement]) {
    for arr in arrangements {
        for step in &mut arr.steps {
            if step.color == [128; 3] {
                step.color = color_for_arranger(&step.loop_id, &step.track_path);
            }
        }
    }
}

/// Sla arrangementen weg naar schijf.
pub fn save_arrangements(arrangements: &[Arrangement]) {
    match serde_json::to_string_pretty(arrangements) {
        Ok(json) => {
            if let Err(e) = std::fs::write(arrangements_path(), &json) {
                log::error!(
                    "Kon arrangementen niet opslaan naar '{}': {}",
                    arrangements_path().display(),
                    e
                );
            }
        }
        Err(e) => {
            log::error!("Kon arrangementen niet serialiseren: {}", e);
        }
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

// ───────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── generate_short_id tests ──

    #[test]
    fn test_generate_short_id_empty() {
        let ids: Vec<String> = vec![];
        assert_eq!(generate_short_id(&ids), "a");
    }

    #[test]
    fn test_generate_short_id_sequential() {
        let mut ids: Vec<String> = vec![];
        for expected in ["a", "b", "c", "d", "e"] {
            let id = generate_short_id(&ids);
            assert_eq!(id, expected);
            ids.push(id);
        }
    }

    #[test]
    fn test_generate_short_id_after_z() {
        let ids: Vec<String> = ('a'..='z').map(|c| c.to_string()).collect();
        let next = generate_short_id(&ids);
        assert_eq!(next, "aa");
    }

    #[test]
    fn test_generate_short_id_with_gaps() {
        let ids = vec!["a".to_string(), "b".to_string(), "aa".to_string()];
        assert_eq!(generate_short_id(&ids), "c");
    }

    #[test]
    fn test_generate_short_id_exhaustive_start() {
        let ids: Vec<String> = vec![];
        let first = generate_short_id(&ids);
        assert_eq!(first, "a", "eerste ID moet 'a' zijn");
    }

    #[test]
    fn test_all_possible_ids_count() {
        let ids = all_possible_ids();
        assert_eq!(ids.len(), 702); // 26 + 26*26
        assert_eq!(ids[0], "a");
        assert_eq!(ids[25], "z");
        assert_eq!(ids[26], "aa");
        assert_eq!(ids[701], "zz");
    }

    #[test]
    fn test_all_possible_ids_no_duplicates() {
        let ids = all_possible_ids();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "IDs mogen geen duplicaten bevatten"
        );
    }

    // ── parse_arranger_string tests ──

    #[test]
    fn test_parse_empty() {
        let result = parse_arranger_string("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_consecutive_letters_become_two_letter_id() {
        // Opeenvolgende letters worden als 2-letter ID gelezen (abc → ab + c)
        let result = parse_arranger_string("ABC").unwrap();
        assert_eq!(result, vec![("ab".to_string(), 1), ("c".to_string(), 1),]);
    }

    #[test]
    fn test_parse_with_repeats() {
        let result = parse_arranger_string("2b3A").unwrap();
        assert_eq!(result, vec![("b".to_string(), 2), ("a".to_string(), 3),]);
    }

    #[test]
    fn test_parse_parentheses_single_letter_inside() {
        // Binnen haakjes kan alleen 1-letter ID, want ')' is delimiter
        let result = parse_arranger_string("(a)b").unwrap();
        assert_eq!(result, vec![("a".to_string(), 1), ("b".to_string(), 1),]);
    }

    #[test]
    fn test_parse_number_before_parentheses() {
        // Getal voor '(' wordt NIET herkend (nummer wordt aan letter gekoppeld)
        // '(3a)' = 3x a, b = b
        let result = parse_arranger_string("(3a)b").unwrap();
        assert_eq!(result, vec![("a".to_string(), 3), ("b".to_string(), 1),]);
    }

    #[test]
    fn test_parse_spaces() {
        let result = parse_arranger_string("A B C").unwrap();
        assert_eq!(
            result,
            vec![
                ("a".to_string(), 1),
                ("b".to_string(), 1),
                ("c".to_string(), 1),
            ]
        );
    }

    #[test]
    fn test_parse_repeat_inside_parentheses() {
        // Herhaling binnen haakjes: (3a)b = 3x a, dan b
        let result = parse_arranger_string("(3a)b").unwrap();
        assert_eq!(result, vec![("a".to_string(), 3), ("b".to_string(), 1),]);
    }

    #[test]
    fn test_parse_uppercase_is_lowercased() {
        // 'aBc' wordt 'abc' → 'ab' + 'c'
        let result = parse_arranger_string("aBc").unwrap();
        assert_eq!(result, vec![("ab".to_string(), 1), ("c".to_string(), 1),]);
    }

    #[test]
    fn test_parse_invalid_char() {
        let result = parse_arranger_string("AB$C");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_closing_paren() {
        let result = parse_arranger_string("(ab");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_double_digit_repeats() {
        let result = parse_arranger_string("12a").unwrap();
        assert_eq!(result, vec![("a".to_string(), 12)]);
    }

    #[test]
    fn test_parse_complex_sequence() {
        // '(3a)' = 3x a, 'b' = b, 'c' = c, '(2d)' = 2x d, 'e' = e
        let result = parse_arranger_string("(3a)b c(2d)e").unwrap();
        assert_eq!(
            result,
            vec![
                ("a".to_string(), 3),
                ("b".to_string(), 1),
                ("c".to_string(), 1),
                ("d".to_string(), 2),
                ("e".to_string(), 1),
            ]
        );
    }

    // ── hsv_to_rgb tests ──

    #[test]
    fn test_hsv_to_rgb_red() {
        // R = 0°, full saturation & value: region 0 → [v, t, p]
        let [r, g, b] = hsv_to_rgb(0, 255, 255);
        assert_eq!((r, g, b), (255, 0, 0), "h=0, s=255, v=255");
    }

    #[test]
    fn test_hsv_to_rgb_green() {
        // G = 120°, full saturation & value: region 2 → [p, v, t]
        let [r, g, b] = hsv_to_rgb(120, 255, 255);
        assert_eq!((r, g, b), (0, 255, 0), "h=120, s=255, v=255");
    }

    #[test]
    fn test_hsv_to_rgb_gray() {
        // s=0 → always gray regardless of hue
        let [r, g, b] = hsv_to_rgb(120, 0, 128);
        assert_eq!((r, g, b), (128, 128, 128));
    }

    #[test]
    fn test_hsv_to_rgb_black() {
        let [r, g, b] = hsv_to_rgb(0, 255, 0);
        assert_eq!((r, g, b), (0, 0, 0));
    }

    #[test]
    fn test_hsv_to_rgb_full_white() {
        let [r, g, b] = hsv_to_rgb(300, 0, 255);
        assert_eq!((r, g, b), (255, 255, 255));
    }

    // ── color_for_arranger tests ──

    #[test]
    fn test_color_for_arranger_deterministic() {
        let c1 = color_for_arranger("a", "/track.wav");
        let c2 = color_for_arranger("a", "/track.wav");
        assert_eq!(c1, c2, "zelfde input moet dezelfde kleur geven");
    }

    #[test]
    fn test_color_for_arranger_different_ids() {
        let c1 = color_for_arranger("a", "/track.wav");
        let c2 = color_for_arranger("b", "/track.wav");
        // Bijna zeker verschillend (hash-based, 702 IDs verdeeld over 360° hue)
        assert_ne!(
            c1, c2,
            "verschillende IDs moeten (bijna altijd) andere kleur geven"
        );
    }

    #[test]
    fn test_color_for_arranger_different_tracks() {
        let c1 = color_for_arranger("a", "/track1.wav");
        let c2 = color_for_arranger("a", "/track2.wav");
        assert_ne!(c1, c2, "zelfde ID in andere track = andere kleur");
    }

    #[test]
    fn test_color_for_arranger_bounds() {
        let [r, g, b] = color_for_arranger("zz", "/some/long/path.wav");
        assert!(
            r <= 230 && g <= 230 && b <= 230,
            "value (V) is 230, dus RGB mag niet hoger zijn: got ({},{},{})",
            r,
            g,
            b
        );
    }
}
