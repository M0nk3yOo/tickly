//! Companion-Geräte: Kopplung per Einmalcode, Token-Anmeldung, Ereignisse und Zustand.
use crate::api::{auth, St};
use crate::app::*;
use crate::db::{self, UserRow};
use crate::events::{self, Ev};
use crate::netsec::{self, ClientIp};
use axum::{
    extract::{Path, State},
    Extension,
    http::{header, HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use rand::{distributions::Slice, Rng, RngCore};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::time::{Duration as StdDuration, Instant};
use tickly_core::*;
use uuid::Uuid;

const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const MAX_BATCH: usize = 200;
/// So lange gilt ein Companion als aktiv, nachdem er zuletzt Ereignisse gemeldet hat (er meldet alle 30 Sekunden).
const CLASH_LIVE_SECS: f64 = 90.0;

/// Zwei Companions melden gleichzeitig, und der zweite hat das Tracking gestartet: wer startete, wer lief schon.
/// Die Warnung endet mit der Bestätigung oder sobald einer der beiden nicht mehr meldet.
pub async fn clash(app: &App, uid: &str, today: NaiveDate) -> Res<Option<Value>> {
    let r: Option<(String, String)> = sqlx::query_as("SELECT s.name, f.name FROM device_clash c JOIN devices f ON f.id = c.first JOIN devices s ON s.id = c.second
        WHERE c.user_id = $1 AND c.day = $2 AND NOT c.ack AND f.revoked_at IS NULL AND s.revoked_at IS NULL
        AND f.last_event_at > now() - make_interval(secs => $3) AND s.last_event_at > now() - make_interval(secs => $3)")
        .bind(uid).bind(today).bind(CLASH_LIVE_SECS).fetch_optional(&app.pool).await?;
    Ok(r.map(|(starter, other)| json!({ "starter": starter, "other": other })))
}

pub async fn clash_ack(app: &App, uid: &str) -> Res<()> {
    sqlx::query("UPDATE device_clash SET ack = true WHERE user_id = $1").bind(uid).execute(&app.pool).await?;
    Ok(())
}

#[derive(FromRow)]
pub struct DeviceRow {
    pub id: Uuid,
    pub user_id: String,
    pub name: String,
    pub os: String,
    pub version: String,
    pub last_seq: i64,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    /// Das Tracking darf auf diesem Gerät von selbst starten (Zeitfenster).
    pub auto_track: bool,
}

fn hash_token(t: &str) -> Vec<u8> {
    Sha256::digest(t.as_bytes()).to_vec()
}

/// Meldet ein Gerät anhand des Bearer-Tokens an.
async fn auth_device(app: &App, h: &HeaderMap) -> Res<(DeviceRow, UserRow)> {
    let tok = h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(str::trim);
    let Some(tok) = tok.filter(|t| !t.is_empty()) else { return unauthorized("Gerät nicht gekoppelt") };
    let dev: Option<DeviceRow> = sqlx::query_as("SELECT id, user_id, name, os, version, last_seq, created_at, last_seen_at, auto_track FROM devices WHERE token_hash = $1 AND revoked_at IS NULL")
        .bind(hash_token(tok)).fetch_optional(&app.pool).await?;
    let Some(mut dev) = dev else { return unauthorized("Kopplung wurde aufgehoben – bitte das Gerät neu koppeln") };
    let Some(u) = db::user_by_id(&app.pool, &dev.user_id).await? else { return unauthorized("Benutzer existiert nicht mehr") };
    if u.status != "active" { return forbidden("Konto ist nicht aktiv") }
    // Die App meldet ihre Version im User-Agent („tickly-companion/0.2.0“); so sieht man veraltete Geräte
    if let Some(v) = h.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("tickly-companion/")).map(|v| v.split_whitespace().next().unwrap_or("").to_string()).filter(|v| !v.is_empty() && v != &dev.version) {
        let _ = sqlx::query("UPDATE devices SET version = $2 WHERE id = $1").bind(dev.id).bind(&v).execute(&app.pool).await;
        dev.version = v;
    }
    Ok((dev, u))
}

/// Einmalcode für die Kopplung eines Geräts erzeugen (gilt 10 Minuten).
pub async fn pair_code(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    sqlx::query("DELETE FROM pair_codes WHERE expires_at < now()").execute(&app.pool).await?;
    let open: i64 = sqlx::query_scalar("SELECT count(*) FROM pair_codes WHERE user_id = $1").bind(&u.id).fetch_one(&app.pool).await?;
    if open >= 5 { return err(StatusCode::TOO_MANY_REQUESTS, "Es sind schon mehrere Codes offen – bitte kurz warten oder einen verwenden") }
    let code = {
        let mut rng = rand::thread_rng();
        let alpha = Slice::new(CODE_ALPHABET).unwrap();
        let c: String = (0..8).map(|_| *rng.sample(alpha) as char).collect();
        format!("{}-{}", &c[..4], &c[4..])
    };
    sqlx::query("INSERT INTO pair_codes (code, user_id, expires_at) VALUES ($1, $2, now() + interval '10 minutes')").bind(&code).bind(&u.id).execute(&app.pool).await?;
    Ok(Json(json!({ "code": code, "expiresIn": 600 })))
}

#[derive(Deserialize)]
pub struct PairReq { code: String, name: String, os: String, version: Option<String> }

/// Gerät mit Code koppeln und Token ausgeben (öffentlich, aber gebremst).
pub async fn pair(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<PairReq>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    {
        let mut f = app.pair_fails.lock().unwrap();
        if f.0.elapsed() > StdDuration::from_secs(600) { *f = (Instant::now(), 0); }
        if f.1 >= 15 { return err(StatusCode::TOO_MANY_REQUESTS, "Zu viele Fehlversuche – bitte in ein paar Minuten erneut versuchen") }
    }
    let code = r.code.trim().to_uppercase().replace([' ', '_'], "-");
    let code = if code.len() == 8 && !code.contains('-') { format!("{}-{}", &code[..4], &code[4..]) } else { code };
    let uid: Option<String> = sqlx::query_scalar("DELETE FROM pair_codes WHERE code = $1 AND expires_at > now() RETURNING user_id").bind(&code).fetch_optional(&app.pool).await?;
    let Some(uid) = uid else {
        app.pair_fails.lock().unwrap().1 += 1;
        netsec::failure(&app, "device_pair", ip, &h, None, None, "Kopplungscode ungültig oder abgelaufen").await;
        return bad("Der Code ist ungültig oder abgelaufen");
    };
    let name = r.name.trim();
    if name.is_empty() || name.chars().count() > 60 { return bad("Bitte einen Gerätenamen angeben") }
    let os = if r.os.to_lowercase().contains("mac") { "macos" } else { "windows" };
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = format!("tk_{}", raw.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO devices (id, user_id, name, os, version, token_hash, last_seen_at) VALUES ($1, $2, $3, $4, $5, $6, now())")
        .bind(id).bind(&uid).bind(name).bind(os).bind(r.version.unwrap_or_default()).bind(hash_token(&token)).execute(&app.pool).await?;
    let u = db::user_by_id(&app.pool, &uid).await?.expect("Benutzer");
    netsec::log(&app, "device_pair", true, ip, &h, Some(&uid), Some(&u.email), &format!("Gerät gekoppelt: {name} ({os})")).await;
    Ok(Json(json!({ "token": token, "deviceId": id, "user": { "name": u.name, "email": u.email } })))
}

pub async fn list_for(app: &App, uid: &str) -> Res<Vec<Value>> {
    let rows: Vec<DeviceRow> = sqlx::query_as("SELECT id, user_id, name, os, version, last_seq, created_at, last_seen_at, auto_track FROM devices WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at").bind(uid).fetch_all(&app.pool).await?;
    let now = Utc::now();
    Ok(rows.into_iter().map(|d| json!({
        "id": d.id, "name": d.name, "os": d.os, "version": d.version, "autoTrack": d.auto_track,
        "online": d.last_seen_at.is_some_and(|t| now - t < Duration::minutes(3)),
        "lastSeen": d.last_seen_at.map(|t| db::ago_label(t, now)).unwrap_or_else(|| "nie".into()),
    })).collect())
}

pub async fn revoke(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let n = sqlx::query("UPDATE devices SET revoked_at = now() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL").bind(id).bind(&u.id).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Gerät nicht gefunden") }
    netsec::log(&app, "device_pair", true, ip, &h, Some(&u.id), Some(&u.email), "Kopplung eines Geräts aufgehoben").await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct SeqEv {
    pub seq: i64,
    #[serde(flatten)]
    pub ev: Ev,
}

#[derive(Deserialize)]
pub struct EventsReq { events: Vec<SeqEv> }

/// Ereignisse eines Geräts entgegennehmen (idempotent über `seq`) und den aktuellen Zustand zurückgeben.
pub async fn device_events(State(app): St, h: HeaderMap, Json(r): Json<EventsReq>) -> Res<Json<Value>> {
    let (dev, u) = auth_device(&app, &h).await?;
    if r.events.len() > MAX_BATCH { return bad("Zu viele Ereignisse auf einmal") }
    let abs = db::absences(&app.pool, &u.id).await?;
    let mut last = dev.last_seq;
    let mut results = Vec::new();
    // Startet dieses Gerät das Tracking, während schon ein anderes Gerät meldet und der Tag läuft: Warnung
    let (today, _) = app.clock(None);
    let mut rival: Option<Uuid> = None;
    if r.events.iter().any(|e| matches!(e.ev.kind.as_str(), "boot" | "start" | "answer")) {
        let others: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM devices WHERE user_id = $1 AND id <> $2 AND revoked_at IS NULL AND last_event_at > now() - make_interval(secs => $3) ORDER BY last_event_at DESC")
            .bind(&u.id).bind(dev.id).bind(CLASH_LIVE_SECS).fetch_all(&app.pool).await?;
        if let Some((o,)) = others.first() {
            let mut c = app.pool.acquire().await?;
            if db::get_day(&mut c, &u.id, today).await?.is_some_and(|d| d.tracking) { rival = Some(*o); }
        }
    }
    let mut started = false;
    for mut e in r.events {
        e.ev.no_auto = !dev.auto_track;
        if e.seq <= last { results.push(json!({ "seq": e.seq, "ok": true, "duplicate": true })); continue }
        let mut tx = app.pool.begin().await?;
        let res = events::apply(&app, &mut tx, &u, &abs, &e.ev).await;
        match res {
            Ok(Ok(())) => {
                started |= matches!(e.ev.kind.as_str(), "boot" | "start" | "answer");
                tx.commit().await?; results.push(json!({ "seq": e.seq, "ok": true }));
            }
            // passt nicht zum Zustand oder zu alt: nicht wiederholen, aber nicht blockieren
            Ok(Err(m)) => { tx.rollback().await?; results.push(json!({ "seq": e.seq, "ok": false, "error": m })); }
            Err(ApiError(c, m)) if c == StatusCode::BAD_REQUEST => { tx.rollback().await?; results.push(json!({ "seq": e.seq, "ok": false, "error": m })); }
            Err(e) => return Err(e),
        }
        last = e.seq;
    }
    sqlx::query("UPDATE devices SET last_seq = $2, last_seen_at = now(), last_event_at = now() WHERE id = $1").bind(dev.id).bind(last).execute(&app.pool).await?;
    if let (true, Some(first)) = (started, rival) {
        sqlx::query("INSERT INTO device_clash (user_id, day, first, second) VALUES ($1, $2, $3, $4) ON CONFLICT (user_id) DO UPDATE SET day = $2, first = $3, second = $4, at = now(), ack = false")
            .bind(&u.id).bind(today).bind(first).bind(dev.id).execute(&app.pool).await?;
    }
    let state = state_json(&app, &u, last, dev.auto_track, &dev).await?;
    Ok(Json(json!({ "ok": true, "lastSeq": last, "results": results, "state": state })))
}

pub async fn device_state(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let (dev, u) = auth_device(&app, &h).await?;
    sqlx::query("UPDATE devices SET last_seen_at = now() WHERE id = $1").bind(dev.id).execute(&app.pool).await?;
    Ok(Json(state_json(&app, &u, dev.last_seq, dev.auto_track, &dev).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReq { auto_track: bool }

/// Das Gerät selbst (Companion) stellt ein, ob das Tracking hier von selbst startet; die Antwort ist der neue Zustand.
pub async fn device_prefs(State(app): St, h: HeaderMap, Json(r): Json<AutoReq>) -> Res<Json<Value>> {
    let (dev, u) = auth_device(&app, &h).await?;
    sqlx::query("UPDATE devices SET auto_track = $2, last_seen_at = now() WHERE id = $1").bind(dev.id).bind(r.auto_track).execute(&app.pool).await?;
    Ok(Json(state_json(&app, &u, dev.last_seq, r.auto_track, &dev).await?))
}

/// Dasselbe aus der Weboberfläche für ein gekoppeltes Gerät des Benutzers.
pub async fn set_auto(State(app): St, h: HeaderMap, Path(id): Path<Uuid>, Json(r): Json<AutoReq>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let n = sqlx::query("UPDATE devices SET auto_track = $3 WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL").bind(id).bind(&u.id).bind(r.auto_track).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Gerät nicht gefunden") }
    Ok(Json(json!({ "ok": true })))
}

/// Was der Companion anzeigen und beachten muss.
async fn state_json(app: &App, u: &UserRow, last_seq: i64, auto: bool, dev: &DeviceRow) -> Res<Value> {
    let (today, now) = app.clock(None);
    let c = db::Calc::load(app, u, today, now).await?;
    let ap = c.absences.clone();
    let (has_day, tz) = (c.has_day, app.cfg.tz.name());
    let mfa = db::mfa_status(&app.pool, u, &db::reg_policy(&app.pool).await?).await?.json();
    let p = track_policy(&c.settings, &ap, today, now);
    let clash = clash(app, &u.id, today).await?;
    let update = crate::update::for_device(app, &dev.os, &dev.version).await;
    Ok(c.with(|c| {
        let t = day_stat(c, c.today);
        let d = c.day;
        let tr = match d.track() {
            Track::Running => "running", Track::Locked => "locked", Track::Paused => "paused",
            Track::Asking => "asking", Track::Idle => "idle", Track::Stopped => "stopped",
        };
        let s = c.settings;
        json!({
            "tz": tz, "mfa": mfa, "settings": s, "day": if has_day { json!(d) } else { Value::Null },
            "serverTime": Utc::now(), "lastSeq": last_seq, "today": today, "now": now,
            "user": { "name": u.name },
            "track": tr, "tracking": d.tracking, "stop": d.stop, "since": d.segs.last().map(|g| g.from),
            "start": t.start, "counted": t.counted, "soll": t.soll, "diff": t.diff_live, "dayOff": t.info.off, "holiday": t.info.holiday,
            "balance": total_balance(c),
            "windowActive": auto && p.auto_from.is_some(),
            "nightCheck": night_check(c).map(|n| json!({ "since": n.since, "hours": n.hours })),
            "deviceClash": clash.filter(|_| d.tracking),
            "update": update,
            "policy": { "trackApps": s.track_apps, "trackLock": s.track_lock, "trackTitles": s.track_titles, "excluded": s.excluded, "idle": s.idle, "autoTrack": s.auto_track, "deviceAuto": auto },
        })
    }))
}
