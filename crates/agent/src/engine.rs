//! Die Tracking-Schleife des Companion.
//!
//! Der Companion führt den Tageszustand **selbst mit**, mit derselben Logik wie der Server (`tickly_core::apply_event`).
//! Dadurch funktioniert alles auch ohne Verbindung (Zug, Kunde ohne WLAN): Rückfrage, Zeitfenster, Pause, Sperre,
//! App-Wechsel und die Anzeige der Arbeitszeit. Jedes Ereignis geht mit seinem echten Zeitpunkt in die Outbox und wird
//! später nachgesendet. Der Server bleibt maßgeblich: Nach erfolgreicher Übertragung übernimmt der Companion seinen Stand.
use crate::outbox::Outbox;
use crate::protocol::{ServerState, Transport, TransportError};
use chrono::{DateTime, Datelike, Duration as Cd, NaiveDate, Timelike, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};
use tickly_core::{apply_event, calc_segs, policy_for, resolve, DayState, Settings, Track};
use tickly_platform::{Platform, SysEvent};

/// Abstand der Lebenszeichen. Daraus ergibt sich bei einem Absturz das Ende der Zeiterfassung.
const BEAT_EVERY: i64 = 30;
/// Größerer Sprung der Uhr zwischen zwei Messungen: der Rechner war im Ruhezustand.
const SLEEP_GAP: i64 = 90;
const BATCH: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Pause,
    Resume,
    /// „work“, „boot“ (ab Rechnerstart) oder „no“
    Answer(String),
}

/// Was die Oberfläche anzeigt.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub connected: bool,
    /// Kopplung wurde aufgehoben, der Companion muss neu gekoppelt werden.
    pub unauthorized: bool,
    pub error: Option<String>,
    /// So viele Ereignisse warten auf die Übertragung.
    pub pending: usize,
    /// Zahlen stammen aus lokaler Rechnung statt vom Server (ohne Verbindung): das Überstundenkonto ist dann geschätzt.
    pub estimated: bool,
    pub locked: bool,
    pub app: Option<String>,
    /// Zeitpunkt (Unix-Millisekunden), auf den sich `state.counted` bezieht. Die Oberfläche zählt von hier aus weiter,
    /// damit die Anzeige nicht von der Dauer einer Übertragung abhängt.
    pub as_of: i64,
    pub state: ServerState,
}

/// Auf der Platte gesicherter Stand, damit auch ein Start ohne Netz alles weiß.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    state: ServerState,
    date: Option<NaiveDate>,
    day: Option<DayState>,
}

pub struct Engine<T: Transport> {
    platform: Box<dyn Platform>,
    transport: T,
    outbox: Outbox,
    store: Option<PathBuf>,
    /// Letzter Stand vom Server (Einstellungen, Zeitzone, Salden)
    state: ServerState,
    /// Lokal mitgeführter Tag
    date: Option<NaiveDate>,
    day: Option<DayState>,
    connected: bool,
    unauthorized: bool,
    error: Option<String>,
    locked: bool,
    app: Option<String>,
    last_app_sent: Option<String>,
    last_tick: Option<DateTime<Utc>>,
    last_beat: Option<DateTime<Utc>>,
    retry_at: Option<DateTime<Utc>>,
    clock: DateTime<Utc>,
    backoff: i64,
    finished: bool,
}

impl<T: Transport> Engine<T> {
    pub fn new(platform: Box<dyn Platform>, transport: T, outbox: Outbox) -> Engine<T> {
        Engine {
            platform, transport, outbox, store: None, state: ServerState::default(), date: None, day: None, connected: false,
            unauthorized: false, error: None, locked: false, app: None, last_app_sent: None, last_tick: None, last_beat: None, retry_at: None,
            clock: Utc::now(), backoff: 2, finished: false,
        }
    }

    /// Stand auf der Platte führen (Einstellungen, Tag), damit er einen Neustart ohne Netz übersteht.
    pub fn with_store(mut self, path: PathBuf) -> Self {
        if let Some(s) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Saved>(&b).ok()) {
            self.state = s.state;
            self.date = s.date;
            self.day = s.day;
        }
        self.store = Some(path);
        self
    }

    fn save(&self) {
        let Some(p) = &self.store else { return };
        let s = Saved { state: self.state.clone(), date: self.date, day: self.day.clone() };
        let tmp = p.with_extension("tmp");
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&tmp, serde_json::to_vec(&s).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }

    fn tz(&self) -> Tz {
        self.state.tz.parse().unwrap_or(chrono_tz::Europe::Berlin)
    }
    fn settings(&self) -> Settings {
        self.state.settings.clone().unwrap_or_default()
    }
    /// Lokales Datum und Minuten seit Mitternacht in der Zeitzone des Servers.
    fn local(&self, at: DateTime<Utc>) -> (NaiveDate, f64) {
        let l = at.with_timezone(&self.tz());
        (l.date_naive(), l.num_seconds_from_midnight() as f64 / 60.0)
    }
    fn track(&self) -> Option<Track> {
        self.day.as_ref().map(|d| d.track())
    }

    pub fn view(&self) -> View {
        self.view_at(self.clock)
    }

    /// Anzeige für den Zeitpunkt `at` (die Oberfläche fragt mit der aktuellen Uhrzeit, nicht mit der des letzten Ticks:
    /// ein Tick mit langsamer Übertragung würde sonst die Zeit zurückspringen lassen).
    pub fn view_at(&self, at: DateTime<Utc>) -> View {
        let mut st = self.state.clone();
        let (date, now) = self.local(at);
        if let Some(d) = &self.day {
            let s = self.settings();
            let k = calc_segs(&resolve(&d.segs, now), &s);
            let same_day = st.today == self.date;
            st.track = d.track().as_str().to_string();
            st.tracking = d.tracking;
            st.since = d.segs.last().map(|g| g.from);
            st.start = d.segs.first().map(|g| g.from);
            // Soll laut Server; für einen Tag, den der Server noch nicht kennt, aus den Einstellungen
            let soll = if same_day { st.soll } else if st.day_off { 0.0 } else { s.hours[date.weekday().num_days_from_monday() as usize] * 60.0 };
            st.balance = if same_day { st.balance + (k.counted - st.counted) } else { st.balance + k.counted - soll };
            st.counted = k.counted;
            st.soll = soll;
            st.diff = k.counted - soll;
            st.window_active = policy_for(&s.auto_track, st.day_off, date, now).auto_from.is_some();
        } else {
            st.track = "stopped".into();
        }
        View { connected: self.connected, unauthorized: self.unauthorized, error: self.error.clone(), pending: self.outbox.len(), estimated: !self.connected, locked: self.locked, app: self.app.clone(), as_of: at.timestamp_millis(), state: st }
    }

    /// Companion gestartet (Anmeldung am Rechner oder manueller Start).
    pub fn start(&mut self, now: DateTime<Utc>, manual: bool) {
        self.clock = now;
        // Nummern abgleichen, falls die Outbox-Datei fehlte; gleichzeitig kennen wir damit Einstellungen und Tag.
        match self.transport.state(Duration::from_secs(8)) {
            Ok(s) => {
                self.outbox.ensure_seq_after(s.last_seq);
                let all_sent = self.outbox.is_empty();
                self.set_state(s, all_sent);
            }
            Err(e) => self.on_error(now, e),
        }
        self.locked = self.platform.is_locked();
        self.last_tick = Some(now);
        self.last_beat = Some(now);
        self.emit("boot", now, None, None);
        self.flush(now, true);
        // Manuell geöffnet heißt: ich will arbeiten. Eine Rückfrage erübrigt sich.
        if manual && self.track() == Some(Track::Asking) {
            self.emit("answer", now, None, Some("work".into()));
            self.flush(now, true);
        }
    }

    /// Regelmäßig aufrufen (alle paar Sekunden).
    pub fn tick(&mut self, now: DateTime<Utc>) {
        self.clock = now;
        if self.finished || self.unauthorized {
            return;
        }
        // Herunterfahren hat Vorrang vor allem anderen
        for e in self.platform.drain_events() {
            match e {
                SysEvent::Shutdown => return self.finish(now, "shutdown"),
            }
        }
        let track_lock = self.state.policy.track_lock;
        // Große Lücke zwischen zwei Messungen: Der Rechner schlief. Das zählt wie eine Sperre.
        if let Some(prev) = self.last_tick {
            if now - prev > Cd::seconds(SLEEP_GAP) && track_lock && !self.locked && self.emit("lock", prev, None, None) {
                self.locked = true;
            }
        }
        self.last_tick = Some(now);

        if track_lock {
            let locked = self.platform.is_locked();
            if locked != self.locked {
                self.emit(if locked { "lock" } else { "unlock" }, now, None, None);
                self.locked = locked;
            }
        }
        if !self.locked {
            self.sample_app(now);
        }
        if self.last_beat.map_or(true, |b| now - b >= Cd::seconds(BEAT_EVERY)) {
            self.emit("beat", now, None, None);
            self.last_beat = Some(now);
        }
        self.flush(now, false);
    }

    /// Benutzeraktion aus der Oberfläche (wirkt sofort, auch ohne Verbindung).
    pub fn action(&mut self, now: DateTime<Utc>, a: Action) {
        self.clock = now;
        match a {
            Action::Pause => self.emit("pause", now, None, None),
            Action::Resume => self.emit("start", now, None, None),
            Action::Answer(v) => self.emit("answer", now, None, Some(v)),
        };
        self.last_app_sent = None;
        self.flush(now, true);
    }

    /// Companion wird beendet („quit“) oder der Rechner fährt herunter („shutdown“). Danach reagiert die Engine nicht mehr.
    pub fn finish(&mut self, now: DateTime<Utc>, kind: &str) {
        self.clock = now;
        if self.finished {
            return;
        }
        self.finished = true;
        self.emit(kind, now, None, None);
        // Beim Herunterfahren bleibt nur wenig Zeit; was nicht ankommt, liegt in der Outbox und geht beim nächsten Start raus.
        self.flush_with(now, true, Duration::from_secs(if kind == "shutdown" { 2 } else { 4 }));
    }

    /// Ereignis lokal anwenden und, wenn es zum Zustand passt, für den Server vormerken.
    fn emit(&mut self, kind: &str, at: DateTime<Utc>, app: Option<String>, answer: Option<String>) -> bool {
        let (date, now) = self.local(at);
        let os = if self.platform.os() == "macos" { "macOS" } else { "Windows" };
        if self.date != Some(date) {
            // Neuer Tag: lief das Tracking um Mitternacht, geht es nahtlos weiter (wie beim Server)
            let carry = match (&self.day, self.date) {
                (Some(d), Some(prev)) if kind != "boot" && d.tracking && d.stop.is_none() && prev.succ_opt() == Some(date) => Some(d.continued(os)),
                _ => None,
            };
            self.day = carry;
            self.date = Some(date);
            // Ob der neue Tag frei ist, weiß der Server; bis dahin gilt „normaler Tag“
            if self.state.today != Some(date) {
                self.state.day_off = false;
            }
        }
        let p = policy_for(&self.settings().auto_track, self.state.day_off, date, now);
        match apply_event(&mut self.day, kind, now, os, &p, app.as_deref(), answer.as_deref()) {
            Ok(()) => {
                self.outbox.push(kind, at, app, Some(self.platform.os().to_string()), answer);
                self.save();
                true
            }
            // passt nicht zum Zustand (z. B. „entsperren“ ohne Sperre): gar nicht erst senden
            Err(_) => false,
        }
    }

    fn running(&self) -> bool {
        self.track() == Some(Track::Running)
    }

    fn sample_app(&mut self, now: DateTime<Utc>) {
        if !self.state.policy.track_apps || !self.running() {
            self.last_app_sent = None;
            return;
        }
        // macOS liefert übersetzte Namen mit weichen Trennstrichen („System­einstellungen“)
        let Some(raw) = self.platform.foreground_app().map(|n| n.replace(['\u{ad}', '\u{200b}'], "").trim().to_string()).filter(|n| !n.is_empty()) else { return };
        // Die eigene Oberfläche ist keine Arbeit: bleibt bei der zuletzt gemeldeten App.
        if raw.to_lowercase().contains("tickly") {
            return;
        }
        let ex = raw.to_lowercase();
        let name = if self.state.policy.excluded.iter().any(|e| !e.trim().is_empty() && ex.contains(&e.to_lowercase())) { "other".to_string() } else { raw };
        self.app = Some(name.clone());
        if self.last_app_sent.as_deref() != Some(name.as_str()) {
            self.emit("app", now, Some(name.clone()), None);
            self.last_app_sent = Some(name);
        }
    }

    fn flush(&mut self, now: DateTime<Utc>, force: bool) {
        self.flush_with(now, force, Duration::from_secs(10));
    }

    fn flush_with(&mut self, now: DateTime<Utc>, force: bool, timeout: Duration) {
        if self.outbox.is_empty() || self.unauthorized {
            return;
        }
        if !force && self.retry_at.is_some_and(|t| now < t) {
            return;
        }
        let batch = self.outbox.batch(BATCH);
        match self.transport.send(&batch, timeout) {
            Ok(r) => {
                self.outbox.ack(r.last_seq.max(batch.last().map_or(0, |e| e.seq)));
                self.error = None;
                self.backoff = 2;
                self.retry_at = None;
                // Alles übertragen: ab jetzt gilt der Stand des Servers
                let all_sent = self.outbox.is_empty();
                self.set_state(r.state, all_sent);
                // mehr als eine Sendung offen: gleich weiter
                if !all_sent {
                    self.flush_with(now, true, timeout);
                }
            }
            Err(e) => self.on_error(now, e),
        }
    }

    fn set_state(&mut self, s: ServerState, adopt_day: bool) {
        self.connected = true;
        if adopt_day {
            if let Some(d) = &s.day {
                self.day = Some(d.clone());
                self.date = s.today.or(self.date);
            }
        }
        self.state = s;
        self.save();
    }

    fn on_error(&mut self, now: DateTime<Utc>, e: TransportError) {
        match e {
            TransportError::Offline(m) => {
                self.connected = false;
                self.error = Some(m);
                self.retry_at = Some(now + Cd::seconds(self.backoff));
                self.backoff = (self.backoff * 2).min(60);
            }
            TransportError::Unauthorized => {
                self.unauthorized = true;
                self.error = Some("Die Kopplung wurde aufgehoben. Bitte das Gerät neu koppeln.".into());
            }
            TransportError::Server(m) => {
                // z. B. Konto gesperrt: erneut versuchen, aber nicht im Sekundentakt
                self.connected = true;
                self.error = Some(m);
                self.retry_at = Some(now + Cd::seconds(30));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{QueuedEvent, Reply};
    use chrono::Timelike;
    use tickly_core::{apply_event, policy_for};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct World {
        app: Option<String>,
        locked: bool,
        sys: Vec<SysEvent>,
        online: bool,
        unauthorized: bool,
        track: String,
        server_last_seq: i64,
        received: Vec<QueuedEvent>,
    }
    type W = Arc<Mutex<World>>;

    struct FakePlatform(W);
    impl Platform for FakePlatform {
        fn os(&self) -> &'static str { "macos" }
        fn foreground_app(&mut self) -> Option<String> { self.0.lock().unwrap().app.clone() }
        fn is_locked(&mut self) -> bool { self.0.lock().unwrap().locked }
        fn drain_events(&mut self) -> Vec<SysEvent> { std::mem::take(&mut self.0.lock().unwrap().sys) }
    }

    struct FakeServer(W);
    impl FakeServer {
        fn st(w: &World) -> ServerState {
            ServerState { track: w.track.clone(), last_seq: w.server_last_seq, ..Default::default() }
        }
    }
    impl Transport for FakeServer {
        fn send(&mut self, batch: &[QueuedEvent], _t: Duration) -> Result<Reply, TransportError> {
            let mut w = self.0.lock().unwrap();
            if w.unauthorized { return Err(TransportError::Unauthorized) }
            if !w.online { return Err(TransportError::Offline("offline".into())) }
            for e in batch {
                if e.seq > w.server_last_seq {
                    w.server_last_seq = e.seq;
                    w.received.push(e.clone());
                    // sehr einfaches Server-Modell
                    match e.kind.as_str() {
                        "boot" => w.track = "running".into(),
                        "pause" => w.track = "paused".into(),
                        "start" => w.track = "running".into(),
                        "lock" => w.track = "locked".into(),
                        "unlock" => w.track = "running".into(),
                        _ => {}
                    }
                }
            }
            Ok(Reply { last_seq: w.server_last_seq, state: Self::st(&w) })
        }
        fn state(&mut self, _t: Duration) -> Result<ServerState, TransportError> {
            let w = self.0.lock().unwrap();
            if !w.online { return Err(TransportError::Offline("offline".into())) }
            Ok(Self::st(&w))
        }
    }

    fn t(sec: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-26T08:00:00Z").unwrap().with_timezone(&Utc) + Cd::seconds(sec)
    }
    fn setup() -> (Engine<FakeServer>, W) {
        let w: W = Arc::new(Mutex::new(World { online: true, track: "stopped".into(), app: Some("Code".into()), ..Default::default() }));
        let e = Engine::new(Box::new(FakePlatform(w.clone())), FakeServer(w.clone()), Outbox::in_memory());
        (e, w)
    }
    fn kinds(w: &W) -> Vec<String> {
        w.lock().unwrap().received.iter().filter(|e| e.kind != "beat").map(|e| match &e.app { Some(a) => format!("{}:{a}", e.kind), None => e.kind.clone() }).collect()
    }

    #[test]
    fn boot_then_app_changes_are_reported_once() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        e.tick(t(2));
        e.tick(t(4));
        w.lock().unwrap().app = Some("Google Chrome".into());
        e.tick(t(6));
        e.tick(t(8));
        assert_eq!(kinds(&w), ["boot", "app:Code", "app:Google Chrome"]);
        assert_eq!(e.view().app.as_deref(), Some("Google Chrome"));
    }

    #[test]
    fn the_view_follows_the_clock_it_is_asked_for() {
        let (mut e, _w) = setup();
        e.start(t(0), false);
        e.tick(t(60));
        assert_eq!(e.view().state.track, "running");
        // Die Anzeige hängt nicht am letzten Tick: wer später fragt, bekommt die spätere Zeit, mit passendem Bezugszeitpunkt
        let (a, b) = (e.view_at(t(60)), e.view_at(t(65)));
        assert_eq!(b.as_of - a.as_of, 5000);
        assert!((b.state.counted - a.state.counted - 5.0 / 60.0).abs() < 1e-6, "{} → {}", a.state.counted, b.state.counted);
    }

    #[test]
    fn own_window_is_not_work_and_excluded_apps_stay_anonymous() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        e.state.policy.excluded = vec!["keepass".into()];
        w.lock().unwrap().app = Some("Tickly Companion".into());
        e.tick(t(2));
        w.lock().unwrap().app = Some("KeePassXC".into());
        e.tick(t(4));
        assert_eq!(kinds(&w), ["boot", "app:other"]);
    }

    #[test]
    fn lock_and_unlock_are_reported_and_no_apps_while_locked() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        e.tick(t(2));
        w.lock().unwrap().locked = true;
        w.lock().unwrap().app = Some("LockApp".into());
        e.tick(t(4));
        w.lock().unwrap().locked = false;
        w.lock().unwrap().app = Some("Code".into());
        e.tick(t(6));
        assert_eq!(kinds(&w), ["boot", "app:Code", "lock", "unlock"]);
    }

    #[test]
    fn sleeping_laptop_counts_as_locked_from_the_last_measurement() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        e.tick(t(2));
        // 40 Minuten später (Deckel war zu), jetzt entsperrt
        e.tick(t(2 + 2400));
        let got: Vec<_> = w.lock().unwrap().received.iter().filter(|e| e.kind == "lock" || e.kind == "unlock").map(|e| (e.kind.clone(), e.at)).collect();
        assert_eq!(got, vec![("lock".to_string(), t(2)), ("unlock".to_string(), t(2402))]);
    }

    #[test]
    fn offline_events_are_kept_in_order_and_sent_later() {
        let (mut e, w) = setup();
        w.lock().unwrap().online = false;
        e.start(t(0), false);
        e.tick(t(2));
        w.lock().unwrap().app = Some("Teams".into());
        e.tick(t(4));
        assert!(!e.view().connected);
        assert!(e.view().pending >= 3);
        // Netz kommt wieder, nach der Wartezeit geht alles raus
        w.lock().unwrap().online = true;
        e.tick(t(80));
        assert!(e.view().connected && e.view().pending == 0, "{:?}", e.view());
        assert_eq!(kinds(&w), ["boot", "app:Code", "app:Teams"]);
        let seqs: Vec<i64> = w.lock().unwrap().received.iter().map(|e| e.seq).collect();
        assert!(seqs.windows(2).all(|p| p[0] < p[1]));
    }

    #[test]
    fn beats_do_not_pile_up_while_offline() {
        let (mut e, w) = setup();
        w.lock().unwrap().online = false;
        e.start(t(0), false);
        for i in 1..=40 { e.tick(t(i * 31)); }
        let beats = e.outbox.batch(1000).iter().filter(|x| x.kind == "beat").count();
        assert!(beats <= 1, "{beats}");
    }

    #[test]
    fn shutdown_is_sent_immediately_and_stops_the_engine() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        w.lock().unwrap().sys.push(SysEvent::Shutdown);
        e.tick(t(10));
        assert_eq!(kinds(&w).last().map(String::as_str), Some("shutdown"));
        w.lock().unwrap().app = Some("Teams".into());
        e.tick(t(12));
        assert_eq!(kinds(&w).last().map(String::as_str), Some("shutdown"), "danach nichts mehr");
    }

    #[test]
    fn shutdown_without_network_stays_queued() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        w.lock().unwrap().online = false;
        e.finish(t(10), "shutdown");
        assert_eq!(e.view().pending, 1);
    }

    #[test]
    fn revoked_pairing_stops_everything() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        w.lock().unwrap().unauthorized = true;
        e.action(t(5), Action::Pause);
        assert!(e.view().unauthorized);
        let n = w.lock().unwrap().received.len();
        e.tick(t(10));
        assert_eq!(w.lock().unwrap().received.len(), n);
    }

    #[test]
    fn sequence_numbers_never_restart_below_the_servers() {
        let (mut e, w) = setup();
        w.lock().unwrap().server_last_seq = 500;
        e.start(t(0), false);
        assert!(w.lock().unwrap().received.iter().all(|x| x.seq > 500));
    }

    #[test]
    fn user_actions_go_out_immediately() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        e.action(t(5), Action::Pause);
        assert_eq!(e.view().state.track, "paused");
        e.action(t(9), Action::Resume);
        assert_eq!(e.view().state.track, "running");
        assert_eq!(kinds(&w), ["boot", "pause", "start"]);
    }

    #[test]
    fn apps_are_resent_after_tracking_resumes() {
        let (mut e, w) = setup();
        e.start(t(0), false);
        e.tick(t(2));
        e.action(t(5), Action::Pause);
        e.tick(t(7));
        w.lock().unwrap().app = Some("Teams".into());
        e.tick(t(9));
        e.action(t(12), Action::Resume);
        e.tick(t(14));
        let k = kinds(&w);
        assert_eq!(k.last().map(String::as_str), Some("app:Teams"), "{k:?}");
    }

    /// Server, der Ereignisse wie der echte mit `tickly_core::apply_event` verarbeitet.
    struct ReplayServer(Arc<Mutex<Replay>>);
    struct Replay {
        online: bool,
        last_seq: i64,
        day: Option<DayState>,
        settings: Settings,
    }
    impl ReplayServer {
        fn state(r: &Replay) -> ServerState {
            ServerState {
                last_seq: r.last_seq, tz: "Europe/Berlin".into(), settings: Some(r.settings.clone()), day: r.day.clone(),
                today: Some(NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()), soll: 480.0, ..Default::default()
            }
        }
    }
    impl Transport for ReplayServer {
        fn send(&mut self, batch: &[QueuedEvent], _t: Duration) -> Result<Reply, TransportError> {
            let mut r = self.0.lock().unwrap();
            if !r.online { return Err(TransportError::Offline("offline".into())) }
            let from = r.last_seq;
            for e in batch.iter().filter(|e| e.seq > from) {
                r.last_seq = e.seq;
                let l = e.at.with_timezone(&chrono_tz::Europe::Berlin);
                let now = l.num_seconds_from_midnight() as f64 / 60.0;
                let p = policy_for(&r.settings.auto_track, false, l.date_naive(), now);
                let mut day = r.day.take();
                let _ = apply_event(&mut day, &e.kind, now, "macOS", &p, e.app.as_deref(), e.answer.as_deref());
                r.day = day;
            }
            Ok(Reply { last_seq: r.last_seq, state: Self::state(&r) })
        }
        fn state(&mut self, _t: Duration) -> Result<ServerState, TransportError> {
            let r = self.0.lock().unwrap();
            if !r.online { return Err(TransportError::Offline("offline".into())) }
            Ok(Self::state(&r))
        }
    }

    fn replay_world(ask: bool, auto: bool) -> Arc<Mutex<Replay>> {
        let mut settings = Settings::default();
        settings.auto_track.ask = ask;
        settings.auto_track.enabled = auto;
        Arc::new(Mutex::new(Replay { online: true, last_seq: 0, day: None, settings }))
    }
    fn pc(app: &str) -> (Arc<Mutex<(Option<String>, bool)>>, Box<dyn Platform>) {
        struct P(Arc<Mutex<(Option<String>, bool)>>);
        impl Platform for P {
            fn os(&self) -> &'static str { "macos" }
            fn foreground_app(&mut self) -> Option<String> { self.0.lock().unwrap().0.clone() }
            fn is_locked(&mut self) -> bool { self.0.lock().unwrap().1 }
        }
        let st = Arc::new(Mutex::new((Some(app.to_string()), false)));
        (st.clone(), Box::new(P(st)))
    }
    /// 13:00 Uhr in Berlin, also außerhalb des Standard-Zeitfensters (6–10 Uhr)
    fn afternoon(sec: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-26T12:00:00Z").unwrap().with_timezone(&Utc) + Cd::seconds(sec)
    }

    /// Zeit in 30-Sekunden-Schritten vergehen lassen (größere Sprünge wären ein Ruhezustand)
    fn run_until_from(e: &mut Engine<ReplayServer>, from: DateTime<Utc>, secs: i64) { run_until(e, from, secs) }
    fn run_until(e: &mut Engine<ReplayServer>, from: DateTime<Utc>, secs: i64) {
        let mut s = 30;
        while s <= secs { e.tick(from + Cd::seconds(s)); s += 30; }
    }

    #[test]
    fn working_offline_gives_the_same_result_as_working_online() {
        // Zug ohne Netz: Rechner startet außerhalb des Zeitfensters, Rückfrage, arbeiten, sperren, App wechseln
        let world = replay_world(true, true);
        world.lock().unwrap().online = false;
        let (pcs, platform) = pc("Code");
        let mut e = Engine::new(platform, ReplayServer(world.clone()), Outbox::in_memory());
        e.start(afternoon(0), false);
        assert_eq!(e.view().state.track, "asking", "die Rückfrage kommt auch ohne Server");
        e.action(afternoon(20), Action::Answer("work".into()));
        assert_eq!(e.view().state.track, "running");
        e.tick(afternoon(22));
        run_until(&mut e, afternoon(0), 600);
        pcs.lock().unwrap().0 = Some("Teams".into());
        run_until(&mut e, afternoon(600), 600);
        pcs.lock().unwrap().1 = true;
        run_until(&mut e, afternoon(1200), 600);
        pcs.lock().unwrap().1 = false;
        run_until(&mut e, afternoon(1800), 600);
        let v = e.view();
        assert!(!v.connected && v.estimated && v.pending >= 6, "{v:?}");
        assert!(v.state.counted > 30.0, "die Arbeitszeit läuft in der Anzeige weiter: {}", v.state.counted);
        let local_day = e.day.clone().unwrap();

        // Verbindung wieder da: alles wird nachgesendet, der Server kommt zum selben Tag
        world.lock().unwrap().online = true;
        e.tick(afternoon(2500));
        let v = e.view();
        assert!(v.connected && v.pending == 0, "{v:?}");
        let server_day = world.lock().unwrap().day.clone().unwrap();
        let shape = |d: &DayState| d.segs.iter().map(|g| (g.kind, g.app.clone(), g.from)).collect::<Vec<_>>();
        let first_n = local_day.segs.len();
        assert_eq!(shape(&server_day)[..first_n], shape(&local_day)[..], "gleiche Abschnitte lokal und beim Server");
        assert!(server_day.segs.iter().any(|g| g.app.as_deref() == Some("Teams")));
        assert!(server_day.segs.iter().any(|g| g.kind == tickly_core::SegKind::Locked));
        assert_eq!(server_day.stop, None);
    }

    #[test]
    fn pause_offline_shows_immediately_and_is_sent_later() {
        let world = replay_world(false, true);
        let (_pcs, platform) = pc("Code");
        let mut e = Engine::new(platform, ReplayServer(world.clone()), Outbox::in_memory());
        e.start(t(0), false);
        world.lock().unwrap().online = false;
        e.action(t(600), Action::Pause);
        assert_eq!(e.view().state.track, "paused");
        e.action(t(1200), Action::Resume);
        assert_eq!(e.view().state.track, "running");
        world.lock().unwrap().online = true;
        e.tick(t(1300));
        assert_eq!(e.view().pending, 0);
        let d = world.lock().unwrap().day.clone().unwrap();
        let gap: Vec<_> = d.segs.iter().filter(|g| g.kind == tickly_core::SegKind::Off).map(|g| (g.from, g.to)).collect();
        assert_eq!(gap.len(), 1, "die Pause von 10 Minuten steht auch beim Server");
        assert!((gap[0].1.unwrap() - gap[0].0 - 10.0).abs() < 0.01);
    }

    #[test]
    fn cold_start_without_network_uses_the_saved_settings() {
        let dir = std::env::temp_dir().join(format!("tickly-agent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = dir.join("agent.json");
        // Einstellungen: nicht nachfragen, nicht automatisch starten → „aus“
        let world = replay_world(false, false);
        let (_p, platform) = pc("Code");
        let mut e = Engine::new(platform, ReplayServer(world.clone()), Outbox::open(dir.join("outbox.json"))).with_store(store.clone());
        e.start(afternoon(0), false);
        assert_eq!(e.view().state.track, "idle");
        drop(e);
        // Neustart am nächsten Tag ohne Netz: die gesicherten Einstellungen gelten (sonst würde nachgefragt)
        world.lock().unwrap().online = false;
        let (_p, platform) = pc("Code");
        let mut e = Engine::new(platform, ReplayServer(world.clone()), Outbox::open(dir.join("outbox.json"))).with_store(store);
        e.start(afternoon(86_400), false);
        assert_eq!(e.view().state.track, "idle");
        assert!(!e.view().connected);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn midnight_while_offline_keeps_tracking() {
        let world = replay_world(false, true);
        world.lock().unwrap().online = false;
        let (_p, platform) = pc("Code");
        let mut e = Engine::new(platform, ReplayServer(world.clone()), Outbox::in_memory());
        // 23:50 Berlin (21:50 UTC) gestartet, Tracking läuft bis nach Mitternacht
        let late = DateTime::parse_from_rfc3339("2026-10-26T22:50:00Z").unwrap().with_timezone(&Utc);
        e.state.settings = Some(world.lock().unwrap().settings.clone());
        e.start(late, true);
        e.action(late, Action::Resume);
        let mut t = late;
        for _ in 0..40 { t += Cd::seconds(30); e.tick(t); }
        assert_eq!(e.view().state.track, "running");
        run_until_from(&mut e, t, 1200);
        assert_eq!(e.view().state.track, "running", "nach Mitternacht läuft es weiter");
        assert_eq!(e.date, Some(NaiveDate::from_ymd_opt(2026, 10, 27).unwrap()));
    }
}
