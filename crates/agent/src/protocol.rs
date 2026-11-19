use chrono::{DateTime, NaiveDate, Utc};
use tickly_core::{Away, DayState, Settings};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Ein Ereignis wartet in der Outbox. Die Felder entsprechen dem Format von `POST /api/device/events`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QueuedEvent {
    pub seq: i64,
    pub kind: String,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoTrackPolicy {
    pub enabled: bool,
    pub ask: bool,
}

/// Einstellungen, die der Server dem Companion vorgibt.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Policy {
    pub track_apps: bool,
    pub track_lock: bool,
    pub excluded: Vec<String>,
    pub auto_track: AutoTrackPolicy,
    /// Das Tracking darf auf diesem Gerät von selbst starten (Einstellung je Gerät).
    pub device_auto: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { track_apps: true, track_lock: true, excluded: vec![], auto_track: AutoTrackPolicy::default(), device_auto: true }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct UserInfo {
    pub name: String,
}

/// Pflicht zur Zwei-Faktor-Anmeldung (Passkey oder Authenticator-App) und wie weit der Benutzer ist.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MfaInfo {
    pub required: bool,
    pub compliant: bool,
    pub days_left: Option<i64>,
    pub expired: bool,
}

/// Zustand laut Server (`GET /api/device/state`).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerState {
    pub last_seq: i64,
    pub user: UserInfo,
    /// running | locked | paused | asking | idle | stopped
    pub track: String,
    pub tracking: bool,
    pub since: Option<f64>,
    pub start: Option<f64>,
    /// Zeitpunkt des Rechnerstarts (Minuten seit Mitternacht): so früh beginnt die Zählung bei „ab Rechnerstart“.
    pub boot_at: Option<f64>,
    pub counted: f64,
    pub soll: f64,
    pub diff: f64,
    pub balance: f64,
    pub window_active: bool,
    pub day_off: bool,
    pub holiday: Option<String>,
    pub policy: Policy,
    /// Zeitzone, in der der Server Tage abgrenzt.
    pub tz: String,
    pub today: Option<NaiveDate>,
    /// Alle Einstellungen des Benutzers: damit rechnet der Companion auch ohne Verbindung.
    pub settings: Option<Settings>,
    /// Der Tageszustand laut Server (fehlt, solange heute nichts gemeldet wurde).
    pub day: Option<DayState>,
    pub mfa: Option<MfaInfo>,
    /// Das Tracking läuft über Nacht und passt nicht zu den bisherigen Zeiten: der Benutzer soll es prüfen.
    pub night_check: Option<NightInfo>,
    /// Ein zweiter Companion hat das Tracking gestartet, während dieser lief (oder umgekehrt).
    pub device_clash: Option<ClashInfo>,
    /// Neuere Companion-Version verfügbar (der Server hat sie bei GitHub gefunden und nennt den Installer für dieses System).
    pub update: Option<UpdateInfo>,
    /// Offene Rückfrage nach einer längeren Sperre (Pause oder Arbeitszeit?). Kommt aus dem lokal geführten Tag.
    #[serde(skip_deserializing)]
    pub away: Option<Away>,
}

/// Hinweis auf eine neuere Version. Der Companion geht dafür nicht selbst ins Netz.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateInfo {
    pub latest: String,
    /// Versionshinweise (gekürzt)
    pub notes: String,
    /// Seite des Releases
    pub url: String,
    /// Installer für das Betriebssystem dieses Geräts
    pub download: String,
    pub size: Option<u64>,
}

fn version_parts(v: &str) -> Option<Vec<u64>> {
    let core = v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut p: Vec<u64> = core.split('.').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    p.resize(p.len().max(3), 0);
    Some(p)
}

/// Ist `latest` eine höhere Version als `current` (0.9.9 < 0.9.18)? Unlesbares gilt nicht als neuer.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (version_parts(latest), version_parts(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ClashInfo {
    /// Name des Geräts, das gestartet hat
    pub starter: String,
    /// Name des Geräts, das schon lief
    pub other: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NightInfo {
    /// Seit wann es ohne Unterbrechung läuft (Minuten seit Mitternacht des Vortags)
    pub since: f64,
    pub hours: f64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reply {
    pub last_seq: i64,
    pub state: ServerState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransportError {
    /// Server nicht erreichbar (Netz, Zeitüberschreitung).
    Offline(String),
    /// Token ungültig oder Kopplung aufgehoben.
    Unauthorized,
    /// Der Server hat die Anfrage abgelehnt.
    Server(String),
}

pub trait Transport: Send {
    fn send(&mut self, batch: &[QueuedEvent], timeout: Duration) -> Result<Reply, TransportError>;
    fn state(&mut self, timeout: Duration) -> Result<ServerState, TransportError>;
    /// Stellt für dieses Gerät ein, ob das Tracking von selbst startet; liefert den neuen Zustand.
    fn set_auto_track(&mut self, _on: bool, _timeout: Duration) -> Result<ServerState, TransportError> {
        Err(TransportError::Server("Wird von dieser Verbindung nicht unterstützt".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versionsvergleich() {
        assert!(is_newer("0.9.18", "0.9.9"));
        assert!(!is_newer("0.9.9", "0.9.18"));
        assert!(!is_newer("0.9.18", "0.9.18"));
        assert!(is_newer("v1.0.0", "0.9.18"));
        assert!(is_newer("1.0.1", "1.0"));
        assert!(!is_newer("", "0.9.18"));
        assert!(!is_newer("0.9.18", "unbekannt"));
    }

    #[test]
    fn update_im_zustand() {
        let s: ServerState = serde_json::from_str(r#"{"track":"idle","update":{"latest":"0.9.19","notes":"Neu","url":"https://x/r","download":"https://x/a.exe","size":12}}"#).unwrap();
        let u = s.update.clone().unwrap();
        assert_eq!((u.latest.as_str(), u.download.as_str(), u.size), ("0.9.19", "https://x/a.exe", Some(12)));
        // der gespeicherte Stand behält die Information (offline weiterhin sichtbar)
        let back: ServerState = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.update, s.update);
        // ältere Server kennen das Feld nicht, `null` bedeutet „kein Update“
        assert!(serde_json::from_str::<ServerState>(r#"{"track":"idle"}"#).unwrap().update.is_none());
        assert!(serde_json::from_str::<ServerState>(r#"{"update":null}"#).unwrap().update.is_none());
    }
}
