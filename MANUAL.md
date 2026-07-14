# Waveform Loop Editor — Handleiding

## 1. Snelstart

1. **Bestand openen**: Klik "📂 Open bestand" of `Ctrl+O`, of sleep een audiobestand in het venster.
2. **Eerste loop maken**:
   - Druk `[` om punt A te zetten (waar de playhead staat)
   - Druk `]` om punt B te zetten
   - Druk `Spatie` om de loop af te spelen
3. **Loop opslaan**: Klik "💾 Save Loop" of `Ctrl+S`.
4. **Exporteren**: Klik "📤 Export" of `Ctrl+E`, selecteer loops en kies een locatie.

---

## 2. Bestanden

### Openen
- **Knop**: "📂 Open bestand" in de toolbar
- **Shortcut**: `Ctrl+O`
- **Drag & drop**: Sleep een bestand in het venster
- **Pad typen**: Typ het pad in het tekstveld en druk `Enter`

### Ondersteunde formaten
| Formaat | Extensie |
|---------|----------|
| MP3     | `.mp3`   |
| WAV     | `.wav`   |
| FLAC    | `.flac`  |
| OGG     | `.ogg`   |
| M4A/AAC | `.m4a`   |

### Kanaalmodus
Kies hoe stereobestanden naar mono worden gemixed:
| Modus | Omschrijving |
|-------|-------------|
| Mono (L+R) | Beide kanalen gemiddeld |
| Links (L)  | Alleen linkerkanaal |
| Rechts (R) | Alleen rechterkanaal |
| Mid (center) | Mid-kanaal (L+R) |
| Side (breedte) | Side-kanaal (L-R) |

---

## 3. Waveform & Playback

### Playback bediening
| Actie | Toets | Omschrijving |
|-------|-------|-------------|
| Play / Pause | `Spatie` | Start/stoppen afspelen |
| Stop | `Escape` | Volledig stoppen |
| Playhead links | `←` | 0.20s terug |
| Playhead rechts | `→` | 0.20s vooruit |
| Snel terug | `↓` | 2s terug |
| Snel vooruit | `↑` | 2s vooruit |
| Herstart loop/file | `Enter` | Speel vanaf loop-A of begin bestand |

### Navigatie
- **Klik** op de waveform om de playhead te verplaatsen.
- **Scroll** om in/uit te zoomen.
- **Sleep** (zonder Ctrl) om horizontaal te scrollen.

### Zoom
| Actie | Toets |
|-------|-------|
| Inzoomen | `Ctrl+0` of knop 🔍+ |
| Uitzoomen | `Ctrl+Shift+0` of knop 🔍− |
| Reset | `Ctrl+R` of knop "⟲ Reset zoom/scroll" |
| Scrollen: volgend deel | `PageDown` — verspring één paginabreedte vooruit op huidig zoomniveau |
| Scrollen: vorig deel | `PageUp` — verspring één paginabreedte terug op huidig zoomniveau |
| Follow playhead aan/uit | `F` — playhead blijft gecentreerd tijdens afspelen, waveform scrollt mee |

---

## 4. Loops

### A-B loop maken
| Methode | Actie |
|---------|-------|
| Aparte toetsen | `[` = punt A, `]` = punt B |
| 1-toets toggle | Druk 1x voor punt A, 2x voor A-B |
| Ctrl+sleep | Sleep over de waveform met Ctrl ingedrukt |
| Dubbelklik | Dubbelklik = punt A |
| Shift+dubbelklik | Shift+dubbelklik = punt B |

### Loop bewerken
| Actie | Toets | Omschrijving |
|-------|-------|-------------|
| Wissen | `Ctrl+Backspace` of rechterklik | Huidige Begin-Eind verwijderen |
| Nudge loop links | `Shift+←` | Schuif loop naar links (behoud lengte) |
| Nudge loop rechts | `Shift+→` | Schuif loop naar rechts (behoud lengte) |
| Nudge Begin links | `J` | Verschuif Begin 0.05s naar links |
| Nudge Begin rechts | `Shift+J` | Verschuif Begin 0.05s naar rechts |
| Nudge Eind links | `L` | Verschuif Eind 0.05s naar links |
| Nudge Eind rechts | `Shift+L` | Verschuif Eind 0.05s naar rechts |
| **Verdubbelen** | `Ctrl+D` | Verdubbel de loop-lengte (vanaf Begin) |
| **Halveren** | `Ctrl+Shift+D` | Halveer de loop-lengte (vanaf Begin) |
| **Snap links** | `Q` | Verplaats loop naar dichtstbijzijnde marker links |
| **Snap rechts** | `W` | Verplaats loop naar dichtstbijzijnde marker rechts |
| Bypass | `Ctrl+B` | Schakel looping uit (speelt door naar einde) |
| Center in viewport | `C` | Centreer de loop in het venster |

### Herhaal telling
Stel een aantal herhalingen in met de "Herhaal:" teller. `0` = oneindig herhalen.

### Loop opslaan
- **Knop**: Klik "💾 Save Loop"
- **Shortcut**: `Ctrl+S`
- De loop wordt opgeslagen in `library.json` met een automatische naam (`{bestand} - Loop {n}`)

### Loop laden
- Klik `▶` in de "Opgeslagen Loops" lijst om een loop te laden.
- De loop A, B, pitch en tempo worden hersteld.

### Loop hernoemen
- **Dubbelklik** op de naam van een loop in de lijst.
- Typ een nieuwe naam en druk `Enter` of klik buiten het veld.

---

## 5. Markers

Markers zijn bladwijzers op de tijdlijn. Er zijn 3 types:

| Type | Toets | Kleur | Prioriteit | Gebruik |
|------|-------|-------|-----------|---------|
| Section | `S` | Goud `#dcb432` | **Hoogste** | Intro, Chorus, Bridge... |
| Measure | `M` | Blauw `#50a0ff` | Midden | Maat 1, Maat 2... |
| Beat | `B` | Groen `#50dc78` | Laagste | Beat-markeringen |

### Marker plaatsen/verwijderen (toggle)
Alle markers werken met **toggle**: druk nogmaals op dezelfde toets op dezelfde positie om de marker te verwijderen.

| Actie | Toets | Omschrijving |
|-------|-------|-------------|
| Section togglen | `S` | Zet/verwijder Section op playhead |
| Measure togglen | `M` | Zet/verwijder Measure op playhead |
| Beat togglen | `B` | Zet/verwijder Beat op playhead |
| Verwijder dichtstbijzijnde | `Backspace` | Verwijdert marker ongeacht type |

### Marker via muis
| Actie | Resultaat |
|-------|-----------|
| Dubbelklik in marker zone | Toggle Section marker |
| Shift+dubbelklik | Toggle Measure marker |
| Ctrl+dubbelklik | Toggle Beat marker |
| Klik op marker | Verplaats playhead naar marker |
| Sleep marker | Versleep marker naar nieuwe positie |
| Rechterklik op marker | Verwijder marker |
| Dubbelklik op marker | Hernoem marker |

### Marker navigatie
| Actie | Toets | Omschrijving |
|-------|-------|-------------|
| Vorige marker | `Ctrl+←` | Spring playhead naar marker links |
| Volgende marker | `Ctrl+→` | Spring playhead naar marker rechts |

### Marker prioriteit op dezelfde positie
Als meerdere markers op exact dezelfde positie staan, wordt alleen de **hoogste prioriteit** getoond:

> **Section** (goud) > **Measure** (blauw) > **Beat** (groen)

De naam en kleur van de hoogste prioriteit is zichtbaar. De verborgen markers blijven wel interactief (klik, sleep, verwijder).

### Marker indicator
Onder de waveform wordt getoond welke markers op de huidige playhead-positie staan:

```
📍 S1, B    |  120.0s  |  44100 Hz  |  Zoom: 100x
```

---

## 6. Audio Bewerking

### Pitch
- **Bereik**: -12 tot +12 halve tonen
- Schakel de pitch aan/uit tijdens playback
- Reset met de `⟲` knop
- **Toolbar**: Voeg `⬆ Pitch +1` en `⬇ Pitch −1` knoppen toe via ⋮ → Toolbar aanpassen

### Tempo
- **Bereik**: 0.1× tot 3.0× (10% – 300%)
- Past de afspeelsnelheid aan zonder toonhoogte te veranderen
- Reset met de `⟲` knop
- **Toolbar**: Voeg `🔄↑ Tempo +10%` en `🔄↓ Tempo −10%` knoppen toe via ⋮ → Toolbar aanpassen

### Volume
- **Bereik**: 0.0× tot 2.0× (stil – dubbel volume)
- Reset met de `⟲` knop

### Chroma detectie
Analyseer de A-B selectie op toonhoogtes:
1. Zet een A-B loop
2. Klik "🔍 Detecteer noten"
3. De meest waarschijnlijke noot wordt getoond met een betrouwbaarheidspercentage

---

## 7. Setup — Latency & Beat Audit

Het setup-venster (⚙ **Setup** in de toolbar) bevat alle instellingen voor
latency-compensatie, beat-audit, BPM-detectie en marker-beheer.

### 7.1 Latency Kalibratie

Als je tijdens het afspelen een marker zet (`B`/`M`/`S`), komt deze **te laat**
omdat de audio door de SoundTouch-processor en audiokaart-buffer moet. Deze
vertraging verschilt per computer.

#### Manual marker latency
- **Slider**: 0–200 ms (stap 5 ms)
- **Default**: 40 ms
- De marker wordt zoveel ms **vóór** de playhead gezet als compensatie
- **Te vroeg** → verlaag de waarde
- **Te laat** → verhoog de waarde

#### Kalibratie-test
1. Klik **"🔔 Start kalibratie-test"**
2. Er worden 8 clicks (om de 1.5s) afgespeeld met een **visuele flits** in het setup-venster
3. De flits is exact op tijd, de click hoor je later door de audiobuffers
4. Pas de latency aan tot het flits-ritme en click-ritme synchroon lopen
5. Tip: begin bij 0 ms en werk omhoog tot je de vertraging ziet

### 7.2 Beat Audit — Kliktrack

Schakel de kliktrack in om te horen waar de beat-markers vallen. De click wordt
sample-accuraat in de audio gemixed (geen sync-problemen).

| Optie | Omschrijving |
|-------|-------------|
| **Beat audit aan** | Checkbox: hoorbare clicks op markers |
| **Clicks op: Auto-BPM beats** | Clicks op SoundTouch BPM-detectie resultaten |
| **Clicks op: Handmatige markers** | Clicks op eigen S/M/B markers |
| **"🔄 Ververs click-posities"** | Herlaad de click-posities na wijzigingen |

### 7.3 BPM Detectie

#### Detectie-drempel (strength)
- **Slider**: 0.0–1.0 (stap 0.05)
- **Default**: 0.3
- Hoe hoger, hoe strenger — alleen sterke beats worden marker
- Klik **"🔄 Herplaats BPM beats"** om de drempel + offset toe te passen

#### Beat offset-correctie
- **Slider**: −50…+50 ms (stap 1 ms)
- Alleen voor auto-BPM markers (SoundTouch)
- Positief = beats later, negatief = beats vroeger

### 7.4 Snelacties

| Actie | Omschrijving |
|-------|-------------|
| **"🔄 Start auto-detectie"** | Voer chroma + BPM + beat-detectie uit (zelfde als 🔍 Detecteer) |
| **"🗑 Wis alle markers"** | Verwijder alle markers in één keer |
| **Schuif markers** | Schuif alle markers met X ms (bulk shift, −200…+200 ms) |

De bulk shift is handig als alle markers structureel te vroeg of te laat blijken
(bijv. door een vaste offset in de audiokaart).

---

## 8. Export

### Exportvenster openen
- **Knop**: Klik "📤 Export" in de toolbar (alleen actief als de track loops heeft)
- **Shortcut**: `Ctrl+E`

### Loops selecteren
| Actie | Omschrijving |
|-------|-------------|
| Checkbox per loop | Vink loops aan die je wilt exporteren |
| **Select All** | Selecteer alle loops |
| **Deselect All** | Deselecteer alle loops |
| Default | Alle loops uit (eerst selecteren) |

### Instellingen
| Optie | Opties | Omschrijving |
|-------|--------|-------------|
| Basis naam | Tekstveld | Naam voor de exportbestanden (default: `audiotrack_loops`) |
| Formaat | WAV (.wav) | 16-bit mono PCM |
| Modus | Gecombineerd / Apart | Zie hieronder |

### Exportmodi

**Gecombineerd bestand**
- Alle geselecteerde loops worden aan elkaar geplakt (in volgorde van de lijst)
- Er wordt een `.wav` bestand gevraagd
- Handig voor een mix of medley

**Aparte bestanden**
- Elke loop wordt een apart `.wav` bestand
- Er wordt een **map** gevraagd
- Bestandsnaam: `{basisnaam}_{label}.wav`
- Als een bestand al bestaat, wordt `_001`, `_002`, etc. toegevoegd
- Handig voor gebruik in DAW's zoals Ableton, FL Studio, etc.

### Let op
- Geëxporteerde audio is **altijd mono** (volgens de gekozen kanaalmodus)
- Pitch en tempo worden **niet** meegenomen in de export (rauwe audio)

---

## 9. Arranger

De arranger laat je loops in een **sequentie** afspelen.

### Een arrangement maken
1. Zorg dat loops zijn opgeslagen met een `short_id` (wordt automatisch toegekend).
2. Open de arranger met de "ARR" knop rechtsboven.
3. Typ een sequentie in het tekstveld, bijvoorbeeld:

```
song/a x2, song/b, song/c x3
```

Dit speelt: loop `a` 2×, loop `b` 1×, loop `c` 3×.

### Formaat
```
{track_naam}/{loop_id}x{aantal_herhalingen}
```

- `track_naam`: naam van het audiobestand
- `loop_id`: de korte ID (bijv. `a`, `b`, `c`)
- `x{aantal_herhalingen}`: optioneel, standaard 1×
- Scheiding: `,` tussen stappen

### Arrangement afspelen
- Klik "▶ Speel Arrangement" om de sequentie te starten.
- De arranger loopt door de stappen en herhaalt waar aangegeven.
- Het huidige stepnummer wordt getoond.

---

## 10. Shortcuts

### Overzicht
Druk op `F1` voor een volledig overzicht van alle sneltoetsen, gegroepeerd per categorie.

### Shortcuts aanpassen
1. Druk op `F1` om het overzicht te openen.
2. Klik "⚙ Edit Shortcuts".
3. Klik op de huidige toetscombinatie naast een actie.
4. Druk op de gewenste nieuwe toets (met modifiers Ctrl/Shift/Alt).
5. De shortcut wordt automatisch opgeslagen.

### Conflict detectie
Als je een toets probeert te koppelen die al in gebruik is, wordt een waarschuwing getoond.

### Reset
- **Per actie**: Klik de `⟲` knop naast een actie in de editor.
- **Alles**: Klik "🔄 Reset alles naar defaults".

### Standaard shortcuts

#### Playback
| Actie | Toets |
|-------|-------|
| Play / Pause | `Spatie` |
| Stop | `Escape` |
| Snel terug 2s | `↓` |
| Snel vooruit 2s | `↑` |

#### Loop
| Actie | Toets |
|-------|-------|
| Loop A | `[` |
| Loop B | `]` |
| Loop wissen | `Ctrl+Backspace` |
| Bypass toggle | `Ctrl+B` |
| Nudge links | `Shift+←` |
| Nudge rechts | `Shift+→` |
| Nudge A links | `J` |
| Nudge A rechts | `Shift+J` |
| Nudge B links | `L` |
| Nudge B rechts | `Shift+L` |
| Playhead links | `←` |
| Playhead rechts | `→` |
| 1-toets A-B | `[` |
| Center loop | `C` |
| Loop opslaan | `Ctrl+S` |
| Loop herstart | `Enter` |
| **Loop verdubbelen** | `Ctrl+D` |
| **Loop halveren** | `Ctrl+Shift+D` |
| **Snap naar marker links** | `Q` |
| **Snap naar marker rechts** | `W` |

#### Markers
| Actie | Toets |
|-------|-------|
| Section toggle | `S` |
| Measure toggle | `M` |
| Beat toggle | `B` |
| Verwijder dichtstbijzijnde | `Backspace` |
| **Marker vorige** | `Ctrl+←` |
| **Marker volgende** | `Ctrl+→` |

#### View
| Actie | Toets |
|-------|-------|
| Inzoomen | `Ctrl+0` |
| Uitzoomen | `Ctrl+Shift+0` |
| Reset zoom | `Ctrl+R` |
| Scrollen: volgend deel | `PageDown` |
| Scrollen: vorig deel | `PageUp` |
| Follow playhead aan/uit | `F` |
| Shortcuts help | `F1` |

#### File
| Actie | Toets |
|-------|-------|
| Bestand openen | `Ctrl+O` |
| **Export loops** | `Ctrl+E` |

#### Edit
| Actie | Toets |
|-------|-------|
| Undo | `Ctrl+Z` |
| Redo | `Ctrl+Shift+Z` |

---

## 11. Loop Bibliotheek

### Alle Tracks venster
Open met de "📚 Alle Tracks" knop. Hier zie je al je geladen tracks en hun opgeslagen loops.

| Actie | Omschrijving |
|-------|-------------|
| `▶` bij track | Laad de track en de eerste loop |
| `▶` bij loop | Laad die specifieke loop |
| `❌` | Verwijder loop (met bevestiging) |

### Library bestand
Alle loops worden opgeslagen in `library.json`. Dit bestand kun je delen tussen sessies.

---

## 12. Tips & Tricks

### Workflow: markers → loops
1. Zet markers op belangrijke posities (S voor secties, M voor maten, B voor beats).
2. Gebruik `Q`/`W` om loops naar markers te snappen.
3. Gebruik `Ctrl+D`/`Ctrl+Shift+D` om loop-lengte aan te passen.
4. Sla loops op met herkenbare namen.
5. Exporteer alle loops in 1 keer voor gebruik in je DAW.

### Snel exporteren voor DAW
1. Maak loops en geef ze duidelijke namen (bijv. "Intro", "Chorus 1").
2. Kies "Aparte bestanden" modus.
3. Selecteer een map in je DAW project map.
4. Importeer de `.wav` bestanden in je DAW.

### Undo/Redo
- `Ctrl+Z` voor ongedaan maken
- `Ctrl+Shift+Z` voor opnieuw doen
- Tot 50 stappen onthouden

### Sessie herstel
De editor onthoudt je laatste bestand, positie, zoom, loop-instellingen en pitch/tempo in `session.json`. Bij herstart pak je verder waar je gebleven was.

---

## 13. Toolbar aanpassen

De actie-werkbalk (onder de bestandstoolbar) is **volledig aanpasbaar**: je kiest zelf welke knoppen verschijnen en in welke volgorde.

### Openen
Klik op de **⋮** (kebab-knop) rechts in de toolbar om het venster "Toolbar aanpassen" te openen.

### Knoppen ordenen
- **▲ / ▼** pijltjes: verplaats een knop omhoog/omlaag
- **✖**: verwijder een knop uit de toolbar
- Klik op een actie onder **"Beschikbare acties"** om deze toe te voegen

### Voorinstellingen
- **"Standaard instellen"**: reset naar de oorspronkelijke set knoppen
- **"Alles wissen"**: maak de toolbar helemaal leeg

### Context-afhankelijkheid
Knoppen worden **automatisch uitgeschakeld** (grijs) als de actie niet van toepassing is:

| Actie | Actief wanneer |
|-------|---------------|
| 🔍 Detecteer | Een A-B selectie is actief |
| ↗ Verleng beats | ≥ 2 beat markers aanwezig |
| ✕ Wis loop | Een A-B selectie is actief |
| ↩ Undo | Er is een undo-stap beschikbaar |
| ↪ Redo | Er is een redo-stap beschikbaar |
| 💾 Save Loop | Een A-B selectie + bestand is geladen |
| 🎯 Center Loop | Een bestand is geladen |
| 🔍− / 🔍+ / ⟲ Reset | Een bestand is geladen |
| 📌 Beats / ⬆⬇ Pitch / 🔄↑↓ Tempo | Een bestand is geladen |
| 📤 Export | Library heeft loops voor deze track |
| ARR / ⚙ Setup / 🔇 Audit | Altijd beschikbaar |

### Sneltoets in hover
Beweeg over een knop om de **huidige sneltoets** te zien (bv. "Ongedaan maken (Ctrl+Z)"). De tekst past zich aan als je shortcuts wijzigt in de shortcut-editor.

### Persistentie
Je toolbar-configuratie wordt opgeslagen in `session.json` en is dus na een herstart behouden.

### Alle beschikbare acties
| Actie | Icoon | Omschrijving |
|-------|-------|-------------|
| 🔍 Detecteer | `🔍` | Analyseer A-B selectie op toonhoogtes en BPM |
| ↗ Verleng beats | `↗` | Verspreid beat markers over de hele audio |
| ✕ Wis loop | `✕` | Verwijder A-B selectie |
| ↩ Undo | `↩` | Ongedaan maken |
| ↪ Redo | `↪` | Opnieuw doen |
| 💾 Save Loop | `💾` | Bewaar huidige loop in bibliotheek |
| 🎯 Center Loop | `🎯` | Centreer weergave op de A-B loop |
| 🔍− Zoom − | `🔍−` | Uitzoomen |
| 🔍+ Zoom + | `🔍+` | Inzoomen |
| ⟲ Reset zoom | `⟲` | Reset zoom en scroll |
| 📌 Beats | `📌` | Plaats/verwijder beat marker bij playhead |
| ARR | `ARR` | Open/sluit de arranger view |
| 📤 Export | `📤` | Exporteer loops naar WAV |
| ⚙ Setup | `⚙` | Open setup-venster |
| 🔇 Audit | `🔇` | Schakel beat-audit kliktrack aan/uit |
| 🔄↑ Tempo +10% | `🔄↑` | Verhoog afspeelsnelheid met 10% |
| 🔄↓ Tempo −10% | `🔄↓` | Verlaag afspeelsnelheid met 10% |
| ⬆ Pitch +1 | `⬆` | Verhoog toonhoogte met 1 semitone |
| ⬇ Pitch −1 | `⬇` | Verlaag toonhoogte met 1 semitone |

---

## 14. Toetsenreferentie (snelzoekkaart)

```
PLAYBACK
─────────────────────────────────────────
Spatie         Play / Pause
Escape         Stop
←              Playhead 0.20s links
→              Playhead 0.20s rechts
↓              Snel 2s terug
↑              Snel 2s vooruit
Enter          Herstart loop / begin file

LOOPS
─────────────────────────────────────────
[              Loop A
]              Loop B
Ctrl+Backspace Loop wissen
Ctrl+B         Loop bypass toggle
Shift+←        Loop links nudgen
Shift+→        Loop rechts nudgen
J / Shift+J    Nudge A links/rechts
L / Shift+L    Nudge B links/rechts
C              Center loop
Ctrl+S         Loop opslaan
Ctrl+D         Loop verdubbelen
Ctrl+Shift+D   Loop halveren
Q              Snap naar marker links
W              Snap naar marker rechts

MARKERS
─────────────────────────────────────────
S              Section toggle
M              Measure toggle
B              Beat toggle
Backspace      Verwijder dichtstbijzijnde
Ctrl+←         Naar vorige marker
Ctrl+→         Naar volgende marker

VIEW
─────────────────────────────────────────
Ctrl+0         Inzoomen
Ctrl+Shift+0   Uitzoomen
Ctrl+R         Reset zoom
F1             Shortcuts help

FILE / EDIT
─────────────────────────────────────────
Ctrl+O         Bestand openen
Ctrl+E         Export loops
Ctrl+Z         Undo
Ctrl+Shift+Z   Redo
```

---

*Laatst bijgewerkt: juli 2026 (toolbar aanpasbaar, tempo/pitch knoppen, setup & latency)*
