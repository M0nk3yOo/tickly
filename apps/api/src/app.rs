//! Konfiguration, gemeinsamer Zustand, Passwortregeln und Fehlertypen.
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use chrono::{DateTime, NaiveDate, Timelike, Utc};
use chrono_tz::Tz;
use serde_json::json;
use sqlx::PgPool;
use std::{collections::HashMap, path::PathBuf, sync::Mutex, time::Instant};
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration, Url, Webauthn, WebauthnBuilder};

pub const DEFAULT_DEMO_PASSWORD: &str = "demo-passwort";
/// Mindestlänge für Passwörter; Administratoren können sie nur erhöhen.
pub const MIN_PW: u32 = 14;

/// Passwortrichtlinie: Mindestlänge, Groß- und Kleinbuchstaben, Zahl und Sonderzeichen.
pub fn password_problem(pw: &str, min: u32) -> Option<String> {
    let min = min.max(MIN_PW);
    let mut missing = Vec::new();
    if (pw.chars().count() as u32) < min { missing.push(format!("mindestens {min} Zeichen")); }
    if !pw.chars().any(|c| c.is_uppercase()) { missing.push("einen Großbuchstaben".into()); }
    if !pw.chars().any(|c| c.is_lowercase()) { missing.push("einen Kleinbuchstaben".into()); }
    if !pw.chars().any(|c| c.is_ascii_digit()) { missing.push("eine Zahl".into()); }
    if !pw.chars().any(|c| !c.is_alphanumeric() && !c.is_whitespace()) { missing.push("ein Sonderzeichen".into()); }
    if pw.chars().count() > 128 { return Some("Das Passwort darf höchstens 128 Zeichen lang sein".into()); }
    if missing.is_empty() { None } else { Some(format!("Das Passwort braucht {}", missing.join(", "))) }
}

/* ---------- Konfiguration (Umgebungsvariablen) ---------- */
pub struct Config {
    pub bind: String,
    pub public_url: Url,
    pub rp_id: String,
    pub web_dir: PathBuf,
    pub database_url: String,
    /// Zeitzone, in der Tage beginnen und enden (Standard Europe/Berlin).
    pub tz: Tz,
    /// Demo-Modus: festes Datum, Uhrzeit vom Client (Zeitraffer), erzeugte Historie und Demo-Konten.
    pub demo: bool,
    pub demo_today: NaiveDate,
    pub demo_track_start: NaiveDate,
    pub demo_password: String,
    pub secure_cookie: bool,
    /// Erster Administrator im Echtbetrieb (wird angelegt, wenn es noch keine Benutzer gibt).
    pub admin: Option<(String, String, String)>,
    /// Weitere Proxy-Adressen, deren Weiterleitungs-Header gelten (zusätzlich zu lokal/privat).
    pub trusted_proxies: Vec<(std::net::IpAddr, u8)>,
    /// So lange bleibt das Protokoll erhalten.
    pub log_retention_days: u32,
    /// Neueste Version der Companion-App (für den Hinweis auf veraltete Geräte); sonst die höchste gemeldete.
    pub companion_latest: Option<String>,
    /// Verzeichnis für automatische Sicherungen (getrennt von der Datenbank), z. B. ein eingebundenes Volume.
    pub backup_dir: Option<PathBuf>,
    pub secrets: crate::secrets::Secrets,
}

impl Config {
    pub fn from_env() -> Config {
        let env = |k: &str, d: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        let opt = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let port = env("PORT", "3000");
        let public_url = Url::parse(&env("PUBLIC_URL", &format!("http://localhost:{port}"))).expect("PUBLIC_URL ist keine gültige URL");
        let rp_id = env("WEBAUTHN_RP_ID", public_url.host_str().unwrap_or("localhost"));
        let demo = matches!(env("TICKLY_DEMO", "0").to_lowercase().as_str(), "1" | "true" | "yes" | "ja");
        let demo_password = env("DEMO_PASSWORD", DEFAULT_DEMO_PASSWORD);
        let https = public_url.scheme() == "https";
        if demo && https {
            // Öffentlich erreichbar darf das bekannte Demo-Passwort nicht aktiv sein.
            if demo_password == DEFAULT_DEMO_PASSWORD {
                panic!("TICKLY_DEMO ist an und PUBLIC_URL ist HTTPS, aber DEMO_PASSWORD ist nicht gesetzt. Bitte in der .env ein eigenes Passwort für die Demo-Konten festlegen.");
            }
            if let Some(p) = password_problem(&demo_password, MIN_PW) {
                panic!("DEMO_PASSWORD erfüllt die Passwortrichtlinie nicht. {p}.");
            }
        }
        let admin = opt("ADMIN_EMAIL").map(|mail| {
            let pw = opt("ADMIN_PASSWORD").expect("ADMIN_EMAIL ist gesetzt, aber ADMIN_PASSWORD fehlt");
            if let Some(p) = password_problem(&pw, MIN_PW) { panic!("ADMIN_PASSWORD erfüllt die Passwortrichtlinie nicht. {p}."); }
            (mail, env("ADMIN_NAME", "Administrator"), pw)
        });
        let tz: Tz = env("TICKLY_TZ", "Europe/Berlin").parse().expect("TICKLY_TZ: unbekannte Zeitzone, z. B. Europe/Berlin");
        Config {
            bind: env("BIND", &format!("0.0.0.0:{port}")),
            secure_cookie: https,
            public_url,
            rp_id,
            web_dir: env("WEB_DIR", "web").into(),
            database_url: opt("DATABASE_URL").expect("DATABASE_URL fehlt, z. B. postgres://tickly:geheim@localhost:5432/tickly"),
            tz,
            demo,
            demo_today: NaiveDate::parse_from_str(&env("TICKLY_TODAY", "2026-10-26"), "%Y-%m-%d").expect("TICKLY_TODAY: YYYY-MM-DD"),
            demo_track_start: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
            demo_password,
            admin,
            trusted_proxies: crate::netsec::parse_cidrs(&env("TRUSTED_PROXIES", "")),
            log_retention_days: env("LOG_RETENTION_DAYS", "365").parse().unwrap_or(365).clamp(7, 3650),
            companion_latest: opt("COMPANION_LATEST_VERSION"),
            backup_dir: opt("BACKUP_DIR").map(PathBuf::from),
            secrets: crate::secrets::Secrets::from_env(),
        }
    }
}

/// Gemeinsamer Zustand. Alles Dauerhafte liegt in PostgreSQL; hier nur Kurzlebiges (WebAuthn-Zeremonien).
pub struct App {
    pub cfg: Config,
    pub pool: PgPool,
    pub webauthn: Webauthn,
    pub reg_states: Mutex<HashMap<String, (String, PasskeyRegistration)>>,
    pub auth_states: Mutex<HashMap<String, (String, PasskeyAuthentication)>>,
    /// Fehlversuche beim Koppeln (Zeitpunkt des Fensters, Anzahl) – bremst das Raten von Kopplungscodes.
    pub pair_fails: Mutex<(Instant, u32)>,
    /// Zweiter Anmeldeschritt: Ticket → (Benutzer, ausgestellt, Versuche)
    pub mfa_tickets: Mutex<HashMap<String, (String, Instant, u8)>>,
    /// Falsche Codes je Benutzer (Beginn des Fensters, Anzahl)
    pub mfa_fails: Mutex<HashMap<String, (Instant, u32)>>,
}

impl App {
    pub fn new(cfg: Config, pool: PgPool) -> App {
        let webauthn = WebauthnBuilder::new(&cfg.rp_id, &cfg.public_url)
            .expect("WebAuthn-Konfiguration ungültig (WEBAUTHN_RP_ID muss zur PUBLIC_URL passen)")
            .rp_name("Tickly")
            .build()
            .expect("WebAuthn konnte nicht gebaut werden");
        App { cfg, pool, webauthn, reg_states: Default::default(), auth_states: Default::default(), pair_fails: Mutex::new((Instant::now(), 0)), mfa_tickets: Default::default(), mfa_fails: Default::default() }
    }

    /// „Jetzt“ als (Datum, Minuten seit Mitternacht). Im Demo-Modus gilt das feste Datum und die Uhrzeit des Clients.
    pub fn clock(&self, client_now: Option<f64>) -> (NaiveDate, f64) {
        if self.cfg.demo {
            (self.cfg.demo_today, client_now.unwrap_or(872.0).clamp(0.0, 1439.9))
        } else {
            self.local(Utc::now())
        }
    }

    /// Zeitpunkt in lokales Datum und Minuten seit Mitternacht umrechnen.
    pub fn local(&self, at: DateTime<Utc>) -> (NaiveDate, f64) {
        let l = at.with_timezone(&self.cfg.tz);
        (l.date_naive(), l.num_seconds_from_midnight() as f64 / 60.0)
    }
}

/* ---------- Fehler ---------- */
pub struct ApiError(pub StatusCode, pub String);
pub type Res<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("Datenbankfehler: {e}");
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "Interner Fehler (Datenbank)".into())
    }
}
pub fn err<T>(code: StatusCode, msg: impl Into<String>) -> Res<T> {
    Err(ApiError(code, msg.into()))
}
pub fn bad<T>(msg: impl Into<String>) -> Res<T> { err(StatusCode::BAD_REQUEST, msg) }
pub fn unauthorized<T>(msg: impl Into<String>) -> Res<T> { err(StatusCode::UNAUTHORIZED, msg) }
pub fn forbidden<T>(msg: impl Into<String>) -> Res<T> { err(StatusCode::FORBIDDEN, msg) }
pub fn not_found<T>(msg: impl Into<String>) -> Res<T> { err(StatusCode::NOT_FOUND, msg) }
pub fn conflict<T>(msg: impl Into<String>) -> Res<T> { err(StatusCode::CONFLICT, msg) }

pub fn hash_password(pw: &str) -> String {
    use argon2::password_hash::{rand_core::OsRng, PasswordHasher, SaltString};
    argon2::Argon2::default().hash_password(pw.as_bytes(), &SaltString::generate(&mut OsRng)).expect("Hash").to_string()
}

pub fn verify_password(hash: &str, pw: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    PasswordHash::new(hash).map(|h| argon2::Argon2::default().verify_password(pw.as_bytes(), &h).is_ok()).unwrap_or(false)
}
