use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::waveform::Marker;

// ───────────────────────────────────────────────
// Data-model
// ───────────────────────────────────────────────

/// De volledige bibliotheek: tracks met loops, markers en notities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub tracks: Vec<TrackData>,
}

impl Library {
    pub fn empty() -> Self {
        Self { tracks: Vec::new() }
    }

    /// Zoek track-data voor een pad. Maak aan als die nog niet bestaat.
    pub fn track_for_path(&mut self, track_path: &str) -> &mut TrackData {
        let idx = self.tracks.iter().position(|t| t.track_path == track_path);
        if let Some(i) = idx {
            &mut self.tracks[i]
        } else {
            let label = Path::new(track_path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Onbekend".to_string());
            self.tracks.push(TrackData {
                track_path: track_path.to_string(),
                label,
                markers: Vec::new(),
                loops: Vec::new(),
            });
            self.tracks.last_mut().unwrap()
        }
    }

    /// Genereer een uniek label voor een nieuwe loop in een track.
    pub fn generate_label(&self, track_path: &str) -> String {
        let file_stem = Path::new(track_path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Onbekend".to_string());

        // Tel bestaande loops voor deze track
        let count = self
            .tracks
            .iter()
            .filter(|t| t.track_path == track_path)
            .flat_map(|t| &t.loops)
            .count();

        if count == 0 {
            format!("{} - Loop 1", file_stem)
        } else {
            format!("{} - Loop {}", file_stem, count + 1)
        }
    }

    /// Voeg een loop toe aan een track met een automatisch gegenereerde short_id.
    /// Geeft het totale aantal loops in de track terug.
    pub fn add_loop(&mut self, track_path: &str, saved: SavedLoop) -> usize {
        // Verzamel bestaande IDs in deze track
        let existing: Vec<String> = self
            .tracks
            .iter()
            .filter(|t| t.track_path == track_path)
            .flat_map(|t| &t.loops)
            .filter_map(|l| l.short_id.clone())
            .collect();

        let mut saved = saved;
        let new_id = crate::arrangement::generate_short_id(&existing);
        saved.short_id = Some(new_id);

        let track = self.track_for_path(track_path);
        track.loops.push(saved);
        track.loops.len()
    }
}

/// Metadata voor één audiobestand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackData {
    pub track_path: String,
    pub label: String,
    pub markers: Vec<Marker>,
    pub loops: Vec<SavedLoop>,
}

/// Een opgeslagen loop (hoort bij een TrackData).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedLoop {
    pub label: String,
    /// Korte identifier voor arranger (bv. "a", "b", "aa").
    #[serde(default)]
    pub short_id: Option<String>,
    pub loop_a_secs: f32,
    pub loop_b_secs: f32,
    /// Aantal halve tonen pitch-shift.
    #[serde(default)]
    pub pitch_semitones: f32,
    /// Tempo-factor.
    #[serde(default = "default_tempo")]
    pub tempo: f32,
    /// Notities voor deze loop (akkoorden, noten, etc.)
    #[serde(default)]
    pub notes: String,
}

fn default_tempo() -> f32 {
    1.0
}

// ───────────────────────────────────────────────
// Laden / Opslaan
// ───────────────────────────────────────────────

const LIBRARY_FILE: &str = "library.json";
const OLD_LOOPS_FILE: &str = "loops.json";

/// Laad de bibliotheek van schijf. Migreert oude loops.json indien nodig.
/// Wist kort nadien bestaande loops zonder short_id een ID toe.
pub fn load_library() -> Library {
    // Probeer nieuwe format eerst
    match std::fs::read_to_string(LIBRARY_FILE) {
        Ok(json) => match serde_json::from_str::<Library>(&json) {
            Ok(mut lib) => {
                assign_short_ids(&mut lib);
                return lib;
            }
            Err(e) => {
                log::warn!(
                    "Kon '{}' niet parsen ({}), val terug op oude format",
                    LIBRARY_FILE,
                    e
                );
            }
        },
        Err(e) => {
            log::debug!(
                "'{}' niet gevonden ({}), probeer oude format",
                LIBRARY_FILE,
                e
            );
        }
    }

    // Fallback: migreer oude loops.json
    if let Ok(json) = std::fs::read_to_string(OLD_LOOPS_FILE) {
        if let Ok(old_loops) = serde_json::from_str::<Vec<OldSavedLoop>>(&json) {
            let mut lib = Library::empty();
            for old in old_loops {
                let track = lib.track_for_path(&old.track_path);
                track.loops.push(SavedLoop {
                    label: old.label,
                    short_id: None,
                    loop_a_secs: old.loop_a_secs,
                    loop_b_secs: old.loop_b_secs,
                    pitch_semitones: old.pitch_semitones,
                    tempo: old.tempo,
                    notes: String::new(),
                });
            }
            // Sla nieuwe format meteen op
            assign_short_ids(&mut lib);
            save_library(&lib);
            // Verwijder oud bestand (optioneel, maar netjes)
            if let Err(e) = std::fs::remove_file(OLD_LOOPS_FILE) {
                log::warn!(
                    "Kon oud bestand '{}' niet verwijderen: {}",
                    OLD_LOOPS_FILE,
                    e
                );
            }
            return lib;
        }
    }

    Library::empty()
}

/// Sla de bibliotheek weg naar schijf.
pub fn save_library(library: &Library) {
    match serde_json::to_string_pretty(library) {
        Ok(json) => {
            if let Err(e) = std::fs::write(LIBRARY_FILE, &json) {
                log::error!("Kon library niet opslaan naar '{}': {}", LIBRARY_FILE, e);
            }
        }
        Err(e) => {
            log::error!("Kon library niet serialiseren: {}", e);
        }
    }
}

/// Ken aan alle loops zonder short_id een unieke ID toe.
pub fn assign_short_ids(lib: &mut Library) {
    let mut changed = false;
    for track in &mut lib.tracks {
        // Reset: verzamel alle IDs en vervang dubbele
        let ids: Vec<Option<String>> = track.loops.iter().map(|l| l.short_id.clone()).collect();
        let mut seen = std::collections::HashSet::new();
        let mut existing: Vec<String> = Vec::new();

        for (i, id_opt) in ids.iter().enumerate() {
            let needs_new = match id_opt {
                Some(id) if !seen.contains(id) => {
                    seen.insert(id.clone());
                    existing.push(id.clone());
                    false
                }
                _ => true, // None of duplicate
            };

            if needs_new {
                let new_id = crate::arrangement::generate_short_id(&existing);
                track.loops[i].short_id = Some(new_id.clone());
                existing.push(new_id);
                changed = true;
            }
        }
    }
    if changed {
        save_library(lib);
    }
}

// ───────────────────────────────────────────────
// Oude struct (alleen voor migratie)
// ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldSavedLoop {
    track_path: String,
    label: String,
    loop_a_secs: f32,
    loop_b_secs: f32,
    #[serde(default)]
    pitch_semitones: f32,
    #[serde(default = "default_tempo")]
    tempo: f32,
}

// ───────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SavedLoop serde roundtrip ──

    #[test]
    fn test_saved_loop_serde_roundtrip() {
        let loop1 = SavedLoop {
            label: "Mijn Beat".to_string(),
            short_id: Some("a".to_string()),
            loop_a_secs: 0.5,
            loop_b_secs: 4.0,
            pitch_semitones: 2.0,
            tempo: 1.2,
            notes: "Cmaj7".to_string(),
        };

        let json = serde_json::to_string(&loop1).unwrap();
        let loop2: SavedLoop = serde_json::from_str(&json).unwrap();

        assert_eq!(loop1.label, loop2.label);
        assert_eq!(loop1.short_id, loop2.short_id);
        assert_eq!(loop1.loop_a_secs, loop2.loop_a_secs);
        assert_eq!(loop1.loop_b_secs, loop2.loop_b_secs);
        assert_eq!(loop1.pitch_semitones, loop2.pitch_semitones);
        assert_eq!(loop1.tempo, loop2.tempo);
        assert_eq!(loop1.notes, loop2.notes);
    }

    #[test]
    fn test_saved_loop_default_fields() {
        // Zonder short_id, notes, pitch_semitones in JSON moeten defaults gebruikt worden
        let json = r#"{
            "label": "Minimal",
            "loop_a_secs": 0.0,
            "loop_b_secs": 10.0
        }"#;
        let saved: SavedLoop = serde_json::from_str(json).unwrap();
        assert_eq!(saved.short_id, None);
        assert_eq!(saved.pitch_semitones, 0.0);
        assert_eq!(saved.tempo, 1.0);
        assert_eq!(saved.notes, "");
    }

    // ── TrackData / Library tests ──

    #[test]
    fn test_library_empty() {
        let lib = Library::empty();
        assert!(lib.tracks.is_empty());
    }

    #[test]
    fn test_library_track_for_path_creates_new() {
        let mut lib = Library::empty();
        let track = lib.track_for_path("/test/file.wav");
        assert_eq!(track.track_path, "/test/file.wav");
        assert_eq!(track.label, "file");
        assert!(track.loops.is_empty());
        assert!(track.markers.is_empty());
    }

    #[test]
    fn test_library_track_for_path_reuses_existing() {
        let mut lib = Library::empty();
        lib.track_for_path("/test/file.wav");
        let len_before = lib.tracks.len();
        let _ = lib.track_for_path("/test/file.wav");
        assert_eq!(
            lib.tracks.len(),
            len_before,
            "zelfde pad mag geen nieuwe track aanmaken"
        );
    }

    #[test]
    fn test_generate_label_first_loop() {
        let mut lib = Library::empty();
        lib.track_for_path("/test/song.wav");
        let label = lib.generate_label("/test/song.wav");
        assert_eq!(label, "song - Loop 1");
    }

    #[test]
    fn test_generate_label_increment() {
        let mut lib = Library::empty();
        let track_path = "/test/song.wav";
        lib.track_for_path(track_path);

        let saved = SavedLoop {
            label: "first".to_string(),
            short_id: None,
            loop_a_secs: 0.0,
            loop_b_secs: 5.0,
            pitch_semitones: 0.0,
            tempo: 1.0,
            notes: String::new(),
        };
        lib.add_loop(track_path, saved);

        let label = lib.generate_label(track_path);
        assert_eq!(label, "song - Loop 2");
    }

    #[test]
    fn test_add_loop_assigns_short_id() {
        let mut lib = Library::empty();
        let track_path = "/test/song.wav";
        lib.track_for_path(track_path);

        let saved = SavedLoop {
            label: "first".to_string(),
            short_id: None,
            loop_a_secs: 0.0,
            loop_b_secs: 5.0,
            pitch_semitones: 0.0,
            tempo: 1.0,
            notes: String::new(),
        };
        lib.add_loop(track_path, saved);

        let track = lib.track_for_path(track_path);
        assert_eq!(track.loops.len(), 1);
        assert_eq!(track.loops[0].short_id, Some("a".to_string()));
    }

    #[test]
    fn test_add_loop_increments_and_assigns_sequential_ids() {
        let mut lib = Library::empty();
        let track_path = "/test/song.wav";
        lib.track_for_path(track_path);

        for i in 0..3 {
            let saved = SavedLoop {
                label: format!("loop {}", i),
                short_id: None,
                loop_a_secs: 0.0,
                loop_b_secs: 5.0,
                pitch_semitones: 0.0,
                tempo: 1.0,
                notes: String::new(),
            };
            lib.add_loop(track_path, saved);
        }

        let track = lib.track_for_path(track_path);
        assert_eq!(track.loops.len(), 3);
        assert_eq!(track.loops[0].short_id, Some("a".to_string()));
        assert_eq!(track.loops[1].short_id, Some("b".to_string()));
        assert_eq!(track.loops[2].short_id, Some("c".to_string()));
    }

    // ── Library migratie (oude → nieuwe format) ──

    #[test]
    fn test_old_saved_loop_deserialization() {
        // Het oude format had geen short_id, notes, pitch_semitones, tempo
        let json = r#"{
            "track_path": "/old/test.wav",
            "label": "Old Loop",
            "loop_a_secs": 1.0,
            "loop_b_secs": 3.0
        }"#;
        let old: OldSavedLoop = serde_json::from_str(json).unwrap();
        assert_eq!(old.track_path, "/old/test.wav");
        assert_eq!(old.label, "Old Loop");
        assert_eq!(old.loop_a_secs, 1.0);
        assert_eq!(old.loop_b_secs, 3.0);
        assert_eq!(old.pitch_semitones, 0.0); // default
        assert_eq!(old.tempo, 1.0); // default via serde
    }

    #[test]
    fn test_library_migration_creates_short_ids() {
        use std::fs;

        // Opruimen: verwijder library.json zodat load_library() naar loops.json valt
        let _ = fs::remove_file(LIBRARY_FILE);
        let _ = fs::remove_file(OLD_LOOPS_FILE);

        // Schrijf oude format
        let old_json = r#"[
            {
                "track_path": "/test/migration.wav",
                "label": "My Loop",
                "loop_a_secs": 0.0,
                "loop_b_secs": 5.0,
                "pitch_semitones": 0.0,
                "tempo": 1.0
            }
        ]"#;
        fs::write(OLD_LOOPS_FILE, old_json).unwrap();

        // Laad — dit zou de oude loops.json moeten migreren
        let lib = load_library();

        // Verifieer dat migratie gelukt is
        if !lib.tracks.is_empty() {
            for track in &lib.tracks {
                for saved in &track.loops {
                    assert!(
                        saved.short_id.is_some(),
                        "alle loops moeten een short_id krijgen na migratie"
                    );
                    assert_eq!(saved.notes, "", "gemigreerde loop heeft geen notities");
                }
            }
        }

        // Nieuw bestand moet aangemaakt zijn
        assert!(
            Path::new(LIBRARY_FILE).exists(),
            "library.json moet aangemaakt zijn na migratie"
        );

        // Cleanup
        let _ = fs::remove_file(LIBRARY_FILE);
    }

    #[test]
    fn test_assign_short_ids_fixes_duplicates() {
        let mut lib = Library::empty();
        let track = lib.track_for_path("/test/dupes.wav");
        track.loops.push(SavedLoop {
            label: "dup1".to_string(),
            short_id: Some("a".to_string()),
            loop_a_secs: 0.0,
            loop_b_secs: 5.0,
            pitch_semitones: 0.0,
            tempo: 1.0,
            notes: String::new(),
        });
        track.loops.push(SavedLoop {
            label: "dup2".to_string(),
            short_id: Some("a".to_string()), // zelfde ID!
            loop_a_secs: 0.0,
            loop_b_secs: 5.0,
            pitch_semitones: 0.0,
            tempo: 1.0,
            notes: String::new(),
        });
        track.loops.push(SavedLoop {
            label: "dup3".to_string(),
            short_id: None,
            loop_a_secs: 0.0,
            loop_b_secs: 5.0,
            pitch_semitones: 0.0,
            tempo: 1.0,
            notes: String::new(),
        });

        assign_short_ids(&mut lib);

        let ids: Vec<Option<String>> = lib.tracks[0]
            .loops
            .iter()
            .map(|l| l.short_id.clone())
            .collect();
        // Eerste blijft "a", tweede krijgt nieuwe ("b"), derde krijgt volgende ("c")
        assert_eq!(ids[0].as_deref(), Some("a"));
        assert_eq!(ids[1].as_deref(), Some("b"));
        assert_eq!(ids[2].as_deref(), Some("c"));

        // Geen duplicaten
        let mut unique: Vec<&str> = ids.iter().filter_map(|o| o.as_deref()).collect();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "IDs mogen geen duplicaten hebben");
    }
}
