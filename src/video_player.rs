use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
    /// Permanente pipe-connectie — eenmaal open, blijft open voor alle commands.
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

    /// Open video in mpv en maak permanente pipe-connectie.
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
        std::thread::sleep(Duration::from_millis(500));

        // Maak permanente connectie
        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;
            let pipe = OpenOptions::new()
                .read(true)
                .write(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Pipe error bij openen: {}", e))?;
            *self.pipe.lock().unwrap() = Some(pipe);
            log::info!("mpv pipe geopend (permanent)");
        }

        Ok(())
    }

    /// Stuur JSON-commando via de permanente pipe-connectie.
    fn send_command(&self, cmd: &str) -> Result<(), String> {
        let mut guard = self
            .pipe
            .lock()
            .map_err(|e| format!("Mutex error: {}", e))?;
        if let Some(ref mut pipe) = *guard {
            pipe.write_all(cmd.as_bytes())
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
