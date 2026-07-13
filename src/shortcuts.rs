use eframe::egui;
use serde::{Deserialize, Serialize};

/// Acties die als knop in de gebruikers-definieerbare werkbalk kunnen verschijnen.
/// Dit is een subset/bewerkte set van `ShortcutAction` aangevuld met acties
/// die geen sneltoets hebben (bv. Setup, Audit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolbarAction {
    /// 🔍 Detecteer - analyseer A-B selectie
    Detect,
    /// ↗ Verleng beats over de hele audio
    ExtendBeats,
    /// ✕ Wis A-B loop
    ClearLoop,
    /// ↩ Undo
    Undo,
    /// ↪ Redo
    Redo,
    /// 💾 Save huidige loop naar library
    SaveLoop,
    /// 🎯 Center view op A-B loop
    CenterLoop,
    /// 🔍− Zoom uit
    ZoomOut,
    /// 🔍+ Zoom in
    ZoomIn,
    /// ⟲ Reset zoom/scroll
    ResetZoom,
    /// 📌 Plaats beat markers
    PlaceBeats,
    /// ARR schakelaar
    ToggleArranger,
    /// 📤 Export
    Export,
    /// ⚙ Setup-venster
    Setup,
    /// 🔇 Audit toggle
    ToggleAudit,
    /// Tempo −10%
    TempoDown,
    /// Tempo +10%
    TempoUp,
    /// Pitch −1 semitone
    PitchDown,
    /// Pitch +1 semitone
    PitchUp,
}

impl ToolbarAction {
    /// Toonbare naam (kort, voor in de knop)
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Detect => "Detecteer",
            Self::ExtendBeats => "Verleng beats",
            Self::ClearLoop => "Wis loop",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::SaveLoop => "Save Loop",
            Self::CenterLoop => "Center Loop",
            Self::ZoomOut => "Zoom −",
            Self::ZoomIn => "Zoom +",
            Self::ResetZoom => "Reset",
            Self::PlaceBeats => "Beats",
            Self::ToggleArranger => "ARR",
            Self::Export => "Export",
            Self::Setup => "Setup",
            Self::ToggleAudit => "Audit",
            Self::TempoDown => "Tempo −10%",
            Self::TempoUp => "Tempo +10%",
            Self::PitchDown => "Pitch −1",
            Self::PitchUp => "Pitch +1",
        }
    }

    /// Emoji/icoon voor de knop
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Detect => "\u{1F50D}",
            Self::ExtendBeats => "\u{2197}",
            Self::ClearLoop => "\u{2715}",
            Self::Undo => "\u{21A9}",
            Self::Redo => "\u{21AA}",
            Self::SaveLoop => "\u{1F4BE}",
            Self::CenterLoop => "\u{1F3AF}",
            Self::ZoomOut => "\u{1F50D}\u{2212}",
            Self::ZoomIn => "\u{1F50D}\u{002B}",
            Self::ResetZoom => "\u{27F2}",
            Self::PlaceBeats => "\u{1F4CC}",
            Self::ToggleArranger => "ARR",
            Self::Export => "\u{1F4E4}",
            Self::Setup => "\u{2699}",
            Self::ToggleAudit => "\u{1F507}",
            Self::TempoDown => "\u{1F504}\u{2193}",
            Self::TempoUp => "\u{1F504}\u{2191}",
            Self::PitchDown => "\u{2B07}",
            Self::PitchUp => "\u{2B06}",
        }
    }

    /// Lange hover-tekst (met sneltoets info)
    pub fn hover_text(&self) -> &'static str {
        match self {
            Self::Detect => "Analyseer A-B selectie op toonhoogtes en BPM",
            Self::ExtendBeats => "Verspreid beat markers over de hele audio",
            Self::ClearLoop => "Verwijder A-B selectie (Ctrl+Backspace)",
            Self::Undo => "Ongedaan maken (Ctrl+Z)",
            Self::Redo => "Opnieuw doen (Ctrl+Y)",
            Self::SaveLoop => "Bewaar huidige A-B loop in de bibliotheek (Ctrl+S)",
            Self::CenterLoop => "Centreer weergave op de A-B loop (C)",
            Self::ZoomOut => "Uitzoomen (Ctrl+\u{2212})",
            Self::ZoomIn => "Inzoomen (Ctrl+=)",
            Self::ResetZoom => "Reset zoom en scroll (Ctrl+0)",
            Self::PlaceBeats => "Plaats een beat marker bij de playhead (B)",
            Self::ToggleArranger => "Open/sluit de arranger view",
            Self::Export => "Exporteer loops naar WAV (Ctrl+E)",
            Self::Setup => "Open setup-venster voor latency, kalibratie en beat audit",
            Self::ToggleAudit => "Schakel beat-audit kliktrack aan/uit",
            Self::TempoDown => "Verlaag afspeelsnelheid met 10%",
            Self::TempoUp => "Verhoog afspeelsnelheid met 10%",
            Self::PitchDown => "Verlaag toonhoogte met 1 semitone",
            Self::PitchUp => "Verhoog toonhoogte met 1 semitone",
        }
    }

    /// Koppel aan `ShortcutAction` (indien van toepassing)
    pub fn shortcut_action(&self) -> Option<ShortcutAction> {
        match self {
            Self::ClearLoop => Some(ShortcutAction::ClearLoop),
            Self::Undo => Some(ShortcutAction::Undo),
            Self::Redo => Some(ShortcutAction::Redo),
            Self::SaveLoop => Some(ShortcutAction::SaveLoop),
            Self::CenterLoop => Some(ShortcutAction::CenterLoop),
            Self::ZoomOut => Some(ShortcutAction::ZoomOut),
            Self::ZoomIn => Some(ShortcutAction::ZoomIn),
            Self::ResetZoom => Some(ShortcutAction::ResetZoom),
            Self::Export => Some(ShortcutAction::ExportLoops),
            Self::Detect => Some(ShortcutAction::Detect),
            Self::ExtendBeats => Some(ShortcutAction::ExtendBeats),
            Self::PlaceBeats => Some(ShortcutAction::PlaceBeats),
            Self::ToggleArranger => Some(ShortcutAction::ToggleArranger),
            Self::Setup => Some(ShortcutAction::Setup),
            Self::ToggleAudit => Some(ShortcutAction::ToggleAudit),
            Self::TempoDown => Some(ShortcutAction::TempoDown),
            Self::TempoUp => Some(ShortcutAction::TempoUp),
            Self::PitchDown => Some(ShortcutAction::PitchDown),
            Self::PitchUp => Some(ShortcutAction::PitchUp),
        }
    }

    /// Alle beschikbare toolbar-acties (voor het customize-venster)
    pub fn all() -> &'static [ToolbarAction] {
        &[
            Self::Detect,
            Self::ExtendBeats,
            Self::ClearLoop,
            Self::Undo,
            Self::Redo,
            Self::SaveLoop,
            Self::CenterLoop,
            Self::ZoomOut,
            Self::ZoomIn,
            Self::ResetZoom,
            Self::PlaceBeats,
            Self::ToggleArranger,
            Self::Export,
            Self::Setup,
            Self::ToggleAudit,
            Self::TempoDown,
            Self::TempoUp,
            Self::PitchDown,
            Self::PitchUp,
        ]
    }

    /// Standaard toolbar (de oorspronkelijk hardcoded set)
    pub fn default_toolbar() -> Vec<ToolbarAction> {
        vec![
            Self::Detect,
            Self::ExtendBeats,
            Self::ClearLoop,
            Self::Undo,
            Self::Redo,
            Self::Setup,
            Self::ToggleAudit,
        ]
    }
}
use std::collections::HashMap;
use std::fs;

fn shortcuts_path() -> std::path::PathBuf {
    crate::session::data_dir().join("shortcuts.json")
}
const CURRENT_VERSION: u32 = 1;

// ─────────────────────────────────────────────────────────────────────────────
// Alle mogelijke acties in de app
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    // Playback
    PlayPause,
    Stop,
    SeekForward,
    SeekBackward,

    // Toolbaar-acties (geen vaste shortcut, maar wel definieerbaar)
    Detect,
    ExtendBeats,
    PlaceBeats,
    ToggleArranger,
    Setup,
    ToggleAudit,
    TempoDown,
    TempoUp,
    PitchDown,
    PitchUp,

    // Loop
    SetLoopA,
    SetLoopB,
    ClearLoop,
    ToggleLoopBypass,
    NudgeLoopLeft,
    NudgeLoopRight,
    ToggleLoopPoint,
    NudgeALeft,
    NudgeARight,
    NudgeBLeft,
    NudgeBRight,
    NudgePlayheadLeft,
    NudgePlayheadRight,
    CenterLoop,
    SaveLoop,

    // Markers
    AddSectionMarker,
    AddMeasureMarker,
    AddBeatMarker,
    DeleteNearestMarker,

    // View
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ShowShortcuts,

    // File
    OpenFile,
    Undo,
    Redo,
    RestartLoop,
    // Loop lengte
    DoubleLoopLength,
    HalveLoopLength,
    // Snap naar markers
    SnapLoopLeft,
    SnapLoopRight,

    // Marker navigatie
    MarkerPrev,
    MarkerNext,

    // Export
    ExportLoops,
}

impl ShortcutAction {
    /// Menselijke leesbare naam voor de UI
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PlayPause => "Play / Pause",
            Self::Stop => "Stop playback",
            Self::SeekForward => "Seek forward 2s",
            Self::SeekBackward => "Seek backward 2s",
            Self::SetLoopA => "Set loop A at playhead",
            Self::SetLoopB => "Set loop B at playhead",
            Self::ClearLoop => "Clear A-B loop",
            Self::ToggleLoopBypass => "Toggle loop bypass",
            Self::NudgeLoopLeft => "Nudge loop left",
            Self::NudgeLoopRight => "Nudge loop right",
            Self::ToggleLoopPoint => "Toggle loop point (1e=set, 2e=A-B)",
            Self::NudgeALeft => "Nudge marker A left",
            Self::NudgeARight => "Nudge marker A right",
            Self::NudgeBLeft => "Nudge marker B left",
            Self::NudgeBRight => "Nudge marker B right",
            Self::NudgePlayheadLeft => "Nudge playhead left",
            Self::NudgePlayheadRight => "Nudge playhead right",
            Self::CenterLoop => "Center view on loop",
            Self::SaveLoop => "Save current loop",
            Self::AddSectionMarker => "Toggle section marker (S)",
            Self::AddMeasureMarker => "Toggle measure marker (M)",
            Self::AddBeatMarker => "Toggle beat marker (B)",
            Self::DeleteNearestMarker => "Delete nearest marker",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::ResetZoom => "Reset zoom/scroll",
            Self::ShowShortcuts => "Show shortcuts help",
            Self::OpenFile => "Open audio file",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::RestartLoop => "Restart loop (seek to A & play)",
            Self::DoubleLoopLength => "Double loop length",
            Self::HalveLoopLength => "Halve loop length",
            Self::SnapLoopLeft => "Snap loop to nearest marker left",
            Self::SnapLoopRight => "Snap loop to nearest marker right",
            Self::MarkerPrev => "Seek playhead to previous marker",
            Self::MarkerNext => "Seek playhead to next marker",
            Self::Detect => "Detect key & BPM",
            Self::ExtendBeats => "Extend beats across track",
            Self::PlaceBeats => "Place beat markers",
            Self::ToggleArranger => "Toggle arranger window",
            Self::Setup => "Open setup window",
            Self::ToggleAudit => "Toggle beat audit",
            Self::TempoDown => "Tempo -10%",
            Self::TempoUp => "Tempo +10%",
            Self::PitchDown => "Pitch -1 semitone",
            Self::PitchUp => "Pitch +1 semitone",
            Self::ExportLoops => "Export loops...",
        }
    }

    /// Categorie voor groepering in de UI
    pub fn category(&self) -> &'static str {
        match self {
            Self::PlayPause | Self::Stop | Self::SeekForward | Self::SeekBackward => "Playback",
            Self::SetLoopA
            | Self::SetLoopB
            | Self::ClearLoop
            | Self::ToggleLoopBypass
            | Self::NudgeLoopLeft
            | Self::NudgeLoopRight
            | Self::ToggleLoopPoint
            | Self::NudgeALeft
            | Self::NudgeARight
            | Self::NudgeBLeft
            | Self::NudgeBRight
            | Self::NudgePlayheadLeft
            | Self::NudgePlayheadRight
            | Self::CenterLoop
            | Self::SaveLoop
            | Self::DoubleLoopLength
            | Self::HalveLoopLength
            | Self::SnapLoopLeft
            | Self::SnapLoopRight => "Loop",
            Self::AddSectionMarker
            | Self::AddMeasureMarker
            | Self::AddBeatMarker
            | Self::DeleteNearestMarker
            | Self::MarkerPrev
            | Self::MarkerNext => "Markers",
            Self::ZoomIn | Self::ZoomOut | Self::ResetZoom | Self::ShowShortcuts => "View",
            Self::OpenFile => "File",
            Self::Undo | Self::Redo | Self::RestartLoop => "Edit",
            Self::Detect
            | Self::ExtendBeats
            | Self::PlaceBeats
            | Self::ToggleArranger
            | Self::Setup
            | Self::ToggleAudit
            | Self::TempoDown
            | Self::TempoUp
            | Self::PitchDown
            | Self::PitchUp => "Tools",
            Self::ExportLoops => "File",
        }
    }

    /// Alle acties (voor iteratie)
    pub fn all() -> &'static [ShortcutAction] {
        &[
            Self::PlayPause,
            Self::Stop,
            Self::SeekForward,
            Self::SeekBackward,
            Self::SetLoopA,
            Self::SetLoopB,
            Self::ClearLoop,
            Self::ToggleLoopBypass,
            Self::NudgeLoopLeft,
            Self::NudgeLoopRight,
            Self::ToggleLoopPoint,
            Self::NudgeALeft,
            Self::NudgeARight,
            Self::NudgeBLeft,
            Self::NudgeBRight,
            Self::NudgePlayheadLeft,
            Self::NudgePlayheadRight,
            Self::AddSectionMarker,
            Self::AddMeasureMarker,
            Self::AddBeatMarker,
            Self::DeleteNearestMarker,
            Self::MarkerPrev,
            Self::MarkerNext,
            Self::ZoomIn,
            Self::ZoomOut,
            Self::ResetZoom,
            Self::ShowShortcuts,
            Self::OpenFile,
            Self::Undo,
            Self::Redo,
            Self::RestartLoop,
            Self::CenterLoop,
            Self::SaveLoop,
            Self::DoubleLoopLength,
            Self::HalveLoopLength,
            Self::SnapLoopLeft,
            Self::SnapLoopRight,
            Self::ExportLoops,
            Self::Detect,
            Self::ExtendBeats,
            Self::PlaceBeats,
            Self::ToggleArranger,
            Self::Setup,
            Self::ToggleAudit,
            Self::TempoDown,
            Self::TempoUp,
            Self::PitchDown,
            Self::PitchUp,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Een key binding: toets + modifiers
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: SerializableKey,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
}

impl KeyBinding {
    pub fn new(key: SerializableKey) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }
    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }
    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Menselijke leesbare representatie (bijv. "Ctrl+S", "Space", "Shift+→")
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key.display_name());
        parts.join("+")
    }

    /// Check of deze binding overeenkomt met de huidige input state
    pub fn matches(&self, input: &egui::InputState) -> bool {
        let mods = input.modifiers;
        let mods_match = self.ctrl == mods.ctrl && self.shift == mods.shift && self.alt == mods.alt;
        mods_match && input.key_pressed(self.key.into())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Serializeerbare wrapper rond egui::Key
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerializableKey {
    Space,
    Enter,
    Escape,
    Backspace,
    Tab,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    OpenBracket,
    CloseBracket,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl SerializableKey {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Space => "Space",
            Self::Enter => "Enter",
            Self::Escape => "Esc",
            Self::Backspace => "Backspace",
            Self::Tab => "Tab",
            Self::ArrowLeft => "◀",
            Self::ArrowRight => "▶",
            Self::ArrowUp => "↑",
            Self::ArrowDown => "↓",
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",
            Self::OpenBracket => "[",
            Self::CloseBracket => "]",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
        }
    }
}

// Conversie naar egui::Key
impl From<egui::Key> for SerializableKey {
    fn from(k: egui::Key) -> Self {
        match k {
            egui::Key::Space => SerializableKey::Space,
            egui::Key::Enter => SerializableKey::Enter,
            egui::Key::Escape => SerializableKey::Escape,
            egui::Key::Backspace => SerializableKey::Backspace,
            egui::Key::Tab => SerializableKey::Tab,
            egui::Key::ArrowLeft => SerializableKey::ArrowLeft,
            egui::Key::ArrowRight => SerializableKey::ArrowRight,
            egui::Key::ArrowUp => SerializableKey::ArrowUp,
            egui::Key::ArrowDown => SerializableKey::ArrowDown,
            egui::Key::A => SerializableKey::A,
            egui::Key::B => SerializableKey::B,
            egui::Key::C => SerializableKey::C,
            egui::Key::D => SerializableKey::D,
            egui::Key::E => SerializableKey::E,
            egui::Key::F => SerializableKey::F,
            egui::Key::G => SerializableKey::G,
            egui::Key::H => SerializableKey::H,
            egui::Key::I => SerializableKey::I,
            egui::Key::J => SerializableKey::J,
            egui::Key::K => SerializableKey::K,
            egui::Key::L => SerializableKey::L,
            egui::Key::M => SerializableKey::M,
            egui::Key::N => SerializableKey::N,
            egui::Key::O => SerializableKey::O,
            egui::Key::P => SerializableKey::P,
            egui::Key::Q => SerializableKey::Q,
            egui::Key::R => SerializableKey::R,
            egui::Key::S => SerializableKey::S,
            egui::Key::T => SerializableKey::T,
            egui::Key::U => SerializableKey::U,
            egui::Key::V => SerializableKey::V,
            egui::Key::W => SerializableKey::W,
            egui::Key::X => SerializableKey::X,
            egui::Key::Y => SerializableKey::Y,
            egui::Key::Z => SerializableKey::Z,
            egui::Key::Num0 => SerializableKey::Num0,
            egui::Key::Num1 => SerializableKey::Num1,
            egui::Key::Num2 => SerializableKey::Num2,
            egui::Key::Num3 => SerializableKey::Num3,
            egui::Key::Num4 => SerializableKey::Num4,
            egui::Key::Num5 => SerializableKey::Num5,
            egui::Key::Num6 => SerializableKey::Num6,
            egui::Key::Num7 => SerializableKey::Num7,
            egui::Key::Num8 => SerializableKey::Num8,
            egui::Key::Num9 => SerializableKey::Num9,
            egui::Key::OpenBracket => SerializableKey::OpenBracket,
            egui::Key::CloseBracket => SerializableKey::CloseBracket,
            egui::Key::F1 => SerializableKey::F1,
            egui::Key::F2 => SerializableKey::F2,
            egui::Key::F3 => SerializableKey::F3,
            egui::Key::F4 => SerializableKey::F4,
            egui::Key::F5 => SerializableKey::F5,
            egui::Key::F6 => SerializableKey::F6,
            egui::Key::F7 => SerializableKey::F7,
            egui::Key::F8 => SerializableKey::F8,
            egui::Key::F9 => SerializableKey::F9,
            egui::Key::F10 => SerializableKey::F10,
            egui::Key::F11 => SerializableKey::F11,
            egui::Key::F12 => SerializableKey::F12,
            _ => SerializableKey::Space, // Fallback
        }
    }
}

impl From<SerializableKey> for egui::Key {
    fn from(k: SerializableKey) -> Self {
        match k {
            SerializableKey::Space => egui::Key::Space,
            SerializableKey::Enter => egui::Key::Enter,
            SerializableKey::Escape => egui::Key::Escape,
            SerializableKey::Backspace => egui::Key::Backspace,
            SerializableKey::Tab => egui::Key::Tab,
            SerializableKey::ArrowLeft => egui::Key::ArrowLeft,
            SerializableKey::ArrowRight => egui::Key::ArrowRight,
            SerializableKey::ArrowUp => egui::Key::ArrowUp,
            SerializableKey::ArrowDown => egui::Key::ArrowDown,
            SerializableKey::A => egui::Key::A,
            SerializableKey::B => egui::Key::B,
            SerializableKey::C => egui::Key::C,
            SerializableKey::D => egui::Key::D,
            SerializableKey::E => egui::Key::E,
            SerializableKey::F => egui::Key::F,
            SerializableKey::G => egui::Key::G,
            SerializableKey::H => egui::Key::H,
            SerializableKey::I => egui::Key::I,
            SerializableKey::J => egui::Key::J,
            SerializableKey::K => egui::Key::K,
            SerializableKey::L => egui::Key::L,
            SerializableKey::M => egui::Key::M,
            SerializableKey::N => egui::Key::N,
            SerializableKey::O => egui::Key::O,
            SerializableKey::P => egui::Key::P,
            SerializableKey::Q => egui::Key::Q,
            SerializableKey::R => egui::Key::R,
            SerializableKey::S => egui::Key::S,
            SerializableKey::T => egui::Key::T,
            SerializableKey::U => egui::Key::U,
            SerializableKey::V => egui::Key::V,
            SerializableKey::W => egui::Key::W,
            SerializableKey::X => egui::Key::X,
            SerializableKey::Y => egui::Key::Y,
            SerializableKey::Z => egui::Key::Z,
            SerializableKey::Num0 => egui::Key::Num0,
            SerializableKey::Num1 => egui::Key::Num1,
            SerializableKey::Num2 => egui::Key::Num2,
            SerializableKey::Num3 => egui::Key::Num3,
            SerializableKey::Num4 => egui::Key::Num4,
            SerializableKey::Num5 => egui::Key::Num5,
            SerializableKey::Num6 => egui::Key::Num6,
            SerializableKey::Num7 => egui::Key::Num7,
            SerializableKey::Num8 => egui::Key::Num8,
            SerializableKey::Num9 => egui::Key::Num9,
            SerializableKey::OpenBracket => egui::Key::OpenBracket,
            SerializableKey::CloseBracket => egui::Key::CloseBracket,
            SerializableKey::F1 => egui::Key::F1,
            SerializableKey::F2 => egui::Key::F2,
            SerializableKey::F3 => egui::Key::F3,
            SerializableKey::F4 => egui::Key::F4,
            SerializableKey::F5 => egui::Key::F5,
            SerializableKey::F6 => egui::Key::F6,
            SerializableKey::F7 => egui::Key::F7,
            SerializableKey::F8 => egui::Key::F8,
            SerializableKey::F9 => egui::Key::F9,
            SerializableKey::F10 => egui::Key::F10,
            SerializableKey::F11 => egui::Key::F11,
            SerializableKey::F12 => egui::Key::F12,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// De shortcuts config
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    #[serde(default)]
    pub version: u32,
    pub bindings: HashMap<ShortcutAction, KeyBinding>,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        let mut bindings = HashMap::new();

        // Playback
        bindings.insert(
            ShortcutAction::PlayPause,
            KeyBinding::new(SerializableKey::Space),
        );
        bindings.insert(
            ShortcutAction::Stop,
            KeyBinding::new(SerializableKey::Escape),
        );
        bindings.insert(
            ShortcutAction::SeekForward,
            KeyBinding::new(SerializableKey::ArrowUp),
        );
        bindings.insert(
            ShortcutAction::SeekBackward,
            KeyBinding::new(SerializableKey::ArrowDown),
        );

        // Loop
        bindings.insert(
            ShortcutAction::SetLoopA,
            KeyBinding::new(SerializableKey::OpenBracket),
        );
        bindings.insert(
            ShortcutAction::SetLoopB,
            KeyBinding::new(SerializableKey::CloseBracket),
        );
        bindings.insert(
            ShortcutAction::ClearLoop,
            KeyBinding::new(SerializableKey::Backspace).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ToggleLoopBypass,
            KeyBinding::new(SerializableKey::B).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::NudgeLoopLeft,
            KeyBinding::new(SerializableKey::ArrowLeft).with_shift(),
        );
        bindings.insert(
            ShortcutAction::NudgeLoopRight,
            KeyBinding::new(SerializableKey::ArrowRight).with_shift(),
        );
        bindings.insert(
            ShortcutAction::ToggleLoopPoint,
            KeyBinding::new(SerializableKey::OpenBracket),
        );
        bindings.insert(
            ShortcutAction::NudgeALeft,
            KeyBinding::new(SerializableKey::J),
        );
        bindings.insert(
            ShortcutAction::NudgeARight,
            KeyBinding::new(SerializableKey::J).with_shift(),
        );
        bindings.insert(
            ShortcutAction::NudgeBLeft,
            KeyBinding::new(SerializableKey::L),
        );
        bindings.insert(
            ShortcutAction::NudgeBRight,
            KeyBinding::new(SerializableKey::L).with_shift(),
        );
        bindings.insert(
            ShortcutAction::NudgePlayheadLeft,
            KeyBinding::new(SerializableKey::ArrowLeft),
        );
        bindings.insert(
            ShortcutAction::NudgePlayheadRight,
            KeyBinding::new(SerializableKey::ArrowRight),
        );

        // Markers
        bindings.insert(
            ShortcutAction::AddSectionMarker,
            KeyBinding::new(SerializableKey::S),
        );
        bindings.insert(
            ShortcutAction::AddMeasureMarker,
            KeyBinding::new(SerializableKey::M),
        );
        bindings.insert(
            ShortcutAction::AddBeatMarker,
            KeyBinding::new(SerializableKey::B),
        );
        bindings.insert(
            ShortcutAction::DeleteNearestMarker,
            KeyBinding::new(SerializableKey::Backspace),
        );

        // View
        bindings.insert(
            ShortcutAction::ZoomIn,
            KeyBinding::new(SerializableKey::Num0).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ZoomOut,
            KeyBinding::new(SerializableKey::Num0)
                .with_ctrl()
                .with_shift(),
        );
        bindings.insert(
            ShortcutAction::ResetZoom,
            KeyBinding::new(SerializableKey::R).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ShowShortcuts,
            KeyBinding::new(SerializableKey::F1),
        );

        // File
        bindings.insert(
            ShortcutAction::OpenFile,
            KeyBinding::new(SerializableKey::O).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::Undo,
            KeyBinding::new(SerializableKey::Z).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::Redo,
            KeyBinding::new(SerializableKey::Z).with_ctrl().with_shift(),
        );
        bindings.insert(
            ShortcutAction::RestartLoop,
            KeyBinding::new(SerializableKey::Enter),
        );
        bindings.insert(
            ShortcutAction::CenterLoop,
            KeyBinding::new(SerializableKey::C),
        );
        bindings.insert(
            ShortcutAction::SaveLoop,
            KeyBinding::new(SerializableKey::S).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::ExportLoops,
            KeyBinding::new(SerializableKey::E).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::DoubleLoopLength,
            KeyBinding::new(SerializableKey::D).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::HalveLoopLength,
            KeyBinding::new(SerializableKey::D).with_ctrl().with_shift(),
        );
        bindings.insert(
            ShortcutAction::MarkerPrev,
            KeyBinding::new(SerializableKey::ArrowLeft).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::MarkerNext,
            KeyBinding::new(SerializableKey::ArrowRight).with_ctrl(),
        );
        bindings.insert(
            ShortcutAction::SnapLoopLeft,
            KeyBinding::new(SerializableKey::Q),
        );
        bindings.insert(
            ShortcutAction::SnapLoopRight,
            KeyBinding::new(SerializableKey::Q),
        );

        // Tools (geen default shortcuts, maar wel aanpasbaar)
        // Detect, ExtendBeats, PlaceBeats, ToggleArranger, Setup,
        // ToggleAudit, TempoDown, TempoUp, PitchDown, PitchUp

        Self {
            version: CURRENT_VERSION,
            bindings,
        }
    }
}

impl ShortcutsConfig {
    /// Laad config uit bestand, of gebruik defaults als het niet bestaat
    pub fn load() -> Self {
        let path = shortcuts_path();
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(json) => match serde_json::from_str::<ShortcutsConfig>(&json) {
                    Ok(config) => {
                        // Als de versie niet matcht, reset naar defaults
                        if config.version != CURRENT_VERSION {
                            let defaults = Self::default();
                            if let Err(e) = defaults.save() {
                                log::warn!("Kon default shortcuts niet opslaan: {}", e);
                            }
                            return defaults;
                        }
                        // Merge met defaults: ontbrekende acties krijgen hun default
                        let defaults = Self::default();
                        let mut merged: Self = config;
                        for action in ShortcutAction::all() {
                            if !merged.bindings.contains_key(action) {
                                if let Some(binding) = defaults.bindings.get(action) {
                                    merged.bindings.insert(*action, *binding);
                                }
                            }
                        }
                        return merged;
                    }
                    Err(e) => {
                        eprintln!(
                            "[shortcuts] Kon shortcuts.json niet parsen: {}. Gebruik defaults.",
                            e
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[shortcuts] Kon shortcuts.json niet lezen: {}. Gebruik defaults.",
                        e
                    );
                }
            }
        }
        let defaults = Self::default();
        // Sla defaults op zodat het bestand bestaat voor de gebruiker
        let _ = defaults.save();
        defaults
    }

    /// Sla config op naar bestand
    pub fn save(&self) -> Result<(), String> {
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                fs::write(shortcuts_path(), json).map_err(|e| format!("Kon niet opslaan: {}", e))
            }
            Err(e) => Err(format!("Kon niet serialiseren: {}", e)),
        }
    }

    /// Check of een actie geactiveerd is in de huidige input state
    pub fn is_pressed(&self, action: ShortcutAction, input: &egui::InputState) -> bool {
        if let Some(binding) = self.bindings.get(&action) {
            binding.matches(input)
        } else {
            false
        }
    }

    /// Geef de binding voor een actie (voor display in UI)
    pub fn binding_for(&self, action: ShortcutAction) -> Option<&KeyBinding> {
        self.bindings.get(&action)
    }

    /// Update een binding en sla op
    pub fn set_binding(
        &mut self,
        action: ShortcutAction,
        binding: KeyBinding,
    ) -> Result<(), String> {
        self.bindings.insert(action, binding);
        self.save()
    }

    /// Reset een actie naar default
    pub fn reset_action(&mut self, action: ShortcutAction) -> Result<(), String> {
        let defaults = Self::default();
        if let Some(binding) = defaults.bindings.get(&action) {
            self.bindings.insert(action, *binding);
            self.save()
        } else {
            Ok(())
        }
    }

    /// Reset alles naar defaults
    pub fn reset_all(&mut self) -> Result<(), String> {
        *self = Self::default();
        self.save()
    }

    /// Check of een key+modifiers combinatie al in gebruik is (voor conflict-detectie)
    pub fn find_conflict(
        &self,
        binding: &KeyBinding,
        exclude: ShortcutAction,
    ) -> Option<ShortcutAction> {
        for (action, existing) in &self.bindings {
            if *action != exclude && existing == binding {
                return Some(*action);
            }
        }
        None
    }
}

// ───────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── KeyBinding tests ──

    #[test]
    fn test_keybinding_display_simple() {
        let kb = KeyBinding::new(SerializableKey::Space);
        assert_eq!(kb.display(), "Space");
    }

    #[test]
    fn test_keybinding_display_with_modifiers() {
        let kb = KeyBinding::new(SerializableKey::S).with_ctrl().with_shift();
        assert_eq!(kb.display(), "Ctrl+Shift+S");
    }

    #[test]
    fn test_keybinding_eq_exact() {
        let a = KeyBinding::new(SerializableKey::Z).with_ctrl();
        let b = KeyBinding::new(SerializableKey::Z).with_ctrl();
        assert_eq!(a, b);
    }

    #[test]
    fn test_keybinding_eq_different_key() {
        let a = KeyBinding::new(SerializableKey::A).with_ctrl();
        let b = KeyBinding::new(SerializableKey::B).with_ctrl();
        assert_ne!(a, b);
    }

    #[test]
    fn test_keybinding_eq_different_modifier() {
        let a = KeyBinding::new(SerializableKey::X).with_ctrl();
        let b = KeyBinding::new(SerializableKey::X);
        assert_ne!(a, b);
    }

    // ── SerializableKey roundtrip ──

    #[test]
    fn test_serializable_key_roundtrip() {
        let keys = [
            SerializableKey::Space,
            SerializableKey::Enter,
            SerializableKey::Escape,
            SerializableKey::A,
            SerializableKey::Z,
            SerializableKey::Num0,
            SerializableKey::F12,
            SerializableKey::ArrowLeft,
            SerializableKey::OpenBracket,
            SerializableKey::CloseBracket,
        ];

        for key in &keys {
            let egui_key: egui::Key = (*key).into();
            let back: SerializableKey = egui_key.into();
            assert_eq!(*key, back, "roundtrip mislukt voor {:?}", key);
        }
    }

    // ── ShortcutsConfig serde ──

    #[test]
    fn test_shortcuts_config_default_has_bindings() {
        let config = ShortcutsConfig::default();
        assert!(
            !config.bindings.is_empty(),
            "default moet bindings bevatten"
        );
        assert_eq!(config.version, CURRENT_VERSION);
    }

    #[test]
    fn test_shortcuts_config_default_has_playpause() {
        let config = ShortcutsConfig::default();
        assert!(config.bindings.contains_key(&ShortcutAction::PlayPause));
    }

    #[test]
    fn test_shortcuts_config_binding_for() {
        let config = ShortcutsConfig::default();
        let binding = config.binding_for(ShortcutAction::PlayPause);
        assert!(
            binding.is_some(),
            "PlayPause moet een default binding hebben"
        );
    }

    #[test]
    fn test_shortcuts_config_serde_roundtrip() {
        let config = ShortcutsConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let restored: ShortcutsConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.version, restored.version);
        assert_eq!(config.bindings.len(), restored.bindings.len());

        for (action, binding) in &config.bindings {
            let restored_binding = restored.bindings.get(action);
            assert!(
                restored_binding.is_some(),
                "actie {:?} ontbreekt in gerestaureerde config",
                action
            );
            assert_eq!(restored_binding, Some(binding));
        }
    }

    #[test]
    fn test_shortcuts_config_set_binding() {
        let mut config = ShortcutsConfig::default();
        let new_binding = KeyBinding::new(SerializableKey::F1).with_ctrl().with_alt();

        let result = config.set_binding(ShortcutAction::PlayPause, new_binding);
        assert!(result.is_ok());

        let stored = config.binding_for(ShortcutAction::PlayPause);
        assert_eq!(stored, Some(new_binding).as_ref());
    }

    #[test]
    fn test_shortcuts_config_find_conflict() {
        let mut config = ShortcutsConfig::default();
        let binding = KeyBinding::new(SerializableKey::P);
        config
            .set_binding(ShortcutAction::PlayPause, binding)
            .unwrap();

        // Zoek conflict met een andere actie die dezelfde binding probeert te gebruiken
        let conflict = config.find_conflict(
            &KeyBinding::new(SerializableKey::P),
            ShortcutAction::ExportLoops,
        );
        assert_eq!(conflict, Some(ShortcutAction::PlayPause));

        // Test dat conflict-detectie de exclude respecteert
        let no_conflict = config.find_conflict(
            &KeyBinding::new(SerializableKey::P),
            ShortcutAction::PlayPause,
        );
        assert_ne!(no_conflict, Some(ShortcutAction::PlayPause));
    }

    #[test]
    fn test_shortcuts_config_reset_action() {
        let mut config = ShortcutsConfig::default();

        // Cloneer de originele binding
        let original = config.binding_for(ShortcutAction::Stop).cloned();

        // Verander de binding
        config
            .set_binding(
                ShortcutAction::Stop,
                KeyBinding::new(SerializableKey::F5).with_ctrl(),
            )
            .unwrap();

        // Reset
        let _ = config.reset_action(ShortcutAction::Stop);
        let restored = config.binding_for(ShortcutAction::Stop);
        assert_eq!(restored, original.as_ref());
    }
}
