use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Pausen werden automatisch abgezogen.
    Net,
    /// Die gesamte Anwesenheit zählt.
    Gross,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreakRule {
    /// Ab so vielen Stunden Arbeitszeit …
    pub after: f64,
    /// … werden so viele Minuten Pause abgezogen.
    pub min: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Tagessoll in Stunden, Montag bis Sonntag.
    pub hours: [f64; 7],
    pub mode: Mode,
    pub rules: Vec<BreakRule>,
    /// Gesperrte Zeit (Bildschirmsperre) als echte Pause werten.
    pub lock_as_break: bool,
    /// Mindestdauer (Minuten) einer Sperre, damit sie als Pause zählt.
    pub chunk: f64,
    /// Bundesland-Kürzel, z. B. "NW".
    pub state: String,
    /// Übertrag zum Start in Stunden.
    pub start_bal: f64,
    pub idle: f64,
    pub track_apps: bool,
    pub track_titles: bool,
    pub track_lock: bool,
    pub autostart: bool,
    pub excluded: Vec<String>,
    pub pw_login: bool,
    /// Tracking in bestimmten Zeitfenstern automatisch starten.
    #[serde(default)]
    pub auto_track: AutoTrack,
    /// Freie Tage, die keine gesetzlichen Feiertage sind (Rosenmontag, Heiligabend, …).
    #[serde(default)]
    pub extra_days: ExtraDays,
    /// Darstellung: "system" (folgt dem Betriebssystem), "light" oder "dark".
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "system".into()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtraDays {
    pub rosenmontag: bool,
    pub heiligabend: bool,
    pub silvester: bool,
    /// Jährlich wiederkehrende eigene freie Tage.
    #[serde(default)]
    pub custom: Vec<CustomDay>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomDay {
    pub month: u32,
    pub day: u32,
    pub name: String,
}

/// Ein Zeitfenster (Minuten seit Mitternacht) an bestimmten Wochentagen, Montag = Index 0.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoWindow {
    pub days: [bool; 7],
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutoTrack {
    /// Beim Rechnerstart innerhalb eines Zeitfensters startet das Tracking von selbst.
    pub enabled: bool,
    /// Außerhalb der Zeitfenster fragt der Companion, ob gerade gearbeitet wird.
    pub ask: bool,
    pub windows: Vec<AutoWindow>,
}

impl Default for AutoTrack {
    fn default() -> Self {
        AutoTrack { enabled: true, ask: true, windows: vec![AutoWindow { days: [true, true, true, true, true, false, false], from: 360.0, to: 600.0 }] }
    }
}

impl AutoTrack {
    /// Beginn des Zeitfensters, in dem `now` am Wochentag `wd` (Montag = 0) liegt.
    pub fn window_start(&self, wd: usize, now: f64) -> Option<f64> {
        self.windows.iter().find(|w| w.days[wd % 7] && w.from <= now && now < w.to).map(|w| w.from)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hours: [8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0],
            mode: Mode::Net,
            rules: vec![BreakRule { after: 6.0, min: 30.0 }, BreakRule { after: 9.0, min: 45.0 }],
            lock_as_break: true,
            chunk: 15.0,
            state: "NW".into(),
            start_bal: 4.5,
            idle: 10.0,
            track_apps: true,
            track_titles: false,
            track_lock: true,
            autostart: true,
            excluded: vec!["KeePass".into(), "1Password".into()],
            pw_login: true,
            auto_track: AutoTrack::default(),
            extra_days: ExtraDays::default(),
            theme: default_theme(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AbsType {
    Vacation,
    Sick,
    /// Zeitausgleich: Soll bleibt bestehen und baut Überstunden ab.
    Comp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Absence {
    #[serde(default)]
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: AbsType,
    pub from: NaiveDate,
    pub to: NaiveDate,
    #[serde(default)]
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DayKind {
    Holiday,
    Vacation,
    Sick,
    Comp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SegKind {
    App,
    Locked,
    /// Companion war nicht aktiv – wird nicht gezählt.
    Off,
}

/// Ein Zeitabschnitt in Minuten seit Mitternacht. `to == None` = läuft noch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seg {
    pub from: f64,
    pub to: Option<f64>,
    pub kind: SegKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Notiz bei manuell eingetragener Zeit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Seg {
    pub fn app(from: f64, to: f64, app: &str) -> Seg {
        Seg { from, to: Some(to), kind: SegKind::App, app: Some(app.into()), note: None }
    }
    pub fn locked(from: f64, to: f64) -> Seg {
        Seg { from, to: Some(to), kind: SegKind::Locked, app: None, note: None }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub t: f64,
    pub k: String,
    pub txt: String,
}

/// Warum gerade nicht getrackt wird.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stop {
    /// Companion richtig beendet.
    Quit,
    /// Rechner herunterfahren/neu starten, während getrackt wurde: wird beim nächsten Start fortgesetzt.
    Shutdown,
    /// Im Companion manuell gestoppt, die App läuft weiter.
    Pause,
    /// Companion läuft, fragt ob gearbeitet wird.
    Ask,
    /// Companion läuft, Tracking bewusst aus.
    Idle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayState {
    /// Zeitpunkt des Rechnerstarts.
    pub start: f64,
    pub tracking: bool,
    /// Beginn der aktuellen Lücke (Ende, Pause).
    pub ended: Option<f64>,
    pub last_app: String,
    pub segs: Vec<Seg>,
    pub events: Vec<Event>,
    #[serde(default)]
    pub stop: Option<Stop>,
    /// Zeitpunkt, zu dem die Rückfrage mit „Nein“ beantwortet wurde.
    #[serde(default)]
    pub declined: Option<f64>,
    /// Letztes Lebenszeichen des Companion (Minuten). Daraus ergibt sich das Ende nach einem Absturz.
    #[serde(default)]
    pub last_beat: Option<f64>,
    /// Zeiten wurden nachträglich bearbeitet oder ergänzt.
    #[serde(default)]
    pub edited: bool,
    /// Der Tag begann mit einem Tracking, das über Mitternacht weiterlief.
    #[serde(default)]
    pub carried: bool,
    /// Der Benutzer hat bestätigt, dass er über Mitternacht wirklich arbeitet (keine Rückfrage mehr).
    #[serde(default)]
    pub night_ok: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayInfo {
    pub holiday: Option<String>,
    pub absence: Option<Absence>,
    pub soll: f64,
    pub off: bool,
    pub kind: Option<DayKind>,
    pub base: f64,
    pub weekend: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStat {
    pub date: NaiveDate,
    pub info: DayInfo,
    pub soll: f64,
    pub has: bool,
    pub live: bool,
    pub future: bool,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub gross: f64,
    pub locked: f64,
    pub rule: f64,
    pub locked_break: f64,
    /// Tatsächlich abgezogene Pause (nur im Netto-Modus relevant für `counted`).
    pub brk: f64,
    pub net: f64,
    /// Gezählte Arbeitszeit je nach Modus.
    pub counted: f64,
    /// Für den Saldo verbucht (heute offen: nie negativ).
    pub diff: Option<f64>,
    /// Unverbuchte Abweichung (heute live).
    pub diff_live: Option<f64>,
    pub segs: Option<Vec<Seg>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VacationSummary {
    pub total: u32,
    pub taken: u32,
    pub planned: u32,
    pub rest: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HolidayEntry {
    pub date: NaiveDate,
    pub name: String,
    /// Kein gesetzlicher Feiertag, sondern ein eigener freier Tag.
    pub extra: bool,
}
