# Verbeterplan — Top 3 Aanbevelingen

> Datum: 2026-07-13
> Doel: Uitwerking van de drie meest impactvolle verbeteringen uit de codekwaliteit-analyse.
> Geschatte totale tijd: 4-6 uur

---

## Aanbeveling 1: Split `app/mod.rs` `update()` verder op

**Impact:** 🟢 Hoog — maakt de kernel van de applicatie beheersbaar.
**Bestand:** `src/app/mod.rs` (huidig ~3350 regels)
**Doel:** `update()` reduceren van ~2160 → ~600 regels door orchestratie-logica naar eigen modules te verplaatsen.
**Tijd:** 2-3 uur

### Huidige situatie

`LoopEditorApp::update()` (line 1191-3351) doet alles in één functie:

```
update()
├── Event-loop verwerking (1193-1320)   ~130 regels
├── Sessie auto-save (1322-1340)        ~20 regels
├── Toolbar rendering (1342-1500)      ~160 regels
├── File toolbar (1502-1680)           ~180 regels
├── CentralPanel dispatch (1682-3351)  ~1670 regels
│   ├── Geen file geladen → open scherm
│   ├── Setup window
│   ├── Export window
│   ├── Shortcut editor
│   ├── Shortcut help
│   ├── Library window
│   ├── Arranger window
│   ├── Bevestig delete dialog
│   ├── Toolbar editor
│   └── Hoofd waveform paneel
│       ├── Playback controls
│       ├── Pitch/tempo/volume
│       ├── Loop controls
│       ├── Waveform + marker zone
│       └── Loop library lijst
```

### Stappenplan

#### Stap 1.1 — Maak `ui_playback.rs` (~30 min)

Verplaats alle playback-control UI + status-display uit de `CentralPanel` sectie van `update()`.

**Wat verplaatsen:**
- Play/pause/stop knoppen (iconen + logic)
- Positie slider
- Pitch/tempo/volume sliders
- Loop repeat counter
- Loop bypass toggle

**Handtekening:**
```rust
// in ui_playback.rs
impl LoopEditorApp {
    pub fn show_playback_bar(&mut self, ui: &mut egui::Ui) {
        // ...
    }
}
```

**Aanroep in `update()`:**
```rust
// Voorheen 150+ regels inline
self.show_playback_bar(ui);
```

#### Stap 1.2 — Maak `ui_toolbar.rs` (~30 min)

Verplaats de toolbar rendering naar een eigen module.

**Wat verplaatsen:**
- `toolbar_button()` (line 811-897, ~86 regels)
- `execute_toolbar_action()` (line 900-1036, ~136 regels)
- `show_toolbar_editor_window()` (line 1039-1188, ~149 regels)
- De file-toolbar rendering (open file, etc.)

**Handtekening:**
```rust
// in ui_toolbar.rs
impl LoopEditorApp {
    pub fn show_file_toolbar(&mut self, ui: &mut egui::Ui) {
        // ...
    }
    pub fn show_action_toolbar(&mut self, ui: &mut egui::Ui) {
        // ...
    }
}
```

#### Stap 1.3 — Maak `ui_main.rs` of hernoem `mod.rs` (~30 min)

Dit bestand wordt het nieuwe hart: alleen de `update()` dispatch en event-loop, met aanroepen naar submodules.

```rust
// app/ui_main.rs (of mod.rs na refactor)
impl eframe::App for LoopEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_waveform_events(ctx);
        self.auto_save_session();

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.show_file_toolbar(ui);
            self.show_action_toolbar(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_central_content(ui, ctx);
        });

        // Vensters (conditoneel)
        self.show_setup_window(ctx);
        self.show_export_window(ctx);
        self.show_arranger_ui(ctx);
        self.show_library_window(ctx);
        self.show_shortcuts_help(ctx);
        self.show_shortcut_editor(ctx);
        self.show_confirm_delete(ctx);
        self.show_toolbar_editor_window(ctx);

        // Status balk onderin
        self.show_status_bar(ctx);
    }
}
```

#### Stap 1.4 — CentralPanel dispatch naar `ui_central.rs` (~60 min)

Het grootste blok: de `CentralPanel` inhoud (~1670 regels). Dit kan opgesplitst worden in:

| Onderdeel | Geschatte regels | Nieuwe functie |
|---|---|---|
| "Geen bestand" welkom-scherm | ~40 | `show_welcome_screen(ui)` |
| Status message balk | ~30 | `show_status_bar(ctx)` |
| Loop library panel (rechterzijde) | ~140 | `show_loop_panel(ui)` |
| Waveform + markers | ~860 (bestaat al in `render_waveform`) | aanroep `render_waveform()` |
| Marker edit popup | ~40 | `show_marker_edit_popup(ctx)` |
| Track info / channel mode | ~50 | `show_track_info(ui)` |

**Handtekening:**
```rust
pub fn show_central_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
    if self.waveform_state.path.is_none() {
        self.show_welcome_screen(ui);
        return;
    }

    ui.horizontal(|ui| {
        // Linker kolom: waveform
        ui.vertical(|ui| {
            self.show_playback_bar(ui);
            let (_, _, _) = render_waveform(ui, &mut self.waveform_state,
                Some(self.waveform_play_position));
            self.show_track_info(ui);
        });
        // Rechter kolom: loop library
        self.show_loop_panel(ui);
    });
}
```

#### Stap 1.5 — Event-loop naar `event_handler.rs` (~30 min)

De `while let Ok(event) = self.waveform_event_rx.try_recv()` match (lines 1193-1320) verhuist naar een apart bestand.

```rust
// app/event_handler.rs
impl LoopEditorApp {
    pub fn handle_waveform_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.waveform_event_rx.try_recv() {
            match event {
                WaveformEvent::Playing => { ... }
                WaveformEvent::Stopped => { ... }
                WaveformEvent::Position(pos, dur) => { ... }
                WaveformEvent::StepChanged(idx) => { ... }
                WaveformEvent::ArrangementFinished => { ... }
                // ...
            }
        }
    }
}
```

### Eindresultaat

Na de refactor:

```
src/app/
├── mod.rs              (~200 regels) — LoopEditorApp struct, new(), module declaraties
├── event_handler.rs    (~150 regels) — Waveform event verwerking
├── ui_arranger.rs      (~407 regels) — Bestaat al
├── ui_central.rs       (~250 regels) — CentralPanel dispatch
├── ui_export.rs        (~188 regels) — Bestaat al
├── ui_library.rs       (~230 regels) — Bestaat al
├── ui_main.rs          (~150 regels) — Alleen update() dispatch
├── ui_playback.rs      (~180 regels) — Nieuw: playback controls
├── ui_setup.rs         (~529 regels) — Bestaat al
├── ui_shortcuts.rs     (~153 regels) — Bestaat al
└── ui_toolbar.rs       (~300 regels) — Nieuw: toolbar + toolbar editor
```

**Totaal `app/`:** ~2740 regels (was ~3350). **`mod.rs` gaat van ~3350 → ~200.**

### Risico's

| Risico | Mitigatie |
|---|---|
| Vergeten state te updaten na verplaatsing | Werk in kleine stappen, compileer na elke move |
| Methodes die prive-velden nodig hebben | Alle methodes blijven op `impl LoopEditorApp`, dus toegang blijft |
| Git conflicts bij grote refactor | Werk in aparte branch |

---

## Aanbeveling 2: Voeg tests toe voor parser en audio pipeline

**Impact:** 🟢 Hoog — geeft vertrouwen bij refactoren, vooral voor de arranger-parser.
**Bestand:** Nieuw: `tests/` directory of inline `#[cfg(test)]` modules.
**Tijd:** 1-2 uur

### Stappenplan

#### Stap 2.1 — Tests voor `generate_short_id()` (~15 min)

```rust
// in arrangement.rs, onderaan
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_short_id_empty() {
        let ids: Vec<String> = vec![];
        assert_eq!(generate_short_id(&ids), "a");
    }

    #[test]
    fn test_generate_short_id_sequential() {
        let mut ids: Vec<String> = vec![];
        for expected in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"] {
            let id = generate_short_id(&ids);
            assert_eq!(id, expected);
            ids.push(id);
        }
    }

    #[test]
    fn test_generate_short_id_after_z() {
        let mut ids: Vec<String> = ('a'..='z').map(|c| c.to_string()).collect();
        assert_eq!(generate_short_id(&ids), "aa");
    }

    #[test]
    fn test_generate_short_id_double_letters() {
        let ids = vec!["a".to_string(), "b".to_string(), "aa".to_string()];
        assert_eq!(generate_short_id(&ids), "c");
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
}
```

#### Stap 2.2 — Tests voor `parse_arranger_string()` (~30 min)

Dit is de meest waardevolle test-set — de parser heeft duidelijke input/output en wordt vaak aangepast.

```rust
#[test]
fn test_parse_simple() {
    let result = parse_arranger_string("ABC").unwrap();
    assert_eq!(result, vec![
        ("a".to_string(), 1),
        ("b".to_string(), 1),
        ("c".to_string(), 1),
    ]);
}

#[test]
fn test_parse_with_repeats() {
    let result = parse_arranger_string("2b3A").unwrap();
    assert_eq!(result, vec![
        ("b".to_string(), 2),
        ("a".to_string(), 3),
    ]);
}

#[test]
fn test_parse_parentheses() {
    let result = parse_arranger_string("(aa)b").unwrap();
    assert_eq!(result, vec![
        ("aa".to_string(), 1),
        ("b".to_string(), 1),
    ]);
}

#[test]
fn test_parse_within_parentheses() {
    let result = parse_arranger_string("(aa)3(bc)d").unwrap();
    assert_eq!(result, vec![
        ("aa".to_string(), 1),
        ("bc".to_string(), 3),
        ("d".to_string(), 1),
    ]);
}

#[test]
fn test_parse_spaces() {
    let result = parse_arranger_string("A B C").unwrap();
    assert_eq!(result, vec![
        ("a".to_string(), 1),
        ("b".to_string(), 1),
        ("c".to_string(), 1),
    ]);
}

#[test]
fn test_parse_repeat_in_parentheses() {
    let result = parse_arranger_string("3(ab)").unwrap();
    assert_eq!(result, vec![
        ("ab".to_string(), 3),
    ]);
}

#[test]
fn test_parse_empty() {
    let result = parse_arranger_string("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_parse_invalid_char() {
    let result = parse_arranger_string("AB$C");
    assert!(result.is_err());
}

#[test]
fn test_parse_missing_paren() {
    let result = parse_arranger_string("(ab");
    assert!(result.is_err());
}

#[test]
fn test_parse_case_insensitive() {
    let result = parse_arranger_string("aBc").unwrap();
    assert_eq!(result, vec![
        ("a".to_string(), 1),
        ("b".to_string(), 1),
        ("c".to_string(), 1),
    ]);
}
```

#### Stap 2.3 — Tests voor `hsv_to_rgb()` en `color_for_arranger()` (~10 min)

```rust
#[test]
fn test_hsv_to_rgb_red() {
    assert_eq!(hsv_to_rgb(0, 255, 255), [255, 0, 255]); // h=0, s=255, v=255
}

#[test]
fn test_hsv_to_rgb_gray() {
    assert_eq!(hsv_to_rgb(120, 0, 128), [128, 128, 128]); // s=0 → grijs
}

#[test]
fn test_color_for_arranger_deterministic() {
    let c1 = color_for_arranger("a", "/track.wav");
    let c2 = color_for_arranger("a", "/track.wav");
    assert_eq!(c1, c2);
}

#[test]
fn test_color_for_arranger_different_ids() {
    let c1 = color_for_arranger("a", "/track.wav");
    let c2 = color_for_arranger("b", "/track.wav");
    assert_ne!(c1, c2); // zeer waarschijnlijk verschillend
}
```

#### Stap 2.4 — Tests voor serde roundtrip (~15 min)

```rust
#[test]
fn test_saved_loop_serde_roundtrip() {
    let loop1 = SavedLoop {
        label: "Test".to_string(),
        short_id: Some("a".to_string()),
        loop_a_secs: 0.0,
        loop_b_secs: 10.0,
        pitch_semitones: 0.0,
        tempo: 1.0,
        notes: "C major".to_string(),
    };

    let json = serde_json::to_string(&loop1).unwrap();
    let loop2: SavedLoop = serde_json::from_str(&json).unwrap();

    assert_eq!(loop1.label, loop2.label);
    assert_eq!(loop1.short_id, loop2.short_id);
    assert_eq!(loop1.tempo, loop2.tempo);
}

#[test]
fn test_session_state_serde_roundtrip() {
    let state = SessionState {
        file_path: Some("/test.wav".to_string()),
        play_position: 0.0,
        zoom: 1.0,
        scroll_offset: 0.0,
        loop_a_secs: None,
        loop_b_secs: None,
        pitch_semitones: 0.0,
        tempo: 1.0,
        volume: 0.8,
        channel_mode: "Mono".to_string(),
        arr_parse_buf: String::new(),
        last_directory: None,
        bpm_threshold: 0.3,
        playback_latency_ms: 40.0,
        beat_offset_ms: 0.0,
        toolbar_buttons: None,
    };

    let json = serde_json::to_string_pretty(&state).unwrap();
    let restored: SessionState = serde_json::from_str(&json).unwrap();

    assert_eq!(state.file_path, restored.file_path);
    assert_eq!(state.volume, restored.volume);
    assert_eq!(state.bpm_threshold, restored.bpm_threshold);
}
```

#### Stap 2.5 — Integratietest voor library migratie (~15 min)

```rust
#[test]
fn test_library_migration_from_old_format() {
    // Schrijf oude format
    let old_json = r#"[
        {
            "track_path": "/test.wav",
            "label": "Mijn Loop",
            "loop_a_secs": 0.0,
            "loop_b_secs": 5.0,
            "pitch_semitones": 0.0,
            "tempo": 1.0
        }
    ]"#;
    std::fs::write("loops.json", old_json).unwrap();

    // Laad (migreert automatisch)
    let lib = load_library();

    // Verifieer
    assert_eq!(lib.tracks.len(), 1);
    assert_eq!(lib.tracks[0].loops.len(), 1);
    assert!(lib.tracks[0].loops[0].short_id.is_some());

    // Oude bestand moet verwijderd zijn
    assert!(!std::path::Path::new("loops.json").exists());

    // Nieuw bestand moet bestaan
    assert!(std::path::Path::new("library.json").exists());

    // Cleanup (laat library.json staan voor volgende tests)
    let _ = std::fs::remove_file("library.json");
}
```

### Testconfiguratie

Voeg toe aan `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"  # voor tijdelijke bestanden in tests
```

### Draaien

```bash
cargo test                           # Alle tests
cargo test parse_arranger_string     # Alleen parser tests
cargo test -- --ignored              # Als sommige als ignored gemarkeerd zijn
```

**Let op:** Tests die naar schijf schrijven (`test_library_migration`) gebruiken de working directory. Gebruik bij voorkeur `tempfile::TempDir` of een submap voor isolate.

### Risico's

| Risico | Mitigatie |
|---|---|
| Tests die schijf IO doen kunnen andere tests beïnvloeden | Gebruik `tempfile::TempDir` of unieke bestandsnamen |
| `load_library()` verwacht bestanden in CWD | Tests draaien in `target/` — gebruik absolute paden of `std::env::set_current_dir` |

---

## Aanbeveling 3: Vervang stille `let _ =` door logging

**Impact:** 🟡 Medium — maakt debugging aanzienlijk makkelijker zonder gedrag te veranderen.
**Bestanden:** `session.rs`, `loops.rs`, `arrangement.rs`, `app/mod.rs`, `waveform_player.rs`
**Tijd:** 30-45 minuten

### Stappenplan

#### Stap 3.1 — Kies een aanpak (5 min)

Drie opties, oplopend in complexiteit:

| Optie | Complexiteit | Voordeel | Nadeel |
|---|---|---|---|
| **A. `eprintln!`** | ⭐ Laag | Geen dependencies, direct zichtbaar | Geen levels, geen filter |
| **B. `log` crate + env_logger** | ⭐⭐ Medium | Levels (warn/error/info), filterbaar | Extra dependency |
| **C. Eigen `log_error!` macro** | ⭐⭐ Medium | Geen dependency, wel levels | Zelf bouwen |

**Aanbevolen: Optie B** — `log` + `env_logger` is de Rust-standaard.

```toml
# Cargo.toml
[dependencies]
log = "0.4"
```

In `main.rs`:
```rust
fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn")
    ).init();
    // ...
}
```

#### Stap 3.2 — Vervang stille fouten in `session.rs` (~5 min)

```rust
// Voor:
if let Ok(json) = serde_json::to_string_pretty(&state) {
    let _ = std::fs::write(SESSION_FILE, json);
}

// Na:
match serde_json::to_string_pretty(&state) {
    Ok(json) => {
        if let Err(e) = std::fs::write(SESSION_FILE, &json) {
            log::error!("Kon sessie niet opslaan naar '{}': {}", SESSION_FILE, e);
        }
    }
    Err(e) => {
        log::error!("Kon sessie niet serialiseren: {}", e);
    }
}
```

#### Stap 3.3 — Vervang stille fouten in `loops.rs` (~10 min)

```rust
// save_library()
pub fn save_library(library: &Library) {
    match serde_json::to_string_pretty(library) {
        Ok(json) => {
            if let Err(e) = std::fs::write(LIBRARY_FILE, json) {
                log::error!("Kon library niet opslaan naar '{}': {}", LIBRARY_FILE, e);
            }
        }
        Err(e) => {
            log::error!("Kon library niet serialiseren: {}", e);
        }
    }
}
```

Ook in `load_library()` — log wanneer fallback wordt gebruikt:
```rust
pub fn load_library() -> Library {
    if let Ok(json) = std::fs::read_to_string(LIBRARY_FILE) {
        if let Ok(mut lib) = serde_json::from_str::<Library>(&json) {
            assign_short_ids(&mut lib);
            return lib;
        }
        log::warn!("Kon '{}' niet parsen, val terug op oude format", LIBRARY_FILE);
    }
    // ...
}
```

#### Stap 3.4 — Vervang stille fouten in `arrangement.rs` (~5 min)

```rust
pub fn save_arrangements(arrangements: &[Arrangement]) {
    match serde_json::to_string_pretty(arrangements) {
        Ok(json) => {
            if let Err(e) = std::fs::write(ARRANGEMENTS_FILE, json) {
                log::error!("Kon arrangementen niet opslaan: {}", e);
            }
        }
        Err(e) => {
            log::error!("Kon arrangementen niet serialiseren: {}", e);
        }
    }
}
```

#### Stap 3.5 — Log WaveformCommand send fouten (~5 min)

```rust
// Voor:
let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop);

// Na:
if let Err(e) = self.waveform_cmd_tx.send(WaveformCommand::Stop) {
    log::error!("Kon Stop-commando niet sturen naar audio-thread: {}", e);
}
```

Dit is belangrijk omdat een gefaalde `send()` betekent dat de audio-thread gestopt is — de gebruiker hoort niks en weet niet waarom.

#### Stap 3.6 — Log in `load_file` en `decode_audio` (~10 min)

```rust
pub fn load_file(&mut self, path: &str) {
    if !Path::new(path).exists() {
        log::error!("Bestand niet gevonden: {}", path);
        self.status_message = format!("❌ Bestand niet gevonden: {}", path);
        self.status_message_timer = 5 * 60;
        return;
    }
    // ...
    match decode_audio(path, &mut self.waveform_state) {
        Ok(()) => {
            log::info!("Bestand geladen: {} ({:.1}s, {} Hz, {} kanalen)",
                path, self.waveform_state.duration_secs,
                self.waveform_state.sample_rate,
                self.waveform_state.samples.len());
            // ...
        }
        Err(e) => {
            log::error!("Kon bestand niet decoderen '{}': {}", path, e);
            // ...
        }
    }
}
```

### Volledige lijst van te vervangen `let _ =` patronen

| Bestand | Lijn(en) | Huidig | Vervanger |
|---|---|---|---|
| `session.rs:76` | 1 | `let _ = std::fs::write(...)` | `log::error!` |
| `loops.rs:156` | 1 | `let _ = std::fs::remove_file(...)` | `log::warn!` |
| `loops.rs:167` | 1 | `let _ = std::fs::write(...)` | `log::error!` |
| `arrangement.rs:271` | 1 | `let _ = std::fs::write(...)` | `log::error!` |
| `app/mod.rs` | ~15 | `let _ = .send(WaveformCommand::...)` | `log::error!` |
| `app/mod.rs` | ~3 | `let _ = std::fs::write(...)` | `log::error!` |
| `app/mod.rs` | ~2 | `let _ = std::fs::remove_file(...)` | `log::warn!` |
| `ui_library.rs` | ~3 | `let _ = .send(WaveformCommand::...)` | `log::error!` |
| `waveform_player.rs` | ~2 | `let _ = .send(...)` | `log::error!` |

**Totaal:** ~30 plaatsen in de codebase.

### Uitvoering

```bash
# Zoek alle stille fout-negeerders
grep -rn "let _ = " src/
# Focus op IO-gerelateerde negeerders
grep -rn "let _ = std::fs::" src/
grep -rn "let _ = .*\.send(" src/
```

### Risico's

| Risico | Mitigatie |
|---|---|
| `log::error!` kan panicken als logger niet geinitialiseerd is | Gebruik `log::warn!` in fallback of check `log::log_enabled!()` |
| `env_logger` moet vroeg geinitialiseerd worden | Zet in eerste regel van `main()` |
| Toevoegen van dependency | `log` is ~100 regels, `env_logger` is stdlib-only — negligible |

---

## Uitvoeringsvolgorde

De aanbevelingen zijn onafhankelijk, maar de volgende volgorde minimaliseert conflicten:

| Fase | Aanbeveling | Tijd | Waarom hier |
|---|---|---|---|
| **1** | 📝 **3. Logging** | 30-45 min | Kleinste wijziging, geen structuurverandering, geeft meteen beter inzicht |
| **2** | 🧪 **2. Tests** | 1-2 uur | Tests vangen regressies tijdens de grote refactor in fase 3 |
| **3** | 🏗️ **1. Split update()** | 2-3 uur | Grootste verandering, maar door tests en logging heb je vangnet |

**Totale doorlooptijd:** ~4-6 uur, verdeeld over 3 sessies.

---

## Rol van de `update()` functie — voor en na

### Voor (huidig ~2160 regels)

```
update() {
    events();           // 130 regels
    session_save();     // 20 regels
    top_panel {         // 340 regels
        file_toolbar();     // 180 regels
        action_toolbar();   // 160 regels
    }
    central_panel {     // 1670 regels
        if no_file: welcome();
        else: {
            left_column {
                playback_bar();     // 150 regels
                render_waveform();  // 1 regel (functie bestaat al)
            }
            right_column {
                loop_library();     // 140 regels
            }
        }
    }
    // Conditionele vensters
    setup_window();     // 1 regel (functie bestaat al)
    export_window();    // 1 regel (functie bestaat al)
    arranger_ui();      // 1 regel (functie bestaat al)
    library_window();   // 1 regel (functie bestaat al)
    shortcuts_help();   // 1 regel (functie bestaat al)
    shortcut_editor();  // 1 regel (functie bestaat al)
    confirm_delete();   // 1 regel (functie bestaat al)
    toolbar_editor();   // 1 regel (functie bestaat al)
}
```

### Na (~300 regels)

```rust
fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.handle_waveform_events(ctx);
    self.auto_save_session();

    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        self.show_file_toolbar(ui);
        self.show_action_toolbar(ui);
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        self.show_central_content(ui, ctx);
    });

    // Conditionele vensters — allemaal 1-regelig
    self.show_setup_window(ctx);
    self.show_export_window(ctx);
    self.show_arranger_ui(ctx);
    self.show_library_window(ctx);
    self.show_shortcuts_help(ctx);
    self.show_shortcut_editor(ctx);
    self.show_toolbar_editor_window(ctx);
    self.show_confirm_delete(ctx);
    self.show_status_bar(ctx);
}
```

Het doel is niet alleen minder regels in `update()`, maar ook dat elke submodule één verantwoordelijkheid heeft en onafhankelijk leesbaar en aanpasbaar is.
