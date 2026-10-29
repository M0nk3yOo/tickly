//! Hintergrundprüfung des laufenden Trackings:
//! - Hinweis-Mail, wenn das Tracking sehr lange läuft (Standard 10 Stunden Brutto), höchstens einmal je Tag.
//! - Plausibilitätsprüfung nach Mitternacht: läuft das Tracking über Nacht weiter und passt das nicht zu den bisherigen Tagen,
//!   bekommt der Benutzer eine Rückfrage (Dashboard, Companion und, mit SMTP, eine Mail), höchstens einmal je Nacht.
//! - Konten ohne Companion setzen ein laufendes Tracking über Mitternacht fort. Ohne Lebenszeichen vom Rechner würde sonst
//!   der Vortag mit dem letzten Ereignis enden und die erfasste Zeit wäre verloren.
use crate::app::*;
use crate::db;
use chrono::{Duration, NaiveDate};
use std::sync::Arc;
use tickly_core::{day_stat, night_check};

pub fn spawn_watch(app: Arc<App>) {
    if app.cfg.demo { return }
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let (today, now) = app.clock(None);
            if let Err(e) = tick_at(&app, today, now).await { tracing::warn!("Prüfung des Trackings fehlgeschlagen: {e}"); }
        }
    });
}

fn hh(m: f64) -> String {
    let t = m.round() as i64;
    format!("{}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

fn clock(m: f64) -> String {
    let t = m.round() as i64;
    format!("{:02}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

/// Eine Prüfrunde für den Zeitpunkt (`today`, `now` = Minuten seit Mitternacht). Gibt die Zahl der neuen Hinweise zurück.
pub async fn tick_at(app: &Arc<App>, today: NaiveDate, now: f64) -> Result<usize, String> {
    let e = |x: sqlx::Error| x.to_string();
    // 1. Über Mitternacht fortsetzen (nur Konten ohne gekoppelten Companion)
    let carry: Vec<String> = sqlx::query_scalar(
        "SELECT d.user_id FROM days d WHERE d.day = $1 AND (d.state->>'tracking')::boolean \
         AND NOT EXISTS (SELECT 1 FROM days t WHERE t.user_id = d.user_id AND t.day = $2) \
         AND NOT EXISTS (SELECT 1 FROM devices v WHERE v.user_id = d.user_id AND v.revoked_at IS NULL)")
        .bind(today - Duration::days(1)).bind(today).fetch_all(&app.pool).await.map_err(e)?;
    for uid in carry {
        let mut tx = app.pool.begin().await.map_err(e)?;
        db::lock_user(&mut tx, &uid).await.map_err(|x| x.1)?;
        let prev = db::get_day(&mut tx, &uid, today - Duration::days(1)).await.map_err(|x| x.1)?;
        let has_today = db::get_day(&mut tx, &uid, today).await.map_err(|x| x.1)?.is_some();
        if let (Some(mut p), false) = (prev, has_today) {
            if p.tracking && p.stop.is_none() {
                let next = p.continued("Web");
                p.close_at_midnight();
                db::put_day(&mut tx, &uid, today - Duration::days(1), &p).await.map_err(|x| x.1)?;
                db::put_day(&mut tx, &uid, today, &next).await.map_err(|x| x.1)?;
            }
        }
        tx.commit().await.map_err(e)?;
    }
    let mut sent = 0;
    // 2. Tracking läuft über Nacht weiter und passt nicht ins Muster
    let carried: Vec<String> = sqlx::query_scalar("SELECT user_id FROM days WHERE day = $1 AND (state->>'tracking')::boolean AND COALESCE((state->>'carried')::boolean, false) AND NOT COALESCE((state->>'night_ok')::boolean, false)")
        .bind(today).fetch_all(&app.pool).await.map_err(e)?;
    for uid in carried {
        let Some(u) = db::user_by_id(&app.pool, &uid).await.map_err(|x| x.1)? else { continue };
        if u.status != "active" { continue }
        let c = db::Calc::load(app, &u, today, now).await.map_err(|x| x.1)?;
        let Some(n) = c.with(night_check) else { continue };
        if !alert_once(app, &uid, today, "night").await? { continue }
        sent += 1;
        crate::netsec::log_system(app, "tracking", true, &format!("Rückfrage „Tracking läuft über Nacht“ für {}", u.email)).await;
        crate::mail::spawn(app.clone(), vec![u.email.clone()], format!("Läuft dein Tracking noch? Es läuft seit {} Uhr durch", clock(n.since)), "Läuft dein Tracking noch?".into(),
            vec![format!("Dein Tracking läuft seit gestern {} Uhr ohne Unterbrechung, jetzt sind es schon {} Stunden. Das passt nicht zu deinen bisherigen Arbeitszeiten.", clock(n.since), hh(n.hours * 60.0)),
                 "Hast du vergessen, es zu beenden? Du kannst es in Tickly beenden und das Ende auf die Uhrzeit setzen, zu der du wirklich aufgehört hast. Arbeitest du tatsächlich noch, bestätige das einfach in Tickly.".into(),
                 "Bis du antwortest, läuft die Zeit weiter. Diese Nachricht kommt höchstens einmal pro Nacht.".into()],
            Some(("Tracking prüfen".into(), format!("{}/#/dashboard", app.cfg.public_url.as_str().trim_end_matches('/')))));
    }
    // 3. Sehr langes Tracking
    let hours = db::reg_policy(&app.pool).await.map_err(|x| x.1)?.long_track_hours;
    if hours == 0 { return Ok(sent) }
    let running: Vec<String> = sqlx::query_scalar("SELECT user_id FROM days WHERE day = $1 AND (state->>'tracking')::boolean").bind(today).fetch_all(&app.pool).await.map_err(e)?;
    for uid in running {
        let Some(u) = db::user_by_id(&app.pool, &uid).await.map_err(|x| x.1)? else { continue };
        if u.status != "active" { continue }
        let c = db::Calc::load(app, &u, today, now).await.map_err(|x| x.1)?;
        if !c.day.tracking { continue }
        let s = c.with(|x| day_stat(x, today));
        if s.gross < hours as f64 * 60.0 { continue }
        if !alert_once(app, &uid, today, "long").await? { continue }
        sent += 1;
        let start = s.start.map(clock).unwrap_or_default();
        crate::netsec::log_system(app, "tracking", true, &format!("Hinweis „Tracking läuft seit über {hours} Stunden“ für {}", u.email)).await;
        crate::mail::spawn(app.clone(), vec![u.email.clone()], format!("Läuft dein Tracking noch? Seit {start} Uhr erfasst"), "Läuft dein Tracking noch?".into(),
            vec![format!("Dein Tracking läuft noch: Arbeitsbeginn {start} Uhr, bisher {} Stunden Brutto ({} Stunden netto) heute.", hh(s.gross), hh(s.net)),
                 "Hast du vergessen, es zu beenden? Du kannst es in Tickly jetzt beenden oder ein früheres Ende festlegen, zum Beispiel die Uhrzeit, zu der du wirklich aufgehört hast.".into(),
                 "Läuft alles richtig weiter, musst du nichts tun. Diese Nachricht kommt höchstens einmal am Tag.".into()],
            Some(("Tracking öffnen".into(), format!("{}/#/dashboard", app.cfg.public_url.as_str().trim_end_matches('/')))));
    }
    Ok(sent)
}

/// Merkt sich den Hinweis; `true`, wenn er an diesem Tag für diesen Benutzer noch nicht verschickt wurde.
async fn alert_once(app: &App, uid: &str, day: NaiveDate, kind: &str) -> Result<bool, String> {
    let r = sqlx::query("INSERT INTO track_alerts (user_id, day, kind) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING").bind(uid).bind(day).bind(kind).execute(&app.pool).await.map_err(|x| x.to_string())?;
    Ok(r.rows_affected() == 1)
}
