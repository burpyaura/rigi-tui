use anyhow::{anyhow, Result};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Store {
    dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct Credentials {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
}

impl Store {
    pub fn new() -> Result<Self> {
        let base = dirs::config_dir()
            .ok_or_else(|| anyhow!("Config directory nahi mila"))?
            .join("rigi-tui");
        fs::create_dir_all(&base)?;
        let perms = fs::metadata(&base)?.permissions();
        let mut p = perms;
        p.set_mode(0o700);
        fs::set_permissions(&base, p)?;
        Ok(Self { dir: base })
    }

    fn cred_path(&self) -> PathBuf {
        self.dir.join("credentials.json")
    }

    pub fn load(&self) -> Credentials {
        fs::read(self.cred_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save_token(&self, token: &str) -> Result<()> {
        let mut c = self.load();
        c.token = Some(token.to_string());
        self.write_creds(&c)
    }

    pub fn clear_token(&self) -> Result<()> {
        let mut c = self.load();
        c.token = None;
        self.write_creds(&c)
    }

    pub fn device_id(&self) -> String {
        let mut c = self.load();
        if let Some(id) = c.device_id.clone() {
            return id;
        }
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        let id = hex::encode(bytes);
        c.device_id = Some(id.clone());
        let _ = self.write_creds(&c);
        id
    }

    fn write_creds(&self, c: &Credentials) -> Result<()> {
        let path = self.cred_path();
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)?;
        f.write_all(serde_json::to_vec(c)?.as_slice())?;
        f.flush()?;
        drop(f);
        let p = fs::Permissions::from_mode(0o600);
        fs::set_permissions(&path, p)?;
        Ok(())
    }
}
