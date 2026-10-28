//! Netzwerk-Sicherheit: echte Client-IP (auch hinter Proxy), Protokoll für Administratoren und automatische IP-Sperren.
use crate::api::{auth, St};
use crate::app::*;
use axum::{
    extract::{ConnectInfo, Query, Request, State},
    http::{header, HeaderMap},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Extension, Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{types::Json as SJson, PgPool};
use std::{net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr}, sync::Arc};

/// Die Adresse, von der die Anfrage tatsächlich kommt.
#[derive(Clone, Copy, Debug)]
pub struct ClientIp(pub IpAddr, pub Option<[u8; 2]>);

impl ClientIp {
    /// Zweibuchstabiger Ländercode (nur hinter Cloudflare bekannt).
    pub fn country(&self) -> Option<String> {
        self.1.map(|c| String::from_utf8_lossy(&c).into_owned())
    }
}

/// Fehlversuche dieser Arten zählen für die automatische Sperre.
const FAIL_KINDS: [&str; 5] = ["login", "totp", "passkey", "device_pair", "setup"];

/* ---------- Client-IP ---------- */
fn in_cidr(ip: IpAddr, net: IpAddr, bits: u8) -> bool {
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(b)) => bits == 0 || (bits <= 32 && (u32::from(a) ^ u32::from(b)) >> (32 - bits as u32) == 0),
        (IpAddr::V6(a), IpAddr::V6(b)) => bits == 0 || (bits <= 128 && (u128::from(a) ^ u128::from(b)) >> (128 - bits as u32) == 0),
        _ => false,
    }
}

/// Adressen, denen wir die Weiterleitungs-Header glauben: lokal, privat (Docker, Cloudflare-Tunnel) plus `TRUSTED_PROXIES`.
fn trusted(ip: IpAddr, extra: &[(IpAddr, u8)]) -> bool {
    let ip = match ip { IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v)), v => v };
    let builtin: [(IpAddr, u8); 7] = [
        (Ipv4Addr::new(127, 0, 0, 0).into(), 8), (Ipv4Addr::new(10, 0, 0, 0).into(), 8), (Ipv4Addr::new(172, 16, 0, 0).into(), 12),
        (Ipv4Addr::new(192, 168, 0, 0).into(), 16), (Ipv4Addr::new(169, 254, 0, 0).into(), 16),
        (Ipv6Addr::LOCALHOST.into(), 128), (Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0).into(), 7),
    ];
    builtin.iter().chain(extra.iter()).any(|(n, b)| in_cidr(ip, *n, *b))
}

pub fn parse_cidrs(s: &str) -> Vec<(IpAddr, u8)> {
    s.split(',').filter_map(|p| {
        let p = p.trim();
        if p.is_empty() { return None }
        let (a, b) = p.split_once('/').map(|(a, b)| (a, b.parse::<u8>().ok())).unwrap_or((p, None));
        let ip: IpAddr = a.parse().ok()?;
        Some((ip, b.unwrap_or(if ip.is_ipv4() { 32 } else { 128 })))
    }).collect()
}

/// Ermittelt die Client-IP. Weiterleitungs-Header gelten nur, wenn die Verbindung von einem vertrauenswürdigen Proxy kommt;
/// sonst könnte jeder seine Adresse fälschen. Reihenfolge: `CF-Connecting-IP`, `X-Forwarded-For` (von rechts, ohne Proxys), `X-Real-IP`.
pub fn resolve_ip(peer: Option<IpAddr>, h: &HeaderMap, extra: &[(IpAddr, u8)]) -> IpAddr {
    resolve(peer, h, extra).0
}

/// Wie [`resolve_ip`], zusätzlich das Land aus `CF-IPCountry` (nur hinter einem vertrauenswürdigen Proxy wie Cloudflare).
pub fn resolve(peer: Option<IpAddr>, h: &HeaderMap, extra: &[(IpAddr, u8)]) -> (IpAddr, Option<[u8; 2]>) {
    let get = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).map(str::trim);
    let parse = |s: &str| s.trim().trim_matches(['[', ']']).parse::<IpAddr>().ok();
    // Kein Peer (z. B. in Tests ohne Socket) gilt als lokaler Proxy
    if peer.is_none_or(|p| trusted(p, extra)) {
        let country = get("cf-ipcountry").map(str::to_uppercase).filter(|c| c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase()) && c != "XX" && c != "T1").map(|c| [c.as_bytes()[0], c.as_bytes()[1]]);
        if let Some(ip) = get("cf-connecting-ip").and_then(parse) { return (ip, country) }
        if let Some(x) = get("x-forwarded-for") {
            let hops: Vec<IpAddr> = x.split(',').filter_map(parse).collect();
            if let Some(ip) = hops.iter().rev().find(|ip| !trusted(**ip, extra)).or(hops.first()) { return (*ip, country) }
        }
        if let Some(ip) = get("x-real-ip").and_then(parse) { return (ip, country) }
        return (peer.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)), country);
    }
    (peer.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)), None)
}

pub async fn client_ip_layer(State(app): St, mut req: Request, next: Next) -> Response {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>().map(|c| c.0.ip());
    let (ip, country) = resolve(peer, req.headers(), &app.cfg.trusted_proxies);
    req.extensions_mut().insert(ClientIp(ip, country));
    next.run(req).await
}

/// Sperrschlüssel: IPv4 einzeln, IPv6 als /64-Netz (sonst wechselt ein Angreifer einfach die Adresse).
pub fn ip_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v) => v.to_string(),
        IpAddr::V6(v) => match v.to_ipv4_mapped() {
            Some(m) => m.to_string(),
            None => { let s = v.segments(); format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3]) }
        },
    }
}

/* ---------- Einstellungen ---------- */
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPolicy {
    pub enabled: bool,
    /// So viele Fehlversuche …
    pub max_fails: u32,
    /// … innerhalb so vieler Minuten führen zur Sperre.
    pub window_min: u32,
    /// Dauer der Sperre in Minuten, 0 = bis ein Administrator entsperrt.
    pub block_min: u32,
    /// Diese Adressen werden nie gesperrt (z. B. Büro).
    pub allowlist: Vec<String>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        SecurityPolicy { enabled: true, max_fails: 5, window_min: 10, block_min: 30, allowlist: vec![] }
    }
}

pub async fn policy(pool: &PgPool) -> Res<SecurityPolicy> {
    let v: Option<(SJson<SecurityPolicy>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'security'").fetch_optional(pool).await?;
    Ok(v.map(|x| x.0 .0).unwrap_or_default())
}

fn allowed(p: &SecurityPolicy, ip: IpAddr) -> bool {
    let list = parse_cidrs(&p.allowlist.join(","));
    list.iter().any(|(n, b)| in_cidr(ip, *n, *b))
}

/* ---------- Protokoll und Sperren ---------- */
pub fn ua(h: &HeaderMap) -> String {
    h.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("").chars().take(200).collect()
}

/// Ereignis ins Protokoll schreiben. Ein Fehler hier darf die Anfrage nie scheitern lassen.
pub async fn log(app: &App, kind: &str, ok: bool, ip: ClientIp, h: &HeaderMap, user_id: Option<&str>, email: Option<&str>, detail: &str) {
    let r = sqlx::query("INSERT INTO audit_log (kind, ok, ip, ip_key, user_agent, user_id, email, detail, country) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(kind).bind(ok).bind(ip.0.to_string()).bind(ip_key(ip.0)).bind(ua(h)).bind(user_id).bind(email.map(|e| e.chars().take(120).collect::<String>())).bind(detail).bind(ip.country())
        .execute(&app.pool).await;
    if let Err(e) = r { tracing::warn!("Protokoll konnte nicht geschrieben werden: {e}"); }
}

/// Eintrag ohne Anfrage (Hintergrundaufgaben wie der Mailversand).
pub async fn log_system(app: &App, kind: &str, ok: bool, detail: &str) {
    let _ = sqlx::query("INSERT INTO audit_log (kind, ok, ip, ip_key, detail) VALUES ($1, $2, '–', '–', $3)").bind(kind).bind(ok).bind(detail).execute(&app.pool).await;
}

/// Fehlversuch protokollieren und bei zu vielen die Adresse sperren.
pub async fn failure(app: &Arc<App>, kind: &str, ip: ClientIp, h: &HeaderMap, user_id: Option<&str>, email: Option<&str>, detail: &str) {
    log(app, kind, false, ip, h, user_id, email, detail).await;
    let Ok(p) = policy(&app.pool).await else { return };
    if !p.enabled || allowed(&p, ip.0) { return }
    let key = ip_key(ip.0);
    let fails: Result<i64, _> = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE ip_key = $1 AND NOT ok AND kind = ANY($2) AND at > greatest(now() - make_interval(mins => $3), \
         coalesce((SELECT max(unblocked_at) FROM ip_blocks WHERE ip_key = $1), '-infinity'::timestamptz))")
        .bind(&key).bind(FAIL_KINDS.map(String::from).to_vec()).bind(p.window_min as i32).fetch_one(&app.pool).await;
    let Ok(n) = fails else { return };
    if n < p.max_fails as i64 { return }
    // schon gesperrt? dann nichts doppelt anlegen
    if matches!(active_block(&app.pool, &key).await, Ok(Some(_))) { return }
    let expires = (p.block_min > 0).then(|| Utc::now() + Duration::minutes(p.block_min as i64));
    let reason = format!("{n} Fehlversuche in {} Min.", p.window_min);
    let _ = sqlx::query("INSERT INTO ip_blocks (ip_key, expires_at, reason) VALUES ($1, $2, $3)").bind(&key).bind(expires).bind(&reason).execute(&app.pool).await;
    log(app, "block", true, ip, h, None, None, &format!("{key} gesperrt: {reason}{}", if p.block_min > 0 { format!(" (für {} Min.)", p.block_min) } else { " (bis zur Entsperrung)".into() })).await;
    if let Some(cfg) = crate::mail::active(&app.pool).await.filter(|c| c.notify_admin_block) {
        let _ = cfg;
        let to = crate::mail::admin_emails(&app.pool).await;
        let origin = app.cfg.public_url.as_str().trim_end_matches('/').to_string();
        crate::mail::spawn(app.clone(), to, format!("IP-Adresse gesperrt: {key}"), "IP-Adresse gesperrt".into(),
            vec![format!("Die Adresse {key}{} wurde automatisch gesperrt: {reason}.", ip.country().map(|c| format!(" ({c})")).unwrap_or_default()), "Du kannst sie in der Administration unter „Sicherheit & Sperren“ wieder entsperren.".into()],
            Some(("Sperren ansehen".into(), format!("{origin}/#/admin"))));
    }
}

async fn active_block(pool: &PgPool, key: &str) -> Result<Option<(i64, Option<DateTime<Utc>>, String)>, sqlx::Error> {
    sqlx::query_as("SELECT id, expires_at, reason FROM ip_blocks WHERE ip_key = $1 AND unblocked_at IS NULL AND (expires_at IS NULL OR expires_at > now()) ORDER BY id DESC LIMIT 1").bind(key).fetch_optional(pool).await
}

/// Vor jedem Anmelde-Versuch: Gesperrte Adressen kommen nicht weiter.
pub async fn guard(app: &App, ip: ClientIp) -> Res<()> {
    if let Some((_, until, _)) = active_block(&app.pool, &ip_key(ip.0)).await? {
        let p = policy(&app.pool).await?;
        if allowed(&p, ip.0) { return Ok(()) }
        let msg = match until {
            Some(t) => format!("Diese Adresse ist wegen zu vieler Fehlversuche gesperrt, bis {}. Bitte später erneut versuchen oder einen Administrator bitten.", t.with_timezone(&app.cfg.tz).format("%H:%M Uhr")),
            None => "Diese Adresse ist wegen zu vieler Fehlversuche gesperrt. Bitte einen Administrator bitten, sie zu entsperren.".into(),
        };
        return forbidden(msg);
    }
    Ok(())
}

/// Eintrag für Verwaltungsaktionen.
pub async fn admin_action(app: &App, who: &crate::db::UserRow, ip: ClientIp, h: &HeaderMap, detail: &str) {
    log(app, "admin", true, ip, h, Some(&who.id), Some(&who.email), detail).await;
}

/// Alte Protokolleinträge und abgelaufene Sperren aufräumen.
pub fn spawn_housekeeping(pool: PgPool, retention_days: u32) {
    tokio::spawn(async move {
        loop {
            let r = sqlx::query("DELETE FROM audit_log WHERE at < now() - make_interval(days => $1)").bind(retention_days as i32).execute(&pool).await;
            if let Err(e) = r { tracing::warn!("Protokoll aufräumen: {e}"); }
            let _ = sqlx::query("DELETE FROM ip_blocks WHERE coalesce(unblocked_at, expires_at, now()) < now() - interval '30 days'").execute(&pool).await;
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
        }
    });
}

/* ---------- Administration ---------- */
pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/admin/log", get(list_log))
        .route("/admin/security", get(security).put(put_security))
        .route("/admin/ip-blocks", post(block_manual))
        .route("/admin/ip-blocks/unblock", post(unblock))
}

async fn admin(app: &App, h: &HeaderMap) -> Res<crate::db::UserRow> {
    let u = auth(app, h, false).await?;
    if u.role != "admin" { return forbidden("Nur für Administratoren") }
    Ok(u)
}

#[derive(Deserialize)]
struct LogQ { kind: Option<String>, q: Option<String>, before: Option<i64>, limit: Option<i64> }

async fn list_log(State(app): St, h: HeaderMap, Query(q): Query<LogQ>) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    // Gruppen für die Filter der Oberfläche
    let kinds: Vec<String> = match q.kind.as_deref() {
        Some("auth") => vec!["login", "totp", "passkey", "register"],
        Some("fail") => vec![],
        Some("block") => vec!["block", "unblock"],
        Some("admin") => vec!["admin", "backup", "restore", "edit"],
        Some("device") => vec!["device_pair"],
        _ => vec![],
    }.into_iter().map(String::from).collect();
    let only_fail = q.kind.as_deref() == Some("fail");
    let like = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| format!("%{}%", s.replace('%', "")));
    let rows: Vec<(i64, DateTime<Utc>, String, bool, String, String, Option<String>, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, at, kind, ok, ip, user_agent, user_id, email, detail, country FROM audit_log \
         WHERE ($1::bigint IS NULL OR id < $1) AND (cardinality($2::text[]) = 0 OR kind = ANY($2)) AND (NOT $3 OR NOT ok) \
         AND ($4::text IS NULL OR ip ILIKE $4 OR email ILIKE $4 OR detail ILIKE $4) ORDER BY id DESC LIMIT $5")
        .bind(q.before).bind(&kinds).bind(only_fail).bind(like).bind(limit + 1).fetch_all(&app.pool).await?;
    let more = rows.len() as i64 > limit;
    let entries: Vec<Value> = rows.into_iter().take(limit as usize).map(|(id, at, kind, ok, ip, ua, uid, email, detail, country)| json!({
        "id": id, "at": at, "kind": kind, "ok": ok, "ip": ip, "country": country, "userAgent": ua, "userId": uid, "email": email, "detail": detail,
    })).collect();
    let next = if more { entries.last().and_then(|e| e["id"].as_i64()) } else { None };
    Ok(Json(json!({ "entries": entries, "next": next })))
}

async fn blocked(app: &App) -> Res<Vec<Value>> {
    let rows: Vec<(i64, String, DateTime<Utc>, Option<DateTime<Utc>>, String, bool)> = sqlx::query_as(
        "SELECT id, ip_key, blocked_at, expires_at, reason, manual FROM ip_blocks WHERE unblocked_at IS NULL AND (expires_at IS NULL OR expires_at > now()) ORDER BY blocked_at DESC").fetch_all(&app.pool).await?;
    Ok(rows.into_iter().map(|(id, ip, at, until, reason, manual)| json!({ "id": id, "ip": ip, "blockedAt": at, "until": until, "reason": reason, "manual": manual })).collect())
}

async fn security(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>) -> Res<Json<Value>> {
    admin(&app, &h).await?;
    Ok(Json(json!({ "policy": policy(&app.pool).await?, "blocked": blocked(&app).await?, "yourIp": ip.0.to_string() })))
}

async fn put_security(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(p): Json<SecurityPolicy>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    if !(3..=100).contains(&p.max_fails) || !(1..=1440).contains(&p.window_min) || p.block_min > 43_200 { return bad("Ungültige Werte: 3–100 Fehlversuche, 1–1440 Minuten, Sperrdauer höchstens 30 Tage") }
    if p.allowlist.len() > 50 || parse_cidrs(&p.allowlist.join(",")).len() != p.allowlist.iter().filter(|s| !s.trim().is_empty()).count() { return bad("Die Ausnahmeliste enthält ungültige Adressen (z. B. 203.0.113.7 oder 10.1.0.0/16)") }
    let p = SecurityPolicy { allowlist: p.allowlist.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(), ..p };
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('security', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(SJson(&p)).execute(&app.pool).await?;
    admin_action(&app, &me, ip, &h, &format!("Sperr-Einstellungen geändert: {} Fehlversuche in {} Min. → {}", p.max_fails, p.window_min, if p.block_min > 0 { format!("{} Min. gesperrt", p.block_min) } else { "Sperre bis zur Entsperrung".into() })).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct BlockReq { ip: String, minutes: Option<u32>, reason: Option<String> }

fn parse_key(s: &str) -> Option<String> {
    let s = s.trim();
    s.parse::<IpAddr>().ok().map(ip_key).or_else(|| s.strip_suffix("::/64").filter(|_| s.ends_with("::/64")).map(|_| s.to_string()))
}

async fn block_manual(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<BlockReq>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let Some(key) = parse_key(&r.ip) else { return bad("Bitte eine gültige IP-Adresse eingeben") };
    if key == ip_key(ip.0) { return bad("Das ist deine eigene Adresse – du würdest dich selbst aussperren") }
    if active_block(&app.pool, &key).await?.is_some() { return conflict("Diese Adresse ist schon gesperrt") }
    let minutes = r.minutes.unwrap_or(0).min(525_600);
    let expires = (minutes > 0).then(|| Utc::now() + Duration::minutes(minutes as i64));
    let reason = r.reason.map(|s| s.trim().chars().take(120).collect::<String>()).filter(|s| !s.is_empty()).unwrap_or_else(|| "manuell gesperrt".into());
    sqlx::query("INSERT INTO ip_blocks (ip_key, expires_at, reason, manual) VALUES ($1, $2, $3, true)").bind(&key).bind(expires).bind(&reason).execute(&app.pool).await?;
    log(&app, "block", true, ip, &h, Some(&me.id), Some(&me.email), &format!("{key} manuell gesperrt: {reason}")).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct UnblockReq { ip: String }

/// Entsperren; die bisherigen Fehlversuche zählen danach nicht mehr.
async fn unblock(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<UnblockReq>) -> Res<Json<Value>> {
    let me = admin(&app, &h).await?;
    let Some(key) = parse_key(&r.ip) else { return bad("Bitte eine gültige IP-Adresse angeben") };
    let n = sqlx::query("UPDATE ip_blocks SET unblocked_at = now(), unblocked_by = $2 WHERE ip_key = $1 AND unblocked_at IS NULL AND (expires_at IS NULL OR expires_at > now())").bind(&key).bind(&me.id).execute(&app.pool).await?.rows_affected();
    if n == 0 { return not_found("Diese Adresse ist nicht gesperrt") }
    log(&app, "unblock", true, ip, &h, Some(&me.id), Some(&me.email), &format!("{key} entsperrt")).await;
    Ok(Json(json!({ "ok": true })))
}
