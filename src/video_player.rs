use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

// ✅ FIX 1: Correcte Windows Named Pipe prefix (\\.\pipe\)
const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
    /// Bevat nu ALLEEN de write-pipe om deadlocks te voorkomen
    pipe: Mutex<Option<std::fs::File>>,
}

impl VideoPlayer {
    pub fn new(mpv_path: &str) -> Self {
        Self {
            process: None,
            mpv_path: mpv_path.to_string(),
            pipe: Mutex::new(None),
        }
    }

    pub fn open(&mut self, video_path: &str) -> Result<(), String> {
        self.close();

        let process = Command::new(&self.mpv_path)
            .args(&[
                "--no-terminal",
                "--pause",
                "--volume=0",
                &format!("--input-ipc-server={}", MPV_PIPE),
                video_path,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Kan mpv niet starten: {}", e))?;

        self.process = Some(process);

        // Wacht tot mpv de pipe heeft aangemaakt
        std::thread::sleep(Duration::from_millis(1000));

        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;

            // ✅ FIX 2: Open TWEE aparte connecties naar de pipe.
            // Windows serialiseert I/O op een enkele HANDLE, wat tot deadlocks leidt
            // als je tegelijkertijd wilt lezen en schrijven. mpv accepteert meerdere clients.
            let write_pipe = OpenOptions::new()
                .write(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Pipe write error: {}", e))?;

            let read_pipe = OpenOptions::new()
                .read(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Pipe read error: {}", e))?;

            // Achtergrond-thread voor het uitlezen van mpv's antwoorden
            thread::spawn(move || {
                let reader = BufReader::new(read_pipe);
                for line in reader.lines() {
                    match line {
                        Ok(text) => log::debug!("mpv: {}", text),
                        Err(_) => break, // pipe gesloten
                    }
                }
                log::debug!("mpv reader gestopt");
            });

            // We slaan ALLEEN de write-pipe op voor onze commando's
            *self.pipe.lock().unwrap() = Some(write_pipe);
            log::info!("mpv pipe geopend (gesplitst voor read/write)");
        }
        Ok(())
    }

    /// Stuur JSON-commando via de dedicated write-pipe.
    fn send_command(&self, cmd: &str) -> Result<(), String> {
        let mut guard = self
            .pipe
            .lock()
            .map_err(|e| format!("Mutex error: {}", e))?;
        if let Some(ref mut pipe) = *guard {
            // Newline is VERPLICHT
            let cmd_with_newline = format!("{}\n", cmd);

            // Omdat we nu een dedicated write-pipe gebruiken, blokkeert dit
            // niet meer op de reader en blijft de UI responsive!
            pipe.write_all(cmd_with_newline.as_bytes())
                .map_err(|e| format!("Write error: {}", e))?;
            pipe.flush().ok();
            Ok(())
        } else {
            Err("Pipe niet geopend".to_string())
        }
    }

    pub fn seek(&self, pos_secs: f32) {
        if let Err(e) = self.send_command(&format!(
            r#"{{ "command": ["set_property", "time-pos", {}] }}"#,
            pos_secs
        )) {
            log::warn!("seek: {}", e);
        }
    }

    pub fn pause(&self) {
        if let Err(e) = self.send_command(r#"{ "command": ["set_property", "pause", true] }"#) {
            log::warn!("pause: {}", e);
        }
    }

    pub fn resume(&self) {
        if let Err(e) = self.send_command(r#"{ "command": ["set_property", "pause", false] }"#) {
            log::warn!("resume: {}", e);
        }
    }

    pub fn set_speed(&self, speed: f32) {
        if let Err(e) = self.send_command(&format!(
            r#"{{ "command": ["set_property", "speed", {}] }}"#,
            speed
        )) {
            log::warn!("speed: {}", e);
        }
    }

    pub fn set_loop_a(&self, secs: f32) {
        let _ = self.send_command(&format!(
            r#"{{ "command": ["set_property", "ab-loop-a", {}] }}"#,
            secs
        ));
    }

    pub fn set_loop_b(&self, secs: f32) {
        let _ = self.send_command(&format!(
            r#"{{ "command": ["set_property", "ab-loop-b", {}] }}"#,
            secs
        ));
    }

    pub fn clear_loop(&self) {
        let _ = self.send_command(r#"{ "command": ["set_property", "ab-loop-a", "no"] }"#);
        let _ = self.send_command(r#"{ "command": ["set_property", "ab-loop-b", "no"] }"#);
    }

    pub fn close(&mut self) {
        *self.pipe.lock().unwrap() = None;
        if let Some(mut p) = self.process.take() {
            let _ = p.kill();
            let _ = p.wait();
        }
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.close();
    }
}
