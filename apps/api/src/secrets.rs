//! Verschlüsselung ruhender Daten: Sicherungen, TOTP-Geheimnisse und das SMTP-Passwort.
//!
//! Der Schlüssel entsteht aus dem Geheimnis `TICKLY_SECRET` (oder dem Inhalt der Datei `TICKLY_SECRET_FILE`, z. B. ein
//! Docker-Secret). Das Geheimnis liegt **nicht** in der Datenbank und nicht in den Sicherungen, sondern nur in der
//! Umgebung des Servers. Ohne Geheimnis bleiben die Daten unverschlüsselt (und die Oberfläche weist darauf hin).
//! AES-256-GCM, Schlüssel per HKDF-SHA256 abgeleitet, je Wert ein zufälliger 96-Bit-Nonce.
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use sqlx::PgPool;

/// Beginn verschlüsselter Dateien.
pub const MAGIC: &[u8; 4] = b"TKY1";
/// Beginn verschlüsselter Textwerte in der Datenbank.
pub const PREFIX: &str = "enc1:";
pub const MIN_LEN: usize = 24;

#[derive(Clone, Default)]
pub struct Secrets {
    key: Option<[u8; 32]>,
}

impl Secrets {
    pub fn from_env() -> Secrets {
        let raw = std::env::var("TICKLY_SECRET").ok().filter(|s| !s.is_empty()).or_else(|| {
            std::env::var("TICKLY_SECRET_FILE").ok().filter(|p| !p.is_empty()).map(|p| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("TICKLY_SECRET_FILE ({p}) nicht lesbar: {e}")))
        });
        match raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
            Some(s) => Secrets::with(&s),
            None => Secrets::default(),
        }
    }

    pub fn with(secret: &str) -> Secrets {
        assert!(secret.chars().count() >= MIN_LEN, "TICKLY_SECRET ist zu kurz: mindestens {MIN_LEN} Zeichen (z. B. `openssl rand -base64 48`)");
        let mut key = [0u8; 32];
        Hkdf::<Sha256>::new(Some(b"tickly-v1"), secret.as_bytes()).expand(b"data", &mut key).expect("HKDF");
        Secrets { key: Some(key) }
    }

    pub fn active(&self) -> bool {
        self.key.is_some()
    }

    pub fn is_sealed(data: &[u8]) -> bool {
        data.starts_with(MAGIC)
    }

    /// Verschlüsselt; ohne Geheimnis unverändert.
    pub fn seal(&self, data: &[u8]) -> Vec<u8> {
        let Some(k) = &self.key else { return data.to_vec() };
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ct = Aes256Gcm::new(k.into()).encrypt(Nonce::from_slice(&nonce), data).expect("AES-GCM");
        [MAGIC.as_slice(), &nonce, &ct].concat()
    }

    /// Entschlüsselt; unverschlüsselte Daten (ohne Kennung) werden unverändert durchgereicht.
    pub fn open(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if !Self::is_sealed(data) { return Ok(data.to_vec()) }
        let Some(k) = &self.key else { return Err("Die Daten sind verschlüsselt, aber TICKLY_SECRET ist nicht gesetzt".into()) };
        if data.len() < 4 + 12 + 16 { return Err("Verschlüsselte Daten sind beschädigt".into()) }
        Aes256Gcm::new(k.into()).decrypt(Nonce::from_slice(&data[4..16]), &data[16..]).map_err(|_| "Entschlüsselung fehlgeschlagen – falsches TICKLY_SECRET oder beschädigte Daten".to_string())
    }

    pub fn seal_str(&self, s: &str) -> String {
        if !self.active() { return s.to_string() }
        format!("{PREFIX}{}", B64.encode(self.seal(s.as_bytes())))
    }

    pub fn open_str(&self, s: &str) -> Result<String, String> {
        let Some(b) = s.strip_prefix(PREFIX) else { return Ok(s.to_string()) };
        let raw = B64.decode(b).map_err(|_| "Verschlüsselter Wert ist beschädigt".to_string())?;
        String::from_utf8(self.open(&raw)?).map_err(|_| "Entschlüsselter Wert ist kein Text".to_string())
    }
}

/// Beim Start: Passt das Geheimnis zu den vorhandenen Daten? Verschlüsselt Altbestände (z. B. TOTP-Geheimnisse), sobald
/// ein Geheimnis gesetzt wird. Bricht bei einem Widerspruch ab, statt Daten unlesbar zu machen.
pub async fn verify_and_migrate(pool: &PgPool, s: &Secrets) -> Result<(), String> {
    let check: Option<(String,)> = sqlx::query_as("SELECT value #>> '{}' FROM app_settings WHERE key = 'key_check'").fetch_optional(pool).await.map_err(|e| e.to_string())?;
    match (&check, s.active()) {
        (Some((v,)), true) => { s.open_str(v).map_err(|e| format!("TICKLY_SECRET passt nicht zu den vorhandenen Daten. {e}"))?; }
        (Some(_), false) => return Err("Die Daten sind mit TICKLY_SECRET verschlüsselt, aber es ist nicht gesetzt. Ohne das ursprüngliche Geheimnis sind TOTP-Geheimnisse, das SMTP-Passwort und verschlüsselte Sicherungen unlesbar.".into()),
        (None, true) => {
            sqlx::query("INSERT INTO app_settings (key, value) VALUES ('key_check', to_jsonb($1::text))").bind(s.seal_str("tickly")).execute(pool).await.map_err(|e| e.to_string())?;
            // Bisher unverschlüsselte Werte nachträglich schützen
            let rows: Vec<(String, String)> = sqlx::query_as("SELECT id, totp_secret FROM users WHERE totp_secret IS NOT NULL AND totp_secret NOT LIKE 'enc1:%'").fetch_all(pool).await.map_err(|e| e.to_string())?;
            for (id, plain) in rows {
                sqlx::query("UPDATE users SET totp_secret = $2 WHERE id = $1").bind(id).bind(s.seal_str(&plain)).execute(pool).await.map_err(|e| e.to_string())?;
            }
            tracing::info!("Verschlüsselung aktiviert: vorhandene Geheimnisse wurden geschützt");
        }
        (None, false) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper_detection() {
        let s = Secrets::with("ein-ausreichend-langes-geheimnis-42");
        let ct = s.seal(b"hallo");
        assert!(Secrets::is_sealed(&ct) && !ct.windows(5).any(|w| w == b"hallo"));
        assert_eq!(s.open(&ct).unwrap(), b"hallo");
        let mut bad = ct.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(s.open(&bad).is_err());
        assert!(Secrets::with("ein-anderes-ausreichend-langes-geheimnis").open(&ct).is_err());
        assert!(Secrets::default().open(&ct).is_err());
        // Klartext bleibt Klartext
        assert_eq!(s.open(b"{\"a\":1}").unwrap(), b"{\"a\":1}");
        assert_eq!(s.open_str(&s.seal_str("JBSWY3DP")).unwrap(), "JBSWY3DP");
        assert_eq!(Secrets::default().seal_str("x"), "x");
    }
}
