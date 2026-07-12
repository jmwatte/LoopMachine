use crate::timestretch::TimeStretch;
use crossbeam_channel::{Receiver, Sender};
use rodio::{OutputStream, Sink, Source};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::arrangement::SequenceStep;

pub enum WaveformCommand {
    Play {
        samples: Arc<Vec<f32>>,
        sample_rate: u32,
        start_sample: usize,
        segment_start_sec: f32,
        a_sample: usize,
        b_sample: usize,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
        /// Beat click posities (seconden) voor audit-modus.
        click_positions: Arc<Mutex<Vec<f32>>>,
        /// Schakelaar voor click-generatie.
        click_enabled: Arc<AtomicBool>,
    },
    Stop,
    #[allow(dead_code)]
    Pause,
    #[allow(dead_code)]
    Resume,
    TogglePause,
    SetLoopBounds {
        a_secs: f32,
        b_secs: f32,
    },
    Seek {
        pos_secs: f32,
    },
    SetPitch(f32),
    SetTempo(f32),
    SetVolume(f32),
    SetLoopEnabled(bool),
    /// Speel een hele sequentie in één keer af (gapless, fire & forget).
    PlaySequence {
        sequence_steps: Vec<SequenceStep>,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
    },
}

#[derive(Debug, Clone)]
pub enum WaveformEvent {
    Playing,
    Stopped,
    Paused,
    Resumed,
    Error(String),
    Position(f32, f32),
    /// Audio thread is naar een nieuwe stap in een arrangement gesprongen.
    StepChanged(usize),
    /// Audio thread herhaalt een stap opnieuw.
    StepRepeated(usize),
    /// Hele arrangement is klaar.
    ArrangementFinished,
}

#[derive(Debug, Clone, Copy)]
pub struct LoopBounds {
    pub a: usize,
    pub b: usize,
    pub enabled: bool,
}

impl LoopBounds {
    pub fn enabled(&self) -> bool {
        self.enabled && self.b > self.a
    }
}

pub fn start_waveform_thread() -> (Sender<WaveformCommand>, Receiver<WaveformEvent>) {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded();
    let (event_tx, event_rx) = crossbeam_channel::unbounded();
    std::thread::spawn(move || run_waveform_audio(cmd_rx, event_tx));
    (cmd_tx, event_rx)
}

// ─── Helper: maak een verse OutputStream + Sink aan ───
fn recreate_stream(
    stream: &mut Option<OutputStream>,
    sink: &mut Option<Sink>,
    event_tx: &Sender<WaveformEvent>,
) -> bool {
    match OutputStream::try_default() {
        Ok((new_stream, handle)) => match Sink::try_new(&handle) {
            Ok(new_sink) => {
                *stream = Some(new_stream);
                *sink = Some(new_sink);
                true
            }
            Err(e) => {
                let _ = event_tx.send(WaveformEvent::Error(format!("Sink fout: {}", e)));
                false
            }
        },
        Err(e) => {
            let _ = event_tx.send(WaveformEvent::Error(format!("Audio-apparaat fout: {}", e)));
            false
        }
    }
}

// ─── Helper: herbouw de stream alleen als nodig (lazy) ───
fn check_and_recreate_stream(
    stream: &mut Option<OutputStream>,
    sink: &mut Option<Sink>,
    event_tx: &Sender<WaveformEvent>,
    last_activity: &mut std::time::Instant,
    stream_is_dead: &mut bool,
) -> bool {
    let idle_time = last_activity.elapsed();
    let needs_recreation =
        *stream_is_dead || sink.is_none() || idle_time > std::time::Duration::from_secs(120);

    if needs_recreation {
        if recreate_stream(stream, sink, event_tx) {
            *stream_is_dead = false;
            *last_activity = std::time::Instant::now();
            true
        } else {
            false
        }
    } else {
        *last_activity = std::time::Instant::now();
        true
    }
}

fn run_waveform_audio(rx: Receiver<WaveformCommand>, event_tx: Sender<WaveformEvent>) {
    let mut _stream: Option<OutputStream> = None;
    let mut sink: Option<Sink> = None;
    let mut is_playing = false;
    let mut is_paused = false;
    let mut samples: Arc<Vec<f32>> = Arc::new(Vec::new());
    let pitch_semitones: Arc<AtomicU32> = Arc::new(AtomicU32::new(f32::to_bits(0.0)));
    let tempo: Arc<AtomicU32> = Arc::new(AtomicU32::new(f32::to_bits(1.0)));
    let loop_bounds: Arc<Mutex<LoopBounds>> = Arc::new(Mutex::new(LoopBounds {
        a: 0,
        b: 0,
        enabled: false,
    }));
    let source_pos: Arc<AtomicU64> = Arc::new(AtomicU64::new(f64::to_bits(0.0)));
    let seek_requested: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let seek_target: Arc<AtomicU64> = Arc::new(AtomicU64::new(f64::to_bits(0.0)));
    let volume: Arc<AtomicU32> = Arc::new(AtomicU32::new(f32::to_bits(1.0)));

    // Watchdog: detecteert als source_pos niet meer verandert (audio-device weggevallen)
    let mut last_source_pos: u64 = f64::to_bits(0.0);
    let mut stuck_frames: u32 = 0;
    let mut last_audio_activity = std::time::Instant::now();
    let mut stream_is_dead = true; // start met dood → dwingt 1e stream-creatie af

    let segment_start_sec: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let segment_dur: Arc<Mutex<f32>> = Arc::new(Mutex::new(0.0));
    let current_sample_rate: Arc<Mutex<u32>> = Arc::new(Mutex::new(44100));

    loop {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                WaveformCommand::Play {
                    samples: new_samples,
                    sample_rate: sr,
                    start_sample,
                    segment_start_sec: seg_start,
                    a_sample,
                    b_sample,
                    pitch_semitones: ps,
                    tempo: t,
                    click_positions: ref incoming_clicks,
                    click_enabled: ref incoming_enabled,
                } => {
                    samples = new_samples.clone();
                    pitch_semitones.store(ps.load(Ordering::Relaxed), Ordering::Relaxed);
                    tempo.store(t.load(Ordering::Relaxed), Ordering::Relaxed);
                    *current_sample_rate.lock().unwrap() = sr;
                    source_pos.store(f64::to_bits(start_sample as f64), Ordering::Relaxed);
                    *segment_start_sec.lock().unwrap() = seg_start;
                    seek_requested.store(false, Ordering::Relaxed);
                    seek_target.store(f64::to_bits(start_sample as f64), Ordering::Relaxed);

                    let len = samples.len();
                    *segment_dur.lock().unwrap() = len as f32 / sr as f32;

                    if b_sample > a_sample && b_sample <= len {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: a_sample,
                            b: b_sample,
                            enabled: true,
                        };
                    } else {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: 0,
                            b: 0,
                            enabled: false,
                        };
                    }

                    // ✅ Lazy recreation: herbouw stream alleen als nodig (OS-slaap / crash)
                    if !check_and_recreate_stream(
                        &mut _stream,
                        &mut sink,
                        &event_tx,
                        &mut last_audio_activity,
                        &mut stream_is_dead,
                    ) {
                        continue;
                    }

                    // Gebruik de UI-thread z'n Arc's direct, zodat click-posities
                    // live updaten zonder de playback te herstarten.
                    let source = SoundTouchSource::new(
                        samples.clone(),
                        sr,
                        pitch_semitones.clone(),
                        tempo.clone(),
                        loop_bounds.clone(),
                        source_pos.clone(),
                        seek_requested.clone(),
                        seek_target.clone(),
                        volume.clone(),
                        incoming_clicks.clone(),
                        incoming_enabled.clone(),
                    );

                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                        s.append(source);
                        s.play();
                        is_playing = true;
                        is_paused = false;
                        stuck_frames = 0;
                        last_source_pos = source_pos.load(Ordering::Relaxed);
                        let _ = event_tx.send(WaveformEvent::Playing);
                    }
                }
                WaveformCommand::Stop => {
                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                    }
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                }
                WaveformCommand::Pause => {
                    if let Some(s) = &sink {
                        if !s.is_paused() {
                            s.pause();
                            is_paused = true;
                            let _ = event_tx.send(WaveformEvent::Paused);
                        }
                    }
                }
                WaveformCommand::Resume => {
                    if !check_and_recreate_stream(
                        &mut _stream,
                        &mut sink,
                        &event_tx,
                        &mut last_audio_activity,
                        &mut stream_is_dead,
                    ) {
                        continue;
                    }
                    if let Some(s) = &sink {
                        if s.is_paused() {
                            s.play();
                            is_paused = false;
                            let _ = event_tx.send(WaveformEvent::Resumed);
                        }
                    }
                }
                WaveformCommand::TogglePause => {
                    if sink.as_ref().map_or(false, |s| s.is_paused()) {
                        // ✅ Hervatten: controleer of stream nog leeft
                        if !check_and_recreate_stream(
                            &mut _stream,
                            &mut sink,
                            &event_tx,
                            &mut last_audio_activity,
                            &mut stream_is_dead,
                        ) {
                            continue;
                        }
                        if let Some(s) = &sink {
                            s.play();
                            is_paused = false;
                            let _ = event_tx.send(WaveformEvent::Resumed);
                        }
                    } else if let Some(s) = &sink {
                        s.pause();
                        is_paused = true;
                        let _ = event_tx.send(WaveformEvent::Paused);
                    }
                }
                WaveformCommand::SetLoopBounds { a_secs, b_secs } => {
                    let sr = *current_sample_rate.lock().unwrap();
                    let a_sample = (a_secs.max(0.0) * sr as f32) as usize;
                    let b_sample = (b_secs.max(0.0) * sr as f32) as usize;
                    if b_sample > a_sample {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: a_sample,
                            b: b_sample,
                            enabled: true,
                        };
                        *segment_dur.lock().unwrap() = (b_secs - a_secs).max(0.001);
                    } else {
                        *loop_bounds.lock().unwrap() = LoopBounds {
                            a: 0,
                            b: 0,
                            enabled: false,
                        };
                    }
                }
                WaveformCommand::Seek { pos_secs } => {
                    let sr = *current_sample_rate.lock().unwrap();
                    let start_sec = *segment_start_sec.lock().unwrap();
                    let rel_secs = (pos_secs - start_sec).max(0.0);
                    let sample = (rel_secs * sr as f32) as f64;
                    source_pos.store(f64::to_bits(sample), Ordering::Relaxed);
                    seek_target.store(f64::to_bits(sample), Ordering::Relaxed);
                    seek_requested.store(true, Ordering::Relaxed);
                }
                WaveformCommand::SetPitch(semitones) => {
                    pitch_semitones.store(f32::to_bits(semitones), Ordering::Relaxed);
                }
                WaveformCommand::SetTempo(new_tempo) => {
                    tempo.store(f32::to_bits(new_tempo), Ordering::Relaxed);
                }
                WaveformCommand::SetVolume(new_volume) => {
                    volume.store(f32::to_bits(new_volume), Ordering::Relaxed);
                }
                WaveformCommand::SetLoopEnabled(enabled) => {
                    loop_bounds.lock().unwrap().enabled = enabled;
                }
                WaveformCommand::PlaySequence {
                    sequence_steps,
                    pitch_semitones: ps,
                    tempo: t,
                } => {
                    if sequence_steps.is_empty() {
                        continue;
                    }

                    pitch_semitones.store(ps.load(Ordering::Relaxed), Ordering::Relaxed);
                    tempo.store(t.load(Ordering::Relaxed), Ordering::Relaxed);

                    if !check_and_recreate_stream(
                        &mut _stream,
                        &mut sink,
                        &event_tx,
                        &mut last_audio_activity,
                        &mut stream_is_dead,
                    ) {
                        continue;
                    }

                    let source = SequenceSource::new(
                        sequence_steps,
                        pitch_semitones.clone(),
                        tempo.clone(),
                        source_pos.clone(),
                        volume.clone(),
                        event_tx.clone(),
                    );

                    if let Some(s) = &sink {
                        s.stop();
                        s.clear();
                        s.append(source);
                        s.play();
                        is_playing = true;
                        is_paused = false;
                        stuck_frames = 0;
                        last_source_pos = source_pos.load(Ordering::Relaxed);
                        let _ = event_tx.send(WaveformEvent::Playing);
                    }
                }
            }
        }

        if is_playing && !is_paused {
            if let Some(s) = &sink {
                if s.empty() {
                    is_playing = false;
                    let _ = event_tx.send(WaveformEvent::Stopped);
                } else {
                    let pos_samples = f64::from_bits(source_pos.load(Ordering::Relaxed));
                    let sr = *current_sample_rate.lock().unwrap();
                    let bounds = *loop_bounds.lock().unwrap();
                    let total_dur = samples.len() as f32 / sr as f32;
                    let pos_secs = pos_samples as f32 / sr as f32;

                    let effective_pos = if bounds.enabled() {
                        let loop_start_sec = bounds.a as f32 / sr as f32;
                        let loop_end_sec = bounds.b as f32 / sr as f32;
                        let loop_dur = loop_end_sec - loop_start_sec;
                        if loop_dur > 0.0 && pos_secs >= loop_end_sec {
                            loop_start_sec + ((pos_secs - loop_start_sec) % loop_dur)
                        } else {
                            pos_secs
                        }
                    } else {
                        pos_secs
                    };

                    let _ = event_tx.send(WaveformEvent::Position(effective_pos, total_dur));

                    // Watchdog: als source_pos niet verandert, is het audio-device
                    // waarschijnlijk weggevallen (bv. Windows power management).
                    // Markeer de stream als "dood" zodat bij de volgende Play
                    // check_and_recreate_stream een verse stream aanmaakt.
                    let cur_pos = source_pos.load(Ordering::Relaxed);
                    if cur_pos == last_source_pos {
                        stuck_frames += 1;
                        if stuck_frames > 60 {
                            // ~1 seconde stilstand (16ms per frame)
                            stream_is_dead = true;
                            if let Some(s) = &sink {
                                s.stop();
                                s.clear();
                            }
                            is_playing = false;
                            let _ = event_tx.send(WaveformEvent::Stopped);
                        }
                    } else {
                        stuck_frames = 0;
                        last_source_pos = cur_pos;
                        last_audio_activity = std::time::Instant::now();
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

// ───────────────────────────────────────────────
// Gedeelde hulpfuncties voor SoundTouchSource en SequenceSource
// ───────────────────────────────────────────────

/// Update pitch en tempo vanuit atomic variabelen als ze gewijzigd zijn.
fn update_pitch_tempo(
    ts: &mut TimeStretch,
    pitch_semitones: &Arc<AtomicU32>,
    tempo: &Arc<AtomicU32>,
    current_pitch: &mut f32,
    current_tempo: &mut f32,
    cached_tempo: &mut f64,
) {
    let new_pitch = f32::from_bits(pitch_semitones.load(Ordering::Relaxed));
    let new_tempo = f32::from_bits(tempo.load(Ordering::Relaxed));
    if (new_pitch - *current_pitch).abs() > 0.01 {
        ts.set_pitch_semitones(new_pitch);
        *current_pitch = new_pitch;
    }
    if (new_tempo - *current_tempo).abs() > 0.01 {
        ts.set_speed(new_tempo);
        *current_tempo = new_tempo;
        *cached_tempo = new_tempo as f64;
    }
}

/// Voer input_chunk door TimeStretch en verzamel output in out_buf.
/// temp_out wordt herbruikt als scratch-buffer.
fn process_through_timestretch(
    ts: &mut TimeStretch,
    input_chunk: &[f32],
    out_buf: &mut Vec<f32>,
    temp_out: &mut Vec<f32>,
) {
    ts.put_samples(input_chunk, input_chunk.len());
    loop {
        let received = ts.receive_samples(temp_out, 4096);
        if received == 0 {
            break;
        }
        out_buf.extend_from_slice(&temp_out[..received]);
    }
}

/// Flush wat er nog in TimeStretch zit na einde van de input.
fn flush_timestretch(ts: &mut TimeStretch, out_buf: &mut Vec<f32>, flush_buf: &mut Vec<f32>) {
    let received = ts.receive_samples(flush_buf, 4096);
    if received > 0 {
        out_buf.extend_from_slice(&flush_buf[..received]);
    }
}

/// Volume toepassen met soft-clip limiter.
fn apply_volume_soft_clip(raw_val: f32, volume: &Arc<AtomicU32>) -> f32 {
    let vol = f32::from_bits(volume.load(Ordering::Relaxed));
    let mut val = raw_val * vol;
    if val > 1.0 {
        val = 1.0 - 1.0 / (val + 1.0);
    } else if val < -1.0 {
        val = -1.0 + 1.0 / (-val + 1.0);
    }
    val
}

struct SoundTouchSource {
    raw_samples: Arc<Vec<f32>>,
    sample_rate: u32,
    pitch_semitones: Arc<AtomicU32>,
    tempo: Arc<AtomicU32>,
    loop_bounds: Arc<Mutex<LoopBounds>>,
    source_pos: Arc<AtomicU64>,
    seek_requested: Arc<AtomicBool>,
    seek_target: Arc<AtomicU64>,
    volume: Arc<AtomicU32>,
    ts: TimeStretch,
    read_pos: usize,
    out_buf: Vec<f32>,
    out_idx: usize,
    current_pitch: f32,
    current_tempo: f32,
    cached_tempo: f64,
    cached_loop_enabled: bool,
    cached_loop_start: f64,
    cached_loop_end: f64,
    cached_loop_dur: f64,
    current_audio_pos: f64,
    /// Herbruikbare buffers om allocaties in fill_buffer te voorkomen
    input_chunk: Vec<f32>,
    temp_out: Vec<f32>,
    flush_buf: Vec<f32>,
    // ── Click/audit generatie ──
    click_positions: Arc<Mutex<Vec<f32>>>,
    click_enabled: Arc<AtomicBool>,
}

// ───────────────────────────────────────────────
// SequenceSource — voor arrangement playback
// ───────────────────────────────────────────────

struct SequenceSource {
    /// Huidige samples (kunnen wisselen per stap)
    raw_samples: Arc<Vec<f32>>,
    /// De hele sequentie
    sequence: Vec<SequenceStep>,
    /// Huidige stap index
    current_step_idx: usize,
    /// Huidige read positie in raw_samples
    read_pos: usize,
    sample_rate: u32,
    pitch_semitones: Arc<AtomicU32>,
    tempo: Arc<AtomicU32>,
    source_pos: Arc<AtomicU64>,
    volume: Arc<AtomicU32>,
    ts: TimeStretch,
    out_buf: Vec<f32>,
    out_idx: usize,
    current_pitch: f32,
    current_tempo: f32,
    cached_tempo: f64,
    current_audio_pos: f64,
    /// Kanaal om events terug naar UI te sturen
    step_event_tx: Sender<WaveformEvent>,
    /// Herbruikbare buffers om allocaties in fill_buffer te voorkomen
    input_chunk: Vec<f32>,
    temp_out: Vec<f32>,
    flush_buf: Vec<f32>,
}

impl SequenceSource {
    fn new(
        sequence: Vec<SequenceStep>,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
        source_pos: Arc<AtomicU64>,
        volume: Arc<AtomicU32>,
        step_event_tx: Sender<WaveformEvent>,
    ) -> Self {
        let first = sequence[0].clone();
        let total_frames = first.samples.len();

        let mut ts = TimeStretch::new(first.sample_rate, 1, total_frames);
        let initial_pitch = f32::from_bits(pitch_semitones.load(Ordering::Relaxed));
        let initial_tempo = f32::from_bits(tempo.load(Ordering::Relaxed));
        ts.set_speed(initial_tempo);
        ts.set_pitch_semitones(initial_pitch);

        let start_pos = first.start_sample;
        let sr = first.sample_rate;
        source_pos.store(f64::to_bits(start_pos as f64), Ordering::Relaxed);

        Self {
            raw_samples: first.samples.clone(),
            sequence,
            current_step_idx: 0,
            read_pos: start_pos,
            sample_rate: sr,
            pitch_semitones,
            tempo,
            source_pos,
            volume,
            ts,
            out_buf: Vec::with_capacity(4096),
            out_idx: 0,
            current_pitch: initial_pitch,
            current_tempo: initial_tempo,
            cached_tempo: initial_tempo as f64,
            current_audio_pos: start_pos as f64,
            step_event_tx,
            input_chunk: Vec::with_capacity(4096),
            temp_out: vec![0.0; 4096],
            flush_buf: vec![0.0; 4096],
        }
    }

    fn fill_buffer(&mut self) {
        update_pitch_tempo(
            &mut self.ts,
            &self.pitch_semitones,
            &self.tempo,
            &mut self.current_pitch,
            &mut self.current_tempo,
            &mut self.cached_tempo,
        );

        self.out_buf.clear();
        self.out_idx = 0;

        let target_out = 4096;
        self.input_chunk.clear();

        while self.out_buf.len() < target_out {
            // Huidige stap ophalen
            let step = &self.sequence[self.current_step_idx];

            self.input_chunk.clear();

            // Samples inlezen van huidige read_pos tot einde van deze loop
            while self.input_chunk.len() < 4096 {
                if self.read_pos >= step.end_sample {
                    break;
                }
                let to_read = (4096 - self.input_chunk.len()).min(step.end_sample - self.read_pos);
                self.input_chunk
                    .extend_from_slice(&self.raw_samples[self.read_pos..self.read_pos + to_read]);
                self.read_pos += to_read;
            }

            if self.input_chunk.is_empty() {
                // Geen input meer → verwerk herhalingen of volgende stap
                let step_idx = self.current_step_idx;
                let repeats = self.sequence[step_idx].repeats;
                if repeats > 1 {
                    self.sequence[step_idx].repeats -= 1;
                    self.read_pos = self.sequence[step_idx].start_sample;
                    let _ = self
                        .step_event_tx
                        .send(WaveformEvent::StepRepeated(step_idx));
                    continue;
                }
                if step_idx + 1 < self.sequence.len() {
                    self.current_step_idx += 1;
                    let next_idx = self.current_step_idx;
                    self.raw_samples = self.sequence[next_idx].samples.clone();
                    self.read_pos = self.sequence[next_idx].start_sample;
                    self.sample_rate = self.sequence[next_idx].sample_rate;
                    self.ts.clear();
                    let _ = self
                        .step_event_tx
                        .send(WaveformEvent::StepChanged(next_idx));
                    continue;
                }
                // Einde arrangement
                let _ = self.step_event_tx.send(WaveformEvent::ArrangementFinished);
                flush_timestretch(&mut self.ts, &mut self.out_buf, &mut self.flush_buf);
                break;
            }

            process_through_timestretch(
                &mut self.ts,
                &self.input_chunk,
                &mut self.out_buf,
                &mut self.temp_out,
            );
        }
    }
}

impl Iterator for SequenceSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.out_idx >= self.out_buf.len() {
            self.fill_buffer();
            if self.out_buf.is_empty() {
                return None;
            }
        }

        if self.out_idx < self.out_buf.len() {
            let raw_val = self.out_buf[self.out_idx];
            let val = apply_volume_soft_clip(raw_val, &self.volume);
            self.out_idx += 1;

            self.current_audio_pos += self.cached_tempo;
            self.source_pos
                .store(f64::to_bits(self.current_audio_pos), Ordering::Relaxed);

            Some(val)
        } else {
            None
        }
    }
}

impl Source for SequenceSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(usize::MAX)
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

// ───────────────────────────────────────────────
// SoundTouchSource — voor enkele loop playback
// ───────────────────────────────────────────────

impl SoundTouchSource {
    fn new(
        raw_samples: Arc<Vec<f32>>,
        sample_rate: u32,
        pitch_semitones: Arc<AtomicU32>,
        tempo: Arc<AtomicU32>,
        loop_bounds: Arc<Mutex<LoopBounds>>,
        source_pos: Arc<AtomicU64>,
        seek_requested: Arc<AtomicBool>,
        seek_target: Arc<AtomicU64>,
        volume: Arc<AtomicU32>,
        click_positions: Arc<Mutex<Vec<f32>>>,
        click_enabled: Arc<AtomicBool>,
    ) -> Self {
        let total_frames = raw_samples.len();

        let mut ts = TimeStretch::new(sample_rate, 1, total_frames);

        let initial_pitch = f32::from_bits(pitch_semitones.load(Ordering::Relaxed));
        let initial_tempo = f32::from_bits(tempo.load(Ordering::Relaxed));

        ts.set_speed(initial_tempo);
        ts.set_pitch_semitones(initial_pitch);

        let start_pos = f64::from_bits(source_pos.load(Ordering::Relaxed)) as usize;

        let bounds = loop_bounds.lock().unwrap();
        let c_enabled = bounds.enabled();
        let (c_start, c_end, c_dur) = if c_enabled {
            let s = bounds.a as f64;
            let e = bounds.b as f64;
            (s, e, e - s)
        } else {
            (0.0, 0.0, 0.0)
        };
        drop(bounds);

        Self {
            raw_samples,
            sample_rate,
            pitch_semitones,
            tempo,
            loop_bounds,
            source_pos,
            seek_requested,
            seek_target,
            volume,
            ts,
            read_pos: start_pos,
            out_buf: Vec::with_capacity(4096),
            out_idx: 0,
            current_pitch: initial_pitch,
            current_tempo: initial_tempo,
            cached_tempo: initial_tempo as f64,
            cached_loop_enabled: c_enabled,
            cached_loop_start: c_start,
            cached_loop_end: c_end,
            cached_loop_dur: c_dur,
            current_audio_pos: start_pos as f64,
            input_chunk: Vec::with_capacity(4096),
            temp_out: vec![0.0; 4096],
            flush_buf: vec![0.0; 4096],
            click_positions,
            click_enabled,
        }
    }

    fn fill_buffer(&mut self) {
        // 1. Echte seek detectie
        if self.seek_requested.swap(false, Ordering::Relaxed) {
            let atomic_pos = f64::from_bits(self.seek_target.load(Ordering::Relaxed));
            self.read_pos = atomic_pos as usize;
            self.current_audio_pos = atomic_pos;
            self.source_pos
                .store(f64::to_bits(self.read_pos as f64), Ordering::Relaxed);
            self.ts.clear();
            self.out_buf.clear();
            self.out_idx = 0;
        }

        update_pitch_tempo(
            &mut self.ts,
            &self.pitch_semitones,
            &self.tempo,
            &mut self.current_pitch,
            &mut self.current_tempo,
            &mut self.cached_tempo,
        );

        // 3. Update cached loop bounds
        {
            let bounds = self.loop_bounds.lock().unwrap();
            self.cached_loop_enabled = bounds.enabled();
            if self.cached_loop_enabled {
                self.cached_loop_start = bounds.a as f64;
                self.cached_loop_end = bounds.b as f64;
                self.cached_loop_dur = self.cached_loop_end - self.cached_loop_start;
            }
        }

        // 4. Buffer leegmaken voor nieuwe data
        self.out_buf.clear();
        self.out_idx = 0;

        // 5. Buffer vulling via SoundTouch
        let total_len = self.raw_samples.len();
        if total_len == 0 {
            return;
        }

        let target_out = 4096;
        self.input_chunk.clear();

        while self.out_buf.len() < target_out {
            self.input_chunk.clear();

            while self.input_chunk.len() < 4096 {
                let end_pos = if self.cached_loop_enabled {
                    self.cached_loop_end as usize
                } else {
                    total_len
                };

                if self.read_pos >= end_pos {
                    if self.cached_loop_enabled {
                        self.read_pos = self.cached_loop_start as usize;
                        continue;
                    } else {
                        break;
                    }
                }

                let to_read = (4096 - self.input_chunk.len()).min(end_pos - self.read_pos);
                if to_read == 0 {
                    break;
                }

                self.input_chunk
                    .extend_from_slice(&self.raw_samples[self.read_pos..self.read_pos + to_read]);
                self.read_pos += to_read;
            }

            if self.input_chunk.is_empty() {
                let received = self.ts.receive_samples(&mut self.flush_buf, 4096);
                if received > 0 {
                    self.out_buf.extend_from_slice(&self.flush_buf[..received]);
                }
                break;
            }

            self.ts
                .put_samples(&self.input_chunk, self.input_chunk.len());

            // Drain ALLES wat Rubber Band beschikbaar heeft, niet maar 1 batch
            loop {
                let received = self.ts.receive_samples(&mut self.temp_out, 4096);
                if received == 0 {
                    break;
                }
                self.out_buf.extend_from_slice(&self.temp_out[..received]);
            }
        }

        // ── Click-generatie voor beat audit ──
        // Mix clicks direct in de output buffer op de marker-posities.
        // Omdat dit NA SoundTouch gebeurt maar VOOR rodio's buffering,
        // zijn de clicks sample-accuraat synced met de audio.
        if self.click_enabled.load(Ordering::Relaxed) && !self.out_buf.is_empty() {
            let sr = self.sample_rate as f64;
            let tempo = self.cached_tempo;
            let buf_len = self.out_buf.len();

            // Bereken het sample-bereik dat deze buffer beslaat in de INPUT
            let buf_start_sample = self.current_audio_pos;
            let buf_end_sample = buf_start_sample + buf_len as f64 * tempo;

            let positions = self.click_positions.lock().unwrap();
            for &click_sec in positions.iter() {
                let click_sample = click_sec as f64 * sr;

                // Check of de click binnen deze buffer valt
                if click_sample < buf_start_sample || click_sample >= buf_end_sample {
                    continue;
                }

                // Bepaal de exacte sample-index in de output buffer
                let out_idx_rel = ((click_sample - buf_start_sample) / tempo).round() as isize;
                if out_idx_rel < 0 || out_idx_rel >= buf_len as isize {
                    continue;
                }
                let out_idx = out_idx_rel as usize;

                // Genereer een korte click (8ms sinus van 1000Hz)
                let click_duration_samples = (sr * 0.008) as usize;
                for j in 0..click_duration_samples {
                    let buf_pos = out_idx + j;
                    if buf_pos >= buf_len {
                        break;
                    }
                    let t = j as f64 / sr;
                    let click_sample_val = (t * 1000.0 * 2.0 * std::f64::consts::PI).sin() as f32;
                    // Click op 40% volume, gemixed met bestaande audio
                    self.out_buf[buf_pos] =
                        (self.out_buf[buf_pos] + click_sample_val * 0.4).clamp(-1.0, 1.0);
                }
            }
        }
    }
}

impl Iterator for SoundTouchSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.out_idx >= self.out_buf.len() {
            self.fill_buffer();
            // ✅ FIX: Als fill_buffer niks opleverde (tempo-wijziging, opstart),
            // geef dan stilte ipv None zodat de source blijft leven.
            if self.out_buf.is_empty() {
                self.out_idx = 1; // blijf fill_buffer triggeren
                return Some(0.0);
            }
        }

        if self.out_idx < self.out_buf.len() {
            let raw_val = self.out_buf[self.out_idx];
            let val = apply_volume_soft_clip(raw_val, &self.volume);
            self.out_idx += 1;
            // ✅ Positie accumuleren en wrappen — subtractie ipv modulo
            self.current_audio_pos += self.cached_tempo;
            {
                let bounds = self.loop_bounds.lock().unwrap();
                if bounds.enabled() {
                    let loop_start = bounds.a as f64;
                    let loop_end = bounds.b as f64;
                    let loop_dur = loop_end - loop_start;
                    // Wrap door subtractie: blijf in [loop_start, loop_end)
                    if loop_dur > 0.0 {
                        while self.current_audio_pos >= loop_end {
                            self.current_audio_pos -= loop_dur;
                        }
                        if self.current_audio_pos < loop_start {
                            self.current_audio_pos = loop_start;
                        }
                    }
                }
            }
            self.source_pos
                .store(f64::to_bits(self.current_audio_pos), Ordering::Relaxed);

            Some(val)
        } else {
            None
        }
    }
}

impl Source for SoundTouchSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(usize::MAX)
    }
    fn channels(&self) -> u16 {
        1
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
