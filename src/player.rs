use anyhow::{anyhow, Result};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct PlayerHandle {
    child: Child,
    workdir: PathBuf,
}

pub fn find_mpv() -> Option<PathBuf> {
    let path = std::env::var("PATH").ok()?;
    for dir in path.split(':') {
        let p = PathBuf::from(dir).join("mpv");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

impl PlayerHandle {
    pub fn spawn(mpv: &PathBuf, uri: &str, cookie: Option<&str>, title: &str) -> Result<Self> {
        let workdir = std::env::temp_dir().join(format!("rigi-tui-{}", std::process::id()));
        std::fs::create_dir_all(&workdir)?;
        let mut perms = std::fs::metadata(&workdir)?.permissions();
        perms.set_mode(0o700);
        std::fs::set_permissions(&workdir, perms)?;

        let sock = workdir.join("mpv.sock");
        let mut child = Command::new(mpv)
            .arg("--no-terminal")
            .arg("--really-quiet")
            .arg("--idle=once")
            .arg(format!("--input-ipc-server={}", sock.display()))
            .arg(format!("--force-media-title={}", title))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stream = None;
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            match UnixStream::connect(&sock) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => {
                    if child.try_wait()?.is_some() {
                        let _ = std::fs::remove_dir_all(&workdir);
                        return Err(anyhow!("mpv turant hi band ho gaya"));
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
            }
        }
        let sock_stream = stream.ok_or_else(|| anyhow!("mpv IPC socket nahi mila"))?;
        sock_stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut reader = BufReader::new(sock_stream.try_clone()?);
        let mut writer = sock_stream;

        if let Some(c) = cookie.filter(|c| !c.is_empty()) {
            let hdr = json!({"command": ["set_property", "http-header-fields", json!([format!("Cookie: {}", c)])]});
            writeln!(writer, "{}", hdr)?;
            writer.flush()?;
            let _ = read_ack(&mut reader, Duration::from_secs(5));
        }
        let cmd = json!({"command": ["loadfile", uri, "replace"]});
        writeln!(writer, "{}", cmd)?;
        writer.flush()?;
        let ok = read_ack(&mut reader, Duration::from_secs(5)).unwrap_or(false);

        if !ok {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&workdir);
            return Err(anyhow!("mpv loadfile fail hua (URL ya header reject hua)"));
        }

        Ok(Self { child, workdir })
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PlayerHandle {
    fn drop(&mut self) {
        self.stop();
        let _ = std::fs::remove_dir_all(&self.workdir);
    }
}

fn read_ack<R: BufRead>(r: &mut R, timeout: Duration) -> Option<bool> {
    let deadline = Instant::now() + timeout;
    let mut line = String::new();
    loop {
        if Instant::now() > deadline {
            return None;
        }
        line.clear();
        match r.read_line(&mut line) {
            Ok(0) => return None,
            Ok(_) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                        return Some(err == "success");
                    }
                }
            }
            Err(_) => return None,
        }
    }
}
