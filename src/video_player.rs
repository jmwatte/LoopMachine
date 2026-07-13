use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
    is_open: bool,
}

impl VideoPlayer {
    pub fn new(mpv_path: &str) -> Self {
        Self {
            process: None,
            mpv_path: mpv_path.to_string(),
            is_open: false,
        }
    }

    /// Open video in mpv — start gepauzeerd, mute audio (wij luisteren via LoopMachine).
    pub fn open(&mut self, video_path: &str) -> Result<(), String> {
        self.close();
        let process = Command::new(&self.mpv_path)
            .args(&[
                "--no-terminal",
                "--pause",    // start gepauzeerd — wij bepalen wanneer het gaat spelen
                "--volume=0", // audio uit — wij luisteren via LoopMachine
                &format!("--input-ipc-server={}", MPV_PIPE),
                video_path,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Kan mpv niet starten: {}", e))?;
        self.process = Some(process);
        self.is_open = true;

        // Wacht tot de named pipe beschikbaar is (mpv maakt hem aan bij start)
        let max_retries = 50;
        for attempt in 0..max_retries {
            #[cfg(target_os = "windows")]
            {
                use std::fs::OpenOptions;
                if OpenOptions::new().write(true).open(MPV_PIPE).is_ok() {
                    return Ok(());
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                use std::os::unix::net::UnixStream;
                if UnixStream::connect(MPV_PIPE).is_ok() {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // Pipe na 1 seconde nog niet beschikbaar → geef wel ok, sync faalt dan stil
        log::warn!("mpv pipe niet beschikbaar na 1s, ga door zonder sync");
        Ok(())
    }

    /// Stuur JSON-commando naar mpv via named pipe
    fn send_command(&self, cmd: &str) -> Result<(), String> {
        if !self.is_open {
            return Err("mpv niet geopend".to_string());
        }
        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;
            let mut pipe = OpenOptions::new()
                .write(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Kan pipe niet openen (mpv misschien gesloten?): {}", e))?;
            pipe.write_all(cmd.as_bytes())
                .map_err(|e| format!("Kan niet schrijven naar pipe: {}", e))?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::net::UnixStream;
            let mut stream = UnixStream::connect(MPV_PIPE)
                .map_err(|e| format!("Kan socket niet openen (mpv misschien gesloten?): {}", e))?;
            stream
                .write_all(cmd.as_bytes())
                .map_err(|e| format!("Kan niet schrijven naar socket: {}", e))?;
        }
        Ok(())
    }

    pub fn seek(&self, pos_secs: f32) {
        let _ = self.send_command(&format!(
            r#"{{ "command": ["set_property", "time-pos", {}] }}"#,
            pos_secs
        ));
    }

    pub fn pause(&self) {
        let _ = self.send_command(r#"{ "command": ["set_property", "pause", true] }"#);
    }

    pub fn resume(&self) {
        let _ = self.send_command(r#"{ "command": ["set_property", "pause", false] }"#);
    }

    pub fn set_speed(&self, speed: f32) {
        let _ = self.send_command(&format!(
            r#"{{ "command": ["set_property", "speed", {}] }}"#,
            speed
        ));
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

    /// Forceer mpv om te stoppen — kill direct, geen wait.
    /// Als de gebruiker mpv zelf heeft gesloten doet `kill()` niets (proces is al weg).
    pub fn close(&mut self) {
        self.is_open = false;
        if let Some(mut p) = self.process.take() {
            let _ = p.kill(); // force kill, geen wait — geen hang meer
            let _ = p.wait(); // wacht nog even zodat resources worden opgeruimd
        }
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.close();
    }
}
