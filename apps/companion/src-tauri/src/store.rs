//! Zugangsdaten des gekoppelten Geräts: Einstellungen in einer Datei, das Token im Schlüsselbund des Systems.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tickly_agent::Credentials;

const SERVICE: &str = "de.tickly.companion";

#[derive(Serialize, Deserialize)]
struct Meta {
    server: String,
    device_id: String,
    device_name: String,
    user_name: String,
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: &Path) -> Store {
        let _ = std::fs::create_dir_all(dir);
        Store { dir: dir.to_path_buf() }
    }

    pub fn outbox_path(&self) -> PathBuf {
        self.dir.join("outbox.json")
    }
    /// Lokaler Stand (Einstellungen, Tag), damit auch ein Start ohne Netz alles weiß.
    pub fn agent_path(&self) -> PathBuf {
        self.dir.join("agent.json")
    }
    fn meta_path(&self) -> PathBuf {
        self.dir.join("device.json")
    }
    fn token_file(&self) -> PathBuf {
        self.dir.join("device.token")
    }

    pub fn load(&self) -> Option<Credentials> {
        let m: Meta = serde_json::from_slice(&std::fs::read(self.meta_path()).ok()?).ok()?;
        let token = keyring::Entry::new(SERVICE, &m.device_id).ok().and_then(|e| e.get_password().ok()).or_else(|| std::fs::read_to_string(self.token_file()).ok())?;
        Some(Credentials { server: m.server, token: token.trim().to_string(), device_id: m.device_id, device_name: m.device_name, user_name: m.user_name })
    }

    pub fn save(&self, c: &Credentials) -> Result<(), String> {
        let meta = Meta { server: c.server.clone(), device_id: c.device_id.clone(), device_name: c.device_name.clone(), user_name: c.user_name.clone() };
        std::fs::write(self.meta_path(), serde_json::to_vec_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;
        // Bevorzugt Schlüsselbund; ist der nicht verfügbar, bleibt eine Datei nur für den Benutzer lesbar.
        let ok = keyring::Entry::new(SERVICE, &c.device_id).and_then(|e| e.set_password(&c.token)).is_ok();
        if ok {
            let _ = std::fs::remove_file(self.token_file());
        } else {
            std::fs::write(self.token_file(), &c.token).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(self.token_file(), std::fs::Permissions::from_mode(0o600));
            }
        }
        Ok(())
    }

    pub fn clear(&self) {
        if let Some(c) = self.load() {
            if let Ok(e) = keyring::Entry::new(SERVICE, &c.device_id) {
                let _ = e.delete_credential();
            }
        }
        let _ = std::fs::remove_file(self.meta_path());
        let _ = std::fs::remove_file(self.token_file());
        let _ = std::fs::remove_file(self.outbox_path());
        let _ = std::fs::remove_file(self.agent_path());
    }
}
