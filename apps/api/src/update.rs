//! Update-Hinweise: fragt die neueste Version bei GitHub ab (höchstens alle 6 Stunden), merkt sie sich in der
//! Datenbank und liefert sie an Administratoren (Server-Update), Benutzer (Download des Companion) und Companions.
//!
//! Gesendet wird nur eine anonyme GET-Anfrage an die Releases-Schnittstelle (User-Agent `tickly-server/<Version>`),
//! keine Benutzerdaten. Abschaltbar mit `TICKLY_UPDATE_CHECK=0` oder in der Verwaltung; im Demo-Modus nie.
use crate::api::{auth, St};
use crate::app::*;
use crate::db::UserRow;
use crate::netsec::{self, ClientIp};
use axum::{extract::State, http::HeaderMap, routing::{get, post}, Extension, Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Json as SJson;
use std::{cmp::Ordering, sync::Arc, time::Duration as Std};

pub const DEFAULT_REPO: &str = "M0nk3yOo/tickly";
const DEFAULT_API: &str = "https://api.github.com";
/// Nach einer erfolgreichen Abfrage so lange warten, nach einem Fehler kürzer.
const OK_EVERY: i64 = 6 * 3600;
const ERR_EVERY: i64 = 3600;
const NOTES_MAX: usize = 1500;

/// Konfiguration aus der Umgebung. `api_base` ist nur für Tests und GitHub Enterprise gedacht.
#[derive(Clone, Debug)]
pub struct UpdateCfg {
    pub env_enabled: bool,
    pub repo: String,
    pub api_base: String,
}

impl UpdateCfg {
    pub fn from_env() -> UpdateCfg {
        let env = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let off = env("TICKLY_UPDATE_CHECK").is_some_and(|v| matches!(v.to_lowercase().as_str(), "0" | "false" | "no" | "nein" | "off" | "aus"));
        let repo = env("TICKLY_UPDATE_REPO").filter(|r| valid_repo(r)).unwrap_or_else(|| DEFAULT_REPO.into());
        UpdateCfg { env_enabled: !off, repo, api_base: env("TICKLY_UPDATE_API").unwrap_or_else(|| DEFAULT_API.into()).trim_end_matches('/').to_string() }
    }
    /// Für Tests: kein Netzzugriff.
    pub fn off() -> UpdateCfg {
        UpdateCfg { env_enabled: false, repo: DEFAULT_REPO.into(), api_base: DEFAULT_API.into() }
    }
}

fn valid_repo(r: &str) -> bool {
    let ok = |s: &str| !s.is_empty() && s.len() <= 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    r.split_once('/').is_some_and(|(a, b)| ok(a) && ok(b) && !b.contains('/'))
}

/* ---------- Versionsvergleich ---------- */

/// Zerlegt „v1.2.3-rc.1+build“ in Zahlen und Vorabkennung; `None`, wenn es keine Version ist.
fn parse(v: &str) -> Option<(Vec<u64>, Vec<String>)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let v = v.split('+').next()?;
    let (core, pre) = match v.split_once('-') { Some((c, p)) => (c, p), None => (v, "") };
    let nums: Vec<u64> = core.split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<_>>()?;
    if nums.is_empty() || nums.len() > 4 { return None }
    Some((nums, pre.split('.').filter(|s| !s.is_empty()).map(String::from).collect()))
}

/// Vergleicht zwei Versionen nach Semver (0.9.9 < 0.9.18, 1.0.0-rc.1 < 1.0.0); `None`, wenn eine keine Version ist.
pub fn cmp_versions(a: &str, b: &str) -> Option<Ordering> {
    let ((mut na, pa), (mut nb, pb)) = (parse(a)?, parse(b)?);
    let n = na.len().max(nb.len()).max(3);
    na.resize(n, 0);
    nb.resize(n, 0);
    match na.cmp(&nb) {
        Ordering::Equal => {}
        o => return Some(o),
    }
    Some(match (pa.is_empty(), pb.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        _ => {
            for (x, y) in pa.iter().zip(pb.iter()) {
                let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(i), Ok(j)) => i.cmp(&j),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    _ => x.cmp(y),
                };
                if o != Ordering::Equal { return Some(o) }
            }
            pa.len().cmp(&pb.len())
        }
    })
}

/// Ist `latest` neuer als `current`? Unlesbare Versionen gelten nie als veraltet.
pub fn newer(latest: &str, current: &str) -> bool {
    cmp_versions(latest, current) == Some(Ordering::Greater)
}

/* ---------- Release auswerten ---------- */

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
}

/// Gemerkte Information über die neueste Version.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(default)]
pub struct Info {
    pub latest: String,
    /// Seite des Releases auf GitHub
    pub url: String,
    pub notes: String,
    pub mac: Option<Asset>,
    pub win: Option<Asset>,
    pub repo: String,
    /// Letzte erfolgreiche Prüfung
    pub checked_at: Option<DateTime<Utc>>,
    /// Letzter Versuch (auch bei Fehler)
    pub attempted_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

fn web_url(u: &str, allow_http: bool) -> Option<String> {
    let p = url::Url::parse(u).ok()?;
    (p.scheme() == "https" || (allow_http && p.scheme() == "http")).then(|| p.to_string())
}

fn short_notes(s: &str) -> String {
    let s = s.replace('\r', "");
    let s = s.trim();
    if s.chars().count() <= NOTES_MAX { return s.to_string() }
    let cut: String = s.chars().take(NOTES_MAX).collect();
    let cut = cut.rsplit_once('\n').map(|(a, _)| a.to_string()).filter(|a| a.chars().count() > NOTES_MAX / 2).unwrap_or(cut);
    format!("{}\n…", cut.trim_end())
}

/// Liest die Antwort von `GET /repos/<repo>/releases/latest`.
pub fn parse_release(v: &Value, allow_http: bool) -> Result<Info, String> {
    let tag = v["tag_name"].as_str().unwrap_or("").trim().trim_start_matches(['v', 'V']).to_string();
    if parse(&tag).is_none() { return Err(format!("Unbekanntes Versionsformat: „{tag}“")) }
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) { return Err("Neuestes Release ist ein Entwurf oder eine Vorabversion".into()) }
    let url = v["html_url"].as_str().and_then(|u| web_url(u, allow_http)).unwrap_or_default();
    let assets: Vec<Asset> = v["assets"].as_array().map(|a| a.iter().filter_map(|x| {
        Some(Asset { name: x["name"].as_str()?.to_string(), url: web_url(x["browser_download_url"].as_str()?, allow_http)?, size: x["size"].as_u64() })
    }).collect()).unwrap_or_default();
    let by = |ext: &str| assets.iter().find(|a| a.name.to_lowercase().ends_with(ext)).cloned();
    Ok(Info { latest: tag, url, notes: short_notes(v["body"].as_str().unwrap_or("")), mac: by(".dmg"), win: by(".exe").or_else(|| by(".msi")), ..Default::default() })
}

/* ---------- Abfragen und Merken ---------- */

async fn fetch(cfg: &UpdateCfg) -> Result<Info, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("tickly-server/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Std::from_secs(5))
        .timeout(Std::from_secs(10))
        .build().map_err(|e| e.to_string())?;
    let r = client.get(format!("{}/repos/{}/releases/latest", cfg.api_base, cfg.repo)).header("Accept", "application/vnd.github+json").send().await.map_err(|e| format!("nicht erreichbar: {e}"))?;
    if !r.status().is_success() { return Err(format!("Antwort {}", r.status())) }
    if r.content_length().is_some_and(|n| n > 2_000_000) { return Err("Antwort zu groß".into()) }
    let body = r.bytes().await.map_err(|e| format!("Antwort unvollständig: {e}"))?;
    if body.len() > 2_000_000 { return Err("Antwort zu groß".into()) }
    let v: Value = serde_json::from_slice(&body).map_err(|_| "Antwort ist kein JSON".to_string())?;
    parse_release(&v, cfg.api_base.starts_with("http://"))
}

/// Gespeicherte Information (leer, solange noch nie geprüft oder bei geändertem Repository).
async fn stored(app: &App) -> Info {
    let r: Option<(SJson<Info>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'update_info'").fetch_optional(&app.pool).await.ok().flatten();
    r.map(|r| r.0 .0).filter(|i| i.repo == app.cfg.update.repo).unwrap_or_default()
}

/// Schalter in der Verwaltung (Standard an).
async fn setting(app: &App) -> bool {
    let r: Option<(Value,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'update_check'").fetch_optional(&app.pool).await.ok().flatten();
    r.and_then(|r| r.0["enabled"].as_bool()).unwrap_or(true)
}

/// Wird überhaupt geprüft? Die Umgebungsvariable hat Vorrang zum Abschalten; im Demo-Modus nie.
pub async fn enabled(app: &App) -> bool {
    app.cfg.update.env_enabled && !app.cfg.demo && setting(app).await
}

/// Gültige Information, nur wenn die Prüfung an ist.
pub async fn current(app: &App) -> Option<Info> {
    if !enabled(app).await { return None }
    Some(stored(app).await).filter(|i| !i.latest.is_empty())
}

fn due(i: &Info, now: DateTime<Utc>) -> bool {
    let every = if i.error.is_some() { ERR_EVERY } else { OK_EVERY };
    i.attempted_at.is_none_or(|t| now - t >= Duration::seconds(every))
}

/// Fragt GitHub ab, wenn die Prüfung an ist und die letzte Abfrage lange genug her ist (oder `force`). Fehler werden nur protokolliert.
pub async fn refresh(app: &App, force: bool) -> Info {
    let mut info = stored(app).await;
    if !enabled(app).await || (!force && !due(&info, Utc::now())) { return info }
    let now = Utc::now();
    match fetch(&app.cfg.update).await {
        Ok(f) => info = Info { repo: app.cfg.update.repo.clone(), checked_at: Some(now), attempted_at: Some(now), error: None, ..f },
        Err(e) => {
            tracing::warn!("Update-Prüfung bei {} fehlgeschlagen: {e}", app.cfg.update.repo);
            info.repo = app.cfg.update.repo.clone();
            info.attempted_at = Some(now);
            info.error = Some(e);
        }
    }
    let _ = sqlx::query("INSERT INTO app_settings (key, value) VALUES ('update_info', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(&info)).execute(&app.pool).await;
    info
}

/// Hintergrundaufgabe: prüft kurz nach dem Start und danach, sobald die Zwischenspeicherung abgelaufen ist. Blockiert nichts.
pub fn spawn_checker(app: Arc<App>) {
    if !app.cfg.update.env_enabled || app.cfg.demo { return }
    tokio::spawn(async move {
        tokio::time::sleep(Std::from_secs(20)).await;
        loop {
            refresh(&app, false).await;
            tokio::time::sleep(Std::from_secs(1800)).await;
        }
    });
}

/* ---------- Ausgabe ---------- */

fn asset_json(a: &Option<Asset>) -> Value {
    a.as_ref().map(|a| json!({ "name": a.name, "url": a.url, "size": a.size })).unwrap_or(Value::Null)
}

/// Für Administratoren: Server-Update und Stand der Prüfung.
pub async fn admin_json(app: &App) -> Value {
    let on = enabled(app).await;
    let i = if on { stored(app).await } else { Info::default() };
    let installed = env!("CARGO_PKG_VERSION");
    json!({
        "enabled": on, "setting": setting(app).await, "envOff": !app.cfg.update.env_enabled, "demo": app.cfg.demo, "repo": app.cfg.update.repo,
        "installed": installed, "latest": (!i.latest.is_empty()).then_some(&i.latest), "newer": !i.latest.is_empty() && newer(&i.latest, installed),
        "url": i.url, "notes": i.notes, "checkedAt": i.checked_at, "attemptedAt": i.attempted_at, "error": i.error,
    })
}

/// Für alle angemeldeten Benutzer: Installer des Companion.
pub async fn download_json(app: &App) -> Value {
    match current(app).await {
        Some(i) => json!({ "version": i.latest, "url": i.url, "mac": asset_json(&i.mac), "win": asset_json(&i.win) }),
        None => Value::Null,
    }
}

/// Für einen Companion: nur, wenn seine Version älter ist als die neueste. `download` passt zu seinem Betriebssystem.
pub async fn for_device(app: &App, os: &str, version: &str) -> Value {
    let Some(i) = current(app).await else { return Value::Null };
    if !newer(&i.latest, version) { return Value::Null }
    let a = if os == "macos" { &i.mac } else { &i.win };
    json!({ "latest": i.latest, "notes": i.notes, "url": i.url, "download": a.as_ref().map(|a| a.url.clone()).unwrap_or_else(|| i.url.clone()), "size": a.as_ref().and_then(|a| a.size) })
}

/* ---------- Schnittstelle ---------- */

pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/admin/update", get(get_update).put(put_update)).route("/admin/update/check", post(check_now))
}

async fn admin(app: &App, h: &HeaderMap) -> Res<UserRow> {
    let u = auth(app, h, false).await?;
    if u.role != "admin" { return forbidden("Nur für Administratoren") }
    Ok(u)
}

async fn get_update(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    Ok(Json(admin_json(&app).await))
}

#[derive(Deserialize)]
struct PutReq { enabled: bool }

async fn put_update(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<PutReq>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('update_check', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(json!({ "enabled": r.enabled }))).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, if r.enabled { "Suche nach Updates eingeschaltet" } else { "Suche nach Updates ausgeschaltet" }).await;
    Ok(Json(admin_json(&app).await))
}

async fn check_now(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    if app.cfg.demo { return bad("Im Demo-Modus wird nicht nach Updates gesucht") }
    if !app.cfg.update.env_enabled { return bad("Die Suche nach Updates ist per TICKLY_UPDATE_CHECK=0 abgeschaltet") }
    if !setting(&app).await { return bad("Die Suche nach Updates ist ausgeschaltet") }
    refresh(&app, true).await;
    Ok(Json(admin_json(&app).await))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vergleich() {
        assert!(newer("0.9.18", "0.9.9"));
        assert!(!newer("0.9.9", "0.9.18"));
        assert!(newer("0.10.0", "0.9.18"));
        assert!(newer("v1.0.0", "0.9.18"));
        assert!(!newer("0.9.18", "0.9.18"));
        assert!(!newer("0.9.18", "v0.9.18"));
        assert!(newer("1.0.0", "1.0.0-rc.1"));
        assert!(!newer("1.0.0-rc.1", "1.0.0"));
        assert!(newer("1.0.0-rc.2", "1.0.0-rc.1"));
        assert!(newer("1.0.1", "1.0"));
        assert!(!newer("1.0", "1.0.0"));
        assert!(!newer("1.0.0+b2", "1.0.0+b1"));
        assert_eq!(cmp_versions("abc", "1.0.0"), None);
        assert!(!newer("abc", "1.0.0"));
        assert!(!newer("1.0.0", ""));
    }

    #[test]
    fn repo() {
        assert!(valid_repo("M0nk3yOo/tickly"));
        assert!(!valid_repo("nur-ein-name"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("a/b?x=1"));
    }

    #[test]
    fn release() {
        let v = json!({ "tag_name": "v0.9.19", "html_url": "https://github.com/x/y/releases/tag/v0.9.19", "body": "Neu\r\n- eins", "assets": [
            { "name": "SHA256SUMS", "browser_download_url": "https://github.com/x/y/releases/download/v0.9.19/SHA256SUMS", "size": 3 },
            { "name": "Tickly.Companion_0.9.19_x64_en-US.msi", "browser_download_url": "https://github.com/a.msi", "size": 5 },
            { "name": "Tickly.Companion_0.9.19_x64-setup.exe", "browser_download_url": "https://github.com/a.exe", "size": 7 },
            { "name": "Tickly.Companion_0.9.19_aarch64.dmg", "browser_download_url": "https://github.com/a.dmg", "size": 9 },
            { "name": "boese", "browser_download_url": "javascript:alert(1)" } ] });
        let i = parse_release(&v, false).unwrap();
        assert_eq!(i.latest, "0.9.19");
        assert_eq!(i.notes, "Neu\n- eins");
        assert_eq!(i.win.unwrap().url, "https://github.com/a.exe");
        assert_eq!(i.mac.unwrap().size, Some(9));
        // ohne .exe: die .msi
        let v2 = json!({ "tag_name": "0.9.19", "assets": [{ "name": "a.msi", "browser_download_url": "https://github.com/a.msi" }] });
        let i = parse_release(&v2, false).unwrap();
        assert_eq!(i.win.unwrap().name, "a.msi");
        assert!(i.mac.is_none() && i.url.is_empty());
        assert!(parse_release(&json!({ "tag_name": "nightly" }), false).is_err());
        assert!(parse_release(&json!({ "tag_name": "1.0.0", "prerelease": true }), false).is_err());
        assert!(parse_release(&json!({ "tag_name": "1.0.0", "assets": [{ "name": "a.dmg", "browser_download_url": "http://x/a.dmg" }] }), false).unwrap().mac.is_none());
    }

    #[test]
    fn hinweise_kuerzen() {
        let long = "zeile\n".repeat(1000);
        let s = short_notes(&long);
        assert!(s.chars().count() <= NOTES_MAX + 2 && s.ends_with('…'));
        assert_eq!(short_notes("kurz"), "kurz");
    }
}
