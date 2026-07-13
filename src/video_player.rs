use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

// ✅ Exacte Windows Named Pipe syntax
const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
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

        // 1. Bepaal exact waar de log naartoe gaat en print dit naar je console
        let stderr_log = crate::session::data_dir().join("mpv_stderr.log");
        log::info!("📝 mpv logbestand wordt geschreven naar: {:?}", stderr_log);

        let stderr_file = std::fs::File::create(&stderr_log)
            .map_err(|e| format!("Kan mpv log niet aanmaken: {}", e))?;

        // 2. Maak het videopad absoluut. mpv faalt soms op relatieve paden.
        let abs_video_path = std::path::Path::new(video_path)
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| video_path.to_string());

        // 3. Start mpv met force-window en keep-open om direct sluiten te voorkomen
        let process = Command::new(&self.mpv_path)
            .args(&[
                "--force-window=yes", // ✅ Forceer venster, voorkom direct sluiten bij pauze
                "--keep-open=yes",    // ✅ Sluit niet af bij einde bestand
                "--no-terminal",      // Geen apart zwart console-venster
                "--pause",
                "--volume=0",
                "--msg-level=all=debug", // ✅ Forceer maximale logging voor debugging
                &format!("--input-ipc-server={}", MPV_PIPE),
                &abs_video_path,
            ])
            .stdout(Stdio::null())
            .stderr(stderr_file)
            .spawn()
            .map_err(|e| format!("Kan mpv niet starten: {}", e))?;

        self.process = Some(process);

        // Wacht tot mpv de pipe heeft aangemaakt
        std::thread::sleep(Duration::from_millis(1000));

        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;

            // Retry mechanisme voor extra stabiliteit bij het openen van de pipe
            let mut pipe = None;
            for attempt in 1..=5 {
                match OpenOptions::new().read(true).write(true).open(MPV_PIPE) {
                    Ok(p) => {
                        pipe = Some(p);
                        break;
                    }
                    Err(e) => {
                        if attempt == 5 {
                            return Err(format!("Kan pipe niet openen na 5 pogingen: {}", e));
                        }
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
            }

            let pipe = pipe.ok_or("Pipe niet beschikbaar".to_string())?;

            // Achtergrond-thread die mpv's antwoorden uitleest
            let read_pipe = pipe
                .try_clone()
                .map_err(|e| format!("Kan pipe niet clonen: {}", e))?;

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

            *self.pipe.lock().unwrap() = Some(pipe);
            log::info!("mpv pipe geopend (permanent)");
        }
        Ok(())
    }

    fn send_command(&self, cmd: &str) -> Result<(), String> {
        let mut guard = self
            .pipe
            .lock()
            .map_err(|e| format!("Mutex error: {}", e))?;
        if let Some(ref mut pipe) = *guard {
            // Newline is VERPLICHT
            let cmd_with_newline = format!("{}\n", cmd);
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
