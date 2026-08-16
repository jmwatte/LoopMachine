# Plan: Dependencies up-to-date brengen (LoopMachine)

## Doel

Alle crates in LoopMachine naar een actuele versie brengen. LoopMachine is een apart
project (eigen git-repo) dat ooit voortkwam uit `loop-editor/` in de JukeBox-workspace,
maar inmiddels volledig eigen code heeft (~12.000 regels). De `Cargo.toml` gebruikt nog
dezelfde oude versies als JukeBox destijds: eframe 0.28 en rodio 0.19.

## Audit (geïnstalleerd → actueel, aug 2026)

| Crate | Geïnstalleerd | Actueel | Achterstand | Oordeel |
|---|---|---|---|---|
| eframe | 0.28.1 | 0.36.1 | ~2 j, 8 majors | Hoog — grote refactor |
| egui_extras | 0.28.1 | 0.36.1 | 8 majors | volgt eframe |
| egui-file-dialog | 0.6.1 | 0.15.0 | 9 majors | Hoog — gebonden aan egui, API gewijzigd |
| rodio | 0.19.0 | 0.22.2 | ~2 j, 3 majors | Middel — `Sink`→`Player` |
| symphonia | 0.5.5 | 0.6.1 | 1 major | Niet upgraden (rodio 0.22 gebruikt zelf 0.5.5) |
| soundtouch | 0.5.4 | 0.5.4 | 0 | actueel |
| rustfft | 6.4.1 | 6.4.1 | 0 | actueel |
| hound | 3.5.1 | 3.5.1 | 0 | actueel |
| crossbeam-channel | 0.5.15 | 0.5.16 | patch | actueel |
| serde / serde_json | 1.0.228 / 1.0.150 | 1.0.229 / 1.0.151 | patch | actueel |
| log / env_logger | 0.4.33 / 0.11.11 | 0.4.33 / 0.11.11 | 0 | actueel |
| cc (build) | 1.x | 1.4.3 | minors | build-dep |
| embed-resource (build) | 3.0 | 3.0.11 | patch | build-dep |

## LoopMachine-specifieke risico's

- **Rubber Band C++-build** (`vendor/rubberband`, default feature): geen crate-upgrade,
  maar moet blijven compileren via `build.rs` — extra testpunt.
- **mpv video player** (`video_player.rs`): externe executable via named pipes — geen impact.
- **`SequenceSource`** in `waveform_player.rs` implementeert rodio's `Source`-trait:
  in rodio 0.21+ zijn samples `f32` (bij JukeBox hoefde dit niet aangepast, hier checken).
- **egui-file-dialog 0.15** gebruikt een widget-patroon (`ui.add(dialog)`) i.p.v. het oude
  `dialog.update(ctx)` — de 3 dialogs in `src/app/mod.rs` (open, relink, export) herbouwen.

---

## Fase 0 — Voorbereiding

- [x] `Cargo.lock` uit `.gitignore` halen en committen (was ook bij JukeBox de basis voor
      reproduceerbare builds en traceerbare upgrades).
      Commit: `Commit Cargo.lock for reproducible builds`

## Fase 1 — Audio-kern: rodio 0.19 → 0.22

- [x] `src/waveform_player.rs` migreren:
  - `OutputStream::try_default()` + `Sink::try_new(&handle)` →
    `DeviceSinkBuilder::open_default_sink()` + `Player::connect_new(handle.mixer())`
  - `Option<OutputStream>` / `Option<Sink>` → `Option<MixerDeviceSink>` / `Option<Player>`
  - `stop`, `clear`, `is_paused`, `pause`, `play`, `empty`, `append` bestaan op `Player`
    onder dezelfde namen (geen `try_seek`-gebruik in dit bestand)
  - `SequenceSource`/`SoundTouchSource`: `current_frame_len` → `current_span_len`,
    `channels()` → `NonZeroU16`, `sample_rate()` → `NonZeroU32` (rodio 0.21+ API);
    samples waren al `f32` — geen Item-wijziging nodig
- [x] Validatie: `cargo build` (incl. rubberband-feature!), 61/61 tests groen
- [x] Commit: `Upgrade rodio to 0.22`

## Fase 2 — UI-sprong: eframe/egui_extras 0.28 → 0.36 + egui-file-dialog 0.6 → 0.15

- [x] `eframe::App::update(ctx, frame)` → `App::ui(ui, frame)` + `let ctx = ui.ctx().clone();`
- [x] Panels: `TopBottomPanel::top` → `Panel::top` (`show_file_toolbar`/`show_action_toolbar`
      kregen een `ui`-parameter); `CentralPanel::show(ctx, …)` → `.show(ui, …)`;
      `Window::show(ctx, …)` → `.show(&ctx, …)`; alle overige `show_*`-functies (windows)
      konden `ctx` houden
- [x] `on_exit(_gl)` → `on_exit()` (glow-renderer is uit in eframe 0.36)
- [x] Hernoemingen: `id_source` → `id_salt`, `ComboBox::from_id_source` → `from_id_salt`,
      `Frame::none()` → `Frame::NONE`, `ui.close_menu()` → `ui.close()`,
      `allocate_ui_at_rect` → `scope_builder(UiBuilder::new().max_rect(…))`,
      `rect_stroke(…, stroke)` → + `StrokeKind::Inside`, `raw_scroll_delta` →
      `smooth_scroll_delta`, `FontData::from_static(…)` → `.into()` (Arc),
      `DroppedFile::path` (veld) → `path()` (methode)
- [x] egui-file-dialog 0.6 → 0.15: `update(ctx)` → `update(ui)`, `take_selected()` →
      `take_picked()`, `select_file()` → `pick_file()`, `select_directory()` →
      `pick_directory()`, `state()` → `*state()` (geeft nu `&DialogState`),
      `add_file_filter(name, Arc::new(closure))` → `add_file_filter(name, Filter::new(closure))`
- [x] Validatie: 61/61 tests groen, build + release-build (incl. rubberband-feature) ok
- [x] Commit: `Upgrade eframe, egui_extras and egui-file-dialog`

## Fase 3 — Afronding

- [ ] Laatste volledige check: `cargo build --release` (met default features incl. rubberband),
      geen waarschuwingen, handmatige smoke-test
- [ ] `Cargo.toml` opschonen: commentaar actueel maken
- [ ] Commit: `Clean up Cargo.toml after dependency updates`

## Fase 3 — Afronding

- [ ] Laatste volledige check: `cargo build --release` (met default features incl. rubberband),
      geen waarschuwingen, handmatige smoke-test
- [ ] `Cargo.toml` opschonen: commentaar actueel maken
- [ ] Commit: `Clean up Cargo.toml after dependency updates`

---

## Bewust niet upgraden

- **symphonia 0.5.5** — rodio 0.22.2 gebruikt zelf nog `^0.5.5`; losse upgrade geeft twee
  versies naast elkaar en nul winst
- **soundtouch, rustfft, hound, log, env_logger** — actueel

## Definitie van klaar

Alle fasen doorlopen → `Cargo.toml` bevat alleen actuele majors, alle tests slagen,
de release-build (incl. rubberband + icoon) draait en de handmatige test (waveform,
loops, export, video) is doorlopen.
