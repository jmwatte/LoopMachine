use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

// ✅ FIX 1: Exacte Windows Named Pipe syntax (let op de dubbele backslashes)
const MPV_PIPE: &str = r"\\.\pipe\mpv-loopmachine";

pub struct VideoPlayer {
    process: Option<Child>,
    mpv_path: String,
    /// Één duplex pipe handle (read+write) — geen deadlock-risico.
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

        // 1. Bepaal de map waar mpv.exe staat (CRUCIAAL voor portable mpv)
        let mpv_exe = std::path::Path::new(&self.mpv_path);
        let mpv_dir = mpv_exe
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));

        // 2. Gebruik mpv's EIGEN logger (veel betrouwbaarder dan Rust stderr capture)
        let mpv_native_log = crate::session::data_dir().join("mpv_native_log.txt");
        log::info!("📝 mpv's interne logbestand: {:?}", mpv_native_log);

        let mut cmd = Command::new(&self.mpv_path);
        cmd.current_dir(mpv_dir) // ✅ FIX 1: Zet de werkmap op de mpv-map, zodat DLL's/codecs gevonden worden
            .args(&[
                "--no-terminal",
                "--keep-open=yes",
                "--force-window=yes",
                "--pause",
                "--volume=0",
                &format!("--log-file={}", mpv_native_log.display()), // ✅ FIX 2: mpv's eigen interne logger
                &format!("--input-ipc-server={}", MPV_PIPE),
                video_path,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null()); // We hoeven stderr niet meer te vangen, --log-file doet het beter

        // Onderdruk terminalvenster op Windows
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let process = cmd
            .spawn()
            .map_err(|e| format!("Kan mpv niet starten: {}", e))?;

        self.process = Some(process);
        std::thread::sleep(Duration::from_millis(1000));

        #[cfg(target_os = "windows")]
        {
            use std::fs::OpenOptions;

            // ÉÉN duplex pipe openen (read + write) i.p.v. twee aparte handles.
            // Mpv's --input-ipc-server accepteert slechts 1 client-connectie,
            // dus een tweede open zou mislukken (ERROR_PIPE_BUSY).
            let pipe = OpenOptions::new()
                .read(true)
                .write(true)
                .open(MPV_PIPE)
                .map_err(|e| format!("Pipe open error: {}", e))?;

            *self.pipe.lock().unwrap() = Some(pipe);
            log::info!("mpv pipe geopend (duplex)");
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
