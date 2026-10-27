//! Sicherung und Wiederherstellung der gesamten Installation.
//!
//! Eine Sicherung ist gzip-komprimiertes JSON mit dem Inhalt aller Tabellen außer Sitzungen, Kopplungscodes und
//! den Sicherungen selbst. Sie enthält Passwort-Hashes, Passkeys und Geräte-Token-Hashes – wie eine Datenbanksicherung
//! sorgfältig aufbewahren. Mit gesetztem `TICKLY_SECRET` sind Sicherungen (Dateien und Datenbankeinträge) zusätzlich
//! verschlüsselt. Automatische Sicherungen laufen zur eingestellten Uhrzeit, wahlweise in ein getrenntes Verzeichnis (`BACKUP_DIR`).
use crate::api::{auth, St};
use crate::app::*;
use crate::netsec::{self, ClientIp};
use crate::secrets::Secrets;
use axum::Extension;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveTime, Utc};
use chrono::TimeZone;
use serde::{Deserialize, Serialize};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::{json, Value};
use sqlx::{types::Json as SJson, PgPool, Postgres, Transaction};
use std::{io::{Read, Write}, sync::Arc};

const FORMAT: &str = "tickly-backup";
const VERSION: u64 = 1;
const KEEP_PRE_RESTORE: u32 = 5;
/// Tabellen in der Reihenfolge, in der sie wieder eingefügt werden (Fremdschlüssel).
const TABLES: [&str; 7] = ["users", "credentials", "recovery_codes", "absences", "days", "devices", "app_settings"];
/// Fehlt in Sicherungen aus früheren Versionen; gilt dann als leer.
const OPTIONAL: [&str; 1] = ["recovery_codes"];
/// Tabellen mit fortlaufender Nummer, deren Zähler nach dem Einfügen nachgezogen werden muss.
const SERIALS: [(&str, &str); 3] = [("credentials", "id"), ("absences", "id"), ("recovery_codes", "id")];
const MAX_UPLOAD: usize = 512 * 1024 * 1024;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/admin/backup", get(download))
        .route("/admin/backups", get(list).post(backup_now))
        .route("/admin/backup-policy", get(get_policy).put(put_policy))
        .route("/admin/backups/{id}", get(download_saved))
        .route("/admin/backups/{id}/restore", post(restore_saved))
        .route("/admin/restore", post(restore_upload).layer(DefaultBodyLimit::max(MAX_UPLOAD)))
}

fn gzip(v: &Value) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(&serde_json::to_vec(v).unwrap_or_default()).ok();
    e.finish().unwrap_or_default()
}

fn gunzip(b: &[u8]) -> Result<Vec<u8>, String> {
    if b.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(b).take(MAX_UPLOAD as u64 * 4).read_to_end(&mut out).map_err(|e| format!("Datei ist beschädigt: {e}"))?;
        Ok(out)
    } else {
        Ok(b.to_vec())
    }
}

/// Alle Daten als JSON.
pub async fn export(pool: &PgPool) -> Result<Value, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ").execute(&mut *tx).await?;
    let mut data = serde_json::Map::new();
    for t in TABLES {
        let v: Value = sqlx::query_scalar(&format!("SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM {t} t")).fetch_one(&mut *tx).await?;
        data.insert(t.into(), v);
    }
    tx.commit().await?;
    Ok(json!({ "format": FORMAT, "version": VERSION, "createdAt": Utc::now(), "data": data }))
}

pub struct Summary {
    pub users: usize,
    pub days: usize,
}

fn check(v: &Value) -> Result<Summary, String> {
    if v["format"] != FORMAT { return Err("Das ist keine Tickly-Sicherung".into()) }
    if v["version"].as_u64().is_none_or(|n| n > VERSION) { return Err("Die Sicherung stammt aus einer neueren Version von Tickly".into()) }
    let list = |t: &str| v["data"][t].as_array().ok_or_else(|| format!("In der Sicherung fehlt „{t}“"));
    for t in TABLES.iter().filter(|t| !OPTIONAL.contains(t)) { list(t)?; }
    let users = list("users")?;
    if !users.iter().any(|u| u["role"] == "admin" && u["status"] == "active") {
        return Err("Die Sicherung enthält keinen aktiven Administrator – danach käme niemand mehr hinein".into());
    }
    Ok(Summary { users: users.len(), days: list("days")?.len() })
}

/// Ersetzt den gesamten Datenbestand. Alles oder nichts: bei einem Fehler bleibt der alte Stand.
async fn restore(pool: &PgPool, v: &Value, keep_session: Option<&str>) -> Result<Summary, ApiError> {
    let sum = check(v).map_err(|m| ApiError(axum::http::StatusCode::BAD_REQUEST, m))?;
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    // Die Sitzung des Administrators, der gerade wiederherstellt, bleibt erhalten
    let keep: Option<(String, String, bool, DateTime<Utc>)> = match keep_session {
        Some(sid) => sqlx::query_as("SELECT id, user_id, limited, expires_at FROM sessions WHERE id = $1").bind(sid).fetch_optional(&mut *tx).await?,
        None => None,
    };
    sqlx::query("DELETE FROM users").execute(&mut *tx).await?; // löscht über ON DELETE CASCADE alles Abhängige
    sqlx::query("DELETE FROM app_settings").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM pair_codes").execute(&mut *tx).await?;
    for t in TABLES {
        let rows = if v["data"][t].is_array() { v["data"][t].clone() } else { json!([]) };
        let Some(first) = rows.get(0).and_then(|r| r.as_object()) else { continue };
        // Nur Spalten übernehmen, die in der Sicherung stehen und noch existieren. Fehlende (neuere) Spalten bekommen ihren
        // Standardwert, sodass auch Sicherungen aus älteren Versionen einspielbar sind.
        let known: Vec<String> = sqlx::query_scalar("SELECT column_name::text FROM information_schema.columns WHERE table_schema = current_schema() AND table_name = $1").bind(t).fetch_all(&mut *tx).await?;
        let cols: Vec<&str> = first.keys().map(String::as_str).filter(|k| known.iter().any(|c| c == k)).collect();
        if cols.is_empty() { continue }
        let list = cols.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
        sqlx::query(&format!("INSERT INTO {t} ({list}) SELECT {list} FROM jsonb_populate_recordset(NULL::{t}, $1)")).bind(rows).execute(&mut *tx).await
            .map_err(|e| ApiError(axum::http::StatusCode::BAD_REQUEST, format!("Die Sicherung passt nicht zum Schema ({t}): {e}")))?;
    }
    for (t, c) in SERIALS {
        sqlx::query(&format!("SELECT setval(pg_get_serial_sequence('{t}', '{c}'), greatest((SELECT coalesce(max({c}), 0) FROM {t}), 1), (SELECT count(*) > 0 FROM {t}))")).execute(&mut *tx).await?;
    }
    if let Some((id, uid, limited, exp)) = keep {
        sqlx::query("INSERT INTO sessions (id, user_id, limited, expires_at) SELECT $1, $2, $3, $4 WHERE EXISTS (SELECT 1 FROM users WHERE id = $2)")
            .bind(id).bind(uid).bind(limited).bind(exp).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(sum)
}

/* ---------- Einstellungen ---------- */
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPolicy {
    pub enabled: bool,
    /// Uhrzeit der täglichen Sicherung (lokale Zeit, „HH:MM“).
    pub time: String,
    /// So viele Sicherungen je Art bleiben erhalten.
    pub keep: u32,
    /// Ins Verzeichnis `BACKUP_DIR` schreiben statt in die Datenbank (nur wenn eines eingerichtet ist).
    pub to_dir: bool,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        BackupPolicy { enabled: true, time: "02:00".into(), keep: 7, to_dir: true }
    }
}

pub async fn policy(pool: &PgPool) -> Res<BackupPolicy> {
    let v: Option<(SJson<BackupPolicy>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'backup'").fetch_optional(pool).await?;
    Ok(v.map(|x| x.0 .0).unwrap_or_default())
}

/// Prüft, ob das Sicherungsverzeichnis beschreibbar ist.
fn dir_status(app: &App) -> Value {
    let Some(d) = &app.cfg.backup_dir else { return json!({ "path": null, "ok": false, "error": null }) };
    let probe = d.join(".tickly-schreibtest");
    let r = std::fs::create_dir_all(d).and_then(|_| std::fs::write(&probe, b"ok")).and_then(|_| std::fs::remove_file(&probe));
    json!({ "path": d.display().to_string(), "ok": r.is_ok(), "error": r.err().map(|e| e.to_string()) })
}

async fn get_policy(axum::extract::State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    let p = policy(&app.pool).await?;
    Ok(Json(json!({ "policy": p, "dir": dir_status(&app), "encrypted": app.cfg.secrets.active() })))
}

async fn put_policy(axum::extract::State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(p): Json<BackupPolicy>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    if me.role != "admin" { return forbidden("Nur für Administratoren") }
    if NaiveTime::parse_from_str(&p.time, "%H:%M").is_err() { return bad("Bitte die Uhrzeit als HH:MM angeben, z. B. 02:00") }
    if !(1..=365).contains(&p.keep) { return bad("Es müssen zwischen 1 und 365 Sicherungen aufbewahrt werden") }
    if p.to_dir && app.cfg.backup_dir.is_none() { return bad("Es ist kein Sicherungsverzeichnis eingerichtet (Umgebungsvariable BACKUP_DIR)") }
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('backup', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(&p)).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Sicherungs-Einstellungen: {} täglich um {}, {} behalten, Ziel {}", if p.enabled { "automatisch" } else { "aus" }, p.time, p.keep, if p.to_dir { "Verzeichnis" } else { "Datenbank" })).await;
    Ok(Json(json!({ "ok": true })))
}

/* ---------- Ablegen und Aufräumen ---------- */
fn kind_prefix(kind: &str) -> &'static str {
    match kind { "auto" => "auto", "manual" => "manuell", _ => "vor-wiederherstellung" }
}
fn prefix_kind(name: &str) -> Option<&'static str> {
    ["auto", "manuell", "vor-wiederherstellung"].into_iter().find(|p| name.starts_with(&format!("tickly-{p}-"))).map(|p| match p { "auto" => "auto", "manuell" => "manual", _ => "pre-restore" })
}
/// Nur Dateien, die wir selbst angelegt haben (verhindert Zugriff auf beliebige Pfade).
fn valid_file_name(n: &str) -> bool {
    n.len() < 120 && n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) && prefix_kind(n).is_some() && (n.ends_with(".json.gz") || n.ends_with(".json.gz.enc"))
}

struct Entry { id: String, at: DateTime<Utc>, kind: &'static str, users: Option<i32>, size: i64 }

async fn entries(app: &App) -> Res<Vec<Entry>> {
    let rows: Vec<(i64, DateTime<Utc>, String, i32, i32)> = sqlx::query_as("SELECT id, created_at, kind, users, length(data) FROM backups ORDER BY id DESC").fetch_all(&app.pool).await?;
    let mut v: Vec<Entry> = rows.into_iter().map(|(id, at, kind, users, size)| Entry { id: format!("db:{id}"), at, kind: match kind.as_str() { "auto" => "auto", "manual" => "manual", _ => "pre-restore" }, users: Some(users), size: size as i64 }).collect();
    if let Some(d) = &app.cfg.backup_dir {
        if let Ok(rd) = std::fs::read_dir(d) {
            for f in rd.flatten() {
                let name = f.file_name().to_string_lossy().to_string();
                let (Some(kind), true, Ok(m)) = (prefix_kind(&name), valid_file_name(&name), f.metadata()) else { continue };
                v.push(Entry { id: format!("file:{name}"), at: m.modified().map(DateTime::<Utc>::from).unwrap_or_else(|_| Utc::now()), kind, users: None, size: m.len() as i64 });
            }
        }
    }
    v.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(v)
}

/// Legt eine Sicherung an (Verzeichnis oder Datenbank je nach Einstellung) und räumt alte derselben Art auf.
pub async fn save(app: &App, kind: &str) -> Result<String, String> {
    let v = export(&app.pool).await.map_err(|e| e.to_string())?;
    let users = v["data"]["users"].as_array().map_or(0, |a| a.len()) as i32;
    let data = app.cfg.secrets.seal(&gzip(&v));
    let pol = policy(&app.pool).await.map_err(|e| e.1)?;
    let keep = if kind == "pre-restore" { KEEP_PRE_RESTORE } else { pol.keep };
    let id = match (&app.cfg.backup_dir, pol.to_dir) {
        (Some(dir), true) => {
            std::fs::create_dir_all(dir).map_err(|e| format!("Sicherungsverzeichnis: {e}"))?;
            let ts = Utc::now().with_timezone(&app.cfg.tz).format("%Y-%m-%d_%H%M%S");
            let name = format!("tickly-{}-{ts}.json.gz{}", kind_prefix(kind), if app.cfg.secrets.active() { ".enc" } else { "" });
            let tmp = dir.join(format!("{name}.tmp"));
            std::fs::write(&tmp, &data).and_then(|_| std::fs::rename(&tmp, dir.join(&name))).map_err(|e| format!("Sicherungsverzeichnis: {e}"))?;
            format!("file:{name}")
        }
        _ => {
            let id: i64 = sqlx::query_scalar("INSERT INTO backups (kind, users, data) VALUES ($1, $2, $3) RETURNING id").bind(kind).bind(users).bind(data).fetch_one(&app.pool).await.map_err(|e| e.to_string())?;
            format!("db:{id}")
        }
    };
    prune(app, kind, keep).await.map_err(|e| e.1)?;
    Ok(id)
}

/// Von jeder Art (automatisch, manuell, vor Wiederherstellung) bleiben nur die neuesten `keep` – in Datenbank und Verzeichnis.
async fn prune(app: &App, kind: &str, keep: u32) -> Res<()> {
    let mut mine: Vec<Entry> = entries(app).await?.into_iter().filter(|e| e.kind == match kind { "auto" => "auto", "manual" => "manual", _ => "pre-restore" }).collect();
    mine.sort_by(|a, b| b.at.cmp(&a.at));
    for e in mine.into_iter().skip(keep as usize) {
        if let Some(id) = e.id.strip_prefix("db:").and_then(|i| i.parse::<i64>().ok()) {
            sqlx::query("DELETE FROM backups WHERE id = $1").bind(id).execute(&app.pool).await?;
        } else if let (Some(name), Some(dir)) = (e.id.strip_prefix("file:"), &app.cfg.backup_dir) {
            if valid_file_name(name) { let _ = std::fs::remove_file(dir.join(name)); }
        }
    }
    Ok(())
}

/// Führt die tägliche Sicherung aus, wenn die eingestellte Uhrzeit erreicht und heute noch nicht gesichert wurde.
/// Verpasste Zeiten (Server war aus) werden nachgeholt. Gibt die ID der neuen Sicherung zurück.
pub async fn run_due(app: &App, now: DateTime<Utc>) -> Option<String> {
    let p = policy(&app.pool).await.ok()?;
    if !p.enabled { return None }
    let t = NaiveTime::parse_from_str(&p.time, "%H:%M").ok()?;
    let local = now.with_timezone(&app.cfg.tz);
    let scheduled = app.cfg.tz.from_local_datetime(&local.date_naive().and_time(t)).earliest()?.with_timezone(&Utc);
    if now < scheduled { return None }
    let st: Option<(SJson<Value>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'backup_state'").fetch_optional(&app.pool).await.ok()?;
    let ts = |k: &str| st.as_ref().and_then(|s| s.0 .0[k].as_str()).and_then(|x| x.parse::<DateTime<Utc>>().ok());
    if ts("lastAuto").is_some_and(|l| l >= scheduled) { return None }
    // Nach einem Fehler nicht im Sekundentakt wiederholen
    if ts("lastAttempt").is_some_and(|a| now - a < chrono::Duration::minutes(10)) { return None }
    let put = |lt: Option<DateTime<Utc>>| json!({ "lastAuto": lt.or(ts("lastAuto")), "lastAttempt": now });
    let _ = sqlx::query("INSERT INTO app_settings (key, value) VALUES ('backup_state', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(put(None))).execute(&app.pool).await;
    match save(app, "auto").await {
        Ok(id) => {
            let _ = sqlx::query("UPDATE app_settings SET value = $1 WHERE key = 'backup_state'").bind(SJson(put(Some(now)))).execute(&app.pool).await;
            tracing::info!("Automatische Sicherung angelegt: {id}");
            Some(id)
        }
        Err(e) => { tracing::warn!("Automatische Sicherung fehlgeschlagen: {e}"); None }
    }
}

pub fn spawn_scheduler(app: Arc<App>) {
    tokio::spawn(async move {
        loop {
            run_due(&app, Utc::now()).await;
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    });
}

/* ---------- Abrufen ---------- */
fn file_response(name: String, body: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "application/octet-stream".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))], body).into_response()
}

fn filename(app: &App, at: DateTime<Utc>, tag: &str) -> String {
    format!("tickly-sicherung-{}{tag}.json.gz{}", at.with_timezone(&app.cfg.tz).format("%Y-%m-%d_%H%M"), if app.cfg.secrets.active() { ".enc" } else { "" })
}

async fn admin(app: &App, h: &HeaderMap) -> Res<()> {
    let u = auth(app, h, false).await?;
    if u.role != "admin" { return forbidden("Nur für Administratoren") }
    Ok(())
}

async fn download(axum::extract::State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>) -> Res<Response> {
    let me = auth(&app, &h, false).await?;
    if me.role != "admin" { return forbidden("Nur für Administratoren") }
    netsec::log(&app, "backup", true, ip, &h, Some(&me.id), Some(&me.email), "Sicherung heruntergeladen").await;
    let v = export(&app.pool).await?;
    Ok(file_response(filename(&app, Utc::now(), ""), app.cfg.secrets.seal(&gzip(&v))))
}

async fn list(axum::extract::State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    Ok(Json(Value::Array(entries(&app).await?.into_iter().map(|e| json!({ "id": e.id, "createdAt": e.at, "kind": e.kind, "users": e.users, "size": e.size, "file": e.id.starts_with("file:") })).collect())))
}

async fn backup_now(axum::extract::State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    if me.role != "admin" { return forbidden("Nur für Administratoren") }
    let id = save(&app, "manual").await.map_err(|e| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Sicherung fehlgeschlagen: {e}")))?;
    netsec::log(&app, "backup", true, ip, &h, Some(&me.id), Some(&me.email), &format!("Sicherung angelegt: {id}")).await;
    Ok(Json(json!({ "ok": true, "id": id })))
}

/// Inhalt einer gespeicherten Sicherung (Rohdaten, evtl. verschlüsselt) samt Zeitpunkt und Art.
async fn saved(app: &App, id: &str) -> Res<(DateTime<Utc>, &'static str, String, Vec<u8>)> {
    if let Some(n) = id.strip_prefix("db:").and_then(|i| i.parse::<i64>().ok()) {
        let r: Option<(DateTime<Utc>, String, Vec<u8>)> = sqlx::query_as("SELECT created_at, kind, data FROM backups WHERE id = $1").bind(n).fetch_optional(&app.pool).await?;
        let Some((at, kind, data)) = r else { return not_found("Sicherung nicht gefunden") };
        let k = match kind.as_str() { "auto" => "auto", "manual" => "manual", _ => "pre-restore" };
        let name = format!("tickly-{}-{}.json.gz{}", kind_prefix(k), at.with_timezone(&app.cfg.tz).format("%Y-%m-%d_%H%M"), if Secrets::is_sealed(&data) { ".enc" } else { "" });
        return Ok((at, k, name, data));
    }
    if let (Some(name), Some(dir)) = (id.strip_prefix("file:"), &app.cfg.backup_dir) {
        if !valid_file_name(name) { return bad("Ungültiger Dateiname") }
        let p = dir.join(name);
        let (Ok(data), Ok(m)) = (std::fs::read(&p), std::fs::metadata(&p)) else { return not_found("Sicherung nicht gefunden") };
        return Ok((m.modified().map(DateTime::<Utc>::from).unwrap_or_else(|_| Utc::now()), prefix_kind(name).unwrap_or("auto"), name.to_string(), data));
    }
    not_found("Sicherung nicht gefunden")
}

async fn download_saved(axum::extract::State(app): St, h: HeaderMap, Path(id): Path<String>) -> Res<Response> {
    admin(&app, &h).await?;
    let (_, _, name, data) = saved(&app, &id).await?;
    Ok(file_response(name, data))
}

fn session_id(h: &HeaderMap) -> Option<String> {
    let raw = h.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').filter_map(|p| p.trim().strip_prefix("tickly_sid=")).next().map(String::from)
}

/// Entschlüsselt (falls nötig), entpackt und liest die Sicherung als JSON.
fn parse(app: &App, bytes: &[u8]) -> Result<Value, String> {
    let plain = app.cfg.secrets.open(bytes)?;
    let raw = gunzip(&plain)?;
    serde_json::from_slice(&raw).map_err(|_| "Die Datei ist keine gültige Tickly-Sicherung".to_string())
}

async fn do_restore(app: &App, h: &HeaderMap, ip: ClientIp, v: Value) -> Res<Json<Value>> {
    check(&v).map_err(|m| ApiError(axum::http::StatusCode::BAD_REQUEST, m))?;
    // Zuerst den jetzigen Stand sichern, falls die Wiederherstellung ein Fehler war
    let before = save(app, "pre-restore").await.map_err(|e| ApiError(axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Sicherung des jetzigen Stands fehlgeschlagen: {e}")))?;
    let sum = restore(&app.pool, &v, session_id(h).as_deref()).await?;
    netsec::log(app, "restore", true, ip, h, None, None, &format!("Wiederherstellung: {} Benutzer, {} Tage", sum.users, sum.days)).await;
    tracing::warn!("Wiederherstellung durchgeführt: {} Benutzer, {} Tage (vorheriger Stand: Sicherung {before})", sum.users, sum.days);
    Ok(Json(json!({ "ok": true, "users": sum.users, "days": sum.days, "backupBefore": before })))
}

async fn restore_upload(axum::extract::State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, body: Bytes) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    let v = parse(&app, &body).map_err(|m| ApiError(axum::http::StatusCode::BAD_REQUEST, m))?;
    do_restore(&app, &h, ip, v).await
}

async fn restore_saved(axum::extract::State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    let (_, _, _, data) = saved(&app, &id).await?;
    let v = parse(&app, &data).map_err(|m| ApiError(axum::http::StatusCode::BAD_REQUEST, m))?;
    do_restore(&app, &h, ip, v).await
}
