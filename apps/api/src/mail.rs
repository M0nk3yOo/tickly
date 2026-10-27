//! E-Mail: SMTP-Einrichtung in der Oberfläche, Versand (Bestätigung, Passwort-Zurücksetzen, Einladung, Hinweise für
//! Administratoren) und die zugehörigen Einmal-Links.
use crate::api::{auth, St};
use crate::app::*;
use crate::db::{self, UserRow};
use crate::netsec::{self, ClientIp};
use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{Duration, Utc};
use lettre::{
    message::{header::ContentType, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{types::Json as SJson, PgPool};
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmtpCfg {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    /// "starttls" (Port 587), "tls" (Port 465) oder "none"
    pub security: String,
    pub username: String,
    /// Mit `TICKLY_SECRET` verschlüsselt gespeichert, wird nie an die Oberfläche geliefert.
    #[serde(default)]
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    /// Administratoren benachrichtigen, wenn sich jemand registriert und Freigabe braucht.
    #[serde(default = "yes")]
    pub notify_admin_registration: bool,
    /// Administratoren benachrichtigen, wenn eine IP-Adresse gesperrt wurde.
    #[serde(default = "yes")]
    pub notify_admin_block: bool,
    /// Benutzer informieren, wenn ihr Konto freigeschaltet wurde.
    #[serde(default = "yes")]
    pub notify_user_approved: bool,
}
fn yes() -> bool { true }

impl Default for SmtpCfg {
    fn default() -> Self {
        SmtpCfg { enabled: false, host: String::new(), port: 587, security: "starttls".into(), username: String::new(), password: String::new(), from_email: String::new(), from_name: "Tickly".into(),
                  notify_admin_registration: true, notify_admin_block: true, notify_user_approved: true }
    }
}

impl SmtpCfg {
    /// Für die Oberfläche: ohne Passwort, nur ob eines gesetzt ist.
    pub fn public(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        v.as_object_mut().map(|o| o.remove("password"));
        v["passwordSet"] = json!(!self.password.is_empty());
        v
    }
}

pub async fn config(pool: &PgPool) -> Res<SmtpCfg> {
    let v: Option<(SJson<SmtpCfg>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'smtp'").fetch_optional(pool).await?;
    Ok(v.map(|x| x.0 .0).unwrap_or_default())
}

/// Aktive Einstellungen, sonst `None` (dann werden keine Mails verschickt und Bestätigungen nicht verlangt).
pub async fn active(pool: &PgPool) -> Option<SmtpCfg> {
    config(pool).await.ok().filter(|c| c.enabled && !c.host.is_empty() && !c.from_email.is_empty())
}

/* ---------- Versand ---------- */
pub async fn send(app: &App, cfg: &SmtpCfg, to: &str, subject: &str, text: &str, html: &str) -> Result<(), String> {
    let from = Mailbox::new(Some(cfg.from_name.clone()).filter(|n| !n.is_empty()), cfg.from_email.parse().map_err(|_| "Die Absenderadresse ist ungültig".to_string())?);
    let rcpt: Mailbox = to.parse().map_err(|_| format!("Die Empfängeradresse „{to}“ ist ungültig"))?;
    let msg = Message::builder().from(from).to(rcpt).subject(subject)
        .multipart(MultiPart::alternative().singlepart(SinglePart::builder().header(ContentType::TEXT_PLAIN).body(text.to_string())).singlepart(SinglePart::builder().header(ContentType::TEXT_HTML).body(html.to_string())))
        .map_err(|e| e.to_string())?;
    let b = match cfg.security.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host),
        "none" => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host),
    }.map_err(|e| format!("Verbindung zu {} nicht möglich: {e}", cfg.host))?;
    let mut b = b.port(cfg.port).timeout(Some(std::time::Duration::from_secs(15)));
    if !cfg.username.is_empty() {
        let pw = app.cfg.secrets.open_str(&cfg.password)?;
        b = b.credentials(Credentials::new(cfg.username.clone(), pw));
    }
    b.build().send(msg).await.map(|_| ()).map_err(|e| {
        let t = e.to_string();
        if t.chars().count() > 300 { t.chars().take(300).collect::<String>() + "…" } else { t }
    })
}

/// Aufbau von Text- und HTML-Fassung einer Nachricht.
pub fn compose(heading: &str, lines: &[String], button: Option<(&str, &str)>) -> (String, String) {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut text = format!("{heading}\n\n{}\n", lines.join("\n\n"));
    if let Some((label, url)) = button { text.push_str(&format!("\n{label}: {url}\n")); }
    text.push_str("\n—\nTickly Zeiterfassung\n");
    let paras: String = lines.iter().map(|l| format!("<p style=\"margin:0 0 14px;line-height:1.5\">{}</p>", esc(l))).collect();
    let btn = button.map(|(label, url)| format!("<p style=\"margin:22px 0\"><a href=\"{u}\" style=\"background:#4f46e5;color:#fff;text-decoration:none;padding:11px 20px;border-radius:9px;font-weight:600;display:inline-block\">{l}</a></p><p style=\"margin:0 0 14px;color:#6b6b73;font-size:12.5px\">Funktioniert der Knopf nicht, kopiere diesen Link in den Browser:<br><span style=\"word-break:break-all\">{u}</span></p>", u = esc(url), l = esc(label))).unwrap_or_default();
    let html = format!("<div style=\"font-family:system-ui,-apple-system,'Segoe UI',sans-serif;max-width:520px;margin:0 auto;padding:24px;color:#18181b\"><h2 style=\"margin:0 0 18px;font-size:20px\">{}</h2>{paras}{btn}<hr style=\"border:0;border-top:1px solid #e4e4e7;margin:24px 0 12px\"><p style=\"margin:0;color:#8a8a92;font-size:12px\">Tickly Zeiterfassung</p></div>", esc(heading));
    (text, html)
}

/// Versand im Hintergrund; das Ergebnis steht im Protokoll (Art „Mail“).
pub fn spawn(app: Arc<App>, to: Vec<String>, subject: String, heading: String, lines: Vec<String>, button: Option<(String, String)>) {
    tokio::spawn(async move {
        let Some(cfg) = active(&app.pool).await else { return };
        let (text, html) = compose(&heading, &lines, button.as_ref().map(|(a, b)| (a.as_str(), b.as_str())));
        for rcpt in to {
            let r = send(&app, &cfg, &rcpt, &subject, &text, &html).await;
            netsec::log_system(&app, "mail", r.is_ok(), &format!("An {rcpt}: {subject}{}", r.err().map(|e| format!(" – Fehler: {e}")).unwrap_or_default())).await;
        }
    });
}

pub async fn admin_emails(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT email FROM users WHERE role = 'admin' AND status = 'active'").fetch_all(pool).await.unwrap_or_default()
}

/* ---------- Einmal-Links ---------- */
fn hash(t: &str) -> String {
    Sha256::digest(t.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn new_token(pool: &PgPool, uid: &str, kind: &str, ttl: Duration) -> Res<String> {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let t: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    sqlx::query("DELETE FROM email_tokens WHERE user_id = $1 AND kind = $2").bind(uid).bind(kind).execute(pool).await?;
    sqlx::query("INSERT INTO email_tokens (token_hash, user_id, kind, expires_at) VALUES ($1, $2, $3, $4)").bind(hash(&t)).bind(uid).bind(kind).bind(Utc::now() + ttl).execute(pool).await?;
    Ok(t)
}

async fn take_token(pool: &PgPool, token: &str, kind: &str) -> Res<Option<String>> {
    let _ = sqlx::query("DELETE FROM email_tokens WHERE expires_at < now()").execute(pool).await;
    Ok(sqlx::query_scalar("DELETE FROM email_tokens WHERE token_hash = $1 AND kind = $2 AND expires_at > now() RETURNING user_id").bind(hash(token.trim())).bind(kind).fetch_optional(pool).await?)
}

pub fn link(app: &App, route: &str, token: &str) -> String {
    format!("{}/#/{route}?token={token}", app.cfg.public_url.as_str().trim_end_matches('/'))
}

/// Muss dieses Konto seine Adresse noch bestätigen, bevor es sich anmelden darf?
pub async fn needs_verification(app: &App, u: &UserRow) -> Res<bool> {
    if u.email_verified_at.is_some() { return Ok(false) }
    let reg = db::reg_policy(&app.pool).await?;
    if !reg.verify_email || reg.verify_email_since.is_some_and(|s| u.created_at < s) { return Ok(false) }
    Ok(active(&app.pool).await.is_some())
}

pub async fn send_verification(app: &Arc<App>, u: &UserRow) -> Res<()> {
    let t = new_token(&app.pool, &u.id, "verify", Duration::hours(48)).await?;
    spawn(app.clone(), vec![u.email.clone()], "Bitte bestätige deine E-Mail-Adresse".into(), format!("Willkommen bei Tickly, {}", u.name),
        vec!["Bitte bestätige deine E-Mail-Adresse, damit du dich anmelden kannst. Der Link ist 48 Stunden gültig.".into()], Some(("E-Mail-Adresse bestätigen".into(), link(app, "verify", &t))));
    Ok(())
}

/// Konto ist fertig angelegt (und bestätigt): wartet es auf Freigabe, erfahren es die Administratoren.
pub async fn registration_ready(app: &Arc<App>, u: &UserRow) {
    if u.status != "pending" { return }
    let Some(cfg) = active(&app.pool).await else { return };
    if !cfg.notify_admin_registration { return }
    let to = admin_emails(&app.pool).await;
    spawn(app.clone(), to, format!("Neue Registrierung wartet auf Freigabe: {}", u.name), "Neue Registrierung".into(),
        vec![format!("{} ({}) hat sich registriert und wartet auf deine Freigabe.", u.name, u.email)], Some(("Benutzerverwaltung öffnen".into(), format!("{}/#/admin", app.cfg.public_url.as_str().trim_end_matches('/')))));
}

/* ---------- Endpunkte ---------- */
pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/admin/smtp", get(get_smtp).put(put_smtp))
        .route("/admin/smtp/test", post(test_smtp))
        .route("/auth/forgot", post(forgot))
        .route("/auth/reset", post(reset))
        .route("/auth/verify", post(verify))
        .route("/auth/resend-verification", post(resend))
        .route("/me/password", post(change_password))
}

async fn admin(app: &App, h: &HeaderMap) -> Res<UserRow> {
    let u = auth(app, h, false).await?;
    if u.role != "admin" { return forbidden("Nur für Administratoren") }
    Ok(u)
}

async fn get_smtp(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    Ok(Json(json!({ "config": config(&app.pool).await?.public(), "encrypted": app.cfg.secrets.active() })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SmtpIn {
    #[serde(flatten)]
    cfg: SmtpCfg,
    /// Leer/fehlt = Passwort unverändert lassen
    new_password: Option<String>,
    #[serde(default)]
    clear_password: bool,
}

async fn put_smtp(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(mut r): Json<SmtpIn>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let old = config(&app.pool).await?;
    let c = &mut r.cfg;
    c.host = c.host.trim().to_string();
    c.username = c.username.trim().to_string();
    c.from_email = c.from_email.trim().to_string();
    c.from_name = c.from_name.trim().chars().take(60).collect();
    if c.enabled {
        if c.host.is_empty() || c.host.contains(['/', ' ', '@']) || c.host.len() > 200 { return bad("Bitte den Servernamen angeben, z. B. smtp.example.com (ohne smtp://)") }
        if c.port == 0 { return bad("Bitte einen Port angeben (587 für STARTTLS, 465 für TLS, 25 unverschlüsselt)") }
        if !["starttls", "tls", "none"].contains(&c.security.as_str()) { return bad("Unbekannte Verschlüsselung") }
        if !c.from_email.contains('@') || c.from_email.contains(char::is_whitespace) { return bad("Bitte eine gültige Absenderadresse angeben") }
    }
    c.password = if r.clear_password { String::new() } else if let Some(p) = r.new_password.filter(|p| !p.is_empty()) { app.cfg.secrets.seal_str(&p) } else { old.password };
    // Ohne funktionierendes SMTP lässt sich die Bestätigungspflicht nicht erfüllen
    if !c.enabled && db::reg_policy(&app.pool).await?.verify_email { return bad("Die E-Mail-Bestätigung bei der Registrierung ist eingeschaltet – schalte sie zuerst unter Registrierung & Richtlinien aus.") }
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('smtp', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(&*c)).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("SMTP-Einstellungen: {} ({}:{}, {})", if c.enabled { "aktiv" } else { "aus" }, c.host, c.port, c.security)).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TestIn { to: Option<String> }

/// Schickt eine Test-Nachricht mit den gespeicherten Einstellungen und meldet das Ergebnis genau zurück.
async fn test_smtp(State(app): St, h: HeaderMap, Json(r): Json<TestIn>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let cfg = config(&app.pool).await?;
    if cfg.host.is_empty() || cfg.from_email.is_empty() { return bad("Bitte zuerst Server und Absenderadresse speichern") }
    let to = r.to.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| me.email.clone());
    let (text, html) = compose("Test-Nachricht", &["Wenn du diese Nachricht liest, funktioniert der E-Mail-Versand von Tickly.".into(), format!("Gesendet über {}:{} ({}).", cfg.host, cfg.port, cfg.security)], None);
    match send(&app, &cfg, &to, "Tickly: Test-Nachricht", &text, &html).await {
        Ok(()) => { netsec::log_system(&app, "mail", true, &format!("Test-Nachricht an {to}")).await; Ok(Json(json!({ "ok": true, "to": to }))) }
        Err(e) => { netsec::log_system(&app, "mail", false, &format!("Test-Nachricht an {to} – Fehler: {e}")).await; Ok(Json(json!({ "ok": false, "error": e }))) }
    }
}

#[derive(Deserialize)]
struct EmailIn { email: String }

/// „Passwort vergessen“: antwortet immer gleich, damit sich nicht erraten lässt, welche Adressen ein Konto haben.
async fn forgot(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<EmailIn>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    let ok = Json(json!({ "ok": true, "mail": active(&app.pool).await.is_some() }));
    if active(&app.pool).await.is_none() { return Ok(ok) }
    let Some(u) = db::user_by_email(&app.pool, &r.email).await? else {
        netsec::log(&app, "password_reset", false, ip, &h, None, Some(&r.email), "Zurücksetzen angefordert, aber kein Konto mit dieser Adresse").await;
        return Ok(ok);
    };
    if u.status != "active" { return Ok(ok) }
    // höchstens 3 Mails pro Stunde je Konto
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE kind = 'password_reset' AND ok AND user_id = $1 AND at > now() - interval '1 hour'").bind(&u.id).fetch_one(&app.pool).await?;
    if recent >= 3 { return Ok(ok) }
    netsec::log(&app, "password_reset", true, ip, &h, Some(&u.id), Some(&u.email), "Link zum Zurücksetzen verschickt").await;
    let t = new_token(&app.pool, &u.id, "reset", Duration::hours(1)).await?;
    spawn(app.clone(), vec![u.email.clone()], "Passwort zurücksetzen".into(), "Passwort zurücksetzen".into(),
        vec!["Für dein Konto wurde ein neues Passwort angefordert. Der Link ist eine Stunde gültig und funktioniert einmal. Wenn du das nicht warst, kannst du diese Nachricht ignorieren – dein Passwort bleibt unverändert.".into()],
        Some(("Neues Passwort festlegen".into(), link(&app, "reset", &t))));
    Ok(ok)
}

#[derive(Deserialize)]
struct ResetIn { token: String, password: String }

async fn reset(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<ResetIn>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    let reg = db::reg_policy(&app.pool).await?;
    if let Some(m) = password_problem(&r.password, reg.min_pw) { return bad(m) }
    let Some(uid) = take_token(&app.pool, &r.token, "reset").await? else {
        netsec::failure(&app, "login", ip, &h, None, None, "Ungültiger oder abgelaufener Link zum Zurücksetzen").await;
        return bad("Der Link ist ungültig oder abgelaufen. Bitte fordere einen neuen an.");
    };
    sqlx::query("UPDATE users SET password_hash = $2, email_verified_at = coalesce(email_verified_at, now()) WHERE id = $1").bind(&uid).bind(hash_password(&r.password)).execute(&app.pool).await?;
    // Alle bestehenden Sitzungen enden: wer das Passwort nicht kannte, soll nicht angemeldet bleiben
    sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(&uid).execute(&app.pool).await?;
    let u = db::user_by_id(&app.pool, &uid).await?;
    netsec::log(&app, "password_reset", true, ip, &h, Some(&uid), u.as_ref().map(|u| u.email.as_str()), "Passwort per Link geändert, alle Sitzungen beendet").await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct TokenIn { token: String }

/// Bestätigt die Adresse und meldet – wenn das Konto aktiv ist – gleich an.
async fn verify(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<TokenIn>) -> Res<Response> {
    netsec::guard(&app, ip).await?;
    let Some(uid) = take_token(&app.pool, &r.token, "verify").await? else {
        netsec::failure(&app, "login", ip, &h, None, None, "Ungültiger oder abgelaufener Bestätigungslink").await;
        return bad("Der Bestätigungslink ist ungültig oder abgelaufen. Melde dich an, dann kannst du einen neuen anfordern.");
    };
    sqlx::query("UPDATE users SET email_verified_at = now() WHERE id = $1").bind(&uid).execute(&app.pool).await?;
    let u = db::user_by_id(&app.pool, &uid).await?.ok_or(ApiError(axum::http::StatusCode::NOT_FOUND, "Konto nicht gefunden".into()))?;
    netsec::log(&app, "register", true, ip, &h, Some(&u.id), Some(&u.email), "E-Mail-Adresse bestätigt").await;
    registration_ready(&app, &u).await;
    if u.status != "active" { return Ok(Json(json!({ "ok": true, "status": u.status })).into_response()) }
    let reg = db::reg_policy(&app.pool).await?;
    let secs = db::session_secs(&reg, &db::mfa_status(&app.pool, &u, &reg).await?);
    let sid = db::create_session(&app.pool, &u.id, false, secs, &ip, &netsec::ua(&h)).await?;
    let secure = if app.cfg.secure_cookie { "; Secure" } else { "" };
    Ok(([(header::SET_COOKIE, format!("tickly_sid={sid}; Path=/; HttpOnly; SameSite=Lax; Max-Age={secs}{secure}"))], Json(json!({ "ok": true, "status": "active" }))).into_response())
}

async fn resend(State(app): St, Extension(ip): Extension<ClientIp>, Json(r): Json<EmailIn>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    if let Some(u) = db::user_by_email(&app.pool, &r.email).await? {
        if needs_verification(&app, &u).await? { send_verification(&app, &u).await?; }
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct PwIn { current: String, password: String }

/// Passwort ändern (angemeldet). Andere Sitzungen enden.
async fn change_password(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<PwIn>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    netsec::guard(&app, ip).await?;
    if !u.password_hash.as_deref().is_some_and(|ph| verify_password(ph, &r.current)) {
        netsec::failure(&app, "login", ip, &h, Some(&u.id), Some(&u.email), "Passwort ändern: aktuelles Passwort falsch").await;
        return unauthorized("Das aktuelle Passwort ist falsch");
    }
    if let Some(m) = password_problem(&r.password, db::reg_policy(&app.pool).await?.min_pw) { return bad(m) }
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1").bind(&u.id).bind(hash_password(&r.password)).execute(&app.pool).await?;
    let cur = crate::api::sid_from(&h).unwrap_or_default();
    sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id <> $2").bind(&u.id).bind(cur).execute(&app.pool).await?;
    netsec::log(&app, "password_reset", true, ip, &h, Some(&u.id), Some(&u.email), "Passwort geändert").await;
    Ok(Json(json!({ "ok": true })))
}
