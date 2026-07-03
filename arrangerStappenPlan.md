# Arranger Window — Stappenplan

## Overzicht

Een arranger window waar gebruikers een afspeelvolgorde van loops kunnen bepalen.
Loops krijgen korte identifiers (a-z, aa-zz) waarmee je een sequentie opbouwt.
De sequentie wordt in één keer naar de audio-thread gestuurd ("fire & forget").
De audio-thread speelt het naadloos af — 100% gapless, sample-accuraat.

---

## Fase 1 — Data model (`src/arrangement.rs`)

Nieuwe module. Bevat de volledige data voor arrangementen + laden/opslaan + parser.

### Struct `Arrangement`

```rust
pub struct Arrangement {
    pub name: String,
    pub steps: Vec<ArrStep>,
}
```

### Struct `ArrStep`

```rust
pub struct ArrStep {
    /// Korte identifier van de loop (bv. "a", "b", "aa")
    pub loop_id: String,

    /// Pad naar het audiobestand (welke track)
    pub track_path: String,

    /// Hoe vaak deze stap herhaald moet worden (0 = oneindig)
    pub repeats: u32,

    /// Optionele overschrijving van pitch (halve tonen)
    pub pitch_semitones: f32,

    /// Optionele overschrijving van tempo (1.0 = normaal)
    pub tempo: f32,

    /// Kleur voor visuele weergave (automatisch gegenereerd uit loop_id + track_path)
    pub color: [u8; 3],
}
```

### Laden/opslaan

- Bestand: `arrangements.json`
- Bevat `Vec<Arrangement>` (meerdere arrangementen mogelijk)
- Functies: `load_arrangements()`, `save_arrangements()`

### Kleur hashen

Kleuren worden NIET handmatig ingesteld, maar gegenereerd uit een hash van `track_path + ":" + loop_id`, zodat:
- Dezelfde loop in elk arrangement dezelfde kleur heeft
- Geen handmatige configuratie nodig is
- Kleuren mooi verdeeld zijn over de kleurencirkel

```rust
pub fn color_for_arranger(loop_id: &str, track_path: &str) -> [u8; 3] {
    let hash = fxhash::hash64(&format!("{}:{}", track_path, loop_id));
    let hue = (hash % 360) as u8;         // 0-359 graden
    let saturation: u8 = 180;               // 70% pastel
    let value: u8 = 230;                    // 90% helder
    hsv_to_rgb(hue, saturation, value)
}
```

### Tekstuele notatie — Parser

Gebruiker kan een compacte string typen zoals:

| Invoer | Betekenis |
|---|---|
| `ABC` | A dan B dan C, elk 1 keer |
| `BCA` | B dan C dan A, elk 1 keer |
| `2b3A5C` | 2× B, 3× A, 5× C |
| `(aa)b` | AA (2-letter ID) dan B |

Parser: `parse_arranger_string(input: &str, lib: &Library) -> Result<Vec<ArrStep>, String>`

Implementatie als state machine (geen regex):

```rust
fn parse_arranger_string(input: &str, lib: &Library) -> Result<Vec<(String, u32)>, String> {
    let s: Vec<char> = input.to_lowercase().chars().collect();
    let mut i = 0;
    let mut result = Vec::new();
    let mut open_group = false;

    while i < s.len() {
        match s[i] {
            '(' => { open_group = true; i += 1; }
            ')' => { open_group = false; i += 1; }
            '0'..='9' => {
                let mut n = 0u32;
                while i < s.len() && s[i].is_ascii_digit() {
                    n = n * 10 + (s[i] as u8 - b'0') as u32;
                    i += 1;
                }
                // Na getal verwachten we een letter
                if i < s.len() && s[i].is_ascii_lowercase() {
                    let id = lees_id(&s, &mut i, lib);
                    result.push((id, n));
                }
            }
            'a'..='z' => {
                let id = lees_id(&s, &mut i, lib);
                result.push((id, 1));
            }
            ' ' | '\t' => { i += 1; } // spaties overslaan
            _ => return Err(format!("Ongeldig teken '{}' op positie {}", s[i], i)),
        }
    }

    Ok(result)
}

fn lees_id(s: &[char], i: &mut usize, lib: &Library) -> String {
    let mut id = String::new();
    id.push(s[*i]); *i += 1;
    // Tweede letter? Alleen als die bestaat EN niet door een spatie/getal/sluit-haakje gevolgd wordt
    if *i < s.len() && s[*i].is_ascii_lowercase() {
        id.push(s[*i]); *i += 1;
    }
    id
}
```

Let op: 2-letter IDs zoals `aa` worden alleen herkend als de parser ziet dat het NIET om `a` + `a` gaat. Dit wordt bepaald door de context (geen cijfer/spatie tussen). Voor de gebruiker: een 2-letter ID heeft geen scheidingsteken — als je `a` en `a` los wilt, schrijf je `aa` in de tekstuele notatie. Dit is dubbelzinnig, maar in de praktijk zijn 2-letter IDs zeldzaam genoeg om dit acceptabel te maken.

### ID generatie

Functie: `generate_short_id(existing_ids: &[String]) -> String`

- Eerst a-z (26 mogelijkheden)
- Dan aa, ab, ac, ... az, ba, bb, ... (26×26 = 676)
- Totaal: 26 + 676 = 702 IDs per track

---

## Fase 2 — Loop identifiers in library (`src/loops.rs`)

### Wijziging in `SavedLoop`

```rust
pub struct SavedLoop {
    pub label: String,
    pub short_id: Option<String>,  // NIEUW: "a", "b", ..., "aa", "ab"
    pub loop_a_secs: f32,
    pub loop_b_secs: f32,
    #[serde(default)]
    pub pitch_semitones: f32,
    #[serde(default = "default_tempo")]
    pub tempo: f32,
    #[serde(default)]
    pub notes: String,
}
```

### Auto-assign bij opslaan

Wanneer een nieuwe loop wordt opgeslagen in een track:
1. Verzamel alle bestaande `short_id`s in die track
2. Roep `generate_short_id()` aan voor een nieuw unieke ID
3. Wijs die toe aan de nieuwe loop

### Visualisatie in loop library

Toon de `short_id` tussen haakjes naast de label: `"(a) MijnLoop"`, met een gekleurd vierkantje.

---

## Fase 3 — Playback: fire & forget (audio-thread centraal)

### Nieuwe WaveformCommand::PlaySequence

```rust
pub enum WaveformCommand {
    // ... bestaande: Play, Stop, Pause, Resume, TogglePause,
    //     SetLoopBounds, Seek, SetPitch, SetTempo, SetVolume, SetLoopEnabled ...

    /// Nieuw: speel een hele sequentie in één keer af (gapless)
    PlaySequence {
        sequence_steps: Vec<SequenceStep>,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
    },
}
```

```rust
pub struct SequenceStep {
    pub samples: Arc<Vec<f32>>,    // samples van deze track
    pub sample_rate: u32,
    pub start_sample: usize,       // loop A (sample-index)
    pub end_sample: usize,         // loop B (sample-index)
    pub repeats: u32,              // aantal herhalingen
}
```

### Nieuwe SequenceSource iterator

Vervangt SoundTouchSource. Bevat `sequence: Vec<SequenceStep>`, `current_step_idx`, `read_pos`, en `step_event_tx: Sender<WaveformEvent>`.

In `next()`:
1. Lees sample tot `read_pos >= end_sample`
2. Nog herhalingen? `read_pos = start_sample`, repeat aftellen
3. Volgende stap? `current_step_idx += 1`, `read_pos = start_sample`, `ts.clear()`, stuur `StepChanged` event
4. Klaar? Stuur `ArrangementFinished` event, return `None`

### Nieuwe WaveformEvent varianten

```rust
pub enum WaveformEvent {
    // ... bestaande: Playing, Stopped, Paused, Resumed, Error, Position ...
    StepChanged(usize),
    StepRepeated(usize),
    ArrangementFinished,
}
```

### Waarom beter

| Aspect | Oud (ping-pong) | Nieuw (fire & forget) |
|---|---|---|
| Overgang tussen stappen | Latency via UI | 100% gapless |
| UI-belasting | Vangt Stopped af | Alleen display |
| State machine | ArrPlayState in UI | Geen |
| Pauzeren | Per stap, riskant | Hele sequentie |
| Meerdere audiobestanden | Niet mogelijk | Wel |

---

## Fase 4 — UI: Arranger window (`src/app.rs`)

### Nieuwe state in `app.rs`

```rust
pub show_arranger: bool,
pub active_arrangement: Option<usize>,
pub arrangements: Vec<Arrangement>,
pub arr_current_step: Option<usize>,  // alleen display
```

Geen ArrPlayState! Audio thread regelt de playback state.

### Venster

```rust
if self.show_arranger {
    egui::Window::new("🎛️ Arranger")
        .default_size([600.0, 400.0])
        .show(ctx, |ui| { ... });
}
```

### Bovenste balk

- 🔙 Sluit
- Naam (editbaar)
- ▶ Play / ⏹ Stop
- Dropdown arrangement kiezen / "➕ Nieuw"
- ❌ Verwijderen

### Lijst van steps

Elke stap als rij. Huidige stap gehighlight.

```
▶ [🎨] (a) NaamLoop  [▶ Preview] [−] 4 [+] [↑] [↓] [❌]
```

- ▶ Preview: speelt alleen deze stap (via WaveformCommand::Play, arrangement blijft idle)
- Gekleurd vakje uit `color_for_arranger()`
- Aantal herhalingen met − / +
- ↑/↓ volgorde, ❌ verwijderen

### Preview functionalitei

Preview functionaliteit: kleine ▶ knop per stap, stuurt WaveformCommand::Play voor die ene loop.
Zet arr_current_step = None zodat Stopped event niet als "arrangement step" wordt gezien.

### "Play Arrangement" actie

```
if ui.button("▶ Play").clicked() {
    let seq_steps: Vec<SequenceStep> = arrangement.steps.iter()
        .map(|step| converteer_naar_sequence_step(step, &library))
        .collect();
    self.waveform_cmd_tx.send(WaveformCommand::PlaySequence {
        sequence_steps: seq_steps,
        pitch_semitones: self.waveform_state.pitch.clone(),
        tempo: self.waveform_state.tempo.clone(),
    });
}
```

### Events verwerken

```
WaveformEvent::StepChanged(idx) => { arr_current_step = Some(idx); }
WaveformEvent::StepRepeated(idx) => { arr_current_step = Some(idx); }
WaveformEvent::ArrangementFinished => { arr_current_step = None; }
```

### Onderste gedeelte

- Textveld notatie + "Parse" knop
- "➕ Voeg stap toe" — popup met beschikbare loops

---

## Fase 5 — Serialisatie

- Laden bij opstarten (load_arrangements())
- Opslaan bij elke wijziging
- Bestand: arrangements.json
- Als bestand ontbreekt: lege vec

---

## Fase 6 — Later

- Drag & drop step volgorde (via pijltjes voor MVP)
- Tijdslijn met gekleurde blokjes
- BPM sync
- Crossfade tussen stappen

---

## Bestandsoverzicht

| Bestand | Wijziging |
|---|---|
| src/arrangement.rs | Nieuw: data, parser, kleur-hash, laden/opslaan |
| src/loops.rs | short_id toevoegen aan SavedLoop |
| src/waveform_player.rs | PlaySequence, SequenceSource, events |
| src/app.rs | Vereenvoudigde state, arranger UI, preview |
| Cargo.toml | fxhash toevoegen (voor kleur-hashing) |

## Implementatievolgorde

1. src/arrangement.rs — data types, parser, kleur-hash, laden/opslaan
2. src/loops.rs — short_id, auto-assign
3. src/waveform_player.rs — SequenceStep, PlaySequence, SequenceSource, events
4. src/app.rs — arranger window UI, preview, play koppeling, events
5. Testen & finetunen
