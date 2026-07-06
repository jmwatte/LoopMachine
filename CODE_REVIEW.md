# LoopMachine — Code Review & Verbeterplan

> Doorgenomen op 2026-07-06. Items gesorteerd op impact en efficiëntie: we beginnen met kleine, veilige verbeteringen en bouwen op naar grotere refactors. Elk item is onafhankelijk uitvoerbaar, maar de volgorde minimaliseert conflicten.

---

## Uitvoeringsvolgorde

### Fase 1 — Quick Wins (geen gedragsverandering)
_Deze zijn veilig, klein, en geven meteen momentum._

| # | Item | Bestand(en) | Geschatte tijd |
|---|------|-------------|---------------|
| 1 | ✅ Toon `_large_file_warning` in statusbalk i.p.v. weggooien | `waveform.rs`, `app.rs` | 5 min |
| 2 | ✅ Toon `short_id` in loop-lijst (`(a) NaamLoop` formaat) | `app.rs` | 10 min |
| 3 | ✅ Persisteer `arr_parse_buf` tussen sessies | `session.rs`, `app.rs` | 10 min |
| 4 | ✅ Vervang `max(0.0).min(dur)` door `clamp(0.0, dur)` voor consistentie | `app.rs` | 10 min |

### Fase 2 — Bugfixes
_Problemen die verkeerd gedrag veroorzaken, maar klein zijn om te fixen._

| # | Item | Bestand(en) | Geschatte tijd |
|---|------|-------------|---------------|
| 5 | ✅ Fix `waveform_has_content` — zet op `true` na succesvolle `load_file` | `app.rs` | 5 min |
| 6 | ✅ Fix mono kanaalmodus — mono moet door `mode.mix()` gaan, niet overslaan | `waveform.rs` | 10 min |
| 7 | ✅ Expliciete `SetLoopEnabled(false)` bij right-click wissen | `app.rs` | 5 min |
| 8 | ✅ Default color hash i.p.v. `[128; 3]` in `ArrStep` | `arrangement.rs` | 10 min |

### Fase 3 — Duplicate code & kleine refactors
_Code herstructureren zonder functie te veranderen._

| # | Item | Bestand(en) | Geschatte tijd |
|---|------|-------------|---------------|
| 9 | ✅ Verplaats `egui_key_to_serializable` naar `shortcuts.rs` | `app.rs`, `shortcuts.rs` | 15 min |
| 10 | ✅ Cache `all_possible_ids()` met `OnceLock` | `arrangement.rs` | 10 min |
| 11 | ✅ Voeg `UndoState::snapshot_from()` en `.apply_to()` methodes toe | `app.rs` | 20 min |
| 12 | ✅ Pre-allocatie van `input_chunk`/`temp_out` in source structs | `waveform_player.rs` | 20 min |
| — | ✅ Strip `\\?\\` prefix van Windows file dialog paden | `app.rs` | — |
| — | ✅ Vroege `Path::exists()` check in `load_file` voor betere feedback | `app.rs` | — |
| — | ✅ ❌ delete track knop in Alle Tracks window | `app.rs` | — |

### Fase 4 — Grotere refactors
_Deze veranderen de interne structuur en hebben testtijd nodig._

| # | Item | Bestand(en) | Geschatte tijd |
|---|------|-------------|---------------|
| 13 | ✅ Deel fill_buffer-logica tussen `SoundTouchSource` en `SequenceSource` | `waveform_player.rs` | 45 min |
| 14 | ✅ Dubbele `SaveLoop` code consolideren in één methode | `app.rs` | 15 min |
| 15 | ✅ Waveform rendering: mipmap/summary voor grote bestanden | `waveform.rs` | 1 uur |

### Fase 5 — Architectuur
_Grote opsplitsingen die het bestand beheersbaar maken._

| # | Item | Bestand(en) | Geschatte tijd |
|---|------|-------------|---------------|
| 16 | Split `app.rs` op in submmodules (`ui_export.rs`, `ui_playback.rs`, etc.) | `app.rs` (3000→~500 regels per file) | 1-2 uur |

---

## Gedetailleerde beschrijvingen

### Fase 1 — Quick Wins

#### 1. Toon `_large_file_warning` in statusbalk
**Bestand:** `waveform.rs:159-166`
**Huidig:** `_large_file_warning` wordt berekend maar de variabele begint met `_`, dus de compiler waarschuwt niet, maar de warning wordt nergens getoond.
**Fix:** Geef de warning terug als extra returnwaarde of log hem via `eprintln!`/status message.

#### 2. Toon `short_id` in loop-lijst
**Bestand:** `app.rs:2377-2391` (track paneel), `app.rs:2598-2608` (Alle Tracks window)
**Huidig:** Loops worden getoond als `label` zonder de `short_id`.
**Gewenst:** `(a) MijnLoop` formaat, zoals beschreven in het stappenplan. Eventueel met een gekleurd vierkantje.

#### 3. Persisteer `arr_parse_buf` tussen sessies
**Bestand:** `app.rs:180`, `session.rs`
**Huidig:** Het parse-tekstveld voor arrangementen wordt niet opgeslagen in `session.json`.
**Fix:** Voeg `arr_parse_buf` toe aan `SessionState`.

#### 4. Consistent `clamp` gebruik
**Bestand:** `app.rs`, `waveform.rs`
**Huidig:** Sommige plekken gebruiken `.max(0.0).min(dur)`, andere `.clamp(0.0, dur)`.
**Fix:** Vervang alle `.max().min()` patronen door `.clamp()` (sinds Rust 1.50).

---

### Fase 2 — Bugfixes

#### 5. `waveform_has_content` blijft `false` na `load_file`
**Bestand:** `app.rs:288-337`
**Huidig:** Na `decode_audio` → `Ok(...)` wordt `waveform_has_content` niet op `true` gezet.
**Fix:** Zet `self.waveform_has_content = true;` bij een succesvolle decode.
**Impact:** Voorkomt dat de eerste PlayPause shortcut onterecht een nieuwe playback start.

#### 6. Mono kanaalmodus: mono moet door `mode.mix()` gaan
**Bestand:** `waveform.rs:267-276`
**Huidig:** Bij `num_channels == 1` wordt het sample direct gepusht, óók als er een `ChannelMode::Left` of `Right` geselecteerd is. Dit slaat de modus over.
**Fix:** Voor mono moet het sample naar beide virtuele kanalen gekopieerd worden (`left = right = sample`), vervolgens door `mode.mix(left, right)`.

#### 7. Expliciete `SetLoopEnabled(false)` bij wissen
**Bestand:** `app.rs:2058-2063`
**Huidig:** Bij rechterklik: `SetLoopBounds { 0, 0 }` impliciteert "disabled" omdat `0 > 0` false is.
**Fix:** Stuur expliciet `SetLoopEnabled(false)` naast `SetLoopBounds { 0, 0 }`.

#### 8. Default color in `ArrStep` moet gehashed worden
**Bestand:** `arrangement.rs:48-50`
**Huidig:** `default_color()` retourneert `[128; 3]` (grijs). Alleen bij deserializatie wordt deze gebruikt als `color` niet in JSON staat.
**Fix:** Genereer de default color uit een hash van `track_path + ":" + loop_id` (gebruik dezelfde `color_for_arranger` functie), maar dit is lastig bij deserializatie. Alternatief: bereken de kleur in `build_sequence_step` of bij het tonen in de UI.

---

### Fase 3 — Duplicate code & kleine refactors

#### 9. Verplaats `egui_key_to_serializable` naar `shortcuts.rs`
**Bestand:** `app.rs:224-287` → `shortcuts.rs`
**Huidig:** Een 60-regelige match in `app.rs`. De inverse (`From<SerializableKey> for egui::Key`) staat al in `shortcuts.rs`.
**Fix:** Verplaats de functie en voeg een `From<egui::Key> for SerializableKey` implementatie toe naast de bestaande `From<SerializableKey> for egui::Key`.

#### 10. Cache `all_possible_ids()`
**Bestand:** `arrangement.rs:106-122`
**Huidig:** Bouwt elke aanroep de hele set van 702 IDs opnieuw (2× Vec push).
**Fix:** Gebruik `std::sync::OnceLock<Vec<String>>` om de lijst één keer te bouwen. Verhoog dit naar een generatie voor 3-letter IDs (aaa-zzz) voor 18.278 mogelijke IDs.

#### 11. `UndoState` snapshot/apply methodes
**Bestand:** `app.rs:410-462`, `app.rs:1728-1765`
**Huidig:** Bij elke undo/redo push staat dezelfde 9-veld assignment herhaald (twee keer!).
**Fix:** Voeg toe:
```rust
impl UndoState {
    fn snapshot_from(app: &LoopEditorApp) -> Self { ... }
    fn apply_to(self, app: &mut LoopEditorApp) { ... }
}
```

#### 12. Pre-allocatie in `fill_buffer`
**Bestand:** `waveform_player.rs:550,607,781,824`
**Huidig:** `let mut input_chunk = Vec::with_capacity(4096);` en `let mut temp_out = vec![0.0; 4096];` worden elke `fill_buffer`-aanroep (~60x/s) gealloceerd.
**Fix:** Maak deze buffers velden van de source-structs en hergebruik ze met `clear()`.

---

### Fase 4 — Grotere refactors

#### 13. Gedeelde AudioProcessor trait
**Bestand:** `waveform_player.rs:463-834`
**Huidig:** `SoundTouchSource` en `SequenceSource` hebben bijna identieke logica voor: pitch/tempo wijzigingen checken, input chunks lezen uit samples, TimeStretch voeden, output drainen, volume toepassen, soft-clip.
**Fix:** Trek een trait (bv. `AudioSource`) of gedeelde `AudioProcessor` struct uit die beide sources gebruiken. Of: laat `SequenceSource` per stap een `SoundTouchSource`-achtige interne source gebruiken.

#### 14. Consolideer SaveLoop code
**Bestand:** `app.rs:1625-1663` (shortcut handler) en `app.rs:2284-2318` (UI button)
**Huidig:** Exact dezelfde logica op twee plekken. Verschil: shortcut gebruikt `library.add_loop()`, UI button ook.
**Fix:** Vervang beide door één `save_current_loop()` methode op `LoopEditorApp`.

#### 15. Waveform mipmap rendering
**Bestand:** `waveform.rs:563-598`
**Huidig:** Pixel-by-pixel: voor elke pixel itereert het over alle samples in die pixel. Bij zoom-out zijn dat duizenden samples per pixel, wat de UI-thread kan laten haperen.
**Fix:** Pre-compute per zoom-niveau een summary: `Vec<(f32, f32)>` met min/max voor een geschikt aantal bins (bv. 1 bin per 4 pixels). Bouw een pyramid/summary bij het laden van een bestand.

---

### Fase 5 — Architectuur

#### 16. Split `app.rs` op
**Bestand:** `app.rs` (~3000 regels)
**Huidig:** Eén gigantisch bestand met `update()` van ~2000 regels.
**Fix:** Splits in submmodules:
- `ui_export.rs` — ExportWindow, ExportState, write_wav, execute_export
- `ui_arranger.rs` — show_arranger_ui (eventueel + arrangement.rs)
- `ui_library.rs` — Alle Tracks window, loop library
- `ui_playback.rs` — Playback controls, pitch/tempo/volume sliders
- `ui_shortcuts.rs` — Shortcut editor window + help overlay
- `ui_main.rs` — update() dispatch, top toolbar, CentralPanel dispatch

---

### Extra: Feature suggesties (niet in scope voor deze review-ronde)

| Feature | Beschrijving |
|---------|-------------|
| BPM detectie | Voeg beat-tracking toe naast chroma-analyse |
| Tap tempo | Druk op `T` in ritme om BPM in te stellen |
| Quantize markers | Rond marker posities af op BPM-grid |
| Crossfade tussen arrangement-stappen | 10ms crossfade in SequenceSource |
| Volume-automation per loop | Sla volume op in SavedLoop |
| MIDI learn | Koppel MIDI CC's aan shortcuts |
| Multi-track playback | Mix meerdere audio-bestanden tegelijk |
| Project save/load | `.loopmachine` bestandsformaat |
| Spectrum analyzer | Real-time FFT visualisatie |
| Audio recording | Neem microfoon/line-in op |
| Arrangement timeline | Visuele timeline met gekleurde blokjes |

---

> **Hoe aan te pakken:** Begin bij Fase 1, item 1. De items zijn klein en onafhankelijk, maar de volgorde zorgt dat latere refactors (Fase 3-5) profiteren van eerdere opruimwerk.
