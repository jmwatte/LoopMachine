# LoopMachine — Codekwaliteit Analyse

> Datum: 2026-07-13
> Auteur: Geautomatiseerde analyse

---

## Projectoverzicht

LoopMachine is een minimalistische, toetsenbord-gestuurde hiërarchische muziekspeler geschreven in Rust, met egui voor de GUI, rodio voor audio-weergave, en Rubber Band voor tijd-rekken/pitch-shift.

| Aspect | Score | Toelichting |
|---|---|---|
| **Projectorganisatie** | 🟢 Goed | Duidelijke modulaire opzet, `src/`, `vendor/`, `keyfinder/`, goede scheiding. |
| **Codekwaliteit** | 🟡 Matig | Sterk wisselend — sommige modules zijn strak, andere (vooral `app/mod.rs`) overladen. |
| **Documentatie** | 🟢 Uitstekend | `CODE_REVIEW.md` (met gefaseerd verbeterplan!), `MANUAL.md` (579 regels), `arrangerStappenPlan.md`. |
| **Testdekking** | 🔴 Afwezig | Geen `#[test]` of integratietests gevonden. |
| **Veiligheid (Rust)** | 🟡 Matig | Beperkt `unsafe` in Rubber Band FFI (correct afgeschermd), maar veel `unwrap()` en stille fouten. |
| **Onderhoudbaarheid** | 🟡 Matig | `app/mod.rs` is ~3350 regels — te groot voor één bestand. |

---

## 1. Projectstructuur & Modulariteit

### Bestandsoverzicht

```
LoopMachine/src/
├── main.rs                 (~57 regels)  — Entry point, font setup
├── app/
│   ├── mod.rs              (~3350 regels) — LoopEditorApp + update()
│   ├── ui_arranger.rs      (~407 regels)  — Arranger window
│   ├── ui_export.rs        (~188 regels)  — Export logic + UI
│   ├── ui_library.rs       (~230 regels)  — Loop library / Alle Tracks
│   ├── ui_setup.rs         (~529 regels)  — Setup, latency, BPM audit
│   └── ui_shortcuts.rs     (~153 regels)  — Shortcut editor + help
├── arrangement.rs           (~303 regels) — Arrangement data, parser, kleur-hash
├── chroma.rs                (~572 regels) — Chroma analyse, key/BPM detectie
├── loops.rs                 (~217 regels) — Library data model, laden/opslaan
├── session.rs               (~86 regels)  — Session state, laden/opslaan
├── shortcuts.rs             (~999 regels) — Shortcut config, key bindings
├── timestretch.rs           (~280 regels) — TimeStretch (Rubber Band / SoundTouch)
├── waveform.rs              (~1267 regels) — Waveform state, rendering, audio decode
└── waveform_player.rs       (~1008 regels) — Audio thread, sources, playback
```

### Sterke punten

- Modules logisch gescheiden op basis van verantwoordelijkheid
- UI verder opgesplitst in `app/ui_arranger.rs`, `app/ui_library.rs`, `app/ui_setup.rs`, `app/ui_shortcuts.rs`, `app/ui_export.rs`
- Gebruik van `#[cfg(feature = "rubberband")]` / `#[cfg(not(feature = "rubberband"))]` voor elegante backend-switch
- Externe dependencies (`vendor/rubberband`, `vendor/fonts/`) goed geïsoleerd

### Problemen

| Bestand | Grootte | Probleem |
|---|---|---|
| `app/mod.rs` | ~3350 regels | `update()` is ~2160 regels — alle event-handling, toolbar-logica, sessie-opslag |
| `waveform.rs` | ~1267 regels | `render_waveform()` is ~860 regels — markers, A-B selectie, playhead, kleuren, rendering |
| `waveform_player.rs` | ~1008 regels | `SoundTouchSource` en `SequenceSource` delen ~70% identieke `fill_buffer()` logica |
| `shortcuts.rs` | ~999 regels | SerializableKey enum + conversies is ~200 regels, kan in apart bestand |

---

## 2. Foutafhandeling

### Patroon: stille fouten

```rust
// Overal in de codebase — fouten worden stil genegeerd:
let _ = std::fs::write(SESSION_FILE, json);               // session.rs:76
let _ = self.waveform_cmd_tx.send(WaveformCommand::Stop); // ui_library.rs:145
let _ = std::fs::remove_file(OLD_LOOPS_FILE);             // loops.rs:156
let _ = serde_json::to_string_pretty(library);             // loops.rs:166
```

Dit is een bewuste keuze (geen fatale fout bij write-falen), maar het maakt debuggen lastig. Een `eprintln!` of `log::warn!` zou al helpen zonder gedrag te veranderen.

### Wel goed

- `execute_export()` gebruikt `Result<String, String>` met duidelijke foutmeldingen
- `detect_key_via_cli()` heeft goede error-propagatie met `CREATE_NO_WINDOW`
- `parse_arranger_string()` retourneert `Result<Vec<(String, u32)>, String>` met positie-informatie

### Risico: unwrap op Mutex

```rust
// chroma.rs:165
let fft = planner.lock().unwrap(); // panickt bij poisoned mutex
```

Als een andere thread panicked terwijl hij de lock vasthoudt, crasht de UI-thread.

---

## 3. Veiligheid & `unsafe`

### Rubber Band C FFI (`timestretch.rs`)

```rust
unsafe extern "C" { ... }     // ✅ Correcte ABI declaratie
unsafe impl Send for Backend {} // ✅ Correct: Backend bevat alleen een pointer
impl Drop for Backend {
    fn drop(&mut self) {
        unsafe { rubberband_delete(self.handle); } // ✅ Resource cleanup
    }
}
```

**Minpunten:**
- Geen `!Sync` markering — `&Backend` mag niet gedeeld worden over threads (bevat een raw pointer)
- Geen safety-documentatie op de `unsafe` functies (geen `# Safety` sectie)

---

## 4. Prestaties

### Al doorgevoerde optimalisaties

| Optimalisatie | Locatie |
|---|---|
| ✅ `OnceLock<Vec<String>>` voor ID-caching | `arrangement.rs:106-124` |
| ✅ `WaveformSummary` pre-computatie | `waveform.rs:97-180` |
| ✅ Pre-gealloceerde buffers in `fill_buffer()` | `waveform_player.rs` (item 12 uit review) |
| ✅ `FftPlanner` hergebruik via `OnceLock` | `chroma.rs` |

### Zorgen

| Locatie | Probleem |
|---|---|
| `waveform.rs:render_waveform()` | Herberekent elke frame zichtbare samples — bij ver uitzoomen duizenden samples per pixel |
| `app/mod.rs:update()` | Grote match-blocks elke frame (~60fps) |
| `chroma.rs` | FFT planner `Mutex::lock()` per detectie — potentieel knelpunt |
| `waveform_player.rs` | `fill_buffer()` in beide sources doet vrijwel hetzelfde werk |

---

## 5. Code Duplicatie

### Grootste duplicatie: `fill_buffer()`

`SoundTouchSource::fill_buffer()` (lines ~805-945) en `SequenceSource::fill_buffer()` (lines ~619-689) delen:

- Pitch/tempo wijzigingen checken en doorzetten naar TimeStretch
- Input chunks lezen uit samples
- TimeStretch voeden (`put_samples`)
- Output drainen (`receive_samples`)
- Volume toepassen met soft-clip

Een gedeelde `AudioProcessor` trait zou ~200 regels besparen.

### Kleinere duplicaties

| Patroon | Locaties |
|---|---|
| Laden/opslaan JSON | `load_arrangements()` / `load_library()` / `SessionState::load()` |
| Marker sync | `load_file` vs `place_bpm_markers` |
| Overlappende enums | `ToolbarAction` en `ShortcutAction` delen acties (Export, TempoUp, etc.) |

---

## 6. Benaming & Stijl

### Taalvermenging

De codebase gebruikt Engels voor identifiers en Nederlands voor commentaar/UI:

| Taal | Voorbeelden |
|---|---|
| 🇬🇧 Engels | `WaveformState`, `ChannelMode`, `SequenceStep`, `fill_buffer()` |
| 🇳🇱 Nederlands | `// ── Font setup ──`, `// Laad sessie`, `"Ongeldig teken"`, `"Geen samples"` |
| 🔀 Gemengd | `color_for_arranger()` (Engels) maar documentatie spreekt van `kleur` |

Voor een solo-project niet blokkerend, maar bij samenwerking is consistentie belangrijk.

### Wel positief

- UI-teksten en foutmeldingen zijn consistent Nederlands
- Geen cryptische afkortingen
- Commentaar verklaart *waarom*, niet *wat*

---

## 7. Testbaarheid

### Huidige staat: ❌ Geen tests

Het project bevat geen `#[test]` functies of integratietests.

### Zeer geschikt voor tests

| Component | Testbaarheid | Voorbeeld |
|---|---|---|
| `parse_arranger_string()` | 🟢 Hoog | `"2b3A" → [(b,2), (a,3)]` |
| `generate_short_id()` | 🟢 Hoog | altijd `"a"` bij lege lijst |
| `hsv_to_rgb()` | 🟢 Hoog | deterministisch, geen IO |
| `color_for_arranger()` | 🟢 Hoog | deterministisch, geen IO |
| `SavedLoop` serde | 🟡 Medium | roundtrip test met JSON |
| `SessionState` laden/opslaan | 🟡 Medium | tijdelijke bestanden nodig |
| Audio pipeline | 🔴 Laag | rodio/symphonia dependency, audio hardware |

---

## 8. Architectuur — Audio Thread

### Sterk patroon

```
┌─────────────┐    crossbeam-channel     ┌─────────────────┐
│  UI Thread   │ ──── WaveformCommand ──► │  Audio Thread   │
│  (egui)      │◄──── WaveformEvent ───── │  (rodio sink)   │
└─────────────┘                           └─────────────────┘
```

`WaveformCommand` en `WaveformEvent` enums zijn compleet en dekken alle benodigde acties. De `PlaySequence` fire-and-forget benadering is elegant — geen UI-latency bij stapovergangen.

### Minpunt

`WaveformState` bevat zowel UI-state (`zoom`, `scroll_offset`) als audio-gerelateerde state (`loop_a_secs`, `pitch_semitones`). Dit maakt verantwoordelijkheid onduidelijk: wordt `pitch_semitones` door de UI of audio-thread beheerd? Antwoord: beide, wat tot inconsistentie kan leiden.

---

## 9. Samenvattende Scorekaart

| Criterium | Waardering | Toelichting |
|---|---|---|
| **Projectstructuur** | 🟢 8/10 | Goed modulair, maar `app/mod.rs` en `waveform.rs` zijn te groot. |
| **Foutafhandeling** | 🟡 5/10 | Te veel stille `let _ = `; geen logging. |
| **Documentatie** | 🟢 10/10 | CODE_REVIEW.md, MANUAL.md — zeldzaam goed. |
| **Prestaties** | 🟡 6/10 | Goede optimalisaties, maar waveform rendering is zwaar bij zoom-out. |
| **Veiligheid (unsafe)** | 🟡 6/10 | Correct gebruik, maar `unwrap()` op Mutex kan panicken. |
| **Testdekking** | 🔴 1/10 | Geen tests. |
| **Code Duplicatie** | 🟡 5/10 | `fill_buffer` verdubbeling (200+ regels), overlappende enums. |
| **Consistentie** | 🟡 6/10 | Nederlands/Engels door elkaar; geen vaste stijlgids. |
| **Architectuur** | 🟢 8/10 | Audio-thread patroon is solide; UI-logica decentraal. |

### Eindcijfer: 🟡 **6.1/10**

Een zeer functioneel en ambitieus project met sterke documentatie en een degelijke architectuur, maar met ruimte voor verbetering in testdekking, foutafhandeling, en bestandsgrootte.

---

## 10. Top 3 Aanbevelingen

Gebaseerd op impact en uitvoerbaarheid:

### 1. Split `app/mod.rs` `update()` verder op

De `update()` is ~2160 regels. De UI-panelen zijn al uitbesteed (`ui_arranger.rs`, etc.), maar de orchestratie (event-loop, toolbar-logica, sessie-opslag) staat er nog in. Doorbraak: `ui_main.rs` voor de `update()` dispatch en `ui_playback.rs` voor playback controls.

### 2. Voeg tests toe voor de parser en audio pipeline

`parse_arranger_string()` is perfect voor unit tests — deterministisch, geen IO, duidelijke input/output. `generate_short_id()`, `hsv_to_rgb()`, en `color_for_arranger()` zijn ook laaghangend fruit. Zelfs 10-15 tests geven vertrouwen bij refactoren.

### 3. Vervang stille `let _ =` door `eprintln!` of logging

Fouten bij schrijven van `session.json`, `library.json`, of het sturen van WaveformCommands worden nu volledig genegeerd. Een eenvoudige `fn log_error(msg: String)` of `eprintln!` maakt debugging aanzienlijk makkelijker zonder gedrag te veranderen.
