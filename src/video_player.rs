use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

// ✅ FIX 1: Exacte Windows Named Pipe syntax (let op de dubbele backslashes)
const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
    /// Bevat nu ALLEEN de dedicated write-pipe om deadlocks te voorkomen
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

        let stderr_log = crate::session::data_dir().join("mpv_stderr.log");
        let stderr_file = std::fs::File::create(&stderr_log)
            .map_err(|e| format!("Kan mpv log niet aanmaken: {}", e))?;

        let process = Command::new(&self.mpv_path)
            .args(&[
                "--no-terminal",
                "--keep-open=yes", // ✅ FIX 2: Voorkom dat mpv sluit bij einde bestand
                "--pause",
                "--volume=0",
                &format!("--input-ipc-server={}", MPV_PIPE),
                video_path,
            ])
            .stdout(Stdio::null())
            .stderr(stderr_file)
            .spawn()
            .map_err(|e| format!("Kan mpv niet starten: {}", e))?;

        self.process = Some(process);
        std::thread::sleep(Duration::from_millis(1000));

        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;

            // ✅ FIX 3: Open TWEE volledig onafhankelijke verbindingen naar de pipe.
            // Dit omzeilt de Windows I/O serialisatie deadlock die ontstaat bij try_clone().

            // 1. Dedicated WRITE verbinding
            let write_pipe = OpenOptions::new()
                .write(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Pipe write error: {}", e))?;

            // 2. Dedicated READ verbinding (nieuwe OS handle, geen clone!)
            let read_pipe = OpenOptions::new()
                .read(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Pipe read error: {}", e))?;

            // Achtergrond-thread die de read-pipe continu leegzuigt
            thread::spawn(move || {
                let reader = BufReader::new(read_pipe);
                for line in reader.lines() {
                    match line {
                        Ok(text) => log::debug!("mpv: {}", text),
                        Err(_) => break, // Pipe verbroken (mpv gesloten)
                    }
                }
                log::debug!("mpv reader gestopt");
            });

            // We slaan ALLEEN de write-pipe op voor onze commando's
            *self.pipe.lock().unwrap() = Some(write_pipe);
            log::info!("mpv pipe geopend (gesplitst read/write, deadlock-vrij)");
        }
        Ok(())
    }

    fn send_command(&self, cmd: &str) -> Result<(), String> {
        let mut guard = self
            .pipe
            .lock()
            .map_err(|e| format!("Mutex error: {}", e))?;
        if let Some(ref mut pipe) = *guard {
            // Newline is VERPLICHT voor mpv IPC
            let cmd_with_newline = format!("{}\n", cmd);

            // Omdat dit nu een dedicated write-handle is, zal dit NOOIT meer
            // blokkeren op de lees-actie van de achtergrond-thread.
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
