//! Ereignisse des Companion auf den Tageszustand anwenden (gemeinsam für Gerät und Web-Oberfläche).
use crate::app::*;
use crate::db::{self, UserRow};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use sqlx::PgConnection;
use tickly_core::{track_policy, Absence, DayState};

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Ev {
    pub kind: String,
    /// Zeitpunkt des Ereignisses (UTC). Fehlt er, gilt „jetzt“.
    pub at: Option<DateTime<Utc>>,
    /// Nur Demo-Modus: simulierte Uhrzeit in Minuten.
    pub now: Option<f64>,
    pub app: Option<String>,
    pub os: Option<String>,
    pub answer: Option<String>,
    /// Nur „stopat“: gewünschtes Ende in Minuten seit Mitternacht.
    pub until: Option<f64>,
    /// Kommt aus der Weboberfläche (nicht vom Companion). Setzt der Server, nie der Client.
    #[serde(skip)]
    pub web: bool,
    /// Auf dem meldenden Gerät ist der automatische Start abgeschaltet. Setzt der Server, nie der Client.
    #[serde(skip)]
    pub no_auto: bool,
}

/// Ereignisse, die nur im Demo-Modus gelten (Zeitreise, Tag zurücksetzen).
const DEMO_ONLY: [&str; 1] = ["newday"];

/// Wendet ein Ereignis an. `Ok(Err(text))` heißt: Ereignis passt nicht zum Zustand (z. B. „entsperren“ ohne Sperre).
pub async fn apply(app: &App, conn: &mut PgConnection, u: &UserRow, abs: &[Absence], ev: &Ev) -> Res<Result<(), String>> {
    if DEMO_ONLY.contains(&ev.kind.as_str()) && !app.cfg.demo {
        return forbidden("Nur im Demo-Modus verfügbar");
    }
    let (date, now) = if app.cfg.demo {
        app.clock(ev.now)
    } else {
        let utc = Utc::now();
        let mut at = ev.at.unwrap_or(utc);
        // Geht die Uhr des Rechners etwas vor, zählt der Zeitpunkt des Servers statt das Ereignis zu verlieren
        if at > utc + Duration::hours(1) { return bad("Zeitpunkt liegt in der Zukunft") }
        if at > utc { at = utc; }
        if at < utc - Duration::days(14) { return Ok(Err("Ereignis ist zu alt".into())) }
        app.local(at)
    };
    let os = match ev.os.as_deref().map(|s| s.to_lowercase()) {
        Some(o) if o == "mac" || o == "macos" => "macOS",
        _ => "Windows",
    };
    db::lock_user(conn, &u.id).await?;
    let mut p = track_policy(&u.settings, abs, date, now);
    if ev.no_auto { p.auto_from = None; }
    let kind = ev.kind.as_str();
    // Erster Eintrag des Tages: Lief der Vortag um Mitternacht noch, geht es nahtlos weiter.
    let mut day: Option<tickly_core::DayState> = db::get_day(conn, &u.id, date).await?;
    if day.is_none() && kind != "boot" && kind != "newday" {
        if let Some((pd, mut ps)) = db::previous_day(conn, &u.id, date).await? {
            if Some(pd) == date.pred_opt() && ps.tracking && ps.stop.is_none() {
                day = Some(ps.continued(os));
                ps.close_at_midnight();
                db::put_day(conn, &u.id, pd, &ps).await?;
            }
        }
    }
    // Zeiterfassung im Web (ohne Companion): Start, Beenden jetzt, Beenden zu einem früheren Zeitpunkt
    let web_stop = matches!(kind, "stop" | "stopat");
    let web_start = kind == "start" && ev.web;
    if web_stop && !ev.web && !app.cfg.demo { return bad("Beenden meldet nur die Weboberfläche") }
    if web_stop || web_start {
        let res = if web_stop {
            match day.as_mut() {
                None => Err("Heute läuft kein Tracking.".to_string()),
                Some(d) if kind == "stop" => d.stop_web(now, now),
                Some(d) => match ev.until { Some(at) if at.is_finite() => d.stop_web(now, at), _ => return bad("Bitte ein Ende angeben") },
            }
        } else {
            let mut d = day.take().unwrap_or_else(|| DayState::empty(now));
            let r = d.web_start(now).map_err(String::from);
            day = Some(d);
            r
        };
        return match res {
            Ok(()) => { db::put_day(conn, &u.id, date, &day.expect("Tag")).await?; Ok(Ok(())) }
            Err(m) => Ok(Err(m)),
        };
    }
    match tickly_core::apply_event(&mut day, kind, now, os, &p, ev.app.as_deref(), ev.answer.as_deref()) {
        Ok(()) => {
            db::put_day(conn, &u.id, date, &day.expect("Tag")).await?;
            Ok(Ok(()))
        }
        Err(m) if m == "Unbekanntes Ereignis" => bad(m),
        Err(m) => Ok(Err(m)),
    }
}
