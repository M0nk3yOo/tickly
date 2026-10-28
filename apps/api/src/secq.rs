//! Passwort zurücksetzen per Sicherheitsfragen und Initialpasswort durch Administratoren.
//!
//! Sicherheitsfragen sind ein schwächerer Weg als ein Link per Mail: Antworten lassen sich erraten oder recherchieren.
//! Deshalb gilt: alle drei Antworten müssen stimmen, Fehlversuche sperren das Konto für diesen Weg und die Adresse
//! (siehe `netsec`), ein Passwort-Reset ändert nie den zweiten Faktor, und der Benutzer bekommt eine Mail.
use crate::api::{auth, sid_from, St};
use crate::app::*;
use crate::db::{self, UserRow};
use crate::netsec::{self, ClientIp};
use axum::{extract::{Path, State}, http::HeaderMap, routing::{get, post}, Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};

/// Vorschläge. Man kann auch eine eigene Frage stellen.
pub const QUESTIONS: [&str; 12] = [
    "Wie hieß dein erstes Haustier?",
    "Wie hieß dein bester Freund oder deine beste Freundin in der Grundschule?",
    "Wie hieß dein erstes Auto (Marke und Modell)?",
    "Was war dein Lieblingsbuch als Kind?",
    "Wie heißt die Straße, in der du aufgewachsen bist?",
    "Wie hieß dein erster Arbeitgeber?",
    "In welchem Ort haben sich deine Eltern kennengelernt?",
    "Wie hieß dein Lieblingslehrer oder deine Lieblingslehrerin?",
    "Wie lautete dein Spitzname in der Kindheit?",
    "Welches war das erste Konzert, das du besucht hast?",
    "Wie hieß dein Kuscheltier als Kind?",
    "Welches war dein erstes Handy (Marke und Modell)?",
];

/// So viele Fragen werden gestellt, alle müssen stimmen.
pub const SLOTS: usize = 3;
/// So viele falsche Versuche je Konto und Stunde, danach ist dieser Weg für das Konto gesperrt.
const MAX_FAILS: i64 = 5;

#[derive(Deserialize, Clone)]
pub struct Qa { pub question: String, pub answer: String }

/// Antwort vergleichbar machen: Groß-/Kleinschreibung, Leerzeichen und Satzzeichen spielen keine Rolle.
pub fn normalize(a: &str) -> String {
    a.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Prüft eine Auswahl von Fragen und Antworten (3 verschiedene Fragen, brauchbare und verschiedene Antworten).
pub fn validate(list: &[Qa]) -> Result<(), String> {
    if list.len() != SLOTS { return Err(format!("Bitte genau {SLOTS} Sicherheitsfragen beantworten")) }
    let mut qs: Vec<String> = Vec::new();
    let mut ans: Vec<String> = Vec::new();
    for qa in list {
        let q = qa.question.trim();
        let custom = !QUESTIONS.contains(&q);
        if q.is_empty() || (custom && !(8..=120).contains(&q.chars().count())) { return Err("Eine eigene Frage braucht 8 bis 120 Zeichen".into()) }
        let a = normalize(&qa.answer);
        if a.chars().count() < 3 { return Err("Jede Antwort braucht mindestens 3 Buchstaben oder Ziffern".into()) }
        if qa.answer.chars().count() > 100 { return Err("Die Antworten dürfen höchstens 100 Zeichen lang sein".into()) }
        qs.push(q.to_lowercase());
        ans.push(a);
    }
    for i in 0..SLOTS {
        for j in i + 1..SLOTS {
            if qs[i] == qs[j] { return Err("Bitte drei verschiedene Fragen wählen".into()) }
            if ans[i] == ans[j] { return Err("Bitte drei verschiedene Antworten geben".into()) }
        }
    }
    Ok(())
}

/// Speichert (ersetzt) die Antworten eines Benutzers. Erwartet geprüfte Eingaben (`validate`).
pub async fn store(pool: &PgPool, uid: &str, list: &[Qa]) -> Res<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM security_answers WHERE user_id = $1").bind(uid).execute(&mut *tx).await?;
    for (i, qa) in list.iter().enumerate() {
        sqlx::query("INSERT INTO security_answers (user_id, slot, question, answer_hash) VALUES ($1, $2, $3, $4)")
            .bind(uid).bind(i as i16 + 1).bind(qa.question.trim()).bind(hash_password(&normalize(&qa.answer))).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn rows(pool: &PgPool, uid: &str) -> Res<Vec<(String, String)>> {
    Ok(sqlx::query_as("SELECT question, answer_hash FROM security_answers WHERE user_id = $1 ORDER BY slot").bind(uid).fetch_all(pool).await?)
}

pub async fn has_answers(pool: &PgPool, uid: &str) -> Res<bool> {
    Ok(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM security_answers WHERE user_id = $1").bind(uid).fetch_one(pool).await? as usize == SLOTS)
}

/// Für unbekannte Konten: immer dieselben Scheinfragen je Adresse, damit die Antwort nichts verrät.
fn fake_questions(app: &App, email: &str) -> Vec<String> {
    let mut h = Sha256::new();
    h.update(app.fake_key);
    h.update(email.trim().to_lowercase().as_bytes());
    let d = h.finalize();
    let mut idx: Vec<usize> = Vec::new();
    for b in d.iter() {
        let i = *b as usize % QUESTIONS.len();
        if !idx.contains(&i) { idx.push(i); }
        if idx.len() == SLOTS { break }
    }
    idx.into_iter().map(|i| QUESTIONS[i].to_string()).collect()
}

/// Zufälliges Initialpasswort: erfüllt die Richtlinie, ohne leicht verwechselbare Zeichen.
pub fn generate_password(min_len: usize) -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    const UP: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ";
    const LO: &[u8] = b"abcdefghijkmnpqrstuvwxyz";
    const DI: &[u8] = b"23456789";
    const SP: &[u8] = b"-_.!#%+=?";
    let pick = |set: &[u8]| set[OsRng.next_u32() as usize % set.len()] as char;
    let all: Vec<u8> = [UP, LO, DI, SP].concat();
    let len = min_len.max(20);
    let mut v: Vec<char> = vec![pick(UP), pick(LO), pick(DI), pick(SP)];
    while v.len() < len { v.push(pick(&all)); }
    for i in (1..v.len()).rev() { v.swap(i, OsRng.next_u32() as usize % (i + 1)); }
    // Nicht mit einem Sonderzeichen beginnen (lästig zu tippen)
    if let Some(k) = v.iter().position(|c| c.is_alphanumeric()) { v.swap(0, k); }
    v.into_iter().collect()
}

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/auth/security-questions", post(ask))
        .route("/auth/reset-questions", post(reset))
        .route("/me/security-questions", get(mine).put(set_mine))
        .route("/me/password/required", post(change_required))
        .route("/admin/users/{id}/password", post(admin_set_password))
}

async fn enabled(app: &App) -> Res<()> {
    if db::reg_policy(&app.pool).await?.sec_questions { Ok(()) } else { bad("Das Zurücksetzen per Sicherheitsfragen ist auf diesem Server ausgeschaltet. Bitte wende dich an einen Administrator.") }
}

#[derive(Deserialize)]
struct EmailIn { email: String }

/// Liefert die Fragen eines Kontos. Für unbekannte oder nicht eingerichtete Konten kommen Scheinfragen.
async fn ask(State(app): St, Extension(ip): Extension<ClientIp>, Json(r): Json<EmailIn>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    enabled(&app).await?;
    let real = match db::user_by_email(&app.pool, &r.email).await? {
        Some(u) if u.status == "active" => rows(&app.pool, &u.id).await?.into_iter().map(|x| x.0).collect(),
        _ => Vec::new(),
    };
    let questions = if real.len() == SLOTS { real } else { fake_questions(&app, &r.email) };
    Ok(Json(json!({ "questions": questions })))
}

fn dummy_hash() -> &'static str {
    static H: OnceLock<String> = OnceLock::new();
    H.get_or_init(|| hash_password("tickly-dummy"))
}

#[derive(Deserialize)]
struct ResetIn { email: String, answers: Vec<String>, password: String }

/// Neues Passwort festlegen, wenn alle Antworten stimmen.
async fn reset(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<ResetIn>) -> Res<Json<Value>> {
    netsec::guard(&app, ip).await?;
    enabled(&app).await?;
    let reg = db::reg_policy(&app.pool).await?;
    if let Some(m) = password_problem(&r.password, reg.min_pw) { return bad(m) }
    if r.answers.len() != SLOTS { return bad(format!("Bitte alle {SLOTS} Fragen beantworten")) }
    let email = r.email.trim().to_string();
    let recent: i64 = sqlx::query_scalar("SELECT count(*) FROM audit_log WHERE kind = 'reset_q' AND NOT ok AND lower(email) = lower($1) AND at > now() - interval '1 hour'").bind(&email).fetch_one(&app.pool).await?;
    if recent >= MAX_FAILS {
        netsec::log(&app, "reset_q", false, ip, &h, None, Some(&email), "Gesperrt: zu viele Fehlversuche für dieses Konto").await;
        return forbidden("Für dieses Konto gab es zu viele falsche Antworten. Bitte versuche es in einer Stunde erneut oder bitte einen Administrator um ein neues Passwort.");
    }
    let user = db::user_by_email(&app.pool, &email).await?.filter(|u| u.status == "active");
    let stored = match &user { Some(u) => rows(&app.pool, &u.id).await?, None => Vec::new() };
    // Alle drei Antworten immer prüfen (auch bei unbekannten Konten), damit Laufzeit und Antwort nichts verraten
    let mut all_ok = stored.len() == SLOTS;
    for i in 0..SLOTS {
        let given = normalize(&r.answers[i]);
        let ok = match stored.get(i) { Some((_, hash)) => verify_password(hash, &given), None => { verify_password(dummy_hash(), &given); false } };
        all_ok &= ok;
    }
    let (uid, mail) = (user.as_ref().map(|u| u.id.as_str()), Some(email.as_str()));
    if !all_ok {
        netsec::failure(&app, "reset_q", ip, &h, uid, mail, "Sicherheitsfragen: Antworten falsch").await;
        return bad("Die Antworten stimmen nicht.");
    }
    let u = user.expect("Konto");
    sqlx::query("UPDATE users SET password_hash = $2, must_change_pw = false WHERE id = $1").bind(&u.id).bind(hash_password(&r.password)).execute(&app.pool).await?;
    // Wer das Passwort nicht kannte, soll nirgends angemeldet bleiben
    sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(&u.id).execute(&app.pool).await?;
    netsec::log(&app, "password_reset", true, ip, &h, Some(&u.id), Some(&u.email), "Passwort per Sicherheitsfragen geändert, alle Sitzungen beendet").await;
    crate::mail::spawn(app.clone(), vec![u.email.clone()], "Dein Passwort wurde geändert".into(), "Passwort geändert".into(),
        vec!["Das Passwort deines Kontos wurde über die Sicherheitsfragen zurückgesetzt, alle Geräte wurden abgemeldet. Warst du das nicht, wende dich sofort an einen Administrator.".into()], None);
    Ok(Json(json!({ "ok": true })))
}

/// Müssen für dieses Konto noch Sicherheitsfragen festgelegt werden? (Funktion an, Konto hat ein Passwort, noch keine Fragen)
pub async fn needs_questions(app: &App, u: &UserRow) -> Res<bool> {
    Ok(db::reg_policy(&app.pool).await?.sec_questions && u.password_hash.is_some() && !has_answers(&app.pool, &u.id).await?)
}

/// Eigener Stand: Sind Fragen hinterlegt, und welche?
async fn mine(State(app): St, h: HeaderMap) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    Ok(Json(status(&app, &u).await?))
}

pub async fn status(app: &App, u: &UserRow) -> Res<Value> {
    let enabled = db::reg_policy(&app.pool).await?.sec_questions;
    let qs: Vec<String> = rows(&app.pool, &u.id).await?.into_iter().map(|x| x.0).collect();
    Ok(json!({ "enabled": enabled, "hasPassword": u.password_hash.is_some(), "set": qs.len() == SLOTS, "questions": qs }))
}

#[derive(Deserialize)]
struct SetIn { current: String, questions: Vec<Qa> }

/// Fragen festlegen oder ändern (mit dem aktuellen Passwort bestätigen).
async fn set_mine(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<SetIn>) -> Res<Json<Value>> {
    let u = auth(&app, &h, false).await?;
    netsec::guard(&app, ip).await?;
    enabled(&app).await?;
    let Some(ph) = u.password_hash.as_deref() else { return bad("Dein Konto hat kein Passwort, das man zurücksetzen könnte") };
    if !verify_password(ph, &r.current) {
        netsec::failure(&app, "login", ip, &h, Some(&u.id), Some(&u.email), "Sicherheitsfragen ändern: Passwort falsch").await;
        return unauthorized("Das Passwort ist falsch");
    }
    validate(&r.questions).map_err(|m| ApiError(axum::http::StatusCode::BAD_REQUEST, m))?;
    store(&app.pool, &u.id, &r.questions).await?;
    netsec::log(&app, "password_reset", true, ip, &h, Some(&u.id), Some(&u.email), "Sicherheitsfragen geändert").await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct RequiredIn { password: String, #[serde(default)] questions: Vec<Qa> }

/// Erzwungene Änderung nach einem Initialpasswort. Gilt nur in der gesperrten Sitzung nach der Passwort-Anmeldung.
async fn change_required(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Json(r): Json<RequiredIn>) -> Res<Json<Value>> {
    let u = auth(&app, &h, true).await?;
    netsec::guard(&app, ip).await?;
    if !u.must_change_pw { return bad("Es ist keine Passwortänderung erforderlich") }
    if let Some(m) = password_problem(&r.password, db::reg_policy(&app.pool).await?.min_pw) { return bad(m) }
    if u.password_hash.as_deref().is_some_and(|ph| verify_password(ph, &r.password)) { return bad("Das neue Passwort muss sich vom Initialpasswort unterscheiden") }
    // Fehlen noch Sicherheitsfragen (z. B. bei einem vom Administrator angelegten Konto), werden sie jetzt nachgetragen
    let need_q = needs_questions(&app, &u).await?;
    if need_q { validate(&r.questions).map_err(|m| ApiError(axum::http::StatusCode::BAD_REQUEST, m))?; }
    sqlx::query("UPDATE users SET password_hash = $2, must_change_pw = false WHERE id = $1").bind(&u.id).bind(hash_password(&r.password)).execute(&app.pool).await?;
    if need_q { store(&app.pool, &u.id, &r.questions).await?; }
    let cur = sid_from(&h).unwrap_or_default();
    sqlx::query("UPDATE sessions SET pw_change = false WHERE id = $1").bind(&cur).execute(&app.pool).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id <> $2").bind(&u.id).bind(&cur).execute(&app.pool).await?;
    netsec::log(&app, "password_reset", true, ip, &h, Some(&u.id), Some(&u.email), "Initialpasswort geändert").await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AdminIn { password: Option<String> }

/// Administrator setzt ein neues Passwort. Leer = der Server erzeugt eines. Der Benutzer muss es beim nächsten Login ändern.
async fn admin_set_password(State(app): St, h: HeaderMap, Extension(ip): Extension<ClientIp>, Path(id): Path<String>, Json(r): Json<AdminIn>) -> Res<Json<Value>> {
    let me = auth(&app, &h, false).await?;
    if me.role != "admin" { return forbidden("Nur für Administratoren") }
    if id == me.id { return bad("Für dein eigenes Konto nutze „Passwort ändern“ in den Einstellungen") }
    let Some(u) = db::user_by_id(&app.pool, &id).await? else { return not_found("Benutzer nicht gefunden") };
    if u.status == "invited" { return bad("Eingeladene Konten haben noch kein Passwort – der Benutzer registriert sich über den Einladungslink") }
    let reg = db::reg_policy(&app.pool).await?;
    let pw = match r.password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => { if let Some(m) = password_problem(p, reg.min_pw) { return bad(m) } p.to_string() }
        None => generate_password(reg.min_pw as usize + 4),
    };
    sqlx::query("UPDATE users SET password_hash = $2, must_change_pw = true WHERE id = $1").bind(&id).bind(hash_password(&pw)).execute(&app.pool).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = $1").bind(&id).execute(&app.pool).await?;
    netsec::admin_action(&app, &me, ip, &h, &format!("Initialpasswort gesetzt für {} (muss beim nächsten Login geändert werden)", u.email)).await;
    crate::mail::spawn(app.clone(), vec![u.email.clone()], "Dein Passwort wurde zurückgesetzt".into(), "Neues Passwort".into(),
        vec!["Ein Administrator hat dein Passwort zurückgesetzt und dir ein Initialpasswort gegeben. Alle Geräte wurden abgemeldet. Beim nächsten Anmelden musst du ein eigenes Passwort festlegen.".into()], None);
    // Mit Passkeys und abgeschaltetem Passwort-Login hilft ein Passwort nicht: das sieht der Administrator
    let pw_login = u.settings.pw_login || db::cred_count(&app.pool, &id).await? == 0;
    Ok(Json(json!({ "ok": true, "password": pw, "pwLogin": pw_login })))
}
