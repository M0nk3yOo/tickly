//! Sitzungen und Companion-Geräte: Übersicht für Administratoren (mit Beenden) und für den Benutzer selbst.
use crate::api::{auth, sid_from, St};
use crate::app::*;
use crate::db::UserRow;
use crate::netsec::{self, ClientIp};
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/admin/sessions", get(admin_list))
        .route("/admin/sessions/{id}", delete(admin_end))
        .route("/admin/users/{id}/end-sessions", post(admin_end_all))
        .route("/admin/devices/{id}", delete(admin_revoke_device))
        .route("/sessions", get(own_list))
        .route("/sessions/{id}", delete(own_end))
}

/// „0.2.10“ → [0, 2, 10] zum Vergleichen von Versionen.
pub fn version_key(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(['.', '-', '+']).map_while(|p| p.parse::<u64>().ok()).collect()
}

async fn admin(app: &App, h: &HeaderMap) -> Res<UserRow> {
    let u = auth(app, h, false).await?;
    if u.role != "admin" { return forbidden("Nur für Administratoren") }
    Ok(u)
}

type SessionRow = (Uuid, String, String, String, Option<String>, Option<String>, String, DateTime<Utc>, DateTime<Utc>, DateTime<Utc>, bool, String);

fn session_json(r: SessionRow, current: &str, with_user: bool) -> Value {
    let (id, uid, name, email, ip, country, ua, created, seen, expires, limited, sid) = r;
    let mut v = json!({ "id": id, "ip": ip, "country": country, "userAgent": ua, "createdAt": created, "lastSeenAt": seen, "expiresAt": expires, "limited": limited, "current": sid == current });
    if with_user { v["userId"] = json!(uid); v["name"] = json!(name); v["email"] = json!(email); }
    v
}

const SESSION_SELECT: &str = "SELECT s.public_id, s.user_id, u.name, u.email, s.ip, s.country, s.user_agent, s.created_at, s.last_seen_at, s.expires_at, s.limited, s.id FROM sessions s JOIN users u ON u.id = s.user_id";

async fn admin_list(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    let cur = sid_from(&h).unwrap_or_default();
    let rows: Vec<SessionRow> = sqlx::query_as(&format!("{SESSION_SELECT} WHERE s.expires_at > now() ORDER BY s.last_seen_at DESC")).fetch_all(&app.pool).await?;
    let sessions: Vec<Value> = rows.into_iter().map(|r| session_json(r, &cur, true)).collect();
    let devs: Vec<(Uuid, String, String, String, String, String, String, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT d.id, d.user_id, u.name, u.email, d.name, d.os, d.version, d.created_at, d.last_seen_at FROM devices d JOIN users u ON u.id = d.user_id WHERE d.revoked_at IS NULL ORDER BY d.last_seen_at DESC NULLS LAST").fetch_all(&app.pool).await?;
    // Neueste bekannte Version: höchste gemeldete oder per COMPANION_LATEST_VERSION vorgegeben
    // ohne Vorgabe die neueste bei GitHub gefundene Version (siehe update.rs)
    let upstream = if app.cfg.companion_latest.is_none() { crate::update::current(&app).await.map(|i| i.latest) } else { None };
    let latest = devs.iter().map(|d| d.6.as_str()).chain(app.cfg.companion_latest.as_deref()).chain(upstream.as_deref()).filter(|v| !version_key(v).is_empty()).max_by_key(|v| version_key(v)).map(String::from);
    let now = Utc::now();
    let devices: Vec<Value> = devs.into_iter().map(|(id, uid, user_name, email, name, os, version, created, seen)| {
        let outdated = latest.as_deref().is_some_and(|l| !version_key(&version).is_empty() && version_key(&version) < version_key(l));
        json!({ "id": id, "userId": uid, "userName": user_name, "name": name, "email": email, "os": os, "version": version, "createdAt": created, "lastSeenAt": seen,
                "online": seen.is_some_and(|t| now - t < Duration::minutes(3)), "outdated": outdated })
    }).collect();
    Ok(Json(json!({ "sessions": sessions, "devices": devices, "latestVersion": latest })))
}

async fn admin_end(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let who: Option<(String, Option<String>)> = sqlx::query_as("DELETE FROM sessions s USING users u WHERE s.public_id = $1 AND u.id = s.user_id RETURNING u.email, s.ip").bind(id).fetch_optional(&app.pool).await?;
    let Some((email, from)) = who else { return not_found("Sitzung nicht gefunden") };
    netsec::admin_action(&app, &me, ip, &h, &format!("Sitzung beendet: {email} ({})", from.unwrap_or_else(|| "?".into()))).await;
    Ok(Json(json!({ "ok": true })))
}

async fn admin_end_all(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let cur = sid_from(&h).unwrap_or_default();
    let n = sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id <> $2").bind(&id).bind(&cur).execute(&app.pool).await?.rows_affected();
    netsec::admin_action(&app, &me, ip, &h, &format!("Alle Sitzungen beendet: {id} ({n})")).await;
    Ok(Json(json!({ "ok": true, "ended": n })))
}

async fn admin_revoke_device(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let r: Option<(String, String)> = sqlx::query_as("UPDATE devices d SET revoked_at = now() FROM users u WHERE d.id = $1 AND u.id = d.user_id AND d.revoked_at IS NULL RETURNING u.email, d.name").bind(id).fetch_optional(&app.pool).await?;
    let Some((email, name)) = r else { return not_found("Gerät nicht gefunden") };
    netsec::admin_action(&app, &me, ip, &h, &format!("Kopplung aufgehoben: {name} ({email})")).await;
    Ok(Json(json!({ "ok": true })))
}

async fn own_list(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let cur = sid_from(&h).unwrap_or_default();
    let rows: Vec<SessionRow> = sqlx::query_as(&format!("{SESSION_SELECT} WHERE s.user_id = $1 AND s.expires_at > now() ORDER BY s.last_seen_at DESC")).bind(&u.id).fetch_all(&app.pool).await?;
    Ok(Json(Value::Array(rows.into_iter().map(|r| session_json(r, &cur, false)).collect())))
}

async fn own_end(State(app): St, h: HeaderMap, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    let n = sqlx::query("DELETE FROM sessions WHERE public_id = $1 AND user_id = $2").bind(id).bind(&u.id).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Sitzung nicht gefunden") }
    Ok(Json(json!({ "ok": true })))
}
