//! Datenbankzugriff (PostgreSQL via SQLx): Datentypen, Abfragen, Demo-Daten und Berechnungskontext.
use crate::app::*;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, types::Json, FromRow, PgConnection, PgPool};
use std::collections::HashMap;
use tickly_core::{demo, AbsType, Absence, Ctx, DayState, Seg, Settings};
use uuid::Uuid;
use webauthn_rs::prelude::Passkey;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn connect(url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new().max_connections(10).acquire_timeout(std::time::Duration::from_secs(10)).connect(url).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}

/* ---------- Typen ---------- */
#[derive(Clone, FromRow)]
pub struct UserRow {
    pub id: String,
    pub name: String,
    pub email: String,
    /// "admin" | "user"
    pub role: String,
    /// "active" | "pending" | "blocked" | "invited"
    pub status: String,
    pub password_hash: Option<String>,
    pub webauthn_id: Uuid,
    pub settings: Json<Settings>,
    pub demo: bool,
    pub seed: String,
    pub demo_base: f64,
    pub track_start: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub totp_last_step: i64,
    pub mfa_grace_start: Option<DateTime<Utc>>,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub must_change_pw: bool,
}

#[derive(FromRow)]
pub struct CredRow {
    pub id: i64,
    pub name: String,
    pub passkey: Json<Passkey>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegPolicy {
    pub open: bool,
    pub approval: bool,
    pub domains: String,
    pub weekly: f64,
    pub passkey_prompt: bool,
    pub min_pw: u32,
    /// Jeder muss einen Passkey oder eine Authenticator-App (TOTP) einrichten.
    #[serde(default)]
    pub mfa_required: bool,
    /// So viele Tage darf man danach noch ohne arbeiten (0 = sofort einrichten).
    #[serde(default = "default_grace")]
    pub mfa_grace_days: u32,
    /// Seit wann die Pflicht gilt; setzt der Server selbst.
    #[serde(default)]
    pub mfa_required_since: Option<DateTime<Utc>>,
    /// So lange bleibt man angemeldet (Konten ohne MFA unter Pflicht: 24 Stunden).
    #[serde(default = "default_session_days")]
    pub session_days: u32,
    /// So viele Tage rückwirkend dürfen Zeiten bearbeitet bzw. ergänzt werden (0 = nicht erlaubt).
    #[serde(default = "default_edit_days")]
    pub edit_days_back: u32,
    /// Neue Konten müssen ihre E-Mail-Adresse per Link bestätigen (nur mit eingerichtetem SMTP).
    #[serde(default)]
    pub verify_email: bool,
    /// Seit wann die Pflicht gilt; vorhandene Konten sind davon ausgenommen. Setzt der Server selbst.
    #[serde(default)]
    pub verify_email_since: Option<DateTime<Utc>>,
    /// Passwort zurücksetzen per Sicherheitsfragen; bei der Registrierung werden dann drei Fragen verlangt.
    #[serde(default = "default_true")]
    pub sec_questions: bool,
    /// Läuft das Tracking so viele Stunden (Brutto), bekommt der Benutzer eine Mail und einen Hinweis (0 = aus).
    #[serde(default = "default_long_track")]
    pub long_track_hours: u32,
}

fn default_true() -> bool { true }
fn default_long_track() -> u32 { 10 }

fn default_edit_days() -> u32 { 90 }
fn default_grace() -> u32 { 7 }
fn default_session_days() -> u32 { 30 }

impl Default for RegPolicy {
    fn default() -> Self {
        RegPolicy { open: true, approval: true, domains: String::new(), weekly: 40.0, passkey_prompt: true, min_pw: MIN_PW, mfa_required: false, mfa_grace_days: 7, mfa_required_since: None, session_days: 30, edit_days_back: 90, verify_email: false, verify_email_since: None, sec_questions: true, long_track_hours: 10 }
    }
}

/// Wie weit ein Konto bei der Zwei-Faktor-Pflicht ist.
#[derive(Clone, Debug)]
pub struct MfaStatus {
    pub required: bool,
    /// Passkey oder Authenticator-App ist eingerichtet.
    pub compliant: bool,
    /// Ende der Karenzzeit (nur wenn Pflicht und noch nicht eingerichtet).
    pub deadline: Option<DateTime<Utc>>,
    /// Karenzzeit ist vorbei: bis zur Einrichtung geht nichts anderes mehr.
    pub expired: bool,
}

impl MfaStatus {
    pub fn json(&self) -> Value {
        let left = self.deadline.map(|d| ((d - Utc::now()).num_seconds() as f64 / 86400.0).ceil().max(0.0) as i64);
        json!({ "required": self.required, "compliant": self.compliant, "deadline": self.deadline, "daysLeft": left, "expired": self.expired })
    }
}

pub fn mfa_status_with(u: &UserRow, passkeys: i64, reg: &RegPolicy) -> MfaStatus {
    let compliant = u.totp_enabled || passkeys > 0;
    let required = reg.mfa_required;
    if !required || compliant {
        return MfaStatus { required, compliant, deadline: None, expired: false };
    }
    let start = [u.mfa_grace_start.unwrap_or(u.created_at), reg.mfa_required_since.unwrap_or(u.created_at)].into_iter().max().unwrap_or(u.created_at);
    let deadline = start + Duration::days(reg.mfa_grace_days as i64);
    MfaStatus { required, compliant, deadline: Some(deadline), expired: Utc::now() >= deadline }
}

pub async fn mfa_status(pool: &PgPool, u: &UserRow, reg: &RegPolicy) -> Res<MfaStatus> {
    let n = if u.totp_enabled || !reg.mfa_required { 0 } else { cred_count(pool, &u.id).await? };
    Ok(mfa_status_with(u, n, reg))
}

/// Sitzungsdauer in Sekunden: Konten ohne MFA unter Pflicht melden sich täglich neu an.
pub fn session_secs(reg: &RegPolicy, m: &MfaStatus) -> i64 {
    if m.required && !m.compliant { 24 * 3600 } else { reg.session_days.clamp(1, 365) as i64 * 86400 }
}

/// Nach erfolgreicher Einrichtung gilt wieder die normale Sitzungsdauer.
pub async fn extend_sessions(pool: &PgPool, uid: &str, secs: i64) -> Res<()> {
    sqlx::query("UPDATE sessions SET expires_at = created_at + make_interval(secs => $2) WHERE user_id = $1 AND NOT limited").bind(uid).bind(secs as f64).execute(pool).await?;
    Ok(())
}

/* ---------- Benutzer ---------- */
pub async fn user_by_id(pool: &PgPool, id: &str) -> Res<Option<UserRow>> {
    Ok(sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(id).fetch_optional(pool).await?)
}

pub async fn user_by_email(pool: &PgPool, email: &str) -> Res<Option<UserRow>> {
    Ok(sqlx::query_as("SELECT * FROM users WHERE lower(email) = lower($1)").bind(email.trim()).fetch_optional(pool).await?)
}

pub async fn all_users(pool: &PgPool) -> Res<Vec<UserRow>> {
    Ok(sqlx::query_as("SELECT * FROM users ORDER BY lower(name), id").fetch_all(pool).await?)
}

pub async fn creds(pool: &PgPool, uid: &str) -> Res<Vec<CredRow>> {
    Ok(sqlx::query_as("SELECT id, name, passkey, created_at, last_used_at FROM credentials WHERE user_id = $1 ORDER BY id").bind(uid).fetch_all(pool).await?)
}

pub async fn cred_count(pool: &PgPool, uid: &str) -> Res<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM credentials WHERE user_id = $1").bind(uid).fetch_one(pool).await?)
}

pub fn new_user_id() -> String {
    format!("u{}", &Uuid::new_v4().simple().to_string()[..10])
}

pub async fn set_settings(pool: &PgPool, uid: &str, s: &Settings) -> Res<()> {
    sqlx::query("UPDATE users SET settings = $2 WHERE id = $1").bind(uid).bind(Json(s)).execute(pool).await?;
    Ok(())
}

pub async fn touch_user(pool: &PgPool, uid: &str) {
    let _ = sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1").bind(uid).execute(pool).await;
}

/// „Jetzt aktiv“, „vor 25 Min.“ … für die Benutzerverwaltung.
pub fn last_label(u: &UserRow, now: DateTime<Utc>) -> String {
    match u.status.as_str() {
        "invited" => return "Einladung gesendet".into(),
        "pending" => return format!("Registriert {}", ago_label(u.created_at, now)),
        _ => {}
    }
    match u.last_seen_at {
        None => "noch nie angemeldet".into(),
        Some(t) if now - t < Duration::minutes(5) => "Jetzt aktiv".into(),
        Some(t) => ago_label(t, now),
    }
}

pub fn ago_label(t: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let d = now - t;
    if d < Duration::minutes(1) { "gerade eben".into() }
    else if d < Duration::hours(1) { format!("vor {} Min.", d.num_minutes()) }
    else if d < Duration::days(1) { format!("vor {} Std.", d.num_hours()) }
    else if d < Duration::days(2) { "gestern".into() }
    else if d < Duration::days(14) { format!("vor {} Tagen", d.num_days()) }
    else { format!("vor {} Wochen", d.num_days() / 7) }
}

/* ---------- Abwesenheiten ---------- */
fn kind_str(k: AbsType) -> &'static str {
    match k { AbsType::Vacation => "vacation", AbsType::Sick => "sick", AbsType::Comp => "comp" }
}
fn kind_parse(s: &str) -> AbsType {
    match s { "sick" => AbsType::Sick, "comp" => AbsType::Comp, _ => AbsType::Vacation }
}

pub async fn absences(pool: &PgPool, uid: &str) -> Res<Vec<Absence>> {
    let rows: Vec<(i64, String, NaiveDate, NaiveDate, String)> =
        sqlx::query_as("SELECT id, kind, date_from, date_to, note FROM absences WHERE user_id = $1 ORDER BY date_from, id").bind(uid).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id, k, from, to, note)| Absence { id: id as u64, kind: kind_parse(&k), from, to, note }).collect())
}

pub async fn insert_absence(pool: &PgPool, uid: &str, a: &Absence) -> Res<i64> {
    Ok(sqlx::query_scalar("INSERT INTO absences (user_id, kind, date_from, date_to, note) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(uid).bind(kind_str(a.kind)).bind(a.from).bind(a.to).bind(&a.note).fetch_one(pool).await?)
}

/* ---------- Tage ---------- */
pub async fn get_day(conn: &mut PgConnection, uid: &str, day: NaiveDate) -> Res<Option<DayState>> {
    let r: Option<(Json<DayState>,)> = sqlx::query_as("SELECT state FROM days WHERE user_id = $1 AND day = $2").bind(uid).bind(day).fetch_optional(conn).await?;
    Ok(r.map(|x| x.0 .0))
}

pub async fn put_day(conn: &mut PgConnection, uid: &str, day: NaiveDate, s: &DayState) -> Res<()> {
    sqlx::query("INSERT INTO days (user_id, day, state) VALUES ($1, $2, $3) ON CONFLICT (user_id, day) DO UPDATE SET state = $3, updated_at = now()")
        .bind(uid).bind(day).bind(Json(s)).execute(conn).await?;
    Ok(())
}

/// Serialisiert gleichzeitige Ereignisse desselben Benutzers (innerhalb einer Transaktion).
pub async fn lock_user(conn: &mut PgConnection, uid: &str) -> Res<()> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))").bind(uid).execute(conn).await?;
    Ok(())
}

/// Der jüngste Tag vor `day` (für die Übernahme über Mitternacht).
pub async fn previous_day(conn: &mut PgConnection, uid: &str, day: NaiveDate) -> Res<Option<(NaiveDate, DayState)>> {
    let r: Option<(NaiveDate, Json<DayState>)> = sqlx::query_as("SELECT day, state FROM days WHERE user_id = $1 AND day < $2 ORDER BY day DESC LIMIT 1").bind(uid).bind(day).fetch_optional(conn).await?;
    Ok(r.map(|(d, s)| (d, s.0)))
}

/// Alle erfassten Tage vor `before` als Abschnitte (offene Abschnitte enden beim letzten Lebenszeichen).
pub async fn history(pool: &PgPool, uid: &str, before: NaiveDate) -> Res<HashMap<NaiveDate, Vec<Seg>>> {
    let rows: Vec<(NaiveDate, Json<DayState>)> = sqlx::query_as("SELECT day, state FROM days WHERE user_id = $1 AND day < $2").bind(uid).bind(before).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(d, s)| (d, s.0.finalize())).filter(|(_, v)| !v.is_empty()).collect())
}

/* ---------- Berechnungskontext ---------- */
type Hist = Box<dyn Fn(NaiveDate) -> Option<Vec<Seg>> + Send + Sync>;

/// Alles, was `tickly_core` für einen Benutzer braucht, einmal aus der Datenbank geladen.
pub struct Calc {
    pub settings: Settings,
    pub absences: Vec<Absence>,
    pub today: NaiveDate,
    pub track_start: NaiveDate,
    pub now: f64,
    pub day: DayState,
    /// Heute wurde schon etwas gemeldet (sonst ist `day` nur ein leerer Platzhalter).
    pub has_day: bool,
    hist: Hist,
}

impl Calc {
    /// `day` ist der Zustand von heute; fehlt er, rechnet `Calc` mit einem leeren Tag.
    pub async fn load(app: &App, u: &UserRow, today: NaiveDate, now: f64) -> Res<Calc> {
        let absences = absences(&app.pool, &u.id).await?;
        let mut conn = app.pool.acquire().await?;
        let stored = get_day(&mut conn, &u.id, today).await?;
        let has_day = stored.is_some();
        let day = stored.unwrap_or_else(|| DayState::empty(now));
        drop(conn);
        let real = history(&app.pool, &u.id, today).await?;
        let settings = u.settings.0.clone();
        let demo_fn = u.demo.then(|| demo::history_for(u.seed.clone(), u.demo_base, app.cfg.demo_track_start, today, settings.clone(), absences.clone()));
        let hist: Hist = Box::new(move |d| real.get(&d).cloned().or_else(|| demo_fn.as_ref().and_then(|f| f(d))));
        Ok(Calc { settings, absences, today, track_start: u.track_start, now, day, has_day, hist })
    }

    pub fn with<R>(&self, f: impl FnOnce(&Ctx) -> R) -> R {
        let c = Ctx { settings: &self.settings, absences: &self.absences, today: self.today, track_start: self.track_start, now: self.now, day: &self.day, history: &*self.hist };
        f(&c)
    }
}

/* ---------- Sitzungen ---------- */
/// `pw_change`: Die Sitzung stammt aus einer Passwort-Anmeldung eines Kontos, das sein Initialpasswort ändern muss.
pub async fn create_session(pool: &PgPool, uid: &str, limited: bool, pw_change: bool, secs: i64, ip: &crate::netsec::ClientIp, ua: &str) -> Res<String> {
    let sid = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    sqlx::query("INSERT INTO sessions (id, user_id, limited, pw_change, expires_at, ip, country, user_agent) VALUES ($1, $2, $3, $8, now() + make_interval(secs => $4), $5, $6, $7)")
        .bind(&sid).bind(uid).bind(limited).bind(secs as f64).bind(ip.0.to_string()).bind(ip.country()).bind(ua).bind(pw_change).execute(pool).await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at < now()").execute(pool).await?;
    Ok(sid)
}

/* ---------- Einstellungen der Installation ---------- */
pub async fn reg_policy(pool: &PgPool) -> Res<RegPolicy> {
    let v: Option<(Json<RegPolicy>,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = 'reg'").fetch_optional(pool).await?;
    let mut p = v.map(|x| x.0 .0).unwrap_or_default();
    p.min_pw = p.min_pw.max(MIN_PW);
    Ok(p)
}

pub async fn set_reg_policy(pool: &PgPool, p: &RegPolicy) -> Res<()> {
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ('reg', $1) ON CONFLICT (key) DO UPDATE SET value = $1").bind(Json(p)).execute(pool).await?;
    Ok(())
}

/* ---------- Anlegen erster Daten ---------- */
pub fn weekly_settings(weekly: f64) -> Settings {
    let per = (weekly / 5.0 * 100.0).round() / 100.0;
    Settings { hours: [per, per, per, per, per, 0.0, 0.0], ..Settings::default() }
}

/// Einstellungen für echte Benutzer: wie `weekly_settings`, aber ohne die Demo-Überstunden als Übertrag.
pub fn fresh_settings(weekly: f64) -> Settings {
    Settings { start_bal: 0.0, ..weekly_settings(weekly) }
}

pub fn week_hours(s: &Settings) -> f64 {
    s.hours.iter().sum()
}

#[allow(clippy::too_many_arguments)]
async fn insert_user(pool: &PgPool, id: &str, name: &str, email: &str, role: &str, status: &str, hash: Option<&str>, settings: &Settings, demo: bool, seed: &str, base: f64, track_start: NaiveDate, seen: Option<DateTime<Utc>>) -> Res<()> {
    sqlx::query("INSERT INTO users (id, name, email, role, status, password_hash, webauthn_id, settings, demo, seed, demo_base, track_start, last_seen_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
        .bind(id).bind(name).bind(email).bind(role).bind(status).bind(hash).bind(Uuid::new_v4()).bind(Json(settings)).bind(demo).bind(seed).bind(base).bind(track_start).bind(seen)
        .execute(pool).await?;
    Ok(())
}

/// Legt Benutzer an, solange es noch keine gibt: im Demo-Modus die Demo-Konten, sonst den ersten Administrator.
pub async fn seed_if_empty(app: &App) -> Res<()> {
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&app.pool).await?;
    if n > 0 { return Ok(()) }
    let pool = &app.pool;
    if app.cfg.demo {
        let h = hash_password(&app.cfg.demo_password);
        let now = Utc::now();
        // (id, Name, E-Mail, Rolle, Status, Wochenstunden, zuletzt gesehen vor Minuten)
        let list: [(&str, &str, &str, &str, &str, f64, Option<i64>); 9] = [
            ("u1", "André Becker", "andre@example.de", "admin", "active", 40.0, Some(0)),
            ("u2", "Lena Hoffmann", "lena.hoffmann@example.de", "user", "active", 40.0, Some(0)),
            ("u3", "Murat Yılmaz", "murat.yilmaz@example.de", "user", "active", 32.0, Some(25)),
            ("u4", "Sophie Wagner", "sophie.wagner@example.de", "admin", "active", 40.0, Some(0)),
            ("u5", "Jonas Keller", "jonas.keller@example.de", "user", "active", 40.0, Some(60 * 26)),
            ("u6", "Marta Kowalska", "marta.kowalska@example.de", "user", "pending", 40.0, None),
            ("u7", "David Fischer", "david.fischer@example.de", "user", "pending", 40.0, None),
            ("u8", "Tim Schröder", "tim.schroeder@example.de", "user", "blocked", 30.0, Some(60 * 24 * 21)),
            ("u9", "Anna Lehmann", "anna.lehmann@example.de", "user", "invited", 40.0, None),
        ];
        for (id, name, mail, role, status, week, seen) in list {
            let main = id == "u1";
            let settings = if main { Settings::default() } else { weekly_settings(week) };
            let base = if main { demo::DEFAULT_BASE } else { demo::base_for_hours(week / 5.0) };
            let hash = (status != "invited").then_some(h.as_str());
            insert_user(pool, id, name, mail, role, status, hash, &settings, true, if main { "" } else { id }, base, app.cfg.demo_track_start, seen.map(|m| now - Duration::minutes(m))).await?;
            // Registrierungen warten „seit“ 2 Std. bzw. 20 Min.
            if id == "u6" { sqlx::query("UPDATE users SET created_at = now() - interval '2 hours' WHERE id = 'u6'").execute(pool).await?; }
            if id == "u7" { sqlx::query("UPDATE users SET created_at = now() - interval '20 minutes' WHERE id = 'u7'").execute(pool).await?; }
        }
        for a in demo::example_absences() { insert_absence(pool, "u1", &a).await?; }
        let mut conn = pool.acquire().await?;
        put_day(&mut conn, "u1", app.cfg.demo_today, &demo::example_day()).await?;
        // Die anderen Kollegen sind heute ebenfalls unterwegs (läuft / Rechner gesperrt / Urlaub)
        for (uid, start) in [("u2", 483.0), ("u4", 462.0)] {
            put_day(&mut conn, uid, app.cfg.demo_today, &DayState::boot(start, "Windows")).await?;
        }
        let mut locked = DayState::boot(471.0, "Windows");
        locked.lock(832.0).ok();
        put_day(&mut conn, "u3", app.cfg.demo_today, &locked).await?;
        let d = app.cfg.demo_today;
        insert_absence(pool, "u5", &Absence { id: 0, kind: AbsType::Vacation, from: d, to: d + Duration::days(4), note: "Herbsturlaub".into() }).await?;
        tracing::info!("Demo-Daten angelegt (Anmeldung: andre@example.de / {})", app.cfg.demo_password);
    } else if let Some((mail, name, pw)) = &app.cfg.admin {
        let today = app.local(Utc::now()).0;
        let h = hash_password(pw);
        insert_user(pool, &new_user_id(), name, mail, "admin", "active", Some(&h), &fresh_settings(40.0), false, "", demo::DEFAULT_BASE, today, None).await?;
        tracing::info!("Administrator {mail} angelegt");
    } else {
        // Ersteinrichtung im Browser. Der Code verhindert, dass ein Fremder, der die Adresse zuerst erreicht, den Administrator anlegt.
        let code = Uuid::new_v4().simple().to_string();
        *app.setup_code.lock().unwrap() = Some(code.clone());
        tracing::warn!("Noch kein Benutzer vorhanden. Öffne {} im Browser und lege den Administrator an. Einrichtungscode: {code}", app.cfg.public_url);
    }
    Ok(())
}

/// Legt den ersten Administrator an, aber nur, wenn es noch keinen Benutzer gibt (Rückgabe `None`, falls doch).
pub async fn create_first_admin(app: &App, name: &str, email: &str, password: &str) -> Res<Option<String>> {
    let mut tx = app.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(748201)").execute(&mut *tx).await?;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&mut *tx).await?;
    if n > 0 { return Ok(None) }
    let id = new_user_id();
    let today = app.local(Utc::now()).0;
    sqlx::query("INSERT INTO users (id, name, email, role, status, password_hash, webauthn_id, settings, demo, seed, demo_base, track_start, last_seen_at) VALUES ($1,$2,$3,'admin','active',$4,$5,$6,false,'',$7,$8,now())")
        .bind(&id).bind(name).bind(email).bind(hash_password(password)).bind(Uuid::new_v4()).bind(Json(fresh_settings(40.0))).bind(demo::DEFAULT_BASE).bind(today)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    *app.setup_code.lock().unwrap() = None;
    Ok(Some(id))
}

/* ---------- Hilfen für Antworten ---------- */
pub fn me_json(u: &UserRow) -> Value {
    json!({ "id": u.id, "name": u.name, "email": u.email, "role": u.role })
}
