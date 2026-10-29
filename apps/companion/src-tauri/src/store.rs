//! Zugangsdaten des gekoppelten Geräts: Einstellungen in einer Datei. Das Token liegt unter Windows in der
//! Anmeldeinformationsverwaltung, unter macOS in einer Datei, die nur der Benutzer lesen kann (kein Schlüsselbund: er fragt bei
//! einer unsignierten App ständig nach).
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tickly_agent::Credentials;

#[cfg(windows)]
const SERVICE: &str = "de.tickly.companion";

/// Token im Windows-Tresor ablegen bzw. lesen/löschen; auf anderen Systemen gibt es das nicht (dort bleibt die Datei).
#[cfg(windows)]
fn vault_get(id: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, id).ok().and_then(|e| e.get_password().ok())
}
#[cfg(not(windows))]
fn vault_get(_id: &str) -> Option<String> {
    None
}
#[cfg(windows)]
fn vault_set(id: &str, token: &str) -> bool {
    keyring::Entry::new(SERVICE, id).and_then(|e| e.set_password(token)).is_ok()
}
#[cfg(not(windows))]
fn vault_set(_id: &str, _token: &str) -> bool {
    false
}
#[cfg(windows)]
fn vault_delete(id: &str) {
    if let Ok(e) = keyring::Entry::new(SERVICE, id) {
        let _ = e.delete_credential();
    }
}
#[cfg(not(windows))]
fn vault_delete(_id: &str) {}

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
        let token = vault_get(&m.device_id).or_else(|| std::fs::read_to_string(self.token_file()).ok())?;
        Some(Credentials { server: m.server, token: token.trim().to_string(), device_id: m.device_id, device_name: m.device_name, user_name: m.user_name })
    }

    pub fn save(&self, c: &Credentials) -> Result<(), String> {
        let meta = Meta { server: c.server.clone(), device_id: c.device_id.clone(), device_name: c.device_name.clone(), user_name: c.user_name.clone() };
        std::fs::write(self.meta_path(), serde_json::to_vec_pretty(&meta).unwrap()).map_err(|e| e.to_string())?;
        // Windows: Anmeldeinformationsverwaltung; sonst (oder wenn sie nicht verfügbar ist) eine Datei nur für den Benutzer.
        if vault_set(&c.device_id, &c.token) {
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
            vault_delete(&c.device_id);
        }
        let _ = std::fs::remove_file(self.meta_path());
        let _ = std::fs::remove_file(self.token_file());
        let _ = std::fs::remove_file(self.outbox_path());
        let _ = std::fs::remove_file(self.agent_path());
    }
}
