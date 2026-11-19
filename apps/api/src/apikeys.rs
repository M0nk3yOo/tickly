//! Persönliche API-Schlüssel und die Schnittstelle `/api/v1/*`.
//!
//! - Verwaltung (Cookie-Sitzung): Schlüssel anlegen, auflisten, widerrufen; Administratoren sehen und widerrufen alle
//!   und stellen die Richtlinie ein (`/api/apikeys`, `/api/admin/apikeys`, `/api/admin/apikey-policy`).
//! - Nutzung (`Authorization: Bearer tkl_…`): nur unter `/api/v1/*`. Ein Schlüssel öffnet nie die übrigen Pfade
//!   (Passwörter, Passkeys, Administration, Geräte, Sitzungen): diese lesen ausschließlich das Sitzungs-Cookie bzw. Geräte-Token.
//! - Gespeichert wird nur der SHA-256-Hash des Schlüssels plus ein kurzes Präfix zur Wiedererkennung.
use crate::api::{auth, St};
use crate::app::*;
use crate::db::{self, Calc, UserRow};
use crate::events::{self, Ev};
use crate::netsec::{self, ClientIp};
use axum::{
    extract::{Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{types::Json as SJson, FromRow};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};
use tickly_core::*;

/// Präfix aller API-Schlüssel (Geräte-Token beginnen mit `tk_`).
pub const KEY_PREFIX: &str = "tkl_";
/// So viele aktive Schlüssel darf ein Benutzer haben.
pub const MAX_ACTIVE: i64 = 20;
/// Längster abfragbarer Zeitraum (Tage).
const MAX_RANGE_DAYS: i64 = 366;

/* ---------- Richtlinie ---------- */
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyPolicy {
    /// Schlüssel dürfen angelegt und benutzt werden.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Schlüssel ohne Ablauf sind erlaubt.
    #[serde(default = "yes")]
    pub allow_unlimited: bool,
    /// Längste Laufzeit eines befristeten Schlüssels in Tagen.
    #[serde(default = "default_max_days")]
    pub max_days: u32,
}

fn yes() -> bool { true }
fn default_max_days() -> u32 { 365 }

impl Default for KeyPolicy {
    fn default() -> Self { KeyPolicy { enabled: true, allow_unlimited: true, max_days: 365 } }
}

pub async fn policy(app: &App) -> Res<KeyPolicy> {
    let v: Option<(SJson<KeyPolicy>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'apikeys'").fetch_optional(&app.pool).await?;
    Ok(v.map(|x| x.0 .0).unwrap_or_default())
}

/* ---------- Hilfen ---------- */
fn hash_key(t: &str) -> Vec<u8> {
    Sha256::digest(t.as_bytes()).to_vec()
}

fn status_of(revoked: Option<DateTime<Utc>>, expires: Option<DateTime<Utc>>) -> &'static str {
    if revoked.is_some() { "revoked" } else if expires.is_some_and(|e| e <= Utc::now()) { "expired" } else { "active" }
}

#[derive(FromRow)]
struct KeyRow {
    id: i64,
    user_id: String,
    name: String,
    prefix: String,
    can_write: bool,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    user_name: String,
    user_email: String,
}

const KEY_COLS: &str = "k.id, k.user_id, k.name, k.prefix, k.can_write, k.created_at, k.expires_at, k.last_used_at, k.revoked_at, u.name AS user_name, u.email AS user_email";

fn key_json(k: &KeyRow, with_user: bool) -> Value {
    let mut v = json!({
        "id": k.id, "name": k.name, "prefix": k.prefix, "write": k.can_write, "permission": if k.can_write { "write" } else { "read" },
        "createdAt": k.created_at, "expiresAt": k.expires_at, "lastUsedAt": k.last_used_at, "revokedAt": k.revoked_at,
        "status": status_of(k.revoked_at, k.expires_at),
    });
    if with_user {
        v["userId"] = json!(k.user_id);
        v["userName"] = json!(k.user_name);
        v["email"] = json!(k.user_email);
    }
    v
}

fn policy_json(p: &KeyPolicy) -> Value {
    json!({ "enabled": p.enabled, "allowUnlimited": p.allow_unlimited, "maxDays": p.max_days, "maxActive": MAX_ACTIVE })
}

/* ---------- Verwaltung (Cookie-Sitzung) ---------- */
pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/apikeys", get(list_own).post(create))
        .route("/apikeys/{id}", delete(revoke_own))
        .route("/admin/apikeys", get(admin_list))
        .route("/admin/apikeys/{id}", delete(admin_revoke))
        .route("/admin/apikey-policy", put(put_policy))
}

async fn list_own(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let rows: Vec<KeyRow> = sqlx::query_as(&format!("SELECT {KEY_COLS} FROM api_keys k JOIN users u ON u.id = k.user_id WHERE k.user_id = $1 ORDER BY k.id DESC LIMIT 200")).bind(&u.id).fetch_all(&app.pool).await?;
    Ok(Json(json!({ "keys": rows.iter().map(|k| key_json(k, false)).collect::<Vec<_>>(), "policy": policy_json(&policy(&app).await?) })))
}

#[derive(Deserialize)]
struct NewKey {
    name: String,
    /// Laufzeit in Tagen; fehlt sie (oder 0), ist der Schlüssel unbegrenzt gültig.
    days: Option<u32>,
    /// Lesen und Schreiben (sonst nur lesen).
    #[serde(default)]
    write: bool,
}

async fn create(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<NewKey>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let p = policy(&app).await?;
    if !p.enabled { return forbidden("API-Schlüssel sind vom Administrator ausgeschaltet") }
    let name = r.name.trim();
    if name.is_empty() || name.chars().count() > 60 { return bad("Bitte eine Bezeichnung angeben (höchstens 60 Zeichen)") }
    let days = r.days.filter(|d| *d > 0);
    match days {
        None if !p.allow_unlimited => return bad("Unbegrenzt gültige Schlüssel sind vom Administrator nicht erlaubt – bitte eine Laufzeit wählen"),
        Some(d) if d > p.max_days => return bad(format!("Die Laufzeit darf höchstens {} Tage betragen", p.max_days)),
        _ => {}
    }
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM api_keys WHERE user_id = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())").bind(&u.id).fetch_one(&app.pool).await?;
    if active >= MAX_ACTIVE { return conflict(format!("Du hast schon {MAX_ACTIVE} aktive API-Schlüssel – widerrufe zuerst einen, den du nicht mehr brauchst")) }
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let secret: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let token = format!("{KEY_PREFIX}{secret}");
    let prefix = format!("{KEY_PREFIX}{}", &secret[..8]);
    let expires = days.map(|d| Utc::now() + Duration::days(d as i64));
    let id: i64 = sqlx::query_scalar("INSERT INTO api_keys (user_id, name, prefix, token_hash, can_write, expires_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id")
        .bind(&u.id).bind(name).bind(&prefix).bind(hash_key(&token)).bind(r.write).bind(expires).fetch_one(&app.pool).await?;
    netsec::log(&app, "apikey", true, ip, &h, Some(&u.id), Some(&u.email), &format!("API-Schlüssel „{name}“ ({prefix}…) angelegt: {}, {}", if r.write { "lesen und schreiben" } else { "nur lesen" }, match days { Some(d) => format!("{d} Tage gültig"), None => "unbegrenzt gültig".into() })).await;
    let row: KeyRow = sqlx::query_as(&format!("SELECT {KEY_COLS} FROM api_keys k JOIN users u ON u.id = k.user_id WHERE k.id = $1")).bind(id).fetch_one(&app.pool).await?;
    // Der Klartext steht nur in dieser Antwort; danach ist er nirgends mehr abrufbar
    Ok(Json(json!({ "ok": true, "key": token, "item": key_json(&row, false) })))
}

async fn revoke_own(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<i64>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let r: Option<(String, String)> = sqlx::query_as("UPDATE api_keys SET revoked_at = now(), revoked_by = $2 WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL RETURNING name, prefix").bind(id).bind(&u.id).fetch_optional(&app.pool).await?;
    let Some((name, prefix)) = r else { return not_found("Schlüssel nicht gefunden oder schon widerrufen") };
    netsec::log(&app, "apikey", true, ip, &h, Some(&u.id), Some(&u.email), &format!("API-Schlüssel „{name}“ ({prefix}…) widerrufen")).await;
    Ok(Json(json!({ "ok": true })))
}

fn require_admin(u: &UserRow) -> Res<()> {
    if u.role == "admin" { Ok(()) } else { forbidden("Nur für Administratoren") }
}

#[derive(Deserialize)]
struct AdminQ { q: Option<String>, status: Option<String> }

async fn admin_list(State(app): St, h: HeaderMap, Query(q): Query<AdminQ>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    let like = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{}%", s.replace(['%', '_'], "")));
    let status = q.status.as_deref().filter(|s| ["active", "expired", "revoked"].contains(s));
    let rows: Vec<KeyRow> = sqlx::query_as(&format!("SELECT {KEY_COLS} FROM api_keys k JOIN users u ON u.id = k.user_id WHERE ($1::text IS NULL OR k.name ILIKE $1 OR k.prefix ILIKE $1 OR u.name ILIKE $1 OR u.email ILIKE $1) \
        AND ($2::text IS NULL OR ($2 = 'revoked' AND k.revoked_at IS NOT NULL) OR ($2 = 'expired' AND k.revoked_at IS NULL AND k.expires_at <= now()) OR ($2 = 'active' AND k.revoked_at IS NULL AND (k.expires_at IS NULL OR k.expires_at > now()))) \
        ORDER BY k.id DESC LIMIT 500")).bind(like).bind(status).fetch_all(&app.pool).await?;
    let (total, active): (i64, i64) = sqlx::query_as("SELECT count(*), count(*) FILTER (WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now())) FROM api_keys").fetch_one(&app.pool).await?;
    Ok(Json(json!({ "keys": rows.iter().map(|k| key_json(k, true)).collect::<Vec<_>>(), "policy": policy_json(&policy(&app).await?), "total": total, "active": active })))
}

async fn admin_revoke(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<i64>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    let r: Option<(String, String, String)> = sqlx::query_as("UPDATE api_keys k SET revoked_at = now(), revoked_by = $2 WHERE k.id = $1 AND k.revoked_at IS NULL \
        RETURNING k.name, k.prefix, (SELECT email FROM users WHERE id = k.user_id)").bind(id).bind(&me.id).fetch_optional(&app.pool).await?;
    let Some((name, prefix, owner)) = r else { return not_found("Schlüssel nicht gefunden oder schon widerrufen") };
    netsec::admin_action(&app, &me, ip, &h, &format!("API-Schlüssel „{name}“ ({prefix}…) von {owner} widerrufen")).await;
    Ok(Json(json!({ "ok": true })))
}

async fn put_policy(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(p): Json<KeyPolicy>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if !(1..=3650).contains(&p.max_days) { return bad("Die längste Laufzeit muss zwischen 1 und 3650 Tagen liegen") }
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('apikeys', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(&p)).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Richtlinie API-Schlüssel: {}, unbegrenzt {}, längste Laufzeit {} Tage", if p.enabled { "erlaubt" } else { "ausgeschaltet" }, if p.allow_unlimited { "erlaubt" } else { "nicht erlaubt" }, p.max_days)).await;
    Ok(Json(json!({ "ok": true })))
}

/* ---------- Anmeldung mit Schlüssel ---------- */
/// Wer sich mit welchem Schlüssel angemeldet hat; setzt die Anmeldung für die Handler unter `/api/v1`.
#[derive(Clone)]
pub struct ApiCtx {
    pub user: UserRow,
    pub write: bool,
}

/// Ablehnungen bekannter Schlüssel höchstens alle zehn Minuten je Schlüssel und Grund protokollieren (ein Skript mit
/// abgelaufenem Schlüssel würde sonst das Protokoll fluten).
fn log_due(key: i64, reason: u8) -> bool {
    static SEEN: OnceLock<Mutex<HashMap<(i64, u8), Instant>>> = OnceLock::new();
    let mut m = SEEN.get_or_init(Default::default).lock().unwrap();
    if m.len() > 10_000 { m.clear(); }
    let due = m.get(&(key, reason)).is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(600));
    if due { m.insert((key, reason), Instant::now()); }
    due
}

async fn authenticate(app: &Arc<App>, ip: ClientIp, h: &HeaderMap, method: &Method) -> Res<ApiCtx> {
    netsec::guard(app, ip).await?;
    let tok = h.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(str::trim).filter(|t| !t.is_empty());
    let Some(tok) = tok else { return unauthorized("API-Schlüssel fehlt – sende ihn als „Authorization: Bearer tkl_…“") };
    // Falsche oder erratene Schlüssel zählen wie falsche Anmeldungen für die IP-Sperre
    if !tok.starts_with(KEY_PREFIX) {
        netsec::failure(app, "apikey", ip, h, None, None, "Kein API-Schlüssel (Präfix tkl_ fehlt)").await;
        return unauthorized("Ungültiger API-Schlüssel");
    }
    type Row = (i64, String, String, String, bool, Option<DateTime<Utc>>, Option<DateTime<Utc>>);
    let k: Option<Row> = sqlx::query_as("SELECT id, user_id, name, prefix, can_write, expires_at, revoked_at FROM api_keys WHERE token_hash = $1").bind(hash_key(tok)).fetch_optional(&app.pool).await?;
    let Some((id, uid, name, prefix, can_write, expires, revoked)) = k else {
        netsec::failure(app, "apikey", ip, h, None, None, "Unbekannter API-Schlüssel").await;
        return unauthorized("Ungültiger API-Schlüssel");
    };
    let owner = db::user_by_id(&app.pool, &uid).await?;
    let email = owner.as_ref().map(|u| u.email.clone());
    let deny = |reason: u8, code: StatusCode, msg: &str| {
        let (app, uid, email, label, msg) = (app.clone(), uid.clone(), email.clone(), format!("API-Schlüssel „{name}“ ({prefix}…) abgelehnt: {msg}"), msg.to_string());
        let hh = h.clone();
        async move {
            if log_due(id, reason) { netsec::log(&app, "apikey", false, ip, &hh, Some(&uid), email.as_deref(), &label).await; }
            err::<ApiCtx>(code, msg)
        }
    };
    if revoked.is_some() { return deny(1, StatusCode::UNAUTHORIZED, "Der API-Schlüssel wurde widerrufen").await }
    if expires.is_some_and(|e| e <= Utc::now()) { return deny(2, StatusCode::UNAUTHORIZED, "Der API-Schlüssel ist abgelaufen").await }
    if !policy(app).await?.enabled { return deny(3, StatusCode::FORBIDDEN, "API-Schlüssel sind vom Administrator ausgeschaltet").await }
    let Some(user) = owner else { return unauthorized("Benutzer existiert nicht mehr") };
    if user.status != "active" { return deny(4, StatusCode::FORBIDDEN, "Das Konto ist nicht aktiv").await }
    if !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) && !can_write {
        return deny(5, StatusCode::FORBIDDEN, "Dieser API-Schlüssel darf nur lesen").await;
    }
    // „Zuletzt benutzt“ höchstens einmal je Minute schreiben
    let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')").bind(id).execute(&app.pool).await;
    Ok(ApiCtx { user, write: can_write })
}

async fn v1_auth(State(app): St, mut req: Request, next: Next) -> Response {
    let ip = req.extensions().get::<ClientIp>().copied().unwrap_or(ClientIp(std::net::Ipv4Addr::UNSPECIFIED.into(), None));
    match authenticate(&app, ip, &req.headers().clone(), &req.method().clone()).await {
        Ok(cx) => {
            req.extensions_mut().insert(cx);
            let mut res = next.run(req).await;
            res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            res
        }
        Err(e) => e.into_response(),
    }
}

/* ---------- Schnittstelle /api/v1 ---------- */
pub fn v1(app: Arc<App>) -> Router<Arc<App>> {
    Router::new()
        .route("/me", get(me))
        .route("/status", get(status))
        .route("/days", get(days))
        .route("/days/{date}", get(day))
        .route("/balance", get(balance))
        .route("/absences", get(absences).post(abs_create))
        .route("/absences/{id}", put(abs_update).delete(abs_delete))
        .route("/tracking/start", post(t_start))
        .route("/tracking/pause", post(t_pause))
        .route("/tracking/stop", post(t_stop))
        .route("/tracking/planned-end", delete(t_plan_clear))
        .route("/entries", post(entry))
        .route("/export.csv", get(export_csv))
        .layer(axum::middleware::from_fn_with_state(app, v1_auth))
}

fn r2(x: f64) -> f64 { (x * 100.0).round() / 100.0 }

/// Uhrzeit als „HH:MM“ (Minuten seit Mitternacht).
fn clock(m: f64) -> String {
    let t = m.round() as i64;
    format!("{:02}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

fn parse_clock(s: &str) -> Option<f64> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (m < 60 && (h < 24 || (h == 24 && m == 0))).then_some((h * 60 + m) as f64)
}

fn parse_date(s: &str) -> Res<NaiveDate> {
    match NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d") {
        Ok(d) if (2000..=2100).contains(&d.year()) => Ok(d),
        _ => bad("Ungültiges Datum – erwartet wird JJJJ-MM-TT"),
    }
}

#[derive(Deserialize)]
struct RangeQ { from: Option<String>, to: Option<String> }

/// Zeitraum aus `from`/`to`; Standard: erster des Monats bis heute. Höchstens 366 Tage.
fn range(today: NaiveDate, q: &RangeQ) -> Res<(NaiveDate, NaiveDate)> {
    let to = match &q.to { Some(s) => parse_date(s)?, None => today };
    let from = match &q.from { Some(s) => parse_date(s)?, None => to.with_day(1).unwrap_or(to) };
    if to < from { return bad("Das Ende liegt vor dem Beginn") }
    if (to - from).num_days() + 1 > MAX_RANGE_DAYS { return bad(format!("Der Zeitraum darf höchstens {MAX_RANGE_DAYS} Tage umfassen")) }
    Ok((from, to))
}

fn kind_name(k: Option<DayKind>) -> Value {
    serde_json::to_value(k).unwrap_or(Value::Null)
}

/// Tageswerte; `live_open`: Der Tag läuft gerade, ein Ende gibt es noch nicht.
fn day_json(st: &DayStat, live_open: bool) -> Value {
    json!({
        "date": st.date, "worked": st.has, "future": st.future,
        "start": st.start.map(clock), "end": if live_open { None } else { st.end.map(clock) },
        "grossMin": r2(st.gross), "breakMin": r2(st.brk), "netMin": r2(st.net), "countedMin": r2(st.counted),
        "sollMin": r2(st.soll), "diffMin": st.diff.map(r2),
        "holiday": st.info.holiday, "absence": st.info.absence.as_ref().map(|a| kind_name(Some(match a.kind { AbsType::Vacation => DayKind::Vacation, AbsType::Sick => DayKind::Sick, AbsType::Comp => DayKind::Comp }))),
        "kind": kind_name(st.info.kind),
    })
}

async fn me(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    let u = &cx.user;
    Ok(Json(json!({
        "id": u.id, "name": u.name, "email": u.email, "role": u.role, "timezone": app.cfg.tz.name(),
        "version": env!("CARGO_PKG_VERSION"), "permission": if cx.write { "write" } else { "read" },
    })))
}

fn track_state(c: &Calc) -> &'static str {
    if !c.has_day { return "none" }
    match c.day.track() { Track::Running => "running", Track::Locked => "locked", Track::Paused => "paused", Track::Asking => "asking", Track::Idle => "idle", Track::Stopped => "stopped" }
}

async fn status_json(app: &App, u: &UserRow) -> Res<Value> {
    let (today, now) = app.clock(None);
    let calc = Calc::load(app, u, today, now).await?;
    let reg = db::reg_policy(&app.pool).await?;
    let state = track_state(&calc);
    let mut v = calc.with(|c| {
        let st = day_stat(c, today);
        let active = if c.day.tracking { c.day.segs.last().filter(|g| g.to.is_none() && g.kind == SegKind::App).and_then(|g| g.app.as_deref()).map(app_label) } else { None };
        let mut v = day_json(&st, c.day.tracking);
        v["liveDiffMin"] = json!(st.diff_live.map(r2));
        v["balanceMin"] = json!(r2(total_balance(c)));
        v["activeApp"] = json!(active);
        v["plannedEnd"] = json!(c.day.planned_end.map(clock));
        v["nowMin"] = json!(r2(now));
        v
    });
    v["state"] = json!(state);
    v["tracking"] = json!(state == "running" || state == "locked");
    v["limits"] = json!({ "plannedEnd": reg.allow_planned_end, "startChange": reg.allow_start_change, "editDaysBack": reg.edit_days_back });
    Ok(v)
}

async fn status(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    Ok(Json(status_json(&app, &cx.user).await?))
}

async fn days_in(app: &App, u: &UserRow, from: NaiveDate, to: NaiveDate) -> Res<Vec<Value>> {
    let (today, now) = app.clock(None);
    let calc = Calc::load(app, u, today, now).await?;
    Ok(calc.with(|c| {
        let mut out = Vec::new();
        let mut d = from;
        while d <= to {
            out.push(day_json(&day_stat(c, d), d == today && c.day.tracking));
            d += Duration::days(1);
        }
        out
    }))
}

async fn days(State(app): St, Extension(cx): Extension<ApiCtx>, Query(q): Query<RangeQ>) -> Res<Json<Value>> {
    let (from, to) = range(app.clock(None).0, &q)?;
    Ok(Json(json!({ "from": from, "to": to, "days": days_in(&app, &cx.user, from, to).await? })))
}

async fn day(State(app): St, Extension(cx): Extension<ApiCtx>, Path(date): Path<String>) -> Res<Json<Value>> {
    let date = parse_date(&date)?;
    let (today, now) = app.clock(None);
    let calc = Calc::load(&app, &cx.user, today, now).await?;
    Ok(Json(calc.with(|c| {
        let st = day_stat(c, date);
        let mut v = day_json(&st, date == today && c.day.tracking);
        v["segments"] = Value::Array(st.segs.iter().flatten().map(|g| json!({
            "kind": match g.kind { SegKind::App => "app", SegKind::Locked => "locked", SegKind::Off => "off" },
            "from": clock(g.from), "to": g.to.map(clock), "durationMin": r2(g.to.unwrap_or(g.from) - g.from),
            "app": g.app.as_deref().map(app_label), "note": g.note,
        })).collect());
        v
    })))
}

async fn balance(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    let (today, now) = app.clock(None);
    let calc = Calc::load(&app, &cx.user, today, now).await?;
    Ok(Json(calc.with(|c| {
        let ws = today - Duration::days(today.weekday().num_days_from_monday() as i64);
        let ms = today.with_day(1).unwrap_or(today);
        let rs = |from: NaiveDate| { let r = range_stats(c, from, today); json!({ "from": from, "countedMin": r2(r.counted), "sollMin": r2(r.soll), "diffMin": r2(r.diff) }) };
        json!({
            "date": today, "trackStart": c.track_start, "startBalanceMin": r2(c.settings.start_bal * 60.0), "balanceMin": r2(total_balance(c)),
            "week": rs(ws), "month": rs(ms),
        })
    })))
}

fn abs_json(a: &Absence, s: &Settings) -> Value {
    let mut v = serde_json::to_value(a).unwrap_or(Value::Null);
    v["workdays"] = json!(abs_workdays(a.from, a.to, s));
    v
}

async fn absences(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    let list = db::absences(&app.pool, &cx.user.id).await?;
    Ok(Json(json!({ "absences": list.iter().map(|a| abs_json(a, &cx.user.settings)).collect::<Vec<_>>() })))
}

fn abs_kind(k: AbsType) -> &'static str {
    match k { AbsType::Vacation => "vacation", AbsType::Sick => "sick", AbsType::Comp => "comp" }
}

async fn abs_create(State(app): St, Extension(cx): Extension<ApiCtx>, Json(a): Json<Absence>) -> Res<(StatusCode, Json<Value>)> {
    crate::api::check_abs(&a)?;
    let id = db::insert_absence(&app.pool, &cx.user.id, &a).await?;
    Ok((StatusCode::CREATED, Json(json!({ "ok": true, "id": id }))))
}

async fn abs_update(State(app): St, Extension(cx): Extension<ApiCtx>, Path(id): Path<i64>, Json(a): Json<Absence>) -> Res<Json<Value>> {
    crate::api::check_abs(&a)?;
    let n = sqlx::query("UPDATE absences SET kind = $3, date_from = $4, date_to = $5, note = $6 WHERE id = $1 AND user_id = $2")
        .bind(id).bind(&cx.user.id).bind(abs_kind(a.kind)).bind(a.from).bind(a.to).bind(&a.note).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Eintrag nicht gefunden") }
    Ok(Json(json!({ "ok": true })))
}

async fn abs_delete(State(app): St, Extension(cx): Extension<ApiCtx>, Path(id): Path<i64>) -> Res<Json<Value>> {
    let n = sqlx::query("DELETE FROM absences WHERE id = $1 AND user_id = $2").bind(id).bind(&cx.user.id).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Eintrag nicht gefunden") }
    Ok(Json(json!({ "ok": true })))
}

/* ---------- Tracking: dieselbe Logik wie in der Weboberfläche (events::apply) ---------- */
async fn track_event(app: &App, u: &UserRow, ev: Ev) -> Res<()> {
    let abs = db::absences(&app.pool, &u.id).await?;
    let mut ev = ev;
    ev.at = None;
    ev.web = !app.cfg.demo;
    let mut tx = app.pool.begin().await?;
    if let Err(m) = events::apply(app, &mut tx, u, &abs, &ev).await? { return conflict(m) }
    tx.commit().await?;
    Ok(())
}

fn ev(kind: &str) -> Ev { Ev { kind: kind.into(), ..Default::default() } }

async fn t_start(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    track_event(&app, &cx.user, ev("start")).await?;
    Ok(Json(json!({ "ok": true, "status": status_json(&app, &cx.user).await? })))
}

async fn t_pause(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    track_event(&app, &cx.user, ev("pause")).await?;
    Ok(Json(json!({ "ok": true, "status": status_json(&app, &cx.user).await? })))
}

#[derive(Deserialize, Default)]
struct StopReq {
    /// Ende als „HH:MM“. Ohne Angabe endet das Tracking jetzt; eine spätere Uhrzeit legt das Ende vorab fest.
    at: Option<String>,
}

async fn t_stop(State(app): St, Extension(cx): Extension<ApiCtx>, body: Option<Json<StopReq>>) -> Res<Json<Value>> {
    let r = body.map(|b| b.0).unwrap_or_default();
    let (_, now) = app.clock(None);
    let mut planned = false;
    let e = match r.at.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => ev("stop"),
        Some(s) => {
            let Some(at) = parse_clock(s) else { return bad("Ungültige Uhrzeit – erwartet wird HH:MM") };
            if at > now + 1e-6 {
                // Ende vorab festlegen: nur, wenn der Administrator es erlaubt (wie in der Weboberfläche)
                if !db::reg_policy(&app.pool).await?.allow_planned_end { return forbidden("Ein Ende vorab festzulegen ist vom Administrator ausgeschaltet") }
                planned = true;
                Ev { until: Some(at), ..ev("planend") }
            } else {
                Ev { until: Some(at), ..ev("stopat") }
            }
        }
    };
    track_event(&app, &cx.user, e).await?;
    Ok(Json(json!({ "ok": true, "planned": planned, "status": status_json(&app, &cx.user).await? })))
}

async fn t_plan_clear(State(app): St, Extension(cx): Extension<ApiCtx>) -> Res<Json<Value>> {
    track_event(&app, &cx.user, ev("planclear")).await?;
    Ok(Json(json!({ "ok": true, "status": status_json(&app, &cx.user).await? })))
}

/* ---------- Zeit eintragen: dieselbe Logik wie „Zeiten bearbeiten“ (edit::save_day) ---------- */
#[derive(Deserialize)]
struct EntryReq {
    date: String,
    from: String,
    to: String,
    /// Bezeichnung (Standard „Manuell“).
    app: Option<String>,
    note: Option<String>,
}

async fn entry(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Extension(cx): Extension<ApiCtx>, Json(r): Json<EntryReq>) -> Res<(StatusCode, Json<Value>)> {
    let date = parse_date(&r.date)?;
    let (Some(from), Some(to)) = (parse_clock(&r.from), parse_clock(&r.to)) else { return bad("Ungültige Uhrzeit – erwartet wird HH:MM") };
    let seg = crate::edit::EditSeg { from, to: Some(to), kind: "app".into(), app: r.app, note: r.note };
    let res = crate::edit::save_day(&app, &cx.user, date, vec![seg], true, ip, &h, " (über API-Schlüssel)").await?;
    Ok((StatusCode::CREATED, res))
}

/* ---------- Export ---------- */
async fn export_csv(State(app): St, Extension(cx): Extension<ApiCtx>, Query(q): Query<RangeQ>) -> Res<Response> {
    let (today, now) = app.clock(None);
    let (from, to) = range(today, &q)?;
    let calc = Calc::load(&app, &cx.user, today, now).await?;
    let dur = |m: f64| { let t = m.round() as i64; format!("{}{}:{:02}", if t < 0 { "-" } else { "" }, t.abs() / 60, t.abs() % 60) };
    let mut out = String::from("\u{feff}Datum;Beginn;Ende;Brutto;Pause;Gezählt;Soll;Saldo;Hinweis\n");
    calc.with(|c| {
        let mut d = from.max(c.track_start);
        while d <= to {
            let s = day_stat(c, d);
            if s.has || s.soll > 0.0 || s.info.kind.is_some() {
                let note = match (&s.info.holiday, s.info.kind) { (Some(h), _) => h.clone(), (None, Some(DayKind::Vacation)) => "Urlaub".into(), (None, Some(DayKind::Sick)) => "Krank".into(), (None, Some(DayKind::Comp)) => "Zeitausgleich".into(), _ => String::new() };
                let saldo = if s.future { String::new() } else if s.info.kind.is_some() && !s.has && s.info.kind != Some(DayKind::Comp) { "nicht gewertet".into() } else { dur(s.diff.unwrap_or(0.0)) };
                let live = d == today && c.day.tracking;
                out.push_str(&format!("{d};{};{};{};{};{};{};{saldo};{}\n", s.start.filter(|_| s.has).map(clock).unwrap_or_default(), if live { String::new() } else { s.end.filter(|_| s.has).map(clock).unwrap_or_default() },
                    dur(s.gross), dur(s.brk), dur(s.counted), dur(s.soll), note.replace(';', ",")));
            }
            d += Duration::days(1);
        }
    });
    let mut res = out.into_response();
    res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv; charset=utf-8"));
    res.headers_mut().insert(header::CONTENT_DISPOSITION, HeaderValue::from_static("attachment; filename=\"tickly-export.csv\""));
    Ok(res)
}
