use chrono::{DateTime, NaiveDate, Utc};
use tickly_core::{DayState, Settings};
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
    pub autostart: bool,
    pub auto_track: AutoTrackPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Policy { track_apps: true, track_lock: true, excluded: vec![], autostart: true, auto_track: AutoTrackPolicy::default() }
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
}
