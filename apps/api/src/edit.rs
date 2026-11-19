//! Nachträgliches Bearbeiten von Zeiten und manuelles Eintragen von Arbeitszeit.
use crate::api::{auth, St};
use crate::app::*;
use crate::db::{self};
use crate::netsec::{self, ClientIp};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::put,
    Extension, Json, Router,
};
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tickly_core::{calc_segs, DayState, Event, Seg, SegKind, Stop};

pub const MANUAL_APP: &str = "Manuell";
const MAX_SEGS: usize = 5000;

pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/days/{date}", put(put_day))
}

#[derive(Deserialize)]
pub struct EditSeg { pub from: f64, pub to: Option<f64>, pub kind: String, pub app: Option<String>, pub note: Option<String> }

#[derive(Deserialize)]
struct EditReq { segs: Vec<EditSeg> }

fn hm(m: f64) -> String {
    let t = m.round() as i64;
    format!("{}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

/// Ersetzt die erfassten Abschnitte eines Tages (nur Arbeitszeit mit App und gesperrte Zeit); Lücken zählen nicht.
/// Ein gerade laufender Abschnitt von heute bleibt unangetastet.
async fn put_day(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(date): Path<NaiveDate>, Json(r): Json<EditReq>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    save_day(&app, &u, date, r.segs, false, ip, &h, "").await
}

/// Gemeinsamer Kern von „Zeiten bearbeiten“ (Weboberfläche) und „Zeit eintragen“ (API). Mit `append` werden die vorhandenen
/// abgeschlossenen Abschnitte des Tages behalten und `segs` ergänzt; sonst ersetzt `segs` den Tag. `via` steht im Protokoll.
pub async fn save_day(app: &App, u: &crate::db::UserRow, date: NaiveDate, segs: Vec<EditSeg>, append: bool, ip: ClientIp, h: &HeaderMap, via: &str) -> Res<Json<Value>> {
    let reg = db::reg_policy(&app.pool).await?;
    if reg.edit_days_back == 0 { return forbidden("Nachträgliches Bearbeiten ist vom Administrator ausgeschaltet") }
    let (today, now) = app.clock(None);
    if date > today { return bad("Für die Zukunft lassen sich keine Zeiten eintragen") }
    if date < today - Duration::days(reg.edit_days_back as i64) { return bad(format!("Dieser Tag liegt mehr als {} Tage zurück und lässt sich nicht mehr bearbeiten", reg.edit_days_back)) }
    if date < u.track_start { return bad("Vor dem Beginn deiner Zeiterfassung lassen sich keine Zeiten eintragen") }
    if segs.len() > MAX_SEGS { return bad("Zu viele Abschnitte an einem Tag") }

    let mut tx = app.pool.begin().await?;
    db::lock_user(&mut tx, &u.id).await?;
    let old = db::get_day(&mut tx, &u.id, date).await?;
    let mut input = segs;
    if append {
        if let Some(o) = &old {
            let mut keep: Vec<EditSeg> = o.segs.iter().filter(|g| g.to.is_some() && g.kind != SegKind::Off)
                .map(|g| EditSeg { from: g.from, to: g.to, kind: if g.kind == SegKind::Locked { "locked" } else { "app" }.into(), app: g.app.clone(), note: g.note.clone() }).collect();
            keep.append(&mut input);
            input = keep;
        }
        if input.len() > MAX_SEGS { return bad("Zu viele Abschnitte an einem Tag") }
    }
    // Der laufende Abschnitt von heute gehört dem Companion
    let open: Option<Seg> = old.as_ref().filter(|d| date == today && d.tracking).and_then(|d| d.segs.last().filter(|g| g.to.is_none()).cloned());

    let mut segs: Vec<Seg> = Vec::new();
    for e in &input {
        let Some(to) = e.to else { continue };
        if !e.from.is_finite() || !to.is_finite() || e.from < 0.0 || to > 1440.0 { return bad("Uhrzeiten müssen zwischen 00:00 und 24:00 liegen") }
        if to - e.from < 1.0 / 60.0 { return bad(format!("Der Abschnitt ab {} hat keine Dauer – „Bis“ muss nach „Von“ liegen", hm(e.from))) }
        if date == today && to > now + 0.5 { return bad(format!("{} liegt in der Zukunft – heute lässt sich nur bis jetzt ({}) eintragen", hm(to), hm(now))) }
        let note = e.note.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(|n| n.chars().take(200).collect::<String>());
        match e.kind.as_str() {
            "app" => {
                let a = e.app.as_deref().map(str::trim).filter(|a| !a.is_empty()).unwrap_or(MANUAL_APP);
                if a.chars().count() > 64 { return bad("Der App-Name ist zu lang") }
                segs.push(Seg { from: e.from, to: Some(to), kind: SegKind::App, app: Some(a.to_string()), note });
            }
            "locked" => segs.push(Seg { from: e.from, to: Some(to), kind: SegKind::Locked, app: None, note: None }),
            "off" => continue, // Lücken zählen ohnehin nicht
            _ => return bad("Unbekannte Art des Abschnitts"),
        }
    }
    segs.sort_by(|a, b| a.from.partial_cmp(&b.from).unwrap_or(std::cmp::Ordering::Equal));
    for w in segs.windows(2) {
        if w[1].from < w[0].to.unwrap_or(0.0) - 1e-6 {
            return bad(format!("Die Abschnitte {}–{} und {}–{} überschneiden sich", hm(w[0].from), hm(w[0].to.unwrap_or(0.0)), hm(w[1].from), hm(w[1].to.unwrap_or(0.0))));
        }
    }
    if let (Some(o), Some(last)) = (&open, segs.last()) {
        if last.to.unwrap_or(0.0) > o.from + 1e-6 { return bad(format!("Der laufende Abschnitt beginnt um {} – davor muss alles enden", hm(o.from))) }
    }
    if let Some(o) = &open { segs.push(o.clone()); }

    let settings = &u.settings;
    let net = |v: &[Seg]| calc_segs(&v.iter().cloned().map(|mut s| { if s.to.is_none() { s.to = Some(s.from.max(now)); } s }).collect::<Vec<_>>(), settings).net;
    // Für Tage ohne Eintrag in der Datenbank (z. B. erzeugte Demo-Historie) gilt der bisher berechnete Stand
    let before = match &old {
        Some(d) => net(&d.finalize()),
        None => db::Calc::load(app, u, today, now).await?.with(|c| (c.history)(date)).map(|v| net(&v)).unwrap_or(0.0),
    };
    let after = net(&segs);
    let n = segs.len();
    let last_end = segs.iter().filter_map(|g| g.to).fold(0.0, f64::max);

    let mut day = old.unwrap_or_else(|| {
        let mut d = DayState::empty(segs.first().map(|g| g.from).unwrap_or(0.0));
        d.stop = Some(Stop::Quit);
        d.ended = Some(last_end);
        d
    });
    day.segs = segs;
    day.edited = true;
    // Eine offene Rückfrage zu einer Sperre bezieht sich auf die alten Zeiten
    day.away = None;
    day.start = day.segs.first().map(|g| g.from).unwrap_or(day.start);
    if !day.tracking && day.ended.is_none() { day.ended = Some(last_end); }
    day.events.push(Event { t: if date == today { now } else { last_end }, k: "edit".into(), txt: format!("Zeiten nachträglich bearbeitet ({n} Abschnitte, Arbeitszeit {} → {} h)", hm(before), hm(after)) });
    db::put_day(&mut tx, &u.id, date, &day).await?;
    tx.commit().await?;
    netsec::log(app, "edit", true, ip, h, Some(&u.id), Some(&u.email), &format!("{date}: {n} Abschnitte, Arbeitszeit {} → {} h{via}", hm(before), hm(after))).await;
    Ok(Json(json!({ "ok": true, "segments": n, "before": before, "after": after })))
}
