//! HTTP-API: Authentifizierung (Passwort + Passkeys), Snapshot, Einstellungen, Abwesenheiten, Companion-Ereignisse, Admin.
use crate::app::*;
use crate::db::{self, Calc, RegPolicy, UserRow};
use crate::device;
use crate::events::{self, Ev};
use crate::netsec::{self, ClientIp};
use axum::{
    extract::{Path, Query, State},
    Extension,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::types::Json as SJson;
use std::{collections::HashMap, sync::Arc};
use tickly_core::*;
use tower_http::{compression::CompressionLayer, services::{ServeDir, ServeFile}, set_header::SetResponseHeaderLayer};
use uuid::Uuid;
use webauthn_rs::prelude::{PublicKeyCredential, RegisterPublicKeyCredential};

pub type St = State<Arc<App>>;
const COOKIE: &str = "tickly_sid";

pub fn router(app: Arc<App>) -> Router {
    let index = app.cfg.web_dir.join("index.html");
    let web = ServeDir::new(&app.cfg.web_dir).not_found_service(ServeFile::new(index));
    let api = Router::new()
        .route("/health", get(health))
        .route("/config", get(config))
        .route("/session", get(session))
        .route("/setup", post(setup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/register", post(register))
        .route("/passkey/register/start", post(pk_register_start))
        .route("/passkey/register/finish", post(pk_register_finish))
        .route("/passkey/login/start", post(pk_login_start))
        .route("/passkey/login/finish", post(pk_login_finish))
        .route("/passkeys/{id}", put(pk_rename).delete(pk_delete))
        .route("/snapshot", get(snapshot))
        .route("/today", get(today))
        .route("/settings", put(put_settings))
        .route("/me", post(put_me))
        .route("/me/start-balance", post(set_start_balance))
        .route("/me/track-start", post(set_track_start))
        .route("/absences", post(abs_create))
        .route("/absences/{id}", put(abs_update).delete(abs_delete))
        .route("/companion", post(companion))
        .route("/tracking/stop-preview", post(stop_preview))
        .route("/tracking/night/confirm", post(night_confirm))
        .route("/tracking/device/ack", post(clash_ack))
        .route("/tracking/night/stop", post(night_stop))
        .route("/devices/pair-code", post(device::pair_code))
        .route("/devices/{id}", delete(device::revoke))
        .route("/device/pair", post(device::pair))
        .route("/device/events", post(device::device_events))
        .route("/device/state", get(device::device_state))
        .route("/admin/registration", put(admin_registration))
        .route("/admin/users", post(admin_invite))
        .route("/admin/users/{id}", put(admin_update).delete(admin_delete))
        .route("/admin/users/{id}/status", post(admin_status))
        .route("/admin/users/{id}/reset-passkeys", post(admin_reset_passkeys));
    let api = api.merge(crate::backup::routes()).merge(crate::mfa::routes()).merge(crate::secq::routes()).merge(crate::netsec::routes()).merge(crate::sessions::routes()).merge(crate::edit::routes()).merge(crate::mail::routes());
    Router::new()
        .nest("/api", api)
        .fallback_service(web)
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-cache")))
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn_with_state(app.clone(), crate::netsec::origin_layer))
        .layer(axum::middleware::from_fn_with_state(app.clone(), crate::netsec::client_ip_layer))
        .with_state(app)
}

async fn health(State(app): St) -> Res<&'static str> {
    sqlx::query("SELECT 1").execute(&app.pool).await?;
    Ok("ok")
}

/* ---------- Sitzungen ---------- */
pub fn sid_from(h: &HeaderMap) -> Option<String> {
    let raw = h.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').filter_map(|p| p.trim().strip_prefix(&format!("{COOKIE}="))).next().map(String::from)
}

fn cookie(app: &App, sid: &str, max_age: i64) -> (header::HeaderName, String) {
    let secure = if app.cfg.secure_cookie { "; Secure" } else { "" };
    (header::SET_COOKIE, format!("{COOKIE}={sid}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"))
}

/// Neue Sitzung für ein Konto; die Dauer hängt von der Richtlinie ab (ohne MFA unter Pflicht: 24 Stunden).
async fn start_session(app: &App, u: &UserRow, limited: bool, pw_change: bool, ip: &ClientIp, h: &HeaderMap) -> Res<(header::HeaderName, String)> {
    let reg = db::reg_policy(&app.pool).await?;
    let m = db::mfa_status(&app.pool, u, &reg).await?;
    let secs = db::session_secs(&reg, &m);
    let sid = db::create_session(&app.pool, &u.id, limited, pw_change, secs, ip, &netsec::ua(h)).await?;
    Ok(cookie(app, &sid, secs))
}

/// Prüft die Sitzung und liefert den Benutzer. Eingeschränkte Sitzungen nur, wenn `allow_limited`.
pub async fn auth(app: &App, h: &HeaderMap, allow_limited: bool) -> Res<UserRow> {
    let Some(sid) = sid_from(h) else { return unauthorized("Nicht angemeldet") };
    let s: Option<(String, bool, chrono::DateTime<Utc>, bool)> = sqlx::query_as("SELECT user_id, limited, created_at, pw_change FROM sessions WHERE id = $1 AND expires_at > now()").bind(&sid).fetch_optional(&app.pool).await?;
    let Some((uid, limited, created, pw_change)) = s else { return unauthorized("Sitzung abgelaufen") };
    let Some(u) = db::user_by_id(&app.pool, &uid).await? else { return unauthorized("Benutzer existiert nicht mehr") };
    let _ = sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE id = $1 AND last_seen_at < now() - interval '1 minute'").bind(&sid).execute(&app.pool).await;
    let ok = u.status == "active" || (allow_limited && limited && u.status == "pending");
    if !ok { return forbidden("Konto ist nicht aktiv") }
    if limited && !allow_limited { return forbidden("Konto wartet auf Freigabe") }
    // Initialpasswort eines Administrators: bis zur Änderung geht nichts anderes
    if pw_change && u.must_change_pw && !allow_limited { return forbidden("Bitte lege zuerst ein neues Passwort fest.") }
    if u.status == "active" {
        let reg = db::reg_policy(&app.pool).await?;
        if reg.mfa_required {
            let m = db::mfa_status(&app.pool, &u, &reg).await?;
            if !m.compliant {
                // Ohne Passkey/Authenticator gilt eine Sitzung höchstens 24 Stunden, danach muss man sich neu anmelden
                if Utc::now() - created > Duration::hours(24) {
                    sqlx::query("DELETE FROM sessions WHERE id = $1").bind(&sid).execute(&app.pool).await?;
                    return unauthorized("Sitzung abgelaufen – bitte erneut anmelden");
                }
                // Karenzzeit vorbei: nur noch die Einrichtung (Passkey, Authenticator-App) ist erlaubt
                if m.expired && !allow_limited { return forbidden("Bitte richte zuerst einen Passkey oder eine Authenticator-App ein.") }
            }
        }
    }
    Ok(u)
}

fn require_admin(u: &UserRow) -> Res<()> {
    if u.role == "admin" { Ok(()) } else { forbidden("Nur für Administratoren") }
}

fn valid_email(e: &str) -> bool {
    let e = e.trim();
    !e.contains(char::is_whitespace) && e.split('@').count() == 2 && e.split('@').nth(1).map(|d| d.contains('.') && !d.starts_with('.') && !d.ends_with('.')).unwrap_or(false) && !e.starts_with('@')
}

fn fmt_de(d: NaiveDate) -> String { d.format("%d.%m.%Y").to_string() }

/* ---------- Öffentlich ---------- */
async fn config(State(app): St) -> Res<Json<Value>> {
    let reg = db::reg_policy(&app.pool).await?;
    let (today, now) = app.clock(None);
    Ok(Json(json!({
        "today": today, "trackStart": app.cfg.demo_track_start, "now": now, "demo": app.cfg.demo,
        "registrationOpen": reg.open, "approval": reg.approval, "domains": reg.domains,
        "minPw": reg.min_pw, "passkeyPrompt": reg.passkey_prompt,
        "origin": app.cfg.public_url.as_str().trim_end_matches('/'), "rpId": app.cfg.rp_id,
        // Nur mit dem bekannten Standard-Passwort im Demo-Modus zeigt die Oberfläche Demo-Zugänge an.
        "demoHint": app.cfg.demo && app.cfg.demo_password == DEFAULT_DEMO_PASSWORD,
        // Noch kein Benutzer: Die Oberfläche zeigt die Ersteinrichtung
        "setup": app.setup_code.lock().unwrap().is_some(),
        "secQuestions": reg.sec_questions, "questionList": crate::secq::QUESTIONS,
    })))
}

async fn session(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    Ok(Json(match auth(&app, &h, true).await {
        Ok(u) => {
            let reg = db::reg_policy(&app.pool).await?;
            let m = db::mfa_status(&app.pool, &u, &reg).await?;
            let pw_change = u.must_change_pw && match sid_from(&h) { Some(sid) => sqlx::query_scalar::<_, bool>("SELECT pw_change FROM sessions WHERE id = $1").bind(sid).fetch_optional(&app.pool).await?.unwrap_or(false), None => false };
            // „Eingeschränkt“: Konto wartet auf Freigabe, ein Initialpasswort ist zu ändern oder die Karenzzeit für MFA ist vorbei
            let (limited, reason) = if u.status != "active" { (true, "approval") } else if pw_change { (true, "password") } else if m.expired { (true, "mfa") } else { (false, "") };
            json!({ "loggedIn": true, "limited": limited, "reason": reason, "me": db::me_json(&u), "mfa": m.json(), "needsQuestions": pw_change && crate::secq::needs_questions(&app, &u).await? })
        }
        Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, m)) => return err(StatusCode::INTERNAL_SERVER_ERROR, m),
        Err(_) => json!({ "loggedIn": false }),
    }))
}

#[derive(Deserialize)]
struct LoginReq { email: String, password: String }

fn status_error(u: &UserRow) -> Option<ApiError> {
    match u.status.as_str() {
        "pending" => Some(ApiError(StatusCode::FORBIDDEN, "Dein Konto wartet noch auf die Freigabe durch einen Administrator.".into())),
        "blocked" => Some(ApiError(StatusCode::FORBIDDEN, "Dein Konto wurde gesperrt.".into())),
        "invited" => Some(ApiError(StatusCode::FORBIDDEN, "Die Einladung wurde noch nicht angenommen – bitte zuerst registrieren.".into())),
        _ => None,
    }
}

async fn login(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<LoginReq>) -> Res<Response> {
    netsec::guard(&app, ip).await?;
    let email = r.email.trim().to_string();
    // Konstante Antwort bei unbekanntem Benutzer, um keine Konten zu verraten.
    let Some(u) = db::user_by_email(&app.pool, &r.email).await? else {
        netsec::failure(&app, "login", ip, &h, None, Some(&email), "Unbekanntes Konto").await;
        return unauthorized("E-Mail oder Passwort falsch");
    };
    let ok = u.password_hash.as_deref().map(|hh| verify_password(hh, &r.password)).unwrap_or(false);
    if !ok {
        netsec::failure(&app, "login", ip, &h, Some(&u.id), Some(&u.email), "Passwort falsch").await;
        return unauthorized("E-Mail oder Passwort falsch");
    }
    if crate::mail::needs_verification(&app, &u).await? {
        netsec::log(&app, "login", false, ip, &h, Some(&u.id), Some(&u.email), "E-Mail-Adresse noch nicht bestätigt").await;
        return forbidden("Bitte bestätige zuerst deine E-Mail-Adresse – wir haben dir einen Link geschickt.");
    }
    if let Some(e) = status_error(&u) {
        netsec::log(&app, "login", false, ip, &h, Some(&u.id), Some(&u.email), &e.1).await;
        return Err(e);
    }
    if !u.settings.pw_login && db::cred_count(&app.pool, &u.id).await? > 0 {
        netsec::log(&app, "login", false, ip, &h, Some(&u.id), Some(&u.email), "Passwort-Anmeldung ist für dieses Konto abgeschaltet").await;
        return forbidden("Die Anmeldung mit Passwort ist deaktiviert – bitte Passkey verwenden.");
    }
    // Pflicht zur Zwei-Faktor-Anmeldung: Ein Passwort allein ist kein zweiter Faktor. Wer einen Passkey, aber keine
    // Authenticator-App hat, meldet sich mit dem Passkey an.
    if !u.totp_enabled && db::reg_policy(&app.pool).await?.mfa_required && db::cred_count(&app.pool, &u.id).await? > 0 {
        netsec::log(&app, "login", false, ip, &h, Some(&u.id), Some(&u.email), "Passwort allein genügt nicht (MFA-Pflicht), Passkey verwenden").await;
        return forbidden("Bitte melde dich mit deinem Passkey an – ein Passwort allein genügt nicht mehr.");
    }
    // Mit Authenticator-App: erst nach dem zweiten Schritt (POST /api/auth/totp) gibt es eine Sitzung
    if u.totp_enabled {
        netsec::log(&app, "login", true, ip, &h, Some(&u.id), Some(&u.email), "Passwort richtig, zweiter Faktor folgt").await;
        return Ok(Json(json!({ "ok": true, "needsTotp": true, "ticket": crate::mfa::issue_ticket(&app, &u.id) })).into_response());
    }
    let c = start_session(&app, &u, false, u.must_change_pw, &ip, &h).await?;
    db::touch_user(&app.pool, &u.id).await;
    netsec::log(&app, "login", true, ip, &h, Some(&u.id), Some(&u.email), "Passwort").await;
    Ok(([c], Json(json!({ "ok": true, "me": db::me_json(&u) }))).into_response())
}

async fn logout(State(app): St, h: HeaderMap) -> Res<Response> {
    if let Some(sid) = sid_from(&h) {
        sqlx::query("DELETE FROM sessions WHERE id = $1").bind(sid).execute(&app.pool).await?;
    }
    Ok(([cookie(&app, "", 0)], Json(json!({ "ok": true }))).into_response())
}

#[derive(Deserialize)]
struct SetupReq { code: String, name: String, email: String, password: String, #[serde(default)] questions: Vec<crate::secq::Qa>, #[serde(rename = "startBal")] start_bal: Option<f64> }

/// Ersteinrichtung: legt den ersten Administrator an. Erfordert den Einrichtungscode aus dem Server-Protokoll.
async fn setup(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<SetupReq>) -> Res<Response> {
    netsec::guard(&app, ip).await?;
    let expected = app.setup_code.lock().unwrap().clone();
    let Some(expected) = expected else { return conflict("Die Einrichtung ist bereits abgeschlossen. Bitte melde dich an.") };
    let (name, email) = (r.name.trim().to_string(), r.email.trim().to_string());
    let (a, b) = (r.code.trim().as_bytes(), expected.as_bytes());
    if a.len() != b.len() || a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) != 0 {
        netsec::failure(&app, "setup", ip, &h, None, Some(&email), "Einrichtungscode falsch").await;
        return forbidden("Der Einrichtungscode stimmt nicht. Du findest ihn im Server-Protokoll (docker compose logs api).");
    }
    if name.is_empty() { return bad("Bitte Namen eingeben") }
    if !valid_email(&email) { return bad("Bitte gültige E-Mail-Adresse eingeben") }
    if let Some(m) = password_problem(&r.password, MIN_PW) { return bad(m) }
    if r.start_bal.is_some_and(|b| !b.is_finite() || b.abs() > 2000.0) { return bad("Der Übertrag muss zwischen −2000 und +2000 Stunden liegen") }
    // Sicherheitsfragen sind in einer neuen Installation standardmäßig an
    if db::reg_policy(&app.pool).await?.sec_questions { crate::secq::validate(&r.questions).map_err(|m| ApiError(StatusCode::BAD_REQUEST, m))?; }
    let Some(id) = db::create_first_admin(&app, &name, &email, &r.password, r.start_bal).await? else { return conflict("Die Einrichtung ist bereits abgeschlossen. Bitte melde dich an.") };
    if r.questions.len() == crate::secq::SLOTS { crate::secq::store(&app.pool, &id, &r.questions).await?; }
    netsec::log(&app, "setup", true, ip, &h, Some(&id), Some(&email), "Administrator bei der Einrichtung angelegt").await;
    tracing::info!("Administrator {email} bei der Einrichtung angelegt");
    let me = db::user_by_id(&app.pool, &id).await?.expect("Konto");
    let c = start_session(&app, &me, false, false, &ip, &h).await?;
    Ok(([c], Json(json!({ "ok": true }))).into_response())
}

#[derive(Deserialize)]
struct RegisterReq { name: String, email: String, password: Option<String>, #[serde(rename = "startBal")] start_bal: Option<f64>, #[serde(default)] questions: Vec<crate::secq::Qa> }

async fn register(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<RegisterReq>) -> Res<Response> {
    netsec::guard(&app, ip).await?;
    if app.setup_code.lock().unwrap().is_some() { return forbidden("Die Einrichtung ist noch nicht abgeschlossen") }
    let reg = db::reg_policy(&app.pool).await?;
    let (name, email) = (r.name.trim().to_string(), r.email.trim().to_string());
    if name.is_empty() { return bad("Bitte Namen eingeben") }
    if !valid_email(&email) { return bad("Bitte gültige E-Mail-Adresse eingeben") }
    if !reg.domains.is_empty() && !email.to_lowercase().ends_with(&reg.domains.to_lowercase()) {
        return bad(format!("Nur E-Mail-Adressen mit {} sind erlaubt", reg.domains));
    }
    if let Some(pw) = &r.password {
        if let Some(m) = password_problem(pw, reg.min_pw) { return bad(m) }
    }
    // Mit Passwort und eingeschalteten Sicherheitsfragen gehören drei Fragen dazu (Grundlage für „Passwort vergessen“)
    let with_questions = reg.sec_questions && r.password.is_some();
    // Überstunden-Stand zu Beginn (Stunden): 0 = mit der Messung neu beginnen
    let start_bal = r.start_bal.unwrap_or(0.0);
    if !start_bal.is_finite() || start_bal.abs() > 2000.0 { return bad("Der Übertrag muss zwischen −2000 und +2000 Stunden liegen") }
    let hash = r.password.as_deref().map(hash_password);
    let existing = db::user_by_email(&app.pool, &email).await?;
    // Erst prüfen, wenn die Registrierung für diese Adresse überhaupt möglich ist (sonst meldet sie „geschlossen“ bzw. „vergeben“)
    if with_questions && (existing.as_ref().is_some_and(|u| u.status == "invited") || (existing.is_none() && reg.open)) {
        crate::secq::validate(&r.questions).map_err(|m| ApiError(StatusCode::BAD_REQUEST, m))?;
    }
    let (id, status) = match existing {
        // Einladung annehmen: Konto wird ohne weitere Freigabe aktiv.
        Some(u) if u.status == "invited" => {
            sqlx::query("UPDATE users SET name = $2, password_hash = $3, status = 'active', last_seen_at = now(), mfa_grace_start = now(), settings = jsonb_set(settings, '{startBal}', to_jsonb($4::float8)), start_bal_set = $5 WHERE id = $1").bind(&u.id).bind(&name).bind(&hash).bind(start_bal).bind(r.start_bal.is_some()).execute(&app.pool).await?;
            (u.id, "active".to_string())
        }
        Some(_) => return conflict("Diese E-Mail-Adresse ist bereits registriert"),
        None => {
            if !reg.open { return forbidden("Die Registrierung ist deaktiviert – Zugang nur per Einladung") }
            let status = if reg.approval { "pending" } else { "active" };
            let id = db::new_user_id();
            let today = app.clock(None).0;
            sqlx::query("INSERT INTO users (id, name, email, role, status, password_hash, webauthn_id, settings, track_start, start_bal_set) VALUES ($1,$2,$3,'user',$4,$5,$6,$7,$8,$9)")
                .bind(&id).bind(&name).bind(&email).bind(status).bind(&hash).bind(Uuid::new_v4()).bind(SJson({ let mut st = if app.cfg.demo { db::weekly_settings(reg.weekly) } else { db::fresh_settings(reg.weekly) }; st.start_bal = start_bal; st })).bind(today).bind(r.start_bal.is_some())
                .execute(&app.pool).await?;
            (id, status.to_string())
        }
    };
    if with_questions { crate::secq::store(&app.pool, &id, &r.questions).await?; }
    netsec::log(&app, "register", true, ip, &h, Some(&id), Some(&email), if status == "active" { "Konto angelegt" } else { "Konto angelegt, wartet auf Freigabe" }).await;
    let me = db::user_by_id(&app.pool, &id).await?.expect("Konto");
    // Mit eingeschalteter Bestätigung gibt es erst nach Klick auf den Link in der Mail eine Sitzung
    if crate::mail::needs_verification(&app, &me).await? {
        crate::mail::send_verification(&app, &me).await?;
        return Ok(Json(json!({ "ok": true, "status": "verify" })).into_response());
    }
    crate::mail::registration_ready(&app, &me).await;
    let c = start_session(&app, &me, status != "active", false, &ip, &h).await?;
    Ok(([c], Json(json!({ "ok": true, "status": status }))).into_response())
}

/* ---------- Passkeys ---------- */
#[derive(Deserialize)]
struct PkRegFinish { id: String, credential: RegisterPublicKeyCredential, name: Option<String> }

async fn pk_register_start(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, true).await?;
    let exclude: Vec<_> = db::creds(&app.pool, &u.id).await?.iter().map(|c| c.passkey.cred_id().clone()).collect();
    let (options, state) = app.webauthn
        .start_passkey_registration(u.webauthn_id, &u.email, &u.name, if exclude.is_empty() { None } else { Some(exclude) })
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("Passkey-Start fehlgeschlagen: {e}")))?;
    let id = Uuid::new_v4().to_string();
    app.reg_states.lock().unwrap().insert(id.clone(), (u.id, state));
    Ok(Json(json!({ "id": id, "options": options })))
}

async fn pk_register_finish(State(app): St, h: HeaderMap, Json(r): Json<PkRegFinish>) -> Res<Json<Value>> {
    let u = auth(&app, &h, true).await?;
    let Some((owner, state)) = app.reg_states.lock().unwrap().remove(&r.id) else { return bad("Die Passkey-Einrichtung ist abgelaufen – bitte erneut versuchen") };
    if owner != u.id { return forbidden("Fremde Passkey-Einrichtung") }
    let passkey = app.webauthn.finish_passkey_registration(&r.credential, &state).map_err(|e| ApiError(StatusCode::BAD_REQUEST, format!("Passkey wurde nicht akzeptiert: {e}")))?;
    let n = db::cred_count(&app.pool, &u.id).await? + 1;
    let name = r.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("Passkey {n}"));
    sqlx::query("INSERT INTO credentials (user_id, name, passkey) VALUES ($1, $2, $3)").bind(&u.id).bind(name).bind(SJson(&passkey)).execute(&app.pool).await?;
    if u.status == "active" { db::extend_sessions(&app.pool, &u.id, db::reg_policy(&app.pool).await?.session_days as i64 * 86400).await?; }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PkLoginStart { email: String }

async fn pk_login_start(State(app): St, Extension(ip): Extension<ClientIp>, Json(r): Json<PkLoginStart>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    let Some(u) = db::user_by_email(&app.pool, &r.email).await? else { return not_found("Für diese E-Mail ist kein Passkey eingerichtet.") };
    if let Some(e) = status_error(&u) { return Err(e) }
    let creds = db::creds(&app.pool, &u.id).await?;
    if creds.is_empty() { return not_found("Für diese E-Mail ist kein Passkey eingerichtet.") }
    let keys: Vec<_> = creds.iter().map(|c| c.passkey.0.clone()).collect();
    let (options, state) = app.webauthn.start_passkey_authentication(&keys).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, format!("Passkey-Start fehlgeschlagen: {e}")))?;
    let id = Uuid::new_v4().to_string();
    app.auth_states.lock().unwrap().insert(id.clone(), (u.id, state));
    Ok(Json(json!({ "id": id, "options": options })))
}

#[derive(Deserialize)]
struct PkLoginFinish { id: String, credential: PublicKeyCredential }

async fn pk_login_finish(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<PkLoginFinish>) -> Res<Response> {
    netsec::guard(&app, ip).await?;
    let Some((uid, state)) = app.auth_states.lock().unwrap().remove(&r.id) else { return bad("Die Anmeldung ist abgelaufen – bitte erneut versuchen") };
    let res = match app.webauthn.finish_passkey_authentication(&r.credential, &state) {
        Ok(res) => res,
        Err(e) => {
            netsec::failure(&app, "passkey", ip, &h, Some(&uid), None, &format!("Passkey abgelehnt: {e}")).await;
            return unauthorized(format!("Passkey-Anmeldung fehlgeschlagen: {e}"));
        }
    };
    let Some(u) = db::user_by_id(&app.pool, &uid).await? else { return unauthorized("Benutzer existiert nicht mehr") };
    if let Some(e) = status_error(&u) { return Err(e) }
    if crate::mail::needs_verification(&app, &u).await? { return forbidden("Bitte bestätige zuerst deine E-Mail-Adresse – wir haben dir einen Link geschickt.") }
    for mut c in db::creds(&app.pool, &uid).await? {
        if c.passkey.cred_id() == res.cred_id() {
            c.passkey.0.update_credential(&res);
            sqlx::query("UPDATE credentials SET passkey = $2, last_used_at = now() WHERE id = $1").bind(c.id).bind(&c.passkey).execute(&app.pool).await?;
        }
    }
    db::touch_user(&app.pool, &uid).await;
    netsec::log(&app, "passkey", true, ip, &h, Some(&uid), Some(&u.email), "Passkey").await;
    let c = start_session(&app, &u, false, false, &ip, &h).await?;
    Ok(([c], Json(json!({ "ok": true, "me": db::me_json(&u) }))).into_response())
}

#[derive(Deserialize)]
struct Rename { name: String }

async fn pk_rename(State(app): St, h: HeaderMap, Path(id): Path<i64>, Json(r): Json<Rename>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let name = r.name.trim();
    if name.is_empty() { return bad("Name darf nicht leer sein") }
    let n = sqlx::query("UPDATE credentials SET name = $3 WHERE id = $1 AND user_id = $2").bind(id).bind(&u.id).bind(name).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Passkey nicht gefunden") }
    Ok(Json(json!({ "ok": true })))
}

async fn reenable_password(app: &App, uid: &str) -> Res<()> {
    if db::cred_count(&app.pool, uid).await? == 0 {
        sqlx::query("UPDATE users SET settings = jsonb_set(settings, '{pwLogin}', 'true') WHERE id = $1").bind(uid).execute(&app.pool).await?;
    }
    Ok(())
}

async fn pk_delete(State(app): St, h: HeaderMap, Path(id): Path<i64>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    // Unter MFA-Pflicht bleibt mindestens ein Faktor erhalten
    if !u.totp_enabled && db::cred_count(&app.pool, &u.id).await? <= 1 && db::reg_policy(&app.pool).await?.mfa_required {
        return bad("Der letzte Passkey kann nicht entfernt werden, solange Passkey oder Authenticator-App verpflichtend sind. Richte zuerst die Authenticator-App ein.");
    }
    let n = sqlx::query("DELETE FROM credentials WHERE id = $1 AND user_id = $2").bind(id).bind(&u.id).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Passkey nicht gefunden") }
    reenable_password(&app, &u.id).await?;
    Ok(Json(json!({ "ok": true })))
}

/* ---------- Snapshot ---------- */
#[derive(Deserialize)]
struct NowQ { now: Option<f64> }

fn track_label(d: &DayState) -> &'static str {
    match d.track() { Track::Running => "tracking", Track::Locked => "locked", Track::Paused => "paused", Track::Asking => "asking", Track::Idle | Track::Stopped => "none" }
}

/// Heutiger Status je Benutzer („tracking“, „locked“, „vacation“ …) für die Serververwaltung.
async fn today_labels(app: &App, today: NaiveDate) -> Res<HashMap<String, String>> {
    let mut m = HashMap::new();
    let days: Vec<(String, SJson<DayState>)> = sqlx::query_as("SELECT user_id, state FROM days WHERE day = $1").bind(today).fetch_all(&app.pool).await?;
    for (uid, d) in days { m.insert(uid, track_label(&d.0).to_string()); }
    let vac: Vec<String> = sqlx::query_scalar("SELECT DISTINCT user_id FROM absences WHERE kind = 'vacation' AND date_from <= $1 AND date_to >= $1").bind(today).fetch_all(&app.pool).await?;
    for uid in vac { m.entry(uid).or_insert_with(|| "vacation".into()); }
    Ok(m)
}

/// Die meistgenutzten Apps der letzten 90 Tage erhalten eine eigene Farbe, der Rest läuft als „Sonstiges“.
fn apps_json(c: &Ctx) -> Value {
    let mut mins: HashMap<String, f64> = HashMap::new();
    let mut d = c.today - Duration::days(90);
    while d <= c.today {
        let segs = if d == c.today { Some(resolve(&c.day.segs, c.now)) } else { (c.history)(d) };
        for g in segs.unwrap_or_default() {
            if let (SegKind::App, Some(a)) = (g.kind, g.app.as_ref()) {
                if a != "other" { *mins.entry(a.clone()).or_default() += g.to.unwrap_or(g.from) - g.from; }
            }
        }
        d += Duration::days(1);
    }
    let mut v: Vec<_> = mins.into_iter().collect();
    v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    let mut out: Vec<Value> = v.iter().take(6).enumerate()
        .map(|(i, (k, _))| json!({ "k": k, "n": app_label(k), "c": format!("--s{}", i + 1), "g": app_glyph(k) })).collect();
    out.push(json!({ "k": "other", "n": "Sonstiges", "c": "--s0", "g": "other" }));
    Value::Array(out)
}

async fn snapshot_json(app: &App, u: &UserRow, today: NaiveDate, now: f64) -> Res<Value> {
    let calc = Calc::load(app, u, today, now).await?;
    let clash = device::clash(app, &u.id, today).await?;
    let year = today.year();
    let (first, last) = (NaiveDate::from_ymd_opt(u.track_start.year().min(year), 1, 1).unwrap(), NaiveDate::from_ymd_opt(year + 1, 12, 31).unwrap());
    let abs: Vec<Value> = calc.absences.iter().map(|a| {
        let mut v = serde_json::to_value(a).unwrap();
        v["workdays"] = json!(abs_workdays(a.from, a.to, &calc.settings));
        v
    }).collect();
    let now_utc = Utc::now();
    let creds = db::creds(&app.pool, &u.id).await?;
    let mut v = calc.with(|c| {
        let mut days = Vec::new();
        let mut d = first;
        while d <= last { days.push(day_stat(c, d)); d += Duration::days(1); }
        let t = day_stat(c, c.today);
        json!({
            "today": today, "trackStart": u.track_start, "memberSince": app.local(u.created_at).0, "now": now, "demo": app.cfg.demo,
            "me": db::me_json(u), "settings": c.settings, "absences": abs,
            "passkeys": creds.iter().map(|k| json!({
                "id": k.id, "name": k.name, "created": fmt_de(app.local(k.created_at).0),
                "last": k.last_used_at.map(|t| db::ago_label(t, now_utc)).unwrap_or_else(|| "noch nie".into()),
            })).collect::<Vec<_>>(),
            "day": c.day, "nightCheck": night_check(c).as_ref().map(night_json), "deviceClash": clash.clone().filter(|_| c.day.tracking), "days": days, "forecastEnd": forecast_end(c, &t), "apps": apps_json(c),
            "vacation": vacation_summary(c, 30), "nextHolidays": next_holidays(c, 3),
            "holidays": year_holidays(year, c.settings),
        })
    });
    v["devices"] = Value::Array(device::list_for(app, &u.id).await?);
    let reg = db::reg_policy(&app.pool).await?;
    v["mfa"] = db::mfa_status(&app.pool, u, &reg).await?.json();
    v["limits"] = json!({ "editDaysBack": reg.edit_days_back, "longTrackHours": reg.long_track_hours });
    v["secQ"] = crate::secq::status(app, u).await?;
    v["startBalSet"] = json!(u.start_bal_set);
    v["serverVersion"] = json!(env!("CARGO_PKG_VERSION"));
    v["totp"] = json!({ "enabled": u.totp_enabled, "recoveryLeft": if u.totp_enabled { crate::mfa::recovery_left(app, &u.id).await? } else { 0 } });
    if u.role == "admin" {
        let labels = today_labels(app, today).await?;
        let mut users = Vec::new();
        for x in db::all_users(&app.pool).await? {
            let bal = if x.status == "active" { Calc::load(app, &x, today, now).await?.with(total_balance) } else { 0.0 };
            let pk = db::cred_count(&app.pool, &x.id).await?;
            let xm = db::mfa_status_with(&x, pk, &reg);
            users.push(json!({
                "mfa": if x.status == "active" { json!({ "required": xm.required, "compliant": xm.compliant, "deadline": xm.deadline, "expired": xm.expired }) } else { Value::Null },
                "id": x.id, "name": x.name, "email": x.email, "role": x.role, "status": x.status, "passkeys": pk,
                "totp": x.totp_enabled, "mustChangePw": x.must_change_pw, "startBalSet": x.start_bal_set, "startBal": x.settings.start_bal, "week": db::week_hours(&x.settings), "bal": bal, "last": db::last_label(&x, now_utc),
                "today": labels.get(&x.id).cloned().unwrap_or_else(|| "none".into()),
            }));
        }
        v["reg"] = serde_json::to_value(&reg).unwrap();
        v["users"] = Value::Array(users);
    }
    Ok(v)
}

/// Im Demo-Modus entspricht schon das Öffnen des Dashboards dem Rechnerstart (je nach Zeitfenster Tracking oder Rückfrage).
async fn demo_ensure_day(app: &App, u: &UserRow, now: f64) -> Res<()> {
    if !app.cfg.demo { return Ok(()) }
    let mut tx = app.pool.begin().await?;
    if db::get_day(&mut tx, &u.id, app.cfg.demo_today).await?.is_none() {
        let abs = db::absences(&app.pool, &u.id).await?;
        let ev = Ev { kind: "boot".into(), now: Some(now), ..Default::default() };
        events::apply(app, &mut tx, u, &abs, &ev).await?.ok();
        tx.commit().await?;
    }
    Ok(())
}

async fn snapshot(State(app): St, h: HeaderMap, Query(q): Query<NowQ>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let (today, now) = app.clock(q.now);
    demo_ensure_day(&app, &u, now).await?;
    db::touch_user(&app.pool, &u.id).await;
    Ok(Json(snapshot_json(&app, &u, today, now).await?))
}

/// Leichtgewichtig für den Takt: nur der laufende Tag.
async fn today(State(app): St, h: HeaderMap, Query(q): Query<NowQ>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let (today, now) = app.clock(q.now);
    demo_ensure_day(&app, &u, now).await?;
    let calc = Calc::load(&app, &u, today, now).await?;
    let clash = device::clash(&app, &u.id, today).await?;
    Ok(Json(calc.with(|c| {
        let t = day_stat(c, c.today);
        json!({ "day": c.day, "today": t, "date": today, "now": now, "forecastEnd": forecast_end(c, &t), "nightCheck": night_check(c).as_ref().map(night_json), "deviceClash": clash.filter(|_| c.day.tracking) })
    })))
}

/* ---------- Einstellungen, Profil, Abwesenheiten ---------- */
async fn put_settings(State(app): St, h: HeaderMap, Json(s): Json<Settings>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if s.hours.iter().any(|h| !h.is_finite() || *h < 0.0 || *h > 24.0) { return bad("Tagessoll muss zwischen 0 und 24 Stunden liegen") }
    if s.rules.len() > 8 || s.rules.iter().any(|r| !(0.0..=24.0).contains(&r.after) || !(0.0..=240.0).contains(&r.min)) { return bad("Ungültige Pausenregel") }
    if !(0.0..=240.0).contains(&s.chunk) { return bad("Ungültige Mindestdauer") }
    if !s.start_bal.is_finite() || s.start_bal.abs() > 2000.0 { return bad("Ungültiger Übertrag") }
    if !holidays::STATES.iter().any(|(k, _)| *k == s.state) { return bad("Unbekanntes Bundesland") }
    let x = &s.extra_days;
    if x.custom.len() > 20 || x.custom.iter().any(|c| c.name.trim().is_empty() || c.name.chars().count() > 40 || !(1..=12).contains(&c.month) || NaiveDate::from_ymd_opt(2024, c.month, c.day).is_none()) {
        return bad("Ungültiger freier Tag – Name und Datum (TT.MM.) angeben")
    }
    let a = &s.auto_track;
    if a.windows.len() > 8 || a.windows.iter().any(|w| !(0.0..1440.0).contains(&w.from) || !(0.0..=1440.0).contains(&w.to) || w.to <= w.from || !w.days.iter().any(|d| *d)) {
        return bad("Ungültiges Zeitfenster – Ende muss nach dem Beginn liegen, mindestens ein Wochentag ist nötig")
    }
    if !["system", "light", "dark"].contains(&s.theme.as_str()) { return bad("Unbekannte Darstellung") }
    if s.excluded.len() > 100 || s.excluded.iter().any(|e| e.chars().count() > 64) { return bad("Ungültige Liste ausgeschlossener Apps") }
    if !s.pw_login && db::cred_count(&app.pool, &u.id).await? == 0 { return bad("Richte zuerst einen Passkey ein, bevor du die Passwort-Anmeldung deaktivierst") }
    // Der Übertrag zum Start wird bei der Registrierung festgelegt und ist hier nicht änderbar (nur durch Administratoren)
    let mut s = s;
    s.start_bal = u.settings.start_bal;
    db::set_settings(&app.pool, &u.id, &s).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MeReq { name: String, email: String }

async fn put_me(State(app): St, h: HeaderMap, Json(r): Json<MeReq>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if r.name.trim().is_empty() || !valid_email(&r.email) { return bad("Bitte Name und gültige E-Mail eingeben") }
    if db::user_by_email(&app.pool, &r.email).await?.is_some_and(|x| x.id != u.id) { return conflict("Diese E-Mail-Adresse wird bereits verwendet") }
    sqlx::query("UPDATE users SET name = $2, email = $3 WHERE id = $1").bind(&u.id).bind(r.name.trim()).bind(r.email.trim()).execute(&app.pool).await?;
    Ok(Json(json!({ "ok": true })))
}

fn check_abs(a: &Absence) -> Res<()> {
    if a.to < a.from { return bad("Das Ende liegt vor dem Beginn") }
    if (a.to - a.from).num_days() > 400 { return bad("Zeitraum zu lang") }
    Ok(())
}

fn kind_str(k: AbsType) -> &'static str {
    match k { AbsType::Vacation => "vacation", AbsType::Sick => "sick", AbsType::Comp => "comp" }
}

async fn abs_create(State(app): St, h: HeaderMap, Json(a): Json<Absence>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    check_abs(&a)?;
    let id = db::insert_absence(&app.pool, &u.id, &a).await?;
    Ok(Json(json!({ "ok": true, "id": id })))
}

async fn abs_update(State(app): St, h: HeaderMap, Path(id): Path<i64>, Json(a): Json<Absence>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    check_abs(&a)?;
    let n = sqlx::query("UPDATE absences SET kind = $3, date_from = $4, date_to = $5, note = $6 WHERE id = $1 AND user_id = $2")
        .bind(id).bind(&u.id).bind(kind_str(a.kind)).bind(a.from).bind(a.to).bind(&a.note).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Eintrag nicht gefunden") }
    Ok(Json(json!({ "ok": true })))
}

async fn abs_delete(State(app): St, h: HeaderMap, Path(id): Path<i64>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    sqlx::query("DELETE FROM absences WHERE id = $1 AND user_id = $2").bind(id).bind(&u.id).execute(&app.pool).await?;
    Ok(Json(json!({ "ok": true })))
}

/* ---------- Companion über die Web-Oberfläche ---------- */
/// Im Echtbetrieb steuert die Weboberfläche nur das Tracking; Systemereignisse kommen vom gekoppelten Gerät.
const WEB_KINDS: [&str; 6] = ["pause", "start", "answer", "tick", "stop", "stopat"];

async fn companion(State(app): St, h: HeaderMap, Json(ev): Json<Ev>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if !app.cfg.demo && !WEB_KINDS.contains(&ev.kind.as_str()) { return forbidden("Dieses Ereignis meldet der Companion auf dem Rechner") }
    let abs = db::absences(&app.pool, &u.id).await?;
    let mut ev = ev;
    ev.at = None;
    ev.web = !app.cfg.demo;
    let mut tx = app.pool.begin().await?;
    let res = events::apply(&app, &mut tx, &u, &abs, &ev).await?;
    if let Err(m) = res { return conflict(m) }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PreviewReq { at: Option<f64>, now: Option<f64> }

/// Was passiert, wenn das Tracking jetzt oder zu einem bestimmten Zeitpunkt beendet wird: Zeitpunkt, Brutto, Pause, Netto.
async fn stop_preview(State(app): St, h: HeaderMap, Json(r): Json<PreviewReq>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let (today, now) = app.clock(r.now);
    let mut c = db::Calc::load(&app, &u, today, now).await?;
    if !c.has_day { return conflict("Heute läuft kein Tracking.") }
    let at = r.at.unwrap_or(now);
    let before = c.with(total_balance);
    c.day.stop_web(now, at).map_err(|m| ApiError(StatusCode::CONFLICT, m))?;
    let s = c.with(|x| day_stat(x, today));
    // `diff`: was der Tag nach dem Beenden zum Überstundenkonto beiträgt (Minus wird jetzt verbucht, vorher zählte es nicht)
    Ok(Json(json!({ "at": c.day.ended, "gross": s.gross, "net": s.net, "pause": s.gross - s.net, "counted": s.counted, "soll": s.soll, "start": s.start,
        "diff": s.diff, "balanceBefore": before, "balanceAfter": c.with(total_balance) })))
}

#[derive(Deserialize)]
struct NightStop { /// Minuten seit Mitternacht des Vortags (über 1440 = nach Mitternacht)
    at: f64, #[serde(default)] preview: bool }

fn night_json(c: &tickly_core::NightCheck) -> Value { json!({ "since": c.since, "hours": c.hours, "yesterday": c.yesterday }) }

/// Die Warnung zu zwei gleichzeitigen Companions bestätigen.
async fn clash_ack(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    device::clash_ack(&app, &u.id).await?;
    Ok(Json(json!({ "ok": true })))
}

/// „Ich arbeite wirklich“: die Rückfrage zum Tracking über Nacht entfällt.
async fn night_confirm(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if app.cfg.demo { return bad("Im Demo-Modus nicht verfügbar") }
    let (today, now) = app.clock(None);
    let mut tx = app.pool.begin().await?;
    db::lock_user(&mut tx, &u.id).await?;
    let Some(mut d) = db::get_day(&mut tx, &u.id, today).await? else { return conflict("Heute läuft kein Tracking.") };
    d.confirm_night(now).map_err(|m| ApiError(StatusCode::CONFLICT, m))?;
    db::put_day(&mut tx, &u.id, today, &d).await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

/// Ein über Nacht weitergelaufenes Tracking beenden, mit Ende am Vortag oder nach Mitternacht (`preview`: nur berechnen).
async fn night_stop(State(app): St, h: HeaderMap, Json(r): Json<NightStop>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if app.cfg.demo { return bad("Im Demo-Modus nicht verfügbar") }
    if !r.at.is_finite() || r.at <= 0.0 { return bad("Bitte ein Ende angeben") }
    let (today, now) = app.clock(None);
    let yesterday = today - Duration::days(1);
    let mut tx = app.pool.begin().await?;
    db::lock_user(&mut tx, &u.id).await?;
    let mut cur = match db::get_day(&mut tx, &u.id, today).await? { Some(d) if d.tracking && d.carried => d, _ => return conflict("Es läuft kein Tracking über Mitternacht.") };
    let mut prev = db::get_day(&mut tx, &u.id, yesterday).await?;
    // Ende am Vortag: dort schneiden und heute nichts übernehmen; Ende heute: nur heute schneiden
    let (date, day) = if r.at <= 1440.0 {
        let Some(p) = prev.as_mut() else { return conflict("Der Vortag ist nicht mehr vorhanden.") };
        p.stop_web(1440.0, r.at).map_err(|m| ApiError(StatusCode::CONFLICT, m))?;
        (yesterday, p.clone())
    } else {
        cur.stop_web(now, r.at - 1440.0).map_err(|m| ApiError(StatusCode::CONFLICT, m))?;
        (today, cur.clone())
    };
    let k = calc_segs(&resolve(&day.segs, 1440.0), &u.settings.0);
    let end = day.ended.unwrap_or(0.0);
    let out = json!({ "ok": true, "date": date, "start": day.segs.first().map(|g| g.from), "at": end, "gross": k.gross, "pause": k.gross - k.net, "net": k.net, "counted": k.counted });
    if r.preview { return Ok(Json(out)) }
    if let (Some(p), true) = (prev.as_ref(), date == yesterday) {
        db::put_day(&mut tx, &u.id, yesterday, p).await?;
        db::put_day(&mut tx, &u.id, today, &DayState::empty(now)).await?;
    } else {
        db::put_day(&mut tx, &u.id, today, &cur).await?;
    }
    tx.commit().await?;
    Ok(Json(out))
}

/* ---------- Administration ---------- */
async fn admin_registration(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(p): Json<RegPolicy>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if !(1.0..=80.0).contains(&p.weekly) || !(MIN_PW..=128).contains(&p.min_pw) { return bad(format!("Ungültige Werte – das Passwort braucht mindestens {MIN_PW} Zeichen")) }
    if p.mfa_grace_days > 365 { return bad("Die Karenzzeit darf höchstens 365 Tage betragen") }
    if !(1..=365).contains(&p.session_days) { return bad("Die Sitzungsdauer muss zwischen 1 und 365 Tagen liegen") }
    if p.long_track_hours > 24 { return bad("Die Warnung bei langem Tracking: höchstens 24 Stunden (0 = aus)") }
    if p.edit_days_back > 3650 { return bad("Nachträgliches Bearbeiten: höchstens 3650 Tage rückwirkend") }
    // Die Karenzzeit beginnt, wenn die Pflicht eingeschaltet wird – nicht rückwirkend und nicht bei jedem Speichern neu
    let old = db::reg_policy(&app.pool).await?;
    let mut p = p;
    p.mfa_required_since = if !p.mfa_required { None } else if old.mfa_required { old.mfa_required_since } else { Some(Utc::now()) };
    // Bestätigung per Mail setzt einen funktionierenden SMTP-Versand voraus; sie gilt nur für Konten, die danach entstehen
    if p.verify_email && crate::mail::active(&app.pool).await.is_none() { return bad("Die E-Mail-Bestätigung braucht einen eingerichteten und eingeschalteten SMTP-Server (Administration → E-Mail).") }
    p.verify_email_since = if !p.verify_email { None } else if old.verify_email { old.verify_email_since } else { Some(Utc::now()) };
    db::set_reg_policy(&app.pool, &p).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Richtlinien: Registrierung offen={}, Freigabe={}, Domain={}, MFA-Pflicht={} (Karenzzeit {} Tage), Sitzungsdauer {} Tage", p.open, p.approval, if p.domains.is_empty() { "alle" } else { &p.domains }, p.mfa_required, p.mfa_grace_days, p.session_days)).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct InviteReq { name: String, email: String, role: String, week: f64, /// Konto sofort mit Initialpasswort anlegen statt per Einladungslink
    #[serde(default)] direct: bool, password: Option<String> }

async fn admin_invite(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<InviteReq>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if r.name.trim().is_empty() || !valid_email(&r.email) { return bad("Bitte Name und gültige E-Mail eingeben") }
    if !["admin", "user"].contains(&r.role.as_str()) { return bad("Unbekannte Rolle") }
    if db::user_by_email(&app.pool, &r.email).await?.is_some() { return conflict("Diese E-Mail-Adresse ist bereits vergeben") }
    let week = r.week.clamp(1.0, 80.0);
    // Direkt anlegen: Initialpasswort (eigenes oder erzeugtes), der Benutzer ändert es beim ersten Anmelden
    let initial = if r.direct {
        let min = db::reg_policy(&app.pool).await?.min_pw;
        Some(match r.password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            Some(p) => { if let Some(m) = password_problem(p, min) { return bad(m) } p.to_string() }
            None => crate::secq::generate_password(min as usize + 4),
        })
    } else { None };
    let id = db::new_user_id();
    sqlx::query("INSERT INTO users (id, name, email, role, status, webauthn_id, settings, track_start, demo, seed, demo_base) VALUES ($1,$2,$3,$4,'invited',$5,$6,$7,$8,$1,$9)")
        .bind(&id).bind(r.name.trim()).bind(r.email.trim()).bind(&r.role).bind(Uuid::new_v4()).bind(SJson(if app.cfg.demo { db::weekly_settings(week) } else { db::fresh_settings(week) }))
        .bind(if app.cfg.demo { app.cfg.demo_track_start } else { app.clock(None).0 }).bind(app.cfg.demo).bind(demo::base_for_hours(week / 5.0))
        .execute(&app.pool).await?;
    let mailed = crate::mail::active(&app.pool).await.is_some();
    if let Some(pw) = &initial {
        // Der Administrator verbürgt sich für die Adresse: keine E-Mail-Bestätigung nötig
        sqlx::query("UPDATE users SET status = 'active', password_hash = $2, must_change_pw = true, email_verified_at = now(), mfa_grace_start = now(), last_seen_at = NULL WHERE id = $1").bind(&id).bind(hash_password(pw)).execute(&app.pool).await?;
        netsec::admin_action(&app, &me, ip, &h, &format!("Benutzer angelegt mit Initialpasswort: {} ({})", r.email.trim(), r.role)).await;
        if mailed {
            crate::mail::spawn(app.clone(), vec![r.email.trim().to_string()], "Dein Tickly-Konto".into(), format!("{} hat ein Konto für dich angelegt", me.name),
                vec![format!("Hallo {}, für dich wurde ein Konto bei Tickly eingerichtet. Das Initialpasswort bekommst du von {} (es steht nicht in dieser Mail). Beim ersten Anmelden legst du ein eigenes Passwort und deine Sicherheitsfragen fest.", r.name.trim(), me.name)],
                Some(("Zur Anmeldung".into(), format!("{}/#/login", app.cfg.public_url.as_str().trim_end_matches('/')))));
        }
        return Ok(Json(json!({ "ok": true, "mailed": mailed, "id": id, "password": pw })));
    }
    netsec::admin_action(&app, &me, ip, &h, &format!("Benutzer eingeladen: {} ({})", r.email.trim(), r.role)).await;
    if mailed {
        crate::mail::spawn(app.clone(), vec![r.email.trim().to_string()], "Einladung zu Tickly".into(), format!("{} hat dich zu Tickly eingeladen", me.name),
            vec![format!("Hallo {}, für dich wurde ein Konto bei Tickly eingerichtet. Lege mit dem Knopf unten dein Passwort fest oder richte einen Passkey ein.", r.name.trim())],
            Some(("Konto einrichten".into(), format!("{}/#/register", app.cfg.public_url.as_str().trim_end_matches('/')))));
    }
    Ok(Json(json!({ "ok": true, "mailed": mailed })))
}

#[derive(Deserialize)]
struct TrackStartReq { date: NaiveDate }

/// Ab wann Tickly Zeiten und Überstundenkonto rechnet (z. B. nach ein paar Testtagen). Tage davor zählen nicht;
/// ihre Daten bleiben gespeichert und zählen wieder, wenn das Datum früher gesetzt wird.
async fn set_track_start(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<TrackStartReq>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if app.cfg.demo { return bad("Im Demo-Modus nicht verfügbar") }
    let (today, _) = app.clock(None);
    if r.date < NaiveDate::from_ymd_opt(2000, 1, 1).unwrap() { return bad("Das Startdatum liegt zu weit in der Vergangenheit") }
    if r.date > today + Duration::days(366) { return bad("Das Startdatum darf höchstens ein Jahr in der Zukunft liegen") }
    sqlx::query("UPDATE users SET track_start = $2 WHERE id = $1").bind(&u.id).bind(r.date).execute(&app.pool).await?;
    netsec::log(&app, "trackstart", true, ip, &h, Some(&u.id), Some(&u.email), &format!("Startdatum der Zeiterfassung geändert: {} → {}", fmt_de(u.track_start), fmt_de(r.date))).await;
    Ok(Json(json!({ "ok": true, "trackStart": r.date })))
}

#[derive(Deserialize)]
struct StartBalReq { hours: f64 }

/// Überstunden-Stand zu Beginn: einmalig durch den Benutzer, danach nur noch durch einen Administrator.
async fn set_start_balance(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<StartBalReq>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    if !r.hours.is_finite() || r.hours.abs() > 2000.0 { return bad("Der Übertrag muss zwischen −2000 und +2000 Stunden liegen") }
    let n = sqlx::query("UPDATE users SET settings = jsonb_set(settings, '{startBal}', to_jsonb($2::float8)), start_bal_set = true WHERE id = $1 AND NOT start_bal_set").bind(&u.id).bind(r.hours).execute(&app.pool).await?.rows_affected();
    if n == 0 { return conflict("Der Überstunden-Stand ist schon festgelegt. Änderungen nur durch einen Administrator.") }
    netsec::log(&app, "startbal", true, ip, &h, Some(&u.id), Some(&u.email), &format!("Überstunden-Stand zu Beginn festgelegt: {} h", r.hours)).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct UpdReq { name: String, email: String, role: String, week: f64, #[serde(rename = "startBal")] start_bal: Option<f64> }

async fn admin_update(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>, Json(r): Json<UpdReq>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if r.name.trim().is_empty() || !valid_email(&r.email) { return bad("Bitte Name und gültige E-Mail eingeben") }
    if !["admin", "user"].contains(&r.role.as_str()) { return bad("Unbekannte Rolle") }
    if id == me.id && r.role != "admin" { return bad("Du kannst dir die Administratorrolle nicht selbst entziehen") }
    if db::user_by_email(&app.pool, &r.email).await?.is_some_and(|x| x.id != id) { return conflict("Diese E-Mail-Adresse ist bereits vergeben") }
    let Some(u) = db::user_by_id(&app.pool, &id).await? else { return not_found("Benutzer nicht gefunden") };
    let mut s = u.settings.0.clone();
    let mut changed_bal = false;
    if (db::week_hours(&s) - r.week).abs() > 0.01 { s.hours = db::weekly_settings(r.week.clamp(1.0, 80.0)).hours; }
    if let Some(b) = r.start_bal { if !b.is_finite() || b.abs() > 2000.0 { return bad("Der Übertrag muss zwischen −2000 und +2000 Stunden liegen") } if (s.start_bal - b).abs() > 1e-9 { changed_bal = true; } s.start_bal = b; }
    sqlx::query("UPDATE users SET name = $2, email = $3, role = $4, settings = $5, start_bal_set = start_bal_set OR $6 WHERE id = $1").bind(&id).bind(r.name.trim()).bind(r.email.trim()).bind(&r.role).bind(SJson(&s)).bind(changed_bal).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Benutzer geändert: {} (Rolle {})", r.email.trim(), r.role)).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct StatusReq { status: String }

async fn admin_status(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>, Json(r): Json<StatusReq>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if !["active", "blocked"].contains(&r.status.as_str()) { return bad("Unbekannter Status") }
    if id == me.id && r.status == "blocked" { return bad("Du kannst dich nicht selbst sperren") }
    let n = sqlx::query("UPDATE users SET status = $2, mfa_grace_start = CASE WHEN status <> 'active' AND $2 = 'active' THEN now() ELSE mfa_grace_start END WHERE id = $1").bind(&id).bind(&r.status).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Benutzer nicht gefunden") }
    if r.status == "blocked" { sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(&id).execute(&app.pool).await?; }
    netsec::admin_action(&app, &me, ip, &h, &format!("Konto {}: {id}", if r.status == "active" { "freigeschaltet" } else { "gesperrt" })).await;
    if r.status == "active" {
        if let (Some(cfg), Some(u)) = (crate::mail::active(&app.pool).await, db::user_by_id(&app.pool, &id).await?) {
            if cfg.notify_user_approved {
                crate::mail::spawn(app.clone(), vec![u.email.clone()], "Dein Tickly-Konto wurde freigeschaltet".into(), "Konto freigeschaltet".into(),
                    vec![format!("Hallo {}, dein Konto wurde von einem Administrator freigeschaltet. Du kannst dich jetzt anmelden.", u.name)],
                    Some(("Zur Anmeldung".into(), app.cfg.public_url.as_str().trim_end_matches('/').to_string())));
            }
        }
    }
    Ok(Json(json!({ "ok": true })))
}

async fn admin_delete(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if id == me.id { return bad("Du kannst dich nicht selbst löschen") }
    let gone = db::user_by_id(&app.pool, &id).await?.map(|u| u.email).unwrap_or_else(|| id.clone());
    sqlx::query("DELETE FROM users WHERE id = $1").bind(&id).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Benutzer gelöscht: {gone}")).await;
    Ok(Json(json!({ "ok": true })))
}

async fn admin_reset_passkeys(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    require_admin(&me)?;
    if db::user_by_id(&app.pool, &id).await?.is_none() { return not_found("Benutzer nicht gefunden") }
    sqlx::query("DELETE FROM credentials WHERE user_id = $1").bind(&id).execute(&app.pool).await?;
    reenable_password(&app, &id).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Passkeys zurückgesetzt: {id}")).await;
    Ok(Json(json!({ "ok": true })))
}
