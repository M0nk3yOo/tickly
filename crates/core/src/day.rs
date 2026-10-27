//! Der laufende Arbeitstag: Ereignisse des Companion (Start, Sperre, App-Wechsel, Pause, Beenden).
use crate::model::*;

/// Anzeigename einer App. Die Schlüssel der Demo-Daten bekommen ihren Klarnamen, alles andere bleibt wie gemeldet.
pub fn app_label(key: &str) -> String {
    match key {
        "code" => "VS Code", "web" => "Chrome", "mail" => "Outlook", "teams" => "Teams", "figma" => "Figma", "excel" => "Excel", "other" => "Sonstiges",
        k => return k.to_string(),
    }.to_string()
}

/// Symbolgruppe einer App für die Oberfläche (code, web, mail, teams, figma, excel, other).
pub fn app_glyph(key: &str) -> &'static str {
    let k = key.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| k.contains(x));
    if has(&["code", "xcode", "intellij", "idea", "pycharm", "webstorm", "rider", "clion", "goland", "terminal", "iterm", "powershell", "sublime", "cursor", "android studio", "visual studio"]) { "code" }
    else if has(&["chrome", "firefox", "safari", "edge", "brave", "opera", "vivaldi"]) { "web" }
    else if has(&["outlook", "mail", "thunderbird", "spark", "postbox"]) { "mail" }
    else if has(&["teams", "slack", "zoom", "discord", "webex", "skype", "signal", "whatsapp", "telegram"]) { "teams" }
    else if has(&["figma", "sketch", "photoshop", "illustrator", "affinity", "canva"]) { "figma" }
    else if has(&["excel", "word", "powerpoint", "numbers", "pages", "keynote", "libreoffice", "onenote", "notion", "obsidian"]) { "excel" }
    else { "other" }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Running,
    Locked,
    /// Im Companion manuell gestoppt, die App läuft.
    Paused,
    /// Rückfrage „Arbeitest du gerade?“ offen.
    Asking,
    /// Companion läuft, Tracking aus.
    Idle,
    /// Companion nicht aktiv.
    Stopped,
}

/// Entscheidungsgrundlage für „soll das Tracking von selbst starten?“.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Policy {
    /// Beginn des gerade aktiven Zeitfensters (Auto-Start), sonst `None`.
    pub auto_from: Option<f64>,
    /// Außerhalb der Zeitfenster nachfragen.
    pub ask: bool,
}

fn hm(m: f64) -> String {
    let t = m.round() as i64;
    format!("{:02}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

impl Track {
    pub fn as_str(self) -> &'static str {
        match self {
            Track::Running => "running",
            Track::Locked => "locked",
            Track::Paused => "paused",
            Track::Asking => "asking",
            Track::Idle => "idle",
            Track::Stopped => "stopped",
        }
    }
}

impl DayState {
    fn blank(now: f64) -> DayState {
        DayState { start: now, tracking: false, ended: None, last_app: "other".into(), segs: vec![], events: vec![], stop: Some(Stop::Idle), declined: None, last_beat: None, edited: false }
    }

    /// Noch nichts erfasst: der Companion hat sich heute nicht gemeldet.
    pub fn empty(now: f64) -> DayState {
        let mut d = DayState::blank(now);
        d.stop = Some(Stop::Quit);
        d
    }

    /// Der Vortag lief um Mitternacht noch: Tracking (bzw. Sperre) setzt sich um 00:00 fort.
    pub fn continued(&self, os: &str) -> DayState {
        let mut d = DayState::blank(0.0);
        d.last_app = self.last_app.clone();
        let locked = self.track() == Track::Locked;
        d.begin(0.0, 0.0, format!("{os}: Tracking läuft über Mitternacht weiter"));
        if locked {
            d.segs[0].kind = SegKind::Locked;
            d.segs[0].app = None;
        }
        d
    }

    /// Letzten Abschnitt zum Tagesende (24:00) schließen.
    pub fn close_at_midnight(&mut self) {
        self.close_open(1440.0);
        self.last_beat = Some(1440.0);
    }

    /// Windows-/macOS-Start erkannt: neuer Arbeitstag beginnt, das Tracking läuft sofort.
    pub fn boot(now: f64, os: &str) -> DayState {
        let mut d = DayState::blank(now);
        d.last_app = "mail".into();
        d.begin(now, now, format!("{os} gestartet · Companion automatisch gestartet · Arbeitstag beginnt"));
        d
    }

    /// Erster Start des Tages: je nach Zeitfenster automatisch tracken, nachfragen oder nichts tun.
    pub fn new_day(now: f64, os: &str, p: &Policy) -> DayState {
        let mut d = DayState::blank(now);
        d.apply_policy(now, p, os);
        d
    }

    fn apply_policy(&mut self, now: f64, p: &Policy, os: &str) {
        if let Some(ws) = p.auto_from {
            let tail = if self.segs.is_empty() { "Arbeitstag beginnt" } else { "Lücke wird nicht gezählt" };
            self.begin(now, now, format!("{os} gestartet · Tracking automatisch gestartet (Zeitfenster ab {}) · {tail}", hm(ws)));
        } else if p.ask {
            self.tracking = false;
            self.stop = Some(Stop::Ask);
            self.ev(now, "ask", format!("{os} gestartet · außerhalb der Zeitfenster · Rückfrage: Arbeitest du gerade?"));
        } else {
            self.tracking = false;
            self.stop = Some(Stop::Idle);
            self.ev(now, "start", format!("{os} gestartet · außerhalb der Zeitfenster · Tracking nicht gestartet"));
        }
    }

    pub fn track(&self) -> Track {
        if self.tracking {
            return match self.segs.last() {
                Some(s) if s.kind == SegKind::Locked => Track::Locked,
                _ => Track::Running,
            };
        }
        match self.stop {
            Some(Stop::Pause) => Track::Paused,
            Some(Stop::Ask) => Track::Asking,
            Some(Stop::Idle) => Track::Idle,
            _ => Track::Stopped,
        }
    }

    /// Der Tag gilt als beendet, sobald nicht mehr getrackt wird – außer bei einer bewussten Pause
    /// oder wenn noch gar nicht gearbeitet wurde.
    pub fn is_closed(&self) -> bool {
        !self.tracking && !self.segs.is_empty() && self.stop != Some(Stop::Pause)
    }

    /// Letzter Zeitpunkt, zu dem der Companion sicher aktiv war.
    fn last_alive(&self) -> f64 {
        let from = self.segs.last().map(|s| s.from).unwrap_or(self.start);
        let ev = self.events.last().map(|e| e.t).unwrap_or(from);
        from.max(ev).max(self.last_beat.unwrap_or(0.0))
    }
    fn close_open_at(&mut self, t: f64) {
        self.close_open(t);
    }

    /// Abschnitte für die Auswertung: ein noch offener Abschnitt eines vergangenen Tages endet beim letzten Lebenszeichen.
    pub fn finalize(&self) -> Vec<Seg> {
        let end = self.last_alive();
        self.segs.iter().cloned().map(|mut s| {
            if s.to.is_none() {
                s.to = Some(s.from.max(end));
            }
            s
        }).collect()
    }

    /// Regelmäßiges Lebenszeichen des Companion; prüft auch, ob ein Zeitfenster für den Auto-Start begonnen hat.
    pub fn beat(&mut self, now: f64, p: &Policy) {
        self.last_beat = Some(self.last_beat.map_or(now, |b| b.max(now)));
        self.auto_tick(now, p);
    }

    fn close_open(&mut self, now: f64) {
        if let Some(o) = self.segs.last_mut() {
            if o.to.is_none() {
                o.to = Some(o.from.max(now));
            }
        }
    }
    fn ev(&mut self, now: f64, k: &str, txt: String) {
        self.events.push(Event { t: now, k: k.into(), txt });
    }

    /// Tracking (wieder) beginnen. Die Lücke seit dem letzten Stopp zählt nicht als Arbeitszeit.
    /// Beim ersten Abschnitt des Tages kann `from` rückwirkend (z. B. Rechnerstart) sein.
    fn begin(&mut self, now: f64, from: f64, txt: String) {
        let app = Some(self.last_app.clone());
        if self.segs.is_empty() {
            self.segs.push(Seg { from: from.min(now), to: None, kind: SegKind::App, app, note: None });
        } else {
            let gap = self.ended.unwrap_or(now).min(now);
            self.segs.push(Seg { from: gap, to: Some(now), kind: SegKind::Off, app: None, note: None });
            self.segs.push(Seg { from: now, to: None, kind: SegKind::App, app, note: None });
        }
        self.tracking = true;
        self.ended = None;
        self.stop = None;
        self.declined = None;
        self.ev(now, "start", txt);
    }

    pub fn switch_app(&mut self, app: &str, now: f64) -> Result<(), &'static str> {
        if self.track() != Track::Running {
            return Err("Nur bei laufendem, entsperrtem Companion möglich.");
        }
        let app = app.trim();
        if app.is_empty() || app.chars().count() > 64 {
            return Err("Ungültiger App-Name.");
        }
        self.last_app = app.into();
        if self.segs.last().and_then(|s| s.app.as_deref()) == Some(app) {
            return Ok(());
        }
        self.close_open(now);
        self.segs.push(Seg { from: now, to: None, kind: SegKind::App, app: Some(app.into()), note: None });
        Ok(())
    }

    pub fn lock(&mut self, now: f64) -> Result<(), &'static str> {
        if self.track() != Track::Running {
            return Err("Bildschirm ist bereits gesperrt oder Tracking gestoppt.");
        }
        self.close_open(now);
        self.segs.push(Seg { from: now, to: None, kind: SegKind::Locked, app: None, note: None });
        self.ev(now, "lock", "Bildschirm gesperrt".into());
        Ok(())
    }

    pub fn unlock(&mut self, now: f64) -> Result<(), &'static str> {
        if self.track() != Track::Locked {
            return Err("Bildschirm ist nicht gesperrt.");
        }
        let from = self.segs.last().map(|s| s.from).unwrap_or(now);
        self.close_open(now);
        let app = self.last_app.clone();
        self.segs.push(Seg { from: now, to: None, kind: SegKind::App, app: Some(app), note: None });
        self.ev(now, "unlock", format!("Entsperrt ({} Min. gesperrt)", (now - from).round()));
        Ok(())
    }

    /// Tracking im Companion anhalten; die App läuft weiter und der Tag gilt nicht als beendet.
    pub fn pause(&mut self, now: f64) -> Result<(), &'static str> {
        if self.track() != Track::Running {
            return Err("Nur bei laufendem, entsperrtem Tracking möglich.");
        }
        self.close_open(now);
        self.tracking = false;
        self.ended = Some(now);
        self.stop = Some(Stop::Pause);
        self.ev(now, "pause", "Tracking im Companion gestoppt · Pause wird nicht gezählt".into());
        Ok(())
    }

    /// Tracking manuell starten bzw. nach einer Pause oder dem Beenden fortsetzen.
    pub fn start_tracking(&mut self, now: f64) -> Result<(), &'static str> {
        if self.tracking {
            return Err("Tracking läuft bereits.");
        }
        let txt = match self.stop {
            Some(Stop::Pause) => format!("Tracking fortgesetzt · {} Min. Pause nicht gezählt", (now - self.ended.unwrap_or(now)).round()),
            Some(Stop::Quit) | Some(Stop::Shutdown) => "Companion erneut gestartet · Lücke wird nicht als Arbeitszeit gezählt".to_string(),
            _ => "Tracking manuell gestartet".to_string(),
        };
        self.begin(now, now, txt);
        Ok(())
    }

    /// Companion richtig beendet (nicht nur das Fenster geschlossen): Arbeitsende.
    pub fn quit(&mut self, now: f64) -> Result<(), &'static str> {
        self.end_session(now, Stop::Quit, "Companion beendet (nicht nur Fenster) · Tracking gestoppt · Arbeitsende")
    }

    /// Rechner fährt herunter oder startet neu. Lief das Tracking, wird es nach dem nächsten Start fortgesetzt.
    pub fn shutdown(&mut self, now: f64) -> Result<(), &'static str> {
        self.end_session(now, Stop::Shutdown, "Rechner heruntergefahren / neu gestartet · Tracking wird nach dem Start fortgesetzt")
    }

    fn end_session(&mut self, now: f64, kind: Stop, txt: &str) -> Result<(), &'static str> {
        if self.tracking {
            self.close_open(now);
            self.tracking = false;
            self.ended = Some(now);
            self.stop = Some(kind);
        } else if matches!(self.stop, Some(Stop::Pause) | Some(Stop::Ask) | Some(Stop::Idle)) {
            // Companion lief ohne Tracking: nichts fortzusetzen, nur der Companion ist jetzt weg.
            self.stop = Some(Stop::Quit);
            self.ev(now, "quit", "Companion beendet".into());
            return Ok(());
        } else {
            return Err("Companion läuft nicht.");
        }
        self.ev(now, if kind == Stop::Quit { "quit" } else { "shutdown" }, txt.into());
        Ok(())
    }

    /// Rechner (erneut) gestartet. Nach einem Neustart mit laufendem Tracking geht es nahtlos weiter.
    pub fn on_boot(&mut self, now: f64, os: &str, p: &Policy) {
        match self.stop {
            None => {
                // Kein sauberes Beenden gemeldet (Absturz, Stromausfall): Ende ist das letzte Lebenszeichen.
                let last = self.last_alive().min(now);
                self.close_open_at(last);
                self.ended = Some(last);
                self.begin(now, now, format!("{os} gestartet · Tracking nach unsauberem Ende fortgesetzt · Lücke {} Min. nicht gezählt", (now - last).round()));
            }
            Some(Stop::Shutdown) => {
                let from = self.ended.unwrap_or(now);
                self.begin(now, now, format!("{os} gestartet · Tracking nach Neustart fortgesetzt · Lücke {} Min. nicht gezählt", (now - from).round()));
            }
            Some(Stop::Ask) => {}
            _ => self.apply_policy(now, p, os),
        }
    }

    /// Antwort auf „Arbeitest du gerade?“: `work` (ab jetzt), `boot` (rückwirkend ab Rechnerstart) oder `no`.
    pub fn answer(&mut self, now: f64, ans: &str) -> Result<(), &'static str> {
        if self.stop != Some(Stop::Ask) {
            return Err("Es ist keine Rückfrage offen.");
        }
        match ans {
            "work" => self.begin(now, now, "Rückfrage bestätigt · Tracking gestartet".into()),
            "boot" => {
                let from = if self.segs.is_empty() { self.start } else { now };
                self.begin(now, from, format!("Rückfrage bestätigt · Tracking ab Rechnerstart ({}) gestartet", hm(from)));
            }
            "no" => {
                self.stop = Some(Stop::Idle);
                self.declined = Some(now);
                self.ev(now, "ask", "Rückfrage: arbeitet gerade nicht · Tracking bleibt aus".into());
            }
            _ => return Err("Unbekannte Antwort."),
        }
        Ok(())
    }

    /// Periodische Prüfung des Companion: beginnt ein Zeitfenster, startet das Tracking von selbst.
    /// Eine bewusste Pause und ein „Nein“ innerhalb desselben Fensters bleiben unangetastet.
    pub fn auto_tick(&mut self, now: f64, p: &Policy) -> bool {
        if !matches!(self.stop, Some(Stop::Ask) | Some(Stop::Idle)) {
            return false;
        }
        let Some(ws) = p.auto_from else { return false };
        if self.declined.is_some_and(|d| d >= ws) {
            return false;
        }
        self.begin(now, now, format!("Tracking automatisch gestartet (Zeitfenster ab {})", hm(ws)));
        true
    }
}

/// Ereignisse eines vergangenen Tages aus den Abschnitten ableiten.
pub fn events_from_segs(segs: &[Seg]) -> Vec<Event> {
    let mut ev = Vec::new();
    if let Some(f) = segs.first() {
        ev.push(Event { t: f.from, k: "start".into(), txt: "Windows gestartet · Arbeitstag beginnt".into() });
    }
    for s in segs.iter().filter(|s| s.kind == SegKind::Locked) {
        let to = s.to.unwrap_or(s.from);
        ev.push(Event { t: s.from, k: "lock".into(), txt: "Bildschirm gesperrt".into() });
        ev.push(Event { t: to, k: "unlock".into(), txt: format!("Entsperrt ({} Min. gesperrt)", (to - s.from).round()) });
    }
    if let Some(l) = segs.last() {
        ev.push(Event { t: l.to.unwrap_or(l.from), k: "quit".into(), txt: "Companion beendet · Arbeitsende".into() });
    }
    ev
}

/// Wendet ein Ereignis auf den Tag an. Server und Companion nutzen genau diese Funktion: Der Companion rechnet damit
/// auch ohne Verbindung mit und kommt später beim Server zum gleichen Ergebnis.
///
/// `day = None` heißt: heute gibt es noch keinen Tag; dann entsteht er je nach Zeitfenster (läuft, fragt oder bleibt aus).
pub fn apply_event(day: &mut Option<DayState>, kind: &str, now: f64, os: &str, p: &Policy, app: Option<&str>, answer: Option<&str>) -> Result<(), String> {
    const KINDS: [&str; 11] = ["boot", "newday", "beat", "lock", "unlock", "quit", "shutdown", "pause", "start", "answer", "tick"];
    if !KINDS.contains(&kind) && kind != "app" {
        return Err("Unbekanntes Ereignis".into());
    }
    if day.is_none() {
        *day = Some(DayState::new_day(now, os, p));
        if kind == "boot" || kind == "newday" {
            return Ok(());
        }
    }
    let d = day.as_mut().expect("Tag");
    let res: Result<(), &str> = match kind {
        "boot" => { d.on_boot(now, os, p); Ok(()) }
        "newday" => { *d = DayState::new_day(now, os, p); Ok(()) }
        "beat" => { d.beat(now, p); Ok(()) }
        "lock" => d.lock(now),
        "unlock" => d.unlock(now),
        "quit" => d.quit(now),
        "shutdown" => d.shutdown(now),
        "pause" => d.pause(now),
        "start" => d.start_tracking(now),
        "answer" => d.answer(now, answer.unwrap_or("")),
        "tick" => { d.auto_tick(now, p); Ok(()) }
        _ => d.switch_app(app.unwrap_or(""), now),
    };
    res.map_err(String::from)?;
    d.last_beat = Some(d.last_beat.map_or(now, |b| b.max(now)));
    Ok(())
}
