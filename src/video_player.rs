use std::io::Write;
use std::process::{Child, Command, Stdio};

const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
}

impl VideoPlayer {
    pub fn new(mpv_path: &str) -> Self {
        Self {
            process: None,
            mpv_path: mpv_path.to_string(),
        }
    }

    /// Open video in mpv (gelinked met onze pipe) — start muted, want audio komt via LoopMachine.
    pub fn open(&mut self, video_path: &str) -> Result<(), String> {
        self.close();
        let process = Command::new(&self.mpv_path)
            .args(&[
                "--no-terminal",
                "--volume=0",
                &format!("--input-ipc-server={}", MPV_PIPE),
                &format!("--start=0"),
                video_path,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Kan mpv niet starten: {}", e))?;
        self.process = Some(process);
        Ok(())
    }

    /// Stuur JSON-commando naar mpv via named pipe
    fn send_command(&self, cmd: &str) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;
            let mut pipe = OpenOptions::new()
                .write(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Kan pipe niet openen: {}", e))?;
            pipe.write_all(cmd.as_bytes())
                .map_err(|e| format!("Kan niet schrijven naar pipe: {}", e))?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::net::UnixStream;
            let mut stream = UnixStream::connect(MPV_PIPE)
                .map_err(|e| format!("Kan socket niet openen: {}", e))?;
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

    /// Zet mpv volume (0-100). 0 = stil — wij luisteren via LoopMachine.
    pub fn set_volume(&self, vol: f32) {
        let _ = self.send_command(&format!(
            r#"{{ "command": ["set_property", "volume", {}] }}"#,
            vol
        ));
    }

    pub fn close(&mut self) {
        if let Some(mut p) = self.process.take() {
            let _ = self.send_command(r#"{ "command": ["quit"] }"#);
            let _ = p.wait();
        }
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.close();
    }
}
