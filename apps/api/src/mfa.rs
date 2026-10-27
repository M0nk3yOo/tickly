//! TOTP (Authenticator-App) als zweiter Faktor bei der Passwort-Anmeldung, mit Wiederherstellungscodes.
//! Wer sich mit Passkey anmeldet, braucht keinen zweiten Faktor: der Passkey ist bereits einer.
use crate::api::{auth, St};
use crate::app::*;
use crate::db::{self, UserRow};
use crate::netsec::{self, ClientIp};
use axum::{
    extract::{Path, State},
    Extension,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const RECOVERY_CODES: usize = 10;
const TICKET_TTL: Duration = Duration::from_secs(300);
const MAX_FAILS: u32 = 10;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/auth/totp", post(login_second_step))
        .route("/totp/setup", post(setup))
        .route("/totp/enable", post(enable))
        .route("/totp/disable", post(disable))
        .route("/totp/recovery", post(new_recovery_codes))
        .route("/admin/users/{id}/reset-totp", post(admin_reset))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn make(app: &App, secret_stored: &str, email: &str) -> Result<TOTP, ApiError> {
    let secret_b32 = &app.cfg.secrets.open_str(secret_stored).map_err(|e| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let bytes = Secret::Encoded(secret_b32.to_string()).to_bytes().map_err(|_| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, "TOTP-Geheimnis ist beschädigt".into()))?;
    TOTP::new(Algorithm::SHA1, 6, 0, 30, bytes, Some("Tickly".into()), email.to_string())
        .map_err(|e| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("TOTP: {e}")))
}

/// Passender 30-Sekunden-Schritt (±1 für Uhrabweichung), falls der Code stimmt.
fn matching_step(t: &TOTP, code: &str, at: u64) -> Option<i64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) { return None }
    [-1i64, 0, 1].into_iter().map(|d| (at as i64 + d * 30) as u64).find(|&ts| t.check(&code, ts)).map(|ts| (ts / 30) as i64)
}

fn hash_code(c: &str) -> String {
    Sha256::digest(c.trim().to_uppercase().replace([' ', '_'], "-").as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn new_codes() -> Vec<String> {
    let mut rng = rand::thread_rng();
    (0..RECOVERY_CODES).map(|_| {
        let c: String = (0..8).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect();
        format!("{}-{}", &c[..4], &c[4..])
    }).collect()
}

async fn store_codes(app: &App, uid: &str, codes: &[String]) -> Res<()> {
    let mut tx = app.pool.begin().await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1").bind(uid).execute(&mut *tx).await?;
    for c in codes {
        sqlx::query("INSERT INTO recovery_codes (user_id, code_hash) VALUES ($1, $2)").bind(uid).bind(hash_code(c)).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn recovery_left(app: &App, uid: &str) -> Res<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM recovery_codes WHERE user_id = $1 AND used_at IS NULL").bind(uid).fetch_one(&app.pool).await?)
}

/// Zu viele falsche Codes in kurzer Zeit sperren weitere Versuche für diesen Benutzer.
fn throttle(app: &App, uid: &str) -> Res<()> {
    let mut m = app.mfa_fails.lock().unwrap();
    if let Some((since, n)) = m.get(uid).copied() {
        if since.elapsed() > Duration::from_secs(600) { m.remove(uid); }
        else if n >= MAX_FAILS { return err(axum::http::StatusCode::TOO_MANY_REQUESTS, "Zu viele falsche Codes – bitte in ein paar Minuten erneut versuchen") }
    }
    Ok(())
}
fn failed(app: &App, uid: &str) {
    let mut m = app.mfa_fails.lock().unwrap();
    let e = m.entry(uid.to_string()).or_insert((Instant::now(), 0));
    e.1 += 1;
}

/// Prüft einen TOTP-Code (jeder Code nur einmal) oder einen Wiederherstellungscode.
async fn verify(app: &App, u: &UserRow, code: &str) -> Res<()> {
    throttle(app, &u.id)?;
    let Some(secret) = u.totp_secret.as_deref().filter(|_| u.totp_enabled) else { return bad("TOTP ist nicht eingerichtet") };
    let t = make(app, secret, &u.email)?;
    if let Some(step) = matching_step(&t, code, now_secs()) {
        // Atomar: nur ein neuerer Schritt als der letzte akzeptierte zählt
        let n = sqlx::query("UPDATE users SET totp_last_step = $2 WHERE id = $1 AND totp_last_step < $2").bind(&u.id).bind(step).execute(&app.pool).await?.rows_affected();
        if n == 1 { app.mfa_fails.lock().unwrap().remove(&u.id); return Ok(()) }
        failed(app, &u.id);
        return unauthorized("Dieser Code wurde schon verwendet – bitte den nächsten abwarten");
    }
    let used = sqlx::query("UPDATE recovery_codes SET used_at = now() WHERE id = (SELECT id FROM recovery_codes WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL LIMIT 1)")
        .bind(&u.id).bind(hash_code(code)).execute(&app.pool).await?.rows_affected();
    if used == 1 { app.mfa_fails.lock().unwrap().remove(&u.id); return Ok(()) }
    failed(app, &u.id);
    unauthorized("Der Code ist falsch")
}

/// Wird nach dem Passwort aufgerufen: Ticket für den zweiten Schritt ausstellen.
pub fn issue_ticket(app: &App, uid: &str) -> String {
    let t = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut m = app.mfa_tickets.lock().unwrap();
    m.retain(|_, v| v.1.elapsed() < TICKET_TTL);
    m.insert(t.clone(), (uid.to_string(), Instant::now(), 0));
    t
}

#[derive(Deserialize)]
struct SecondStep { ticket: String, code: String }

async fn login_second_step(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<SecondStep>) -> Res<Response> {
    netsec::guard(&app, ip).await?;
    let uid = {
        let mut m = app.mfa_tickets.lock().unwrap();
        let Some(e) = m.get_mut(&r.ticket) else { return unauthorized("Die Anmeldung ist abgelaufen – bitte erneut mit dem Passwort beginnen") };
        if e.1.elapsed() > TICKET_TTL { m.remove(&r.ticket); return unauthorized("Die Anmeldung ist abgelaufen – bitte erneut mit dem Passwort beginnen") }
        e.2 += 1;
        if e.2 > 5 { m.remove(&r.ticket); return unauthorized("Zu viele Versuche – bitte erneut mit dem Passwort beginnen") }
        e.0.clone()
    };
    let Some(u) = db::user_by_id(&app.pool, &uid).await? else { return unauthorized("Benutzer existiert nicht mehr") };
    if let Err(e) = verify(&app, &u, &r.code).await {
        if e.0 == axum::http::StatusCode::UNAUTHORIZED { netsec::failure(&app, "totp", ip, &h, Some(&u.id), Some(&u.email), &e.1).await; }
        return Err(e);
    }
    netsec::log(&app, "login", true, ip, &h, Some(&u.id), Some(&u.email), "Zweiter Faktor bestätigt").await;
    app.mfa_tickets.lock().unwrap().remove(&r.ticket);
    let reg = db::reg_policy(&app.pool).await?;
    let secs = db::session_secs(&reg, &db::mfa_status_with(&u, 0, &reg));
    let sid = db::create_session(&app.pool, &u.id, false, secs, &ip, &netsec::ua(&h)).await?;
    db::touch_user(&app.pool, &u.id).await;
    let secure = if app.cfg.secure_cookie { "; Secure" } else { "" };
    Ok(([(header::SET_COOKIE, format!("tickly_sid={sid}; Path=/; HttpOnly; SameSite=Lax; Max-Age={secs}{secure}"))], Json(json!({ "ok": true, "me": db::me_json(&u) }))).into_response())
}

/// Neues Geheimnis erzeugen (noch nicht aktiv). Liefert QR-Code und Schlüssel zum Abtippen.
async fn setup(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, true).await?;
    if u.status != "active" { return forbidden("Konto ist nicht aktiv") }
    if u.totp_enabled { return conflict("Die Authenticator-App ist schon eingerichtet. Zum Ändern zuerst deaktivieren.") }
    let mut raw = [0u8; 20];
    rand::thread_rng().fill(&mut raw[..]);
    let secret = Secret::Raw(raw.to_vec()).to_encoded().to_string();
    sqlx::query("UPDATE users SET totp_secret = $2 WHERE id = $1").bind(&u.id).bind(app.cfg.secrets.seal_str(&secret)).execute(&app.pool).await?;
    let t = make(&app, &secret, &u.email)?;
    let qr = t.get_qr_base64().map_err(|e| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("QR-Code: {e}")))?;
    Ok(Json(json!({ "secret": secret, "uri": t.get_url(), "qr": format!("data:image/png;base64,{qr}") })))
}

#[derive(Deserialize)]
struct Code { code: String }

/// Mit dem ersten gültigen Code aktivieren; liefert die Wiederherstellungscodes (nur dieses eine Mal).
async fn enable(State(app): St, h: HeaderMap, Json(r): Json<Code>) -> Res<Json<Value>> {
    let u = auth(&app, &h, true).await?;
    if u.status != "active" { return forbidden("Konto ist nicht aktiv") }
    if u.totp_enabled { return conflict("Schon eingerichtet") }
    throttle(&app, &u.id)?;
    let Some(secret) = u.totp_secret.as_deref() else { return bad("Bitte zuerst die Einrichtung starten") };
    let t = make(&app, secret, &u.email)?;
    let Some(step) = matching_step(&t, &r.code, now_secs()) else { failed(&app, &u.id); return bad("Der Code stimmt nicht – bitte den aktuellen Code aus der App eingeben") };
    sqlx::query("UPDATE users SET totp_enabled = true, totp_last_step = $2 WHERE id = $1").bind(&u.id).bind(step).execute(&app.pool).await?;
    let codes = new_codes();
    store_codes(&app, &u.id, &codes).await?;
    db::extend_sessions(&app.pool, &u.id, db::reg_policy(&app.pool).await?.session_days as i64 * 86400).await?;
    Ok(Json(json!({ "ok": true, "recoveryCodes": codes })))
}

async fn disable(State(app): St, h: HeaderMap, Json(r): Json<Code>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if !u.totp_enabled { return bad("TOTP ist nicht eingerichtet") }
    // Unter MFA-Pflicht bleibt mindestens ein Faktor erhalten
    if db::cred_count(&app.pool, &u.id).await? == 0 && db::reg_policy(&app.pool).await?.mfa_required {
        return bad("Die Authenticator-App kann nicht ausgeschaltet werden, solange Passkey oder Authenticator-App verpflichtend sind. Richte zuerst einen Passkey ein.");
    }
    verify(&app, &u, &r.code).await?;
    sqlx::query("UPDATE users SET totp_enabled = false, totp_secret = NULL, totp_last_step = 0 WHERE id = $1").bind(&u.id).execute(&app.pool).await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1").bind(&u.id).execute(&app.pool).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn new_recovery_codes(State(app): St, h: HeaderMap, Json(r): Json<Code>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if !u.totp_enabled { return bad("TOTP ist nicht eingerichtet") }
    verify(&app, &u, &r.code).await?;
    let codes = new_codes();
    store_codes(&app, &u.id, &codes).await?;
    Ok(Json(json!({ "ok": true, "recoveryCodes": codes })))
}

/// Administrator setzt TOTP zurück, z. B. wenn jemand Handy und Wiederherstellungscodes verloren hat.
async fn admin_reset(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    if me.role != "admin" { return forbidden("Nur für Administratoren") }
    if db::user_by_id(&app.pool, &id).await?.is_none() { return not_found("Benutzer nicht gefunden") }
    sqlx::query("UPDATE users SET totp_enabled = false, totp_secret = NULL, totp_last_step = 0 WHERE id = $1").bind(&id).execute(&app.pool).await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1").bind(&id).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("TOTP zurückgesetzt: {id}")).await;
    Ok(Json(json!({ "ok": true })))
}
