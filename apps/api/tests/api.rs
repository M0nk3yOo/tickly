//! End-to-End-Tests der HTTP-API gegen eine echte PostgreSQL-Datenbank (inkl. Passkey-Zeremonie mit Software-Token).
//!
//! Benötigt `TEST_DATABASE_URL` (Standard: postgres://tickly@localhost:5433/postgres, siehe scripts/dev.sh).
//! Jeder Test arbeitet in einer eigenen, aus einer vorbereiteten Vorlage kopierten Datenbank.
use axum::{body::Body, http::{header, Method, Request, StatusCode}, Router};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::{Connection, PgConnection};
use std::sync::Arc;
use tickly_api::{api, app::{App, Config}, db};
use tower::ServiceExt;
use webauthn_authenticator_rs::{softtoken::SoftToken, WebauthnAuthenticator};
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse, Url};

fn admin_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://tickly@localhost:5433/postgres".into())
}

fn url_for(db: &str) -> String {
    let u = admin_url();
    let (base, _) = u.rsplit_once('/').unwrap();
    format!("{base}/{db}")
}

fn config(database_url: String, demo: bool) -> Config {
    Config {
        bind: "127.0.0.1:0".into(),
        public_url: Url::parse("http://localhost:3000").unwrap(),
        rp_id: "localhost".into(),
        web_dir: "does-not-exist".into(),
        database_url,
        tz: chrono_tz::Europe::Berlin,
        demo,
        demo_today: NaiveDate::from_ymd_opt(2026, 10, 26).unwrap(),
        demo_track_start: NaiveDate::from_ymd_opt(2026, 1, 5).unwrap(),
        demo_password: "demo-passwort".into(),
        secure_cookie: false,
        trusted_proxies: vec![],
        log_retention_days: 365,
        companion_latest: None,
        backup_dir: None,
        secrets: tickly_api::secrets::Secrets::default(),
        origin_secret: None,
        admin: if demo { None } else { Some(("chef@example.de".into(), "Chefin".into(), "Sehr-Langes-Passwort-42".into())) },
    }
}

/// Legt (einmalig) die Vorlagen-Datenbank an und kopiert sie für den Test.
async fn fresh_db(demo: bool) -> String {
    // Die Vorlage gilt nur für genau diesen Stand von Schema und Startdaten
    let sig = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        include_str!("../migrations/0001_init.sql").hash(&mut h);
        include_str!("../migrations/0002_backups.sql").hash(&mut h);
        include_str!("../migrations/0003_totp.sql").hash(&mut h);
        include_str!("../migrations/0004_audit.sql").hash(&mut h);
        include_str!("../migrations/0005_mfa_policy.sql").hash(&mut h);
        include_str!("../migrations/0009_security_questions.sql").hash(&mut h);
        include_str!("../migrations/0010_track_alerts.sql").hash(&mut h);
        include_str!("../migrations/0006_sessions_info.sql").hash(&mut h);
        include_str!("../migrations/0007_backup_kinds.sql").hash(&mut h);
        include_str!("../src/db.rs").hash(&mut h);
        h.finish() % 1_000_000_000
    };
    let tpl = format!("tickly_tpl_{}_{sig}", if demo { "demo" } else { "real" });
    let tpl = tpl.as_str();
    let mut admin = PgConnection::connect(&admin_url()).await.expect("PostgreSQL nicht erreichbar – siehe scripts/dev.sh");
    sqlx::query("SELECT pg_advisory_lock(4711)").execute(&mut admin).await.unwrap();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)").bind(tpl).fetch_one(&mut admin).await.unwrap();
    if !exists {
        sqlx::query(&format!("CREATE DATABASE {tpl}")).execute(&mut admin).await.unwrap();
        let pool = db::connect(&url_for(tpl)).await.unwrap();
        let app = App::new(config(url_for(tpl), demo), pool.clone());
        db::seed_if_empty(&app).await.map_err(|e| e.1).unwrap();
        pool.close().await;
    }
    // Alte Test-Datenbanken aufräumen (laufende werden nicht gelöscht)
    let old: Vec<String> = sqlx::query_scalar("SELECT datname FROM pg_database WHERE datname LIKE 'tickly\\_run\\_%'").fetch_all(&mut admin).await.unwrap();
    let cutoff = chrono::Utc::now().timestamp() - 600;
    for d in old.iter().filter(|d| d.split('_').nth(2).and_then(|t| t.parse::<i64>().ok()).is_some_and(|t| t < cutoff)) {
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS {d}")).execute(&mut admin).await;
    }
    let name = format!("tickly_run_{}_{}", chrono::Utc::now().timestamp(), uuid::Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE {name} TEMPLATE {tpl}")).execute(&mut admin).await.unwrap();
    sqlx::query("SELECT pg_advisory_unlock(4711)").execute(&mut admin).await.unwrap();
    name
}

struct Client { router: Router, cookie: Option<String>, app: Arc<App>, headers: Vec<(String, String)>, peer: Option<std::net::SocketAddr> }

impl Client {
    async fn new() -> Client { Client::with(true).await }
    /// Echtbetrieb: Serveruhr, kein Demo-Konto, Administrator aus der Konfiguration.
    async fn real() -> Client { Client::with(false).await }
    async fn with(demo: bool) -> Client {
        let name = fresh_db(demo).await;
        let url = url_for(&name);
        let pool = db::connect(&url).await.unwrap();
        let app = Arc::new(App::new(config(url, demo), pool));
        Client { router: api::router(app.clone()), cookie: None, app, headers: vec![], peer: None }
    }
    /// Frische Installation ohne Benutzer und ohne Administrator in der Konfiguration (Ersteinrichtung im Browser).
    async fn first_run() -> Client {
        let name = fresh_db(false).await;
        let url = url_for(&name);
        let pool = db::connect(&url).await.unwrap();
        sqlx::query("DELETE FROM users").execute(&pool).await.unwrap();
        let mut cfg = config(url, false);
        cfg.admin = None;
        let app = Arc::new(App::new(cfg, pool));
        db::seed_if_empty(&app).await.map_err(|e| e.1).unwrap();
        Client { router: api::router(app.clone()), cookie: None, app, headers: vec![], peer: None }
    }
    /// Mit Verschlüsselungsgeheimnis und/oder Sicherungsverzeichnis.
    async fn custom(secret: Option<&str>, dir: Option<std::path::PathBuf>) -> Client {
        let name = fresh_db(true).await;
        let url = url_for(&name);
        let pool = db::connect(&url).await.unwrap();
        let mut cfg = config(url, true);
        cfg.backup_dir = dir;
        if let Some(sec) = secret { cfg.secrets = tickly_api::secrets::Secrets::with(sec); }
        let app = Arc::new(App::new(cfg, pool));
        Client { router: api::router(app.clone()), cookie: None, app, headers: vec![], peer: None }
    }
    /// Neuer Server auf derselben Datenbank (Neustart).
    async fn restarted(&self) -> Client {
        let pool = db::connect(&self.app.cfg.database_url).await.unwrap();
        let app = Arc::new(App::new(config(self.app.cfg.database_url.clone(), self.app.cfg.demo), pool));
        Client { router: api::router(app.clone()), cookie: None, app, headers: vec![], peer: None }
    }
    fn fresh(&self) -> Client { Client { router: self.router.clone(), cookie: None, app: self.app.clone(), headers: vec![], peer: None } }
    /// Anfragen kommen von dieser Adresse (Weiterleitungs-Header eines lokalen Proxys, wie hinter Cloudflare).
    fn from_ip(mut self, ip: &str) -> Client { self.headers = vec![("cf-connecting-ip".into(), ip.into())]; self }
    /// Direkte Verbindung von einer öffentlichen Adresse, eventuell mit gefälschten Headern.
    fn direct(mut self, peer: &str, forged: &[(&str, &str)]) -> Client { self.peer = Some(format!("{peer}:50000").parse().unwrap()); self.headers = forged.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(); self }
    async fn call(&mut self, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.call_h(method, uri, body, None).await
    }
    async fn call_h(&mut self, method: Method, uri: &str, body: Option<Value>, bearer: Option<&str>) -> (StatusCode, Value) {
        let mut rq = Request::builder().method(method).uri(uri);
        if let Some(c) = &self.cookie { rq = rq.header(header::COOKIE, c); }
        if let Some(t) = bearer { rq = rq.header(header::AUTHORIZATION, format!("Bearer {t}")); }
        for (k, v) in &self.headers { rq = rq.header(k.as_str(), v.as_str()); }
        let mut rq = match body {
            Some(b) => rq.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
            None => rq.body(Body::empty()).unwrap(),
        };
        if let Some(p) = self.peer { rq.extensions_mut().insert(axum::extract::ConnectInfo(p)); }
        let res = self.router.clone().oneshot(rq).await.unwrap();
        let status = res.status();
        if let Some(sc) = res.headers().get(header::SET_COOKIE).and_then(|v| v.to_str().ok()) {
            let pair = sc.split(';').next().unwrap().to_string();
            self.cookie = if pair.ends_with('=') { None } else { Some(pair) };
        }
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }
    async fn raw(&mut self, method: Method, uri: &str, body: Vec<u8>) -> (StatusCode, Vec<u8>) {
        let mut rq = Request::builder().method(method).uri(uri);
        if let Some(c) = &self.cookie { rq = rq.header(header::COOKIE, c); }
        let res = self.router.clone().oneshot(rq.body(Body::from(body)).unwrap()).await.unwrap();
        let st = res.status();
        (st, res.into_body().collect().await.unwrap().to_bytes().to_vec())
    }
    async fn get(&mut self, u: &str) -> (StatusCode, Value) { self.call(Method::GET, u, None).await }
    async fn post(&mut self, u: &str, b: Value) -> (StatusCode, Value) { self.call(Method::POST, u, Some(b)).await }
    async fn put(&mut self, u: &str, b: Value) -> (StatusCode, Value) { self.call(Method::PUT, u, Some(b)).await }
    async fn login(&mut self, email: &str, pw: &str) -> StatusCode { self.post("/api/auth/login", json!({"email": email, "password": pw})).await.0 }
    async fn login_andre(&mut self) { assert_eq!(self.login("andre@example.de", "demo-passwort").await, StatusCode::OK); }
}

/// Drei Sicherheitsfragen mit Antworten für die Registrierung.
fn qs() -> Value {
    json!([
        {"question": "Wie hieß dein erstes Haustier?", "answer": "Bello"},
        {"question": "Wie heißt die Straße, in der du aufgewachsen bist?", "answer": "Lindenweg"},
        {"question": "Wie lautete dein Spitzname in der Kindheit?", "answer": "Mäuschen"}
    ])
}

fn day<'a>(snap: &'a Value, date: &str) -> &'a Value {
    snap["days"].as_array().unwrap().iter().find(|d| d["date"] == date).unwrap()
}

#[tokio::test]
async fn auth_and_snapshot_numbers() {
    let mut c = Client::new().await;
    assert_eq!(c.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(c.login("andre@example.de", "falsch").await, StatusCode::UNAUTHORIZED);
    assert_eq!(c.login("niemand@example.de", "demo-passwort").await, StatusCode::UNAUTHORIZED);
    c.login_andre().await;

    let (st, s) = c.get("/api/snapshot?now=872").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(s["me"]["role"], "admin");
    let t = day(&s, "2026-10-26");
    assert_eq!(t["counted"], 357.0); // 5:57 netto
    assert_eq!(t["gross"], 400.0);   // 6:40
    assert_eq!(t["brk"], 43.0);
    assert_eq!(t["diff"], 0.0);      // offener Tag erzeugt kein Minus
    assert_eq!(s["forecastEnd"], 995.0); // 16:35
    assert_eq!(s["vacation"], json!({"total": 30, "taken": 24, "planned": 4, "rest": 2}));
    assert_eq!(s["users"].as_array().unwrap().len(), 9);
    assert_eq!(s["holidays"].as_array().unwrap().len(), 11);
    // Urlaub, Feiertag, Krank: Soll 0, kein Balken
    for d in ["2026-10-07", "2026-04-06", "2026-03-10"] {
        assert_eq!(day(&s, d)["soll"], 0.0, "{d}");
        assert_eq!(day(&s, d)["diff"], 0.0, "{d}");
    }
    assert_eq!(day(&s, "2026-06-05")["diff"], -480.0); // Zeitausgleich
    assert_eq!(s["settings"]["startBal"], 4.5);
    let me = c.get("/api/session").await.1;
    assert_eq!(me["loggedIn"], true);
    c.post("/api/auth/logout", json!({})).await;
    assert_eq!(c.get("/api/snapshot").await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn companion_events_and_mode_switch() {
    let mut c = Client::new().await;
    c.login_andre().await;
    assert_eq!(c.post("/api/companion", json!({"kind": "unlock", "now": 872})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "lock", "now": 880})).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", json!({"kind": "lock", "now": 881})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "unlock", "now": 900})).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", json!({"kind": "app", "app": "teams", "now": 905})).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", json!({"kind": "app", "app": "  ", "now": 905})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "quit", "now": 960})).await.0, StatusCode::OK);
    // Nach dem Beenden läuft die Zeit nicht mehr weiter
    let a = c.get("/api/today?now=960").await.1;
    let b = c.get("/api/today?now=1200").await.1;
    assert_eq!(a["today"]["counted"], b["today"]["counted"]);
    assert_eq!(b["day"]["tracking"], false);
    assert_eq!(b["day"]["ended"], 960.0);
    // Tag beendet: das Minus wird jetzt verbucht
    assert!(b["today"]["diff"].as_f64().unwrap() < 0.0);
    assert_eq!(c.post("/api/companion", json!({"kind": "start", "now": 1000})).await.0, StatusCode::OK);
    let r = c.get("/api/today?now=1060").await.1;
    let segs = r["today"]["segs"].as_array().unwrap();
    assert!(segs.iter().any(|s| s["kind"] == "off"));
    // Brutto-Modus ändert die gezählte Zeit
    let mut s = c.get("/api/snapshot?now=1060").await.1["settings"].clone();
    let net = r["today"]["counted"].as_f64().unwrap();
    s["mode"] = json!("gross");
    assert_eq!(c.put("/api/settings", s.clone()).await.0, StatusCode::OK);
    let g = c.get("/api/today?now=1060").await.1["today"]["counted"].as_f64().unwrap();
    assert!(g > net);
    // ungültige Einstellungen werden abgelehnt
    s["hours"][0] = json!(30);
    assert_eq!(c.put("/api/settings", s).await.0, StatusCode::BAD_REQUEST);
    // Windows-Start simuliert einen neuen Tag
    assert_eq!(c.post("/api/companion", json!({"kind": "newday", "now": 478, "os": "mac"})).await.0, StatusCode::OK);
    let t = c.get("/api/today?now=478").await.1;
    assert_eq!(t["day"]["tracking"], true, "07:58 liegt im Standard-Zeitfenster");
    assert!(t["day"]["events"][0]["txt"].as_str().unwrap().starts_with("macOS gestartet"));
}

#[tokio::test]
async fn absences_crud_changes_targets() {
    let mut c = Client::new().await;
    c.login_andre().await;
    assert_eq!(day(&c.get("/api/snapshot").await.1, "2026-11-03")["soll"], 480.0);
    let (st, r) = c.post("/api/absences", json!({"type": "vacation", "from": "2026-11-02", "to": "2026-11-04", "note": "Test"})).await;
    assert_eq!(st, StatusCode::OK);
    let id = r["id"].as_u64().unwrap();
    let s = c.get("/api/snapshot").await.1;
    assert_eq!(day(&s, "2026-11-03")["soll"], 0.0);
    assert_eq!(s["vacation"]["planned"], 7); // 4 + 3 neue Tage
    assert_eq!(c.post("/api/absences", json!({"type": "sick", "from": "2026-11-05", "to": "2026-11-01"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(c.put(&format!("/api/absences/{id}"), json!({"type": "comp", "from": "2026-11-02", "to": "2026-11-02"})).await.0, StatusCode::OK);
    assert_eq!(day(&c.get("/api/snapshot").await.1, "2026-11-02")["info"]["kind"], "comp");
    assert_eq!(c.call(Method::DELETE, &format!("/api/absences/{id}"), None).await.0, StatusCode::OK);
    assert_eq!(day(&c.get("/api/snapshot").await.1, "2026-11-03")["soll"], 480.0);
}

#[tokio::test]
async fn registration_approval_and_admin_rules() {
    let mut admin = Client::new().await;
    admin.login_andre().await;
    let mut anon = admin.fresh();
    let reg = json!({"name": "Test Person", "email": "test.person@example.de", "password": "Ein-Langes-Passwort-42", "questions": qs()});
    assert_eq!(anon.post("/api/auth/register", json!({"name": "", "email": "x", "password": "kurz"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(anon.post("/api/auth/register", json!({"name": "T", "email": "t@example.de", "password": "kurz"})).await.0, StatusCode::BAD_REQUEST);
    let (st, r) = anon.post("/api/auth/register", reg.clone()).await;
    assert_eq!((st, r["status"].as_str()), (StatusCode::OK, Some("pending")));
    assert_eq!(anon.post("/api/auth/register", reg.clone()).await.0, StatusCode::CONFLICT);
    // wartendes Konto: kein Zugriff auf Daten, Passwort-Login abgewiesen
    assert_eq!(anon.get("/api/snapshot").await.0, StatusCode::FORBIDDEN);
    assert_eq!(anon.login("test.person@example.de", "Ein-Langes-Passwort-42").await, StatusCode::FORBIDDEN);
    // Admin sieht das Konto und gibt es frei
    let users = admin.get("/api/snapshot").await.1["users"].clone();
    let u = users.as_array().unwrap().iter().find(|u| u["email"] == "test.person@example.de").unwrap();
    assert_eq!(u["status"], "pending");
    let id = u["id"].as_str().unwrap().to_string();
    assert_eq!(admin.post(&format!("/api/admin/users/{id}/status"), json!({"status": "active"})).await.0, StatusCode::OK);
    let mut p = admin.fresh();
    assert_eq!(p.login("test.person@example.de", "Ein-Langes-Passwort-42").await, StatusCode::OK);
    let s = p.get("/api/snapshot?now=600").await.1;
    assert!(s.get("users").is_none(), "normale Benutzer erhalten keine Benutzerliste");
    assert_eq!(s["settings"]["hours"][0], 8.0);
    assert_eq!(p.put("/api/admin/registration", json!({"open": false, "approval": true, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 14})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(p.post("/api/admin/users", json!({"name": "A", "email": "a@example.de", "role": "user", "week": 40})).await.0, StatusCode::FORBIDDEN);
    // Registrierung schließen
    assert_eq!(admin.put("/api/admin/registration", json!({"open": false, "approval": true, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 14})).await.0, StatusCode::OK);
    assert_eq!(anon.fresh().post("/api/auth/register", json!({"name": "Neu", "email": "neu@example.de", "password": "Ein-Langes-Passwort-42"})).await.0, StatusCode::FORBIDDEN);
    // Einladung annehmen geht trotzdem
    assert_eq!(admin.post("/api/admin/users", json!({"name": "Eingeladen", "email": "eingeladen@example.de", "role": "user", "week": 30})).await.0, StatusCode::OK);
    let (st, r) = anon.fresh().post("/api/auth/register", json!({"name": "Eingeladen", "email": "eingeladen@example.de", "password": "Ein-Langes-Passwort-42", "questions": qs()})).await;
    assert_eq!((st, r["status"].as_str()), (StatusCode::OK, Some("active")));
    // Selbstschutz
    assert_eq!(admin.post("/api/admin/users/u1/status", json!({"status": "blocked"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(admin.call(Method::DELETE, "/api/admin/users/u1", None).await.0, StatusCode::BAD_REQUEST);
    // Domain-Einschränkung
    admin.put("/api/admin/registration", json!({"open": true, "approval": false, "domains": "@firma.de", "weekly": 40, "passkeyPrompt": true, "minPw": 14})).await;
    assert_eq!(anon.fresh().post("/api/auth/register", json!({"name": "X", "email": "x@example.de", "password": "Ein-Langes-Passwort-42"})).await.0, StatusCode::BAD_REQUEST);
    // Gesperrte Benutzer verlieren ihre Sitzung
    assert_eq!(admin.post(&format!("/api/admin/users/{id}/status"), json!({"status": "blocked"})).await.0, StatusCode::OK);
    assert_eq!(p.get("/api/snapshot").await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn passkey_registration_and_login_roundtrip() {
    let mut c = Client::new().await;
    c.login_andre().await;
    let origin = Url::parse("http://localhost:3000").unwrap();
    let (token, _ca) = SoftToken::new(true).unwrap();
    let mut authenticator = WebauthnAuthenticator::new(token);

    // Registrierung
    let (st, start) = c.post("/api/passkey/register/start", json!({})).await;
    assert_eq!(st, StatusCode::OK);
    let options: CreationChallengeResponse = serde_json::from_value(start["options"].clone()).unwrap();
    let cred = authenticator.do_registration(origin.clone(), options).unwrap();
    assert_eq!(c.post("/api/passkey/register/finish", json!({"id": "gibt-es-nicht", "credential": cred})).await.0, StatusCode::BAD_REQUEST);
    let (st, r) = c.post("/api/passkey/register/finish", json!({"id": start["id"], "credential": cred, "name": "Test-Gerät"})).await;
    assert_eq!((st, r["ok"].as_bool()), (StatusCode::OK, Some(true)));
    let s = c.get("/api/snapshot").await.1;
    assert_eq!(s["passkeys"][0]["name"], "Test-Gerät");

    // Anmeldung per Passkey in einer neuen Sitzung
    let mut anon = c.fresh();
    assert_eq!(anon.post("/api/passkey/login/start", json!({"email": "lena.hoffmann@example.de"})).await.0, StatusCode::NOT_FOUND);
    let (st, start) = anon.post("/api/passkey/login/start", json!({"email": "ANDRE@example.de"})).await;
    assert_eq!(st, StatusCode::OK);
    let options: RequestChallengeResponse = serde_json::from_value(start["options"].clone()).unwrap();
    let assertion = authenticator.do_authentication(origin.clone(), options).unwrap();
    let (st, r) = anon.post("/api/passkey/login/finish", json!({"id": start["id"], "credential": assertion})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(anon.get("/api/snapshot").await.0, StatusCode::OK);
    // Challenge ist nur einmal verwendbar
    assert_eq!(c.fresh().post("/api/passkey/login/finish", json!({"id": start["id"], "credential": assertion})).await.0, StatusCode::BAD_REQUEST);

    // Passwort-Anmeldung abschalten ist jetzt erlaubt, und wirkt
    let mut settings = s["settings"].clone();
    settings["pwLogin"] = json!(false);
    assert_eq!(c.put("/api/settings", settings).await.0, StatusCode::OK);
    assert_eq!(c.fresh().login("andre@example.de", "demo-passwort").await, StatusCode::FORBIDDEN);

    // Umbenennen und Löschen; ohne Passkeys ist das Passwort wieder erlaubt
    let id = s["passkeys"][0]["id"].as_u64().unwrap();
    assert_eq!(c.put(&format!("/api/passkeys/{id}"), json!({"name": "Neu"})).await.0, StatusCode::OK);
    assert_eq!(c.get("/api/snapshot").await.1["passkeys"][0]["name"], "Neu");
    assert_eq!(c.call(Method::DELETE, &format!("/api/passkeys/{id}"), None).await.0, StatusCode::OK);
    assert_eq!(c.fresh().login("andre@example.de", "demo-passwort").await, StatusCode::OK);
}

#[tokio::test]
async fn pending_user_can_enroll_passkey_but_not_login_before_approval() {
    let mut anon = Client::new().await;
    let (st, r) = anon.post("/api/auth/register", json!({"name": "Nur Passkey", "email": "nur.passkey@example.de"})).await;
    assert_eq!((st, r["status"].as_str()), (StatusCode::OK, Some("pending")));
    let origin = Url::parse("http://localhost:3000").unwrap();
    let mut a = WebauthnAuthenticator::new(SoftToken::new(true).unwrap().0);
    let (st, start) = anon.post("/api/passkey/register/start", json!({})).await;
    assert_eq!(st, StatusCode::OK);
    let cred = a.do_registration(origin.clone(), serde_json::from_value(start["options"].clone()).unwrap()).unwrap();
    assert_eq!(anon.post("/api/passkey/register/finish", json!({"id": start["id"], "credential": cred})).await.0, StatusCode::OK);
    // Anmeldung vor Freigabe wird abgewiesen
    assert_eq!(anon.fresh().post("/api/passkey/login/start", json!({"email": "nur.passkey@example.de"})).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn state_survives_restart() {
    let mut c = Client::new().await;
    c.login_andre().await;
    c.post("/api/absences", json!({"type": "sick", "from": "2026-11-09", "to": "2026-11-09", "note": "bleibt"})).await;
    c.post("/api/companion", json!({"kind": "lock", "now": 890})).await;
    // Server neu starten: Sitzung, Abwesenheit und Tag liegen in der Datenbank
    let mut c2 = c.restarted().await;
    c2.cookie = c.cookie.clone();
    let s = c2.get("/api/snapshot?now=900").await.1;
    assert!(s["absences"].as_array().unwrap().iter().any(|a| a["note"] == "bleibt"));
    assert_eq!(s["day"]["segs"].as_array().unwrap().last().unwrap()["kind"], "locked");
}

#[tokio::test]
async fn pause_restart_resume_and_auto_start_window() {
    let mut c = Client::new().await;
    c.login_andre().await;
    let ev = |k: &str, now: u32| json!({"kind": k, "now": now});
    // Tracking im Companion stoppen und fortsetzen
    assert_eq!(c.post("/api/companion", ev("pause", 870)).await.0, StatusCode::OK);
    let t = c.get("/api/today?now=880").await.1;
    assert_eq!(t["day"]["stop"], "pause");
    assert_eq!(c.post("/api/companion", ev("pause", 871)).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", ev("start", 900)).await.0, StatusCode::OK);
    // Neustart des Rechners: lief das Tracking, geht es nach dem Start von selbst weiter
    assert_eq!(c.post("/api/companion", ev("shutdown", 960)).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", ev("boot", 975)).await.0, StatusCode::OK);
    let t = c.get("/api/today?now=980").await.1;
    assert_eq!(t["day"]["tracking"], true);
    assert_eq!(t["day"]["segs"].as_array().unwrap().iter().filter(|s| s["kind"] == "off").count(), 2);
    // Beenden + Start außerhalb des Fensters: Rückfrage, die sich beantworten lässt
    assert_eq!(c.post("/api/companion", ev("quit", 1000)).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", ev("boot", 1100)).await.0, StatusCode::OK);
    assert_eq!(c.get("/api/today?now=1100").await.1["day"]["stop"], "ask");
    assert_eq!(c.post("/api/companion", json!({"kind": "answer", "answer": "no", "now": 1105})).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", json!({"kind": "answer", "answer": "work", "now": 1106})).await.0, StatusCode::CONFLICT);
    // Ein Fenster, das schon vor dem „Nein“ begann, überstimmt die Antwort nicht
    let mut s = c.get("/api/snapshot?now=1105").await.1["settings"].clone();
    s["autoTrack"]["windows"] = json!([{"days": [true, true, true, true, true, true, true], "from": 1080, "to": 1200}]);
    assert_eq!(c.put("/api/settings", s.clone()).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", ev("tick", 1110)).await.0, StatusCode::OK);
    assert_eq!(c.get("/api/today?now=1110").await.1["day"]["tracking"], false);
    // Beginnt später ein neues Fenster (18:30), startet das Tracking beim nächsten Takt
    s["autoTrack"]["windows"] = json!([{"days": [true, true, true, true, true, true, true], "from": 1110, "to": 1200}]);
    assert_eq!(c.put("/api/settings", s.clone()).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", ev("tick", 1115)).await.0, StatusCode::OK);
    let t = c.get("/api/today?now=1115").await.1;
    assert_eq!(t["day"]["tracking"], true);
    assert!(t["day"]["events"].as_array().unwrap().last().unwrap()["txt"].as_str().unwrap().contains("Zeitfenster ab 18:30"));
    // ungültige Fenster werden abgelehnt
    s["autoTrack"]["windows"] = json!([{"days": [true, false, false, false, false, false, false], "from": 600, "to": 500}]);
    assert_eq!(c.put("/api/settings", s).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn password_policy_is_enforced_on_registration() {
    let mut anon = Client::new().await;
    let reg = |pw: &str| json!({"name": "Pw Test", "email": "pw.test@example.de", "password": pw, "questions": qs()});
    for bad in ["Kurz-1a", "ein-langes-passwort-42!", "EIN-LANGES-PASSWORT-42!", "Ein-Langes-Passwort-xx", "EinLangesPasswort42", "Ein Langes Passwort 42"] {
        let (st, r) = anon.post("/api/auth/register", reg(bad)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{bad} muss abgelehnt werden: {r}");
    }
    let (st, r) = anon.post("/api/auth/register", reg("ein-langes-passwort")).await;
    assert!(st == StatusCode::BAD_REQUEST && r["error"].as_str().unwrap().contains("Großbuchstaben"));
    assert_eq!(anon.post("/api/auth/register", reg("Ein-Langes-Passwort-42")).await.0, StatusCode::OK);
    // Mindestlänge ist nach unten gesperrt
    let mut admin = anon.fresh();
    admin.login_andre().await;
    let low = json!({"open": true, "approval": true, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 10});
    assert_eq!(admin.put("/api/admin/registration", low).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn device_pairing_and_event_flow_in_real_mode() {
    let mut c = Client::real().await;
    // Erster Administrator kommt aus der Konfiguration; es gibt keine Demo-Konten
    assert_eq!(c.login("andre@example.de", "demo-passwort").await, StatusCode::UNAUTHORIZED);
    assert_eq!(c.login("chef@example.de", "Sehr-Langes-Passwort-42").await, StatusCode::OK);
    let s = c.get("/api/snapshot?now=1").await.1; // die Client-Uhrzeit wird im Echtbetrieb ignoriert
    assert_eq!((s["demo"].clone(), s["users"].as_array().unwrap().len()), (json!(false), 1));
    assert!(s["day"]["segs"].as_array().unwrap().is_empty(), "ohne Companion wurde nichts erfasst");
    assert_eq!(s["settings"]["startBal"], 0.0, "echte Benutzer starten ohne Demo-Übertrag");
    // Systemereignisse meldet nur das gekoppelte Gerät
    assert_eq!(c.post("/api/companion", json!({"kind": "lock"})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(c.post("/api/companion", json!({"kind": "newday"})).await.0, StatusCode::FORBIDDEN);
    // Außerhalb von Zeitfenstern nachfragen, nicht automatisch starten
    let mut set = s["settings"].clone();
    set["autoTrack"]["enabled"] = json!(false);
    set["autoTrack"]["ask"] = json!(true);
    assert_eq!(c.put("/api/settings", set).await.0, StatusCode::OK);

    // Kopplung
    let (st, code) = c.post("/api/devices/pair-code", json!({})).await;
    assert_eq!(st, StatusCode::OK);
    let code = code["code"].as_str().unwrap().to_string();
    let mut dev = c.fresh();
    dev.cookie = None;
    assert_eq!(dev.post("/api/device/pair", json!({"code": "AAAA-BBBB", "name": "x", "os": "macos"})).await.0, StatusCode::BAD_REQUEST);
    let (st, r) = dev.post("/api/device/pair", json!({"code": code.to_lowercase(), "name": "Test-Mac", "os": "macos", "version": "1.0.0"})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let token = r["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("tk_"));
    assert_eq!(dev.post("/api/device/pair", json!({"code": code, "name": "Zweit", "os": "windows"})).await.0, StatusCode::BAD_REQUEST, "Code gilt nur einmal");
    assert_eq!(dev.call_h(Method::GET, "/api/device/state", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(dev.call_h(Method::GET, "/api/device/state", None, Some("tk_falsch")).await.0, StatusCode::UNAUTHORIZED);

    // Ereignisse: boot → Rückfrage → arbeiten → App → sperren → entsperren → stoppen; seq macht Wiederholungen harmlos
    let at = chrono::Utc::now().to_rfc3339();
    let ev = |seq: i64, kind: &str, extra: Value| { let mut v = json!({"seq": seq, "kind": kind, "at": at, "os": "macos"}); v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone()); v };
    let batch = json!({"events": [ev(1, "boot", json!({})), ev(2, "answer", json!({"answer": "work"})), ev(3, "app", json!({"app": "Google Chrome"})), ev(4, "unlock", json!({})), ev(5, "lock", json!({})), ev(6, "unlock", json!({}))]});
    let (st, r) = dev.call_h(Method::POST, "/api/device/events", Some(batch.clone()), Some(&token)).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let ok: Vec<bool> = r["results"].as_array().unwrap().iter().map(|x| x["ok"].as_bool().unwrap()).collect();
    assert_eq!(ok, vec![true, true, true, false, true, true], "entsperren ohne Sperre passt nicht zum Zustand");
    assert_eq!(r["state"]["track"], "running");
    assert_eq!(r["lastSeq"], 6);
    // Dieselbe Sendung noch einmal: nichts passiert doppelt
    let (_, r2) = dev.call_h(Method::POST, "/api/device/events", Some(batch), Some(&token)).await;
    assert!(r2["results"].as_array().unwrap().iter().all(|x| x["duplicate"] == true));
    let (_, st) = dev.call_h(Method::POST, "/api/device/events", Some(json!({"events": [ev(7, "pause", json!({}))]})), Some(&token)).await;
    assert_eq!(st["state"]["track"], "paused");
    assert_eq!(st["state"]["policy"]["autoTrack"]["ask"], true);
    // Zeitpunkte in der Zukunft sind ungültig
    let future = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    let (_, r) = dev.call_h(Method::POST, "/api/device/events", Some(json!({"events": [{"seq": 8, "kind": "start", "at": future}]})), Some(&token)).await;
    assert_eq!(r["results"][0]["ok"], false);

    // Web zeigt Tag und Gerät; App wurde mit ihrem Namen erfasst
    let s = c.get("/api/snapshot").await.1;
    assert_eq!(s["devices"][0]["name"], "Test-Mac");
    assert_eq!(s["devices"][0]["online"], true);
    assert!(s["day"]["segs"].as_array().unwrap().iter().any(|g| g["app"] == "Google Chrome"));
    assert!(s["apps"].as_array().unwrap().iter().any(|a| a["n"] == "Google Chrome" && a["g"] == "web"));
    // Aufheben der Kopplung sperrt das Gerät sofort
    let id = s["devices"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(c.call(Method::DELETE, &format!("/api/devices/{id}"), None).await.0, StatusCode::OK);
    assert_eq!(dev.call_h(Method::GET, "/api/device/state", None, Some(&token)).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn device_pairing_requires_login_and_is_rate_limited() {
    let mut anon = Client::real().await;
    assert_eq!(anon.post("/api/devices/pair-code", json!({})).await.0, StatusCode::UNAUTHORIZED);
    // Mehrere Fehlversuche von einer Adresse führen zur IP-Sperre
    for _ in 0..5 {
        assert_eq!(anon.post("/api/device/pair", json!({"code": "NOPE-NOPE", "name": "x", "os": "windows"})).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(anon.post("/api/device/pair", json!({"code": "NOPE-NOPE", "name": "x", "os": "windows"})).await.0, StatusCode::FORBIDDEN);
    // Und auch verteilt über viele Adressen ist das Raten von Codes insgesamt gebremst
    for i in 0..10 {
        let mut other = anon.fresh().from_ip(&format!("203.0.113.{}", i + 50));
        assert_eq!(other.post("/api/device/pair", json!({"code": "NOPE-NOPE", "name": "x", "os": "windows"})).await.0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(anon.fresh().from_ip("203.0.113.99").post("/api/device/pair", json!({"code": "NOPE-NOPE", "name": "x", "os": "windows"})).await.0, StatusCode::TOO_MANY_REQUESTS);
}

/// Gescripteter „Rechner“ für den Companion-Kern.
struct FakePc(Arc<std::sync::Mutex<(Option<String>, bool)>>);
impl tickly_platform::Platform for FakePc {
    fn os(&self) -> &'static str { "macos" }
    fn foreground_app(&mut self) -> Option<String> { self.0.lock().unwrap().0.clone() }
    fn is_locked(&mut self) -> bool { self.0.lock().unwrap().1 }
}

/// Companion-Kern und echter Server über HTTP: Kopplung, Rückfrage, App-Wechsel, Sperre, Pause und Beenden.
#[tokio::test]
async fn agent_talks_to_real_server_end_to_end() {
    let mut web = Client::real().await;
    assert_eq!(web.login("chef@example.de", "Sehr-Langes-Passwort-42").await, StatusCode::OK);
    let mut set = web.get("/api/snapshot").await.1["settings"].clone();
    set["autoTrack"]["enabled"] = json!(false);
    set["autoTrack"]["ask"] = json!(true);
    assert_eq!(web.put("/api/settings", set).await.0, StatusCode::OK);
    let code = web.post("/api/devices/pair-code", json!({})).await.1["code"].as_str().unwrap().to_string();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = api::router(web.app.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });

    let dir = tempfile::tempdir().unwrap();
    let outbox_file = dir.path().join("outbox.json");
    let pc = Arc::new(std::sync::Mutex::new((Some("Code".to_string()), false)));
    let (pc2, base2) = (pc.clone(), base.clone());
    let view = tokio::task::spawn_blocking(move || {
        use tickly_agent::{Action, Engine, HttpTransport, Outbox};
        let creds = tickly_agent::pair(&base2, &code, "Test-Mac", "macos", "0.1.0").expect("Kopplung");
        assert_eq!(creds.user_name, "Chefin");
        let mut e = Engine::new(Box::new(FakePc(pc2.clone())), HttpTransport::new(&creds), Outbox::open(outbox_file.clone()));
        let now = chrono::Utc::now;
        // Manueller Start außerhalb der Zeitfenster: Rückfrage wird gleich mit „ja“ beantwortet
        e.start(now(), true);
        assert_eq!(e.view().state.track, "running", "{:?}", e.view());
        e.tick(now());
        pc2.lock().unwrap().0 = Some("Google Chrome".into());
        e.tick(now());
        pc2.lock().unwrap().1 = true;
        e.tick(now());
        assert_eq!(e.view().state.track, "locked");
        pc2.lock().unwrap().1 = false;
        e.tick(now());
        e.action(now(), Action::Pause);
        assert_eq!(e.view().state.track, "paused");
        e.action(now(), Action::Resume);
        assert_eq!(e.view().state.track, "running");
        e.finish(now(), "quit");
        assert_eq!(e.view().pending, 0, "alles angekommen");
        // Die Outbox-Datei bleibt leer, aber die Nummern laufen weiter
        assert!(Outbox::open(outbox_file).next_seq() > 8);
        e.view()
    }).await.unwrap();
    assert_eq!(view.state.track, "stopped");

    let s = web.get("/api/snapshot").await.1;
    let segs = s["day"]["segs"].as_array().unwrap();
    let apps: Vec<&str> = segs.iter().filter_map(|g| g["app"].as_str()).collect();
    assert!(apps.contains(&"Code") && apps.contains(&"Google Chrome"), "{apps:?}");
    assert!(segs.iter().any(|g| g["kind"] == "locked") && segs.iter().any(|g| g["kind"] == "off"), "Sperre und Pause stehen im Verlauf");
    assert_eq!(s["day"]["stop"], "quit");
    let texts: Vec<&str> = s["day"]["events"].as_array().unwrap().iter().map(|e| e["txt"].as_str().unwrap()).collect();
    assert!(texts.iter().any(|t| t.contains("Rückfrage")) && texts.iter().any(|t| t.contains("Companion beendet")), "{texts:?}");
    assert_eq!(s["devices"][0]["name"], "Test-Mac");
}

#[tokio::test]
async fn backup_and_restore_roundtrip() {
    let mut c = Client::new().await;
    c.login_andre().await;
    // Ein gekoppeltes Gerät soll eine Wiederherstellung überstehen
    let code = c.post("/api/devices/pair-code", json!({})).await.1["code"].as_str().unwrap().to_string();
    let token = c.fresh().post("/api/device/pair", json!({"code": code, "name": "Backup-Mac", "os": "macos"})).await.1["token"].as_str().unwrap().to_string();
    let before = c.get("/api/snapshot?now=872").await.1;

    // Sicherung herunterladen (gzip)
    let (st, file) = c.raw(Method::GET, "/api/admin/backup", vec![]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(&file[..2], &[0x1f, 0x8b]);

    // Danach alles Mögliche verändern
    c.post("/api/absences", json!({"type": "sick", "from": "2026-11-09", "to": "2026-11-09", "note": "weg damit"})).await;
    let mut set = before["settings"].clone();
    set["mode"] = json!("gross");
    c.put("/api/settings", set).await;
    assert_eq!(c.call(Method::DELETE, "/api/admin/users/u9", None).await.0, StatusCode::OK);
    let id = before["devices"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(c.call(Method::DELETE, &format!("/api/devices/{id}"), None).await.0, StatusCode::OK);
    let changed = c.get("/api/snapshot?now=872").await.1;
    assert_eq!(changed["users"].as_array().unwrap().len(), 8);

    // Kaputte oder fremde Dateien ändern nichts
    assert_eq!(c.raw(Method::POST, "/api/admin/restore", b"kein json".to_vec()).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(c.raw(Method::POST, "/api/admin/restore", br#"{"format":"anderes","version":1}"#.to_vec()).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(c.get("/api/snapshot?now=872").await.1["users"].as_array().unwrap().len(), 8);

    // Wiederherstellen: die eigene Sitzung bleibt gültig
    let (st, r) = c.raw(Method::POST, "/api/admin/restore", file.clone()).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&r));
    let after = c.get("/api/snapshot?now=872").await;
    assert_eq!(after.0, StatusCode::OK, "Sitzung des Administrators bleibt erhalten");
    let after = after.1;
    assert_eq!(after["users"].as_array().unwrap().len(), 9);
    assert!(!after["absences"].as_array().unwrap().iter().any(|a| a["note"] == "weg damit"));
    assert_eq!(after["settings"]["mode"], "net");
    assert_eq!(after["absences"], before["absences"]);
    for d in ["2026-10-26", "2026-03-10", "2026-06-05"] {
        assert_eq!(day(&after, d)["counted"], day(&before, d)["counted"], "{d}");
    }
    assert_eq!(c.fresh().login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK, "Passwort-Hashes sind zurück");
    let mut dev = c.fresh();
    assert_eq!(dev.call_h(Method::GET, "/api/device/state", None, Some(&token)).await.0, StatusCode::OK, "gekoppeltes Gerät funktioniert wieder");
    // Nummernvergabe geht nach der Wiederherstellung korrekt weiter
    let n = c.post("/api/absences", json!({"type": "comp", "from": "2026-12-01", "to": "2026-12-01"})).await;
    assert_eq!(n.0, StatusCode::OK, "{}", n.1);

    // Vor der Wiederherstellung wurde der geänderte Stand gesichert; er lässt sich zurückholen
    let list = c.get("/api/admin/backups").await.1;
    let pre = list.as_array().unwrap().iter().find(|b| b["kind"] == "pre-restore").expect("Sicherung vor Wiederherstellung");
    let pid = pre["id"].as_str().unwrap().to_string();
    assert_eq!(c.raw(Method::GET, &format!("/api/admin/backups/{pid}"), vec![]).await.0, StatusCode::OK);
    assert_eq!(c.post(&format!("/api/admin/backups/{pid}/restore"), json!({})).await.0, StatusCode::OK);
    assert_eq!(c.get("/api/snapshot?now=872").await.1["users"].as_array().unwrap().len(), 8);
}

#[tokio::test]
async fn backup_is_admin_only_and_never_locks_everyone_out() {
    let mut c = Client::new().await;
    let mut lena = c.fresh();
    assert_eq!(lena.login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK);
    assert_eq!(lena.raw(Method::GET, "/api/admin/backup", vec![]).await.0, StatusCode::FORBIDDEN);
    assert_eq!(lena.raw(Method::POST, "/api/admin/restore", vec![1, 2, 3]).await.0, StatusCode::FORBIDDEN);
    assert_eq!(c.raw(Method::GET, "/api/admin/backup", vec![]).await.0, StatusCode::UNAUTHORIZED);
    // Eine Sicherung ohne aktiven Administrator wird abgelehnt
    c.login_andre().await;
    let (_, file) = c.raw(Method::GET, "/api/admin/backup", vec![]).await;
    use std::io::Read;
    let mut raw = String::new();
    flate2::read::GzDecoder::new(&file[..]).read_to_string(&mut raw).unwrap();
    let mut v: Value = serde_json::from_str(&raw).unwrap();
    for u in v["data"]["users"].as_array_mut().unwrap() { u["role"] = json!("user"); }
    let (st, r) = c.raw(Method::POST, "/api/admin/restore", serde_json::to_vec(&v).unwrap()).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{}", String::from_utf8_lossy(&r));
    assert_eq!(c.get("/api/snapshot?now=872").await.0, StatusCode::OK);
}

/// Transport, den der Test „abziehen“ kann (Zug, Tunnel, kein WLAN beim Kunden).
struct Flaky(tickly_agent::HttpTransport, Arc<std::sync::atomic::AtomicBool>);
impl tickly_agent::Transport for Flaky {
    fn send(&mut self, b: &[tickly_agent::QueuedEvent], t: std::time::Duration) -> Result<tickly_agent::Reply, tickly_agent::TransportError> {
        if self.1.load(std::sync::atomic::Ordering::SeqCst) { return Err(tickly_agent::TransportError::Offline("kein Netz".into())) }
        self.0.send(b, t)
    }
    fn state(&mut self, t: std::time::Duration) -> Result<tickly_agent::ServerState, tickly_agent::TransportError> {
        if self.1.load(std::sync::atomic::Ordering::SeqCst) { return Err(tickly_agent::TransportError::Offline("kein Netz".into())) }
        self.0.state(t)
    }
}

/// Ohne Verbindung weiterarbeiten: Der Companion rechnet lokal mit, später stimmt der Server mit ihm überein.
#[tokio::test]
async fn agent_keeps_tracking_offline_and_syncs_after_reconnect() {
    let mut web = Client::real().await;
    assert_eq!(web.login("chef@example.de", "Sehr-Langes-Passwort-42").await, StatusCode::OK);
    let mut set = web.get("/api/snapshot").await.1["settings"].clone();
    set["autoTrack"]["enabled"] = json!(false);
    set["autoTrack"]["ask"] = json!(true);
    assert_eq!(web.put("/api/settings", set).await.0, StatusCode::OK);
    let code = web.post("/api/devices/pair-code", json!({})).await.1["code"].as_str().unwrap().to_string();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = api::router(web.app.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap(); });

    let dir = tempfile::tempdir().unwrap();
    let (store, outbox) = (dir.path().join("agent.json"), dir.path().join("outbox.json"));
    let pc = Arc::new(std::sync::Mutex::new((Some("Code".to_string()), false)));
    let net_down = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (pc2, net2, base2) = (pc.clone(), net_down.clone(), base.clone());
    let (local_segs, pending_offline, track_offline) = tokio::task::spawn_blocking(move || {
        use tickly_agent::{Action, Engine, HttpTransport, Outbox};
        let creds = tickly_agent::pair(&base2, &code, "Zug-Mac", "macos", "0.1.0").expect("Kopplung");
        let mut e = Engine::new(Box::new(FakePc(pc2.clone())), Flaky(HttpTransport::new(&creds), net2.clone()), Outbox::open(outbox)).with_store(store);
        let now = chrono::Utc::now;
        e.start(now(), true); // mit Netz: Einstellungen kommen an, die Rückfrage wird beantwortet
        assert_eq!(e.view().state.track, "running");
        // Netz weg
        net2.store(true, std::sync::atomic::Ordering::SeqCst);
        pc2.lock().unwrap().0 = Some("Teams".into());
        e.tick(now());
        e.action(now(), Action::Pause);
        assert_eq!(e.view().state.track, "paused", "Pause wirkt sofort, auch ohne Server");
        e.action(now(), Action::Resume);
        pc2.lock().unwrap().0 = Some("Figma".into());
        e.tick(now());
        pc2.lock().unwrap().1 = true;
        e.tick(now());
        assert_eq!(e.view().state.track, "locked");
        pc2.lock().unwrap().1 = false;
        e.tick(now());
        let v = e.view();
        assert!(!v.connected && v.estimated && v.pending >= 6, "{v:?}");
        let track_offline = v.state.track.clone();
        // Netz zurück: nach der Wartezeit geht alles raus
        net2.store(false, std::sync::atomic::Ordering::SeqCst);
        e.action(now(), Action::Pause);
        e.action(now(), Action::Resume);
        let v = e.view();
        assert!(v.connected && v.pending == 0, "{v:?}");
        (v.state.day.map(|d| d.segs.len()).unwrap_or(0), 0, track_offline)
    }).await.unwrap();
    assert_eq!(track_offline, "running");
    assert!(local_segs > 0 && pending_offline == 0);

    // Beim Server steht alles, was offline passiert ist
    let s = web.get("/api/snapshot").await.1;
    let segs = s["day"]["segs"].as_array().unwrap();
    let apps: Vec<&str> = segs.iter().filter_map(|g| g["app"].as_str()).collect();
    assert!(apps.contains(&"Teams") && apps.contains(&"Figma"), "{apps:?}");
    assert!(segs.iter().any(|g| g["kind"] == "locked"), "Sperre aus der Offline-Zeit");
    assert!(segs.iter().filter(|g| g["kind"] == "off").count() >= 2, "Pausen aus der Offline-Zeit und danach");
    assert_eq!(s["day"]["tracking"], true);
}

#[tokio::test]
async fn theme_preference_is_saved_per_user() {
    let mut c = Client::new().await;
    c.login_andre().await;
    let mut s = c.get("/api/snapshot?now=872").await.1["settings"].clone();
    assert_eq!(s["theme"], "system", "Standard: dem System folgen");
    s["theme"] = json!("dark");
    assert_eq!(c.put("/api/settings", s.clone()).await.0, StatusCode::OK);
    assert_eq!(c.get("/api/snapshot?now=872").await.1["settings"]["theme"], "dark");
    s["theme"] = json!("neon");
    assert_eq!(c.put("/api/settings", s).await.0, StatusCode::BAD_REQUEST);
    let mut other = c.fresh();
    assert_eq!(other.login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK);
    assert_eq!(other.get("/api/snapshot?now=872").await.1["settings"]["theme"], "system");
}

fn totp_code(secret_b32: &str, offset_steps: i64) -> String {
    let bytes = totp_rs::Secret::Encoded(secret_b32.to_string()).to_bytes().unwrap();
    let t = totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 0, 30, bytes, Some("Tickly".into()), "x@example.de".into()).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    t.generate((now + offset_steps * 30) as u64)
}

#[tokio::test]
async fn totp_second_factor_with_recovery_codes() {
    let mut c = Client::new().await;
    c.login_andre().await;
    // Einrichten: erst Geheimnis und QR-Code, aktiv wird es erst mit einem gültigen Code
    let (st, setup) = c.post("/api/totp/setup", json!({})).await;
    assert_eq!(st, StatusCode::OK);
    assert!(setup["qr"].as_str().unwrap().starts_with("data:image/png;base64,") && setup["uri"].as_str().unwrap().starts_with("otpauth://totp/"));
    let secret = setup["secret"].as_str().unwrap().to_string();
    assert_eq!(c.get("/api/snapshot?now=872").await.1["totp"]["enabled"], false);
    assert_eq!(c.fresh().login("andre@example.de", "demo-passwort").await, StatusCode::OK, "noch ohne zweiten Faktor");
    assert_eq!(c.post("/api/totp/enable", json!({"code": "000000"})).await.0, StatusCode::BAD_REQUEST);
    let (st, en) = c.post("/api/totp/enable", json!({"code": totp_code(&secret, 0)})).await;
    assert_eq!(st, StatusCode::OK, "{en}");
    let recovery: Vec<String> = en["recoveryCodes"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(recovery.len(), 10);
    let snap = c.get("/api/snapshot?now=872").await.1;
    assert_eq!((snap["totp"]["enabled"].clone(), snap["totp"]["recoveryLeft"].clone()), (json!(true), json!(10)));
    assert_eq!(c.post("/api/totp/setup", json!({})).await.0, StatusCode::CONFLICT, "nicht versehentlich überschreiben");

    // Anmeldung braucht jetzt zwei Schritte; ohne zweiten Schritt gibt es keine Sitzung
    let mut b = c.fresh();
    let (st, r) = b.post("/api/auth/login", json!({"email": "andre@example.de", "password": "demo-passwort"})).await;
    assert_eq!((st, r["needsTotp"].clone()), (StatusCode::OK, json!(true)));
    assert_eq!(b.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    let ticket = r["ticket"].as_str().unwrap().to_string();
    assert_eq!(b.post("/api/auth/totp", json!({"ticket": ticket, "code": "123456"})).await.0, StatusCode::UNAUTHORIZED);
    // Der Code, mit dem eingerichtet wurde, gilt nicht noch einmal; der nächste (30 s später) schon
    assert_eq!(b.post("/api/auth/totp", json!({"ticket": ticket, "code": totp_code(&secret, 0)})).await.0, StatusCode::UNAUTHORIZED);
    let (st, r) = b.post("/api/auth/totp", json!({"ticket": ticket, "code": totp_code(&secret, 1)})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(b.get("/api/snapshot?now=872").await.0, StatusCode::OK);
    assert_eq!(b.post("/api/auth/totp", json!({"ticket": ticket, "code": totp_code(&secret, 1)})).await.0, StatusCode::UNAUTHORIZED, "Ticket gilt einmal");

    // Wiederherstellungscode: einmal verwendbar
    let mut d = c.fresh();
    let t = d.post("/api/auth/login", json!({"email": "andre@example.de", "password": "demo-passwort"})).await.1["ticket"].as_str().unwrap().to_string();
    assert_eq!(d.post("/api/auth/totp", json!({"ticket": t, "code": recovery[0].to_lowercase()})).await.0, StatusCode::OK);
    let t = d.fresh().post("/api/auth/login", json!({"email": "andre@example.de", "password": "demo-passwort"})).await.1["ticket"].as_str().unwrap().to_string();
    assert_eq!(c.fresh().post("/api/auth/totp", json!({"ticket": t, "code": recovery[0]})).await.0, StatusCode::UNAUTHORIZED, "benutzt");
    assert_eq!(d.get("/api/snapshot?now=872").await.1["totp"]["recoveryLeft"], 9);

    // Neue Codes brauchen einen gültigen Code; die alten verfallen
    assert_eq!(d.post("/api/totp/recovery", json!({"code": "000000"})).await.0, StatusCode::UNAUTHORIZED);
    let (st, nr) = d.post("/api/totp/recovery", json!({"code": recovery[1]})).await;
    assert_eq!(st, StatusCode::OK);
    let fresh_codes: Vec<String> = nr["recoveryCodes"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_ne!(fresh_codes, recovery);
    let t = d.fresh().post("/api/auth/login", json!({"email": "andre@example.de", "password": "demo-passwort"})).await.1["ticket"].as_str().unwrap().to_string();
    assert_eq!(d.fresh().post("/api/auth/totp", json!({"ticket": t, "code": recovery[2]})).await.0, StatusCode::UNAUTHORIZED, "alte Codes verfallen");

    // Ausschalten verlangt ebenfalls einen Code; danach genügt wieder das Passwort
    assert_eq!(d.post("/api/totp/disable", json!({"code": "111111"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(d.post("/api/totp/disable", json!({"code": fresh_codes[0]})).await.0, StatusCode::OK);
    assert_eq!(c.fresh().login("andre@example.de", "demo-passwort").await, StatusCode::OK);
}

#[tokio::test]
async fn totp_wrong_codes_are_throttled_and_admin_can_reset() {
    let mut c = Client::new().await;
    c.login_andre().await;
    // Lena richtet TOTP ein
    let mut lena = c.fresh();
    lena.login("lena.hoffmann@example.de", "demo-passwort").await;
    let secret = lena.post("/api/totp/setup", json!({})).await.1["secret"].as_str().unwrap().to_string();
    assert_eq!(lena.post("/api/totp/enable", json!({"code": totp_code(&secret, 0)})).await.0, StatusCode::OK);
    // Zehn falsche Codes, dann ist Schluss – auch mit neuem Ticket und richtigem Code
    for i in 0..10 {
        // von wechselnden Adressen, damit die Sperre je Benutzer (nicht je IP) greift
        let mut from = c.fresh().from_ip(&format!("203.0.113.{}", i + 30));
        let t = from.post("/api/auth/login", json!({"email": "lena.hoffmann@example.de", "password": "demo-passwort"})).await.1["ticket"].as_str().unwrap().to_string();
        assert_eq!(from.post("/api/auth/totp", json!({"ticket": t, "code": format!("00000{i}")})).await.0, StatusCode::UNAUTHORIZED);
    }
    let mut from = c.fresh().from_ip("203.0.113.77");
    let t = from.post("/api/auth/login", json!({"email": "lena.hoffmann@example.de", "password": "demo-passwort"})).await.1["ticket"].as_str().unwrap().to_string();
    assert_eq!(from.post("/api/auth/totp", json!({"ticket": t, "code": totp_code(&secret, 1)})).await.0, StatusCode::TOO_MANY_REQUESTS);
    // Der Administrator kann TOTP zurücksetzen (Handy weg, Codes weg)
    assert_eq!(lena.post("/api/admin/users/u2/reset-totp", json!({})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(c.post("/api/admin/users/u2/reset-totp", json!({})).await.0, StatusCode::OK);
    assert_eq!(c.fresh().login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK);
}

fn blocked_ips(sec: &Value) -> Vec<String> {
    sec["blocked"].as_array().unwrap().iter().map(|b| b["ip"].as_str().unwrap().to_string()).collect()
}

#[tokio::test]
async fn repeated_failed_logins_block_the_ip_until_an_admin_unblocks_it() {
    let base = Client::new().await;
    let mut admin = base.fresh().from_ip("203.0.113.10");
    admin.login_andre().await;
    let mut bad = base.fresh().from_ip("203.0.113.9");
    for i in 0..5 {
        assert_eq!(bad.login("andre@example.de", "falsch").await, StatusCode::UNAUTHORIZED, "Versuch {i}");
    }
    // Danach ist die Adresse gesperrt, auch mit dem richtigen Passwort; andere Adressen sind nicht betroffen
    assert_eq!(bad.login("andre@example.de", "demo-passwort").await, StatusCode::FORBIDDEN);
    assert_eq!(bad.post("/api/auth/register", json!({"name": "X", "email": "x@example.de", "password": "Ein-Langes-Passwort-42"})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(base.fresh().from_ip("203.0.113.11").login("andre@example.de", "demo-passwort").await, StatusCode::OK);

    // Der Administrator sieht die Sperre und das Protokoll mit der Original-Adresse
    let sec = admin.get("/api/admin/security").await.1;
    assert_eq!(blocked_ips(&sec), vec!["203.0.113.9"]);
    assert_eq!(sec["yourIp"], "203.0.113.10");
    let log = admin.get("/api/admin/log?kind=fail").await.1;
    let entries = log["entries"].as_array().unwrap();
    assert!(entries.len() >= 5 && entries.iter().all(|e| e["ok"] == false));
    assert!(entries.iter().any(|e| e["ip"] == "203.0.113.9" && e["email"] == "andre@example.de" && e["detail"] == "Passwort falsch"));
    let blocks = admin.get("/api/admin/log?kind=block").await.1;
    assert!(blocks["entries"][0]["detail"].as_str().unwrap().contains("203.0.113.9 gesperrt"));
    assert_eq!(base.fresh().from_ip("203.0.113.12").get("/api/admin/log").await.0, StatusCode::UNAUTHORIZED);

    // Entsperren: sofort wieder möglich, und die alten Fehlversuche zählen nicht mehr
    assert_eq!(admin.post("/api/admin/ip-blocks/unblock", json!({"ip": "203.0.113.9"})).await.0, StatusCode::OK);
    assert_eq!(admin.post("/api/admin/ip-blocks/unblock", json!({"ip": "203.0.113.9"})).await.0, StatusCode::NOT_FOUND);
    for _ in 0..4 { assert_eq!(bad.login("andre@example.de", "falsch").await, StatusCode::UNAUTHORIZED); }
    assert_eq!(bad.login("andre@example.de", "demo-passwort").await, StatusCode::OK, "4 Fehlversuche nach der Entsperrung sperren noch nicht");
    let all = admin.get("/api/admin/log?kind=block").await.1;
    assert!(all["entries"].as_array().unwrap().iter().any(|e| e["kind"] == "unblock" && e["email"] == "andre@example.de"));
}

#[tokio::test]
async fn client_ip_comes_from_trusted_proxy_headers_only() {
    let base = Client::new().await;
    let mut admin = base.fresh().from_ip("203.0.113.10");
    admin.login_andre().await;
    // Hinter dem Proxy (lokaler Peer): X-Forwarded-For von rechts, ohne private Proxys; gefälschte linke Einträge zählen nicht
    let mut xff = base.fresh();
    xff.headers = vec![("x-forwarded-for".into(), "6.6.6.6, 9.9.9.9, 10.0.0.5".into())];
    assert_eq!(xff.login("andre@example.de", "falsch").await, StatusCode::UNAUTHORIZED);
    // Direkte Verbindung aus dem Internet: gefälschte Header werden ignoriert
    let mut direct = base.fresh().direct("198.51.100.7", &[("cf-connecting-ip", "1.2.3.4"), ("x-forwarded-for", "5.6.7.8")]);
    for _ in 0..5 { direct.login("andre@example.de", "falsch").await; }
    assert_eq!(direct.login("andre@example.de", "demo-passwort").await, StatusCode::FORBIDDEN);
    let log = admin.get("/api/admin/log?kind=fail").await.1;
    let ips: Vec<&str> = log["entries"].as_array().unwrap().iter().map(|e| e["ip"].as_str().unwrap()).collect();
    assert!(ips.contains(&"9.9.9.9") && ips.contains(&"198.51.100.7"), "{ips:?}");
    assert!(!ips.contains(&"6.6.6.6") && !ips.contains(&"1.2.3.4") && !ips.contains(&"5.6.7.8"), "{ips:?}");
    assert_eq!(blocked_ips(&admin.get("/api/admin/security").await.1), vec!["198.51.100.7"]);
}

#[tokio::test]
async fn ipv6_is_blocked_by_its_64_network_and_allowlist_and_manual_blocks_work() {
    let base = Client::new().await;
    let mut admin = base.fresh().from_ip("203.0.113.10");
    admin.login_andre().await;
    for _ in 0..5 { base.fresh().from_ip("2001:db8:1:2:aaaa:bbbb:cccc:dddd").login("andre@example.de", "falsch").await; }
    // andere Adresse im selben /64-Netz ist ebenfalls gesperrt
    assert_eq!(base.fresh().from_ip("2001:db8:1:2:1111:2222:3333:4444").login("andre@example.de", "demo-passwort").await, StatusCode::FORBIDDEN);
    assert_eq!(base.fresh().from_ip("2001:db8:1:3::1").login("andre@example.de", "demo-passwort").await, StatusCode::OK);
    let sec = admin.get("/api/admin/security").await.1;
    assert_eq!(blocked_ips(&sec), vec!["2001:db8:1:2::/64"]);
    assert_eq!(admin.post("/api/admin/ip-blocks/unblock", json!({"ip": "2001:db8:1:2::/64"})).await.0, StatusCode::OK);

    // Einstellungen prüfen und Ausnahmeliste: das Büronetz wird nie gesperrt
    let mut p = sec["policy"].clone();
    p["maxFails"] = json!(1);
    assert_eq!(admin.put("/api/admin/security", p.clone()).await.0, StatusCode::BAD_REQUEST);
    p["maxFails"] = json!(3);
    p["allowlist"] = json!(["not-an-ip"]);
    assert_eq!(admin.put("/api/admin/security", p.clone()).await.0, StatusCode::BAD_REQUEST);
    p["allowlist"] = json!(["192.0.2.0/24"]);
    assert_eq!(admin.put("/api/admin/security", p.clone()).await.0, StatusCode::OK);
    let mut office = base.fresh().from_ip("192.0.2.55");
    for _ in 0..8 { office.login("andre@example.de", "falsch").await; }
    assert_eq!(office.login("andre@example.de", "demo-passwort").await, StatusCode::OK, "Ausnahme gilt");
    let mut other = base.fresh().from_ip("192.0.3.55");
    for _ in 0..3 { other.login("andre@example.de", "falsch").await; }
    assert_eq!(other.login("andre@example.de", "demo-passwort").await, StatusCode::FORBIDDEN, "neue Schwelle: 3");

    // Manuell sperren und entsperren; die eigene Adresse lässt sich nicht sperren
    assert_eq!(admin.post("/api/admin/ip-blocks", json!({"ip": "203.0.113.10"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(admin.post("/api/admin/ip-blocks", json!({"ip": "nonsense"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(admin.post("/api/admin/ip-blocks", json!({"ip": "198.18.0.1", "reason": "Scanner"})).await.0, StatusCode::OK);
    assert_eq!(admin.post("/api/admin/ip-blocks", json!({"ip": "198.18.0.1"})).await.0, StatusCode::CONFLICT);
    assert_eq!(base.fresh().from_ip("198.18.0.1").login("andre@example.de", "demo-passwort").await, StatusCode::FORBIDDEN);
    // Ausschalten der automatischen Sperre
    p["enabled"] = json!(false);
    assert_eq!(admin.put("/api/admin/security", p).await.0, StatusCode::OK);
    let mut free = base.fresh().from_ip("198.18.5.5");
    for _ in 0..10 { free.login("andre@example.de", "falsch").await; }
    assert_eq!(free.login("andre@example.de", "demo-passwort").await, StatusCode::OK);
}

#[tokio::test]
async fn admin_actions_and_logins_appear_in_the_log() {
    let base = Client::new().await;
    let mut admin = base.fresh().from_ip("203.0.113.10");
    admin.login_andre().await;
    assert_eq!(admin.call(Method::DELETE, "/api/admin/users/u9", None).await.0, StatusCode::OK);
    assert_eq!(base.fresh().from_ip("203.0.113.20").login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK);
    let (_, file) = admin.raw(Method::GET, "/api/admin/backup", vec![]).await;
    assert!(!file.is_empty());
    let log = admin.get("/api/admin/log?limit=50").await.1;
    let line = |k: &str| log["entries"].as_array().unwrap().iter().find(|e| e["kind"] == k).cloned().unwrap_or(Value::Null);
    assert_eq!(line("backup")["email"], "andre@example.de");
    assert!(line("admin")["detail"].as_str().unwrap().contains("anna.lehmann@example.de"));
    assert_eq!(line("login")["ip"], "203.0.113.20");
    let q = admin.get("/api/admin/log?q=203.0.113.20").await.1;
    assert!(q["entries"].as_array().unwrap().iter().all(|e| e["ip"] == "203.0.113.20"));
    // Blättern: „next“ liefert den Rest
    let page = admin.get("/api/admin/log?limit=2").await.1;
    assert_eq!(page["entries"].as_array().unwrap().len(), 2);
    let next = page["next"].as_i64().unwrap();
    let rest = admin.get(&format!("/api/admin/log?limit=2&before={next}")).await.1;
    assert!(rest["entries"][0]["id"].as_i64().unwrap() < next);
}

async fn set_mfa_policy(admin: &mut Client, required: bool, grace: u32, session_days: u32) -> StatusCode {
    let mut reg = admin.get("/api/snapshot?now=872").await.1["reg"].clone();
    reg["mfaRequired"] = json!(required);
    reg["mfaGraceDays"] = json!(grace);
    reg["sessionDays"] = json!(session_days);
    admin.put("/api/admin/registration", reg).await.0
}

/// Sekunden, die die Sitzung des Clients noch gültig ist bzw. insgesamt gültig war (aus der Datenbank).
async fn session_lifetime_hours(c: &Client, email: &str) -> f64 {
    sqlx::query_scalar("SELECT (extract(epoch from (s.expires_at - s.created_at)) / 3600.0)::float8 FROM sessions s JOIN users u ON u.id = s.user_id WHERE lower(u.email) = lower($1) ORDER BY s.created_at DESC LIMIT 1")
        .bind(email).fetch_one(&c.app.pool).await.unwrap()
}

#[tokio::test]
async fn mfa_requirement_with_grace_period_reminders_and_setup_only_after_expiry() {
    let mut admin = Client::new().await;
    admin.login_andre().await;
    assert!((session_lifetime_hours(&admin, "andre@example.de").await - 720.0).abs() < 0.1, "Standard: 30 Tage");
    // Validierung der Werte
    assert_eq!(set_mfa_policy(&mut admin, true, 366, 30).await, StatusCode::BAD_REQUEST);
    assert_eq!(set_mfa_policy(&mut admin, true, 7, 0).await, StatusCode::BAD_REQUEST);
    assert_eq!(set_mfa_policy(&mut admin, true, 7, 400).await, StatusCode::BAD_REQUEST);
    assert_eq!(set_mfa_policy(&mut admin, true, 7, 30).await, StatusCode::OK);
    let reg = admin.get("/api/snapshot?now=872").await.1["reg"].clone();
    assert_eq!((reg["mfaRequired"].clone(), reg["mfaGraceDays"].clone()), (json!(true), json!(7)));
    assert!(reg["mfaRequiredSince"].is_string(), "der Server hält fest, seit wann die Pflicht gilt");
    // Erneutes Speichern setzt die Karenzzeit nicht zurück
    let since = reg["mfaRequiredSince"].clone();
    assert_eq!(set_mfa_policy(&mut admin, true, 7, 30).await, StatusCode::OK);
    assert_eq!(admin.get("/api/snapshot?now=872").await.1["reg"]["mfaRequiredSince"], since);

    // Innerhalb der Karenzzeit: arbeiten geht, aber 24-Stunden-Sitzung und Hinweis (mit Resttagen) in jeder Antwort
    let mut lena = admin.fresh();
    lena.login("lena.hoffmann@example.de", "demo-passwort").await;
    assert!((session_lifetime_hours(&lena, "lena.hoffmann@example.de").await - 24.0).abs() < 0.1, "ohne MFA nur 24 Stunden");
    let snap = lena.get("/api/snapshot?now=872").await;
    assert_eq!(snap.0, StatusCode::OK);
    let m = &snap.1["mfa"];
    assert_eq!((m["required"].clone(), m["compliant"].clone(), m["expired"].clone(), m["daysLeft"].clone()), (json!(true), json!(false), json!(false), json!(7)));
    let sess = lena.get("/api/session").await.1;
    assert_eq!((sess["limited"].clone(), sess["mfa"]["required"].clone()), (json!(false), json!(true)));
    let users = admin.get("/api/snapshot?now=872").await.1["users"].clone();
    let l = users.as_array().unwrap().iter().find(|u| u["id"] == "u2").unwrap();
    assert!(l["mfa"]["deadline"].is_string() && l["mfa"]["compliant"] == false);

    // Nach 25 Stunden ist die Sitzung abgelaufen (täglich neu anmelden)
    sqlx::query("UPDATE sessions SET created_at = now() - interval '25 hours' WHERE user_id = 'u2'").execute(&lena.app.pool).await.unwrap();
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(lena.login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK);

    // Karenzzeit vorbei: Anmeldung klappt, aber es geht nur noch um die Einrichtung
    sqlx::query("UPDATE app_settings SET value = jsonb_set(value, '{mfaRequiredSince}', to_jsonb(now() - interval '8 days')) WHERE key = 'reg'").execute(&lena.app.pool).await.unwrap();
    sqlx::query("UPDATE users SET created_at = now() - interval '30 days', mfa_grace_start = NULL").execute(&lena.app.pool).await.unwrap();
    let sess = lena.get("/api/session").await.1;
    assert_eq!((sess["limited"].clone(), sess["reason"].clone()), (json!(true), json!("mfa")));
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::FORBIDDEN);
    assert_eq!(lena.post("/api/companion", json!({"kind": "pause"})).await.0, StatusCode::FORBIDDEN);
    // … mit Authenticator-App oder Passkey ist wieder alles frei
    let secret = lena.post("/api/totp/setup", json!({})).await.1["secret"].as_str().unwrap().to_string();
    assert_eq!(lena.post("/api/totp/enable", json!({"code": "000000"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::FORBIDDEN, "ein falscher Code ändert nichts");
    assert_eq!(lena.post("/api/totp/enable", json!({"code": totp_code(&secret, 0)})).await.0, StatusCode::OK);
    let snap = lena.get("/api/snapshot?now=872").await;
    assert_eq!(snap.0, StatusCode::OK);
    assert_eq!(snap.1["mfa"]["compliant"], true);
    assert!((session_lifetime_hours(&lena, "lena.hoffmann@example.de").await - 720.0).abs() < 0.1, "nach der Einrichtung wieder die normale Dauer");
    // Der letzte Faktor lässt sich unter Pflicht nicht entfernen
    let codes: Vec<String> = lena.post("/api/totp/recovery", json!({"code": totp_code(&secret, 1)})).await.1["recoveryCodes"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(lena.post("/api/totp/disable", json!({"code": codes[0]})).await.0, StatusCode::BAD_REQUEST);

    // Der Administrator saß selbst in der abgelaufenen Karenzzeit fest (auch er muss einrichten) und hat nun eine App
    assert_eq!(admin.get("/api/snapshot?now=872").await.0, StatusCode::FORBIDDEN);
    sqlx::query("UPDATE users SET totp_enabled = true, totp_secret = 'JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP' WHERE id = 'u1'").execute(&admin.app.pool).await.unwrap();
    // Die Sitzungsdauer ist einstellbar; ausschalten hebt die Pflicht auf
    assert_eq!(set_mfa_policy(&mut admin, true, 7, 90).await, StatusCode::OK);
    let mut other = admin.fresh();
    other.login("sophie.wagner@example.de", "demo-passwort").await;
    assert!((session_lifetime_hours(&other, "sophie.wagner@example.de").await - 24.0).abs() < 0.1);
    assert_eq!(set_mfa_policy(&mut admin, false, 7, 90).await, StatusCode::OK);
    assert_eq!(admin.get("/api/snapshot?now=872").await.1["reg"]["mfaRequiredSince"], Value::Null);
    let mut third = admin.fresh();
    third.login("murat.yilmaz@example.de", "demo-passwort").await;
    assert!((session_lifetime_hours(&third, "murat.yilmaz@example.de").await - 2160.0).abs() < 0.1, "90 Tage");
    // Admin-Aktionen stehen im Protokoll
    let log = admin.get("/api/admin/log?kind=admin").await.1;
    assert!(log["entries"].as_array().unwrap().iter().any(|e| e["detail"].as_str().unwrap().contains("MFA-Pflicht=true (Karenzzeit 7 Tage)")));
}

#[tokio::test]
async fn with_mfa_required_a_passkey_user_cannot_log_in_with_the_password_alone() {
    let mut c = Client::new().await;
    c.login_andre().await;
    let origin = Url::parse("http://localhost:3000").unwrap();
    let mut authenticator = WebauthnAuthenticator::new(SoftToken::new(true).unwrap().0);
    let (_, start) = c.post("/api/passkey/register/start", json!({})).await;
    let cred = authenticator.do_registration(origin.clone(), serde_json::from_value(start["options"].clone()).unwrap()).unwrap();
    assert_eq!(c.post("/api/passkey/register/finish", json!({"id": start["id"], "credential": cred})).await.0, StatusCode::OK);
    assert_eq!(c.fresh().login("andre@example.de", "demo-passwort").await, StatusCode::OK, "ohne Pflicht darf das Passwort genügen");
    assert_eq!(set_mfa_policy(&mut c, true, 7, 30).await, StatusCode::OK);
    let (st, r) = c.fresh().post("/api/auth/login", json!({"email": "andre@example.de", "password": "demo-passwort"})).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{r}");
    assert!(r["error"].as_str().unwrap().contains("Passkey"));
    // Mit Passkey klappt es und die Sitzung ist lang
    let mut anon = c.fresh();
    let (_, s2) = anon.post("/api/passkey/login/start", json!({"email": "andre@example.de"})).await;
    let assertion = authenticator.do_authentication(origin, serde_json::from_value(s2["options"].clone()).unwrap()).unwrap();
    assert_eq!(anon.post("/api/passkey/login/finish", json!({"id": s2["id"], "credential": assertion})).await.0, StatusCode::OK);
    assert_eq!(anon.get("/api/snapshot?now=872").await.1["mfa"]["compliant"], true);
    assert!((session_lifetime_hours(&anon, "andre@example.de").await - 720.0).abs() < 0.1);
    // Den einzigen Passkey zu löschen ist unter Pflicht nicht erlaubt
    let id = c.get("/api/snapshot?now=872").await.1["passkeys"][0]["id"].as_i64().unwrap();
    assert_eq!(c.call(Method::DELETE, &format!("/api/passkeys/{id}"), None).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn backups_from_older_versions_without_newer_columns_still_restore() {
    let mut c = Client::new().await;
    c.login_andre().await;
    let (_, file) = c.raw(Method::GET, "/api/admin/backup", vec![]).await;
    use std::io::Read;
    let mut raw = String::new();
    flate2::read::GzDecoder::new(&file[..]).read_to_string(&mut raw).unwrap();
    let mut v: Value = serde_json::from_str(&raw).unwrap();
    // So sah eine Sicherung vor TOTP und MFA-Pflicht aus
    for u in v["data"]["users"].as_array_mut().unwrap() {
        for k in ["totp_secret", "totp_enabled", "totp_last_step", "mfa_grace_start"] { u.as_object_mut().unwrap().remove(k); }
    }
    v["data"].as_object_mut().unwrap().remove("recovery_codes");
    let (st, r) = c.raw(Method::POST, "/api/admin/restore", serde_json::to_vec(&v).unwrap()).await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&r));
    let snap = c.get("/api/snapshot?now=872").await;
    assert_eq!(snap.0, StatusCode::OK);
    assert_eq!(snap.1["users"].as_array().unwrap().len(), 9);
    assert_eq!(snap.1["totp"]["enabled"], false);
}

#[tokio::test]
async fn admins_see_and_end_sessions_with_ip_country_and_flag_outdated_companions() {
    let base = Client::new().await;
    let mut admin = base.fresh();
    admin.headers = vec![("cf-connecting-ip".into(), "203.0.113.10".into()), ("cf-ipcountry".into(), "DE".into()), ("user-agent".into(), "Mozilla/5.0 (Macintosh; Mac OS X) Chrome/120".into())];
    admin.login_andre().await;
    let mut lena = base.fresh();
    lena.headers = vec![("cf-connecting-ip".into(), "198.51.100.5".into()), ("cf-ipcountry".into(), "fr".into())];
    lena.login("lena.hoffmann@example.de", "demo-passwort").await;

    // Übersicht mit Adresse, Land, Browser; die eigene Sitzung ist markiert
    let list = admin.get("/api/admin/sessions").await.1;
    let sessions = list["sessions"].as_array().unwrap();
    let l = sessions.iter().find(|s| s["email"] == "lena.hoffmann@example.de").unwrap();
    assert_eq!((l["ip"].clone(), l["country"].clone(), l["current"].clone()), (json!("198.51.100.5"), json!("FR"), json!(false)));
    let me = sessions.iter().find(|s| s["email"] == "andre@example.de").unwrap();
    assert_eq!((me["country"].clone(), me["current"].clone()), (json!("DE"), json!(true)));
    assert!(me["userAgent"].as_str().unwrap().contains("Chrome"));
    // Ohne Cloudflare-Header (direkte Verbindung) gibt es kein Land, und gefälschte Header zählen nicht
    let mut direct = base.fresh().direct("192.0.2.77", &[("cf-ipcountry", "US")]);
    direct.login("sophie.wagner@example.de", "demo-passwort").await;
    let list = admin.get("/api/admin/sessions").await.1;
    let s = list["sessions"].as_array().unwrap().iter().find(|s| s["email"] == "sophie.wagner@example.de").unwrap();
    assert_eq!((s["ip"].clone(), s["country"].clone()), (json!("192.0.2.77"), Value::Null));

    // Normale Benutzer sehen die Übersicht nicht, aber ihre eigenen Sitzungen
    assert_eq!(lena.get("/api/admin/sessions").await.0, StatusCode::FORBIDDEN);
    let own = lena.get("/api/sessions").await.1;
    assert_eq!((own.as_array().unwrap().len(), own[0]["current"].clone()), (1, json!(true)));

    // Der Administrator beendet Lenas Sitzung: sie ist sofort abgemeldet
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::OK);
    let lid = l["id"].as_str().unwrap().to_string();
    assert_eq!(admin.call(Method::DELETE, &format!("/api/admin/sessions/{lid}"), None).await.0, StatusCode::OK);
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(admin.call(Method::DELETE, &format!("/api/admin/sessions/{lid}"), None).await.0, StatusCode::NOT_FOUND);
    // Alle Sitzungen eines Benutzers auf einmal
    lena.login("lena.hoffmann@example.de", "demo-passwort").await;
    let mut lena2 = lena.fresh();
    lena2.login("lena.hoffmann@example.de", "demo-passwort").await;
    let r = admin.post("/api/admin/users/u2/end-sessions", json!({})).await.1;
    assert_eq!(r["ended"], 2);
    assert_eq!(lena2.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    // Eigene Sitzungen beenden
    let mut a2 = admin.fresh();
    a2.login_andre().await;
    let mine = a2.get("/api/sessions").await.1;
    let other = mine.as_array().unwrap().iter().find(|s| s["current"] == false).unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(a2.call(Method::DELETE, &format!("/api/sessions/{other}"), None).await.0, StatusCode::OK);
    assert_eq!(admin.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED, "die andere Sitzung desselben Benutzers wurde beendet");

    // Companion-Versionen: Geräte mit älterer Version werden markiert, die Version aktualisiert sich selbst
    let mut a3 = a2.fresh();
    a3.login_andre().await;
    let mut tokens = vec![];
    for (name, v) in [("Alt", "0.1.0"), ("Neu", "0.10.2")] {
        let code = a3.post("/api/devices/pair-code", json!({})).await.1["code"].as_str().unwrap().to_string();
        let r = a3.fresh().post("/api/device/pair", json!({"code": code, "name": name, "os": "windows", "version": v})).await.1;
        tokens.push(r["token"].as_str().unwrap().to_string());
    }
    let d = a3.get("/api/admin/sessions").await.1;
    assert_eq!(d["latestVersion"], "0.10.2", "0.10.2 ist neuer als 0.9 (numerischer Vergleich)");
    let flag = |name: &str, d: &Value| d["devices"].as_array().unwrap().iter().find(|x| x["name"] == name).unwrap()["outdated"].clone();
    assert_eq!((flag("Alt", &d), flag("Neu", &d)), (json!(true), json!(false)));
    let mut dev = a3.fresh();
    dev.headers = vec![("user-agent".into(), "tickly-companion/0.10.2".into())];
    assert_eq!(dev.call_h(Method::GET, "/api/device/state", None, Some(&tokens[0])).await.0, StatusCode::OK);
    let d = a3.get("/api/admin/sessions").await.1;
    assert_eq!(flag("Alt", &d), json!(false), "das Gerät hat sich mit neuer Version gemeldet");
    // Administrator hebt eine Kopplung auf
    let id = d["devices"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(a3.call(Method::DELETE, &format!("/api/admin/devices/{id}"), None).await.0, StatusCode::OK);
    assert_eq!(a3.get("/api/admin/sessions").await.1["devices"].as_array().unwrap().len(), 1);
}

fn seg_json(s: &Value) -> Value {
    json!({"from": s["from"], "to": s["to"], "kind": s["kind"], "app": s["app"], "note": s["note"]})
}

#[tokio::test]
async fn times_can_be_edited_afterwards_and_added_manually() {
    let mut c = Client::new().await;
    c.login_andre().await;
    let s = c.get("/api/snapshot?now=872").await.1;
    let d = day(&s, "2026-10-20");
    let segs: Vec<Value> = d["segs"].as_array().unwrap().iter().filter(|g| g["kind"] != "off").map(seg_json).collect();
    let before = d["counted"].as_f64().unwrap();
    assert!(segs.len() > 3 && before > 300.0);

    // Letzten Abschnitt entfernen: die Arbeitszeit sinkt, die Änderung ist protokolliert
    let mut shorter = segs.clone();
    let removed = shorter.pop().unwrap();
    let (st, r) = c.put("/api/days/2026-10-20", json!({"segs": shorter})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert!(r["after"].as_f64().unwrap() < r["before"].as_f64().unwrap());
    let after = day(&c.get("/api/snapshot?now=872").await.1, "2026-10-20").clone();
    assert!(after["counted"].as_f64().unwrap() < before);
    assert!((after["end"].as_f64().unwrap() - removed["from"].as_f64().unwrap()).abs() < 1.0);

    // Manueller Eintrag an einem Tag ohne Daten (Samstag): zählt voll als Mehrzeit
    let (st, _) = c.put("/api/days/2026-10-24", json!({"segs": [{"from": 600.0, "to": 690.0, "kind": "app", "app": null, "note": "Kundentermin"}]})).await;
    assert_eq!(st, StatusCode::OK);
    let sat = day(&c.get("/api/snapshot?now=872").await.1, "2026-10-24").clone();
    assert_eq!((sat["has"].clone(), sat["counted"].clone(), sat["soll"].clone()), (json!(true), json!(90.0), json!(0.0)));
    assert_eq!(sat["segs"][0]["app"], "Manuell");
    assert_eq!(sat["segs"][0]["note"], "Kundentermin");

    // Ungültige Eingaben
    let bad = |segs: Value| json!({ "segs": segs });
    let one = |f: f64, t: f64| json!({"from": f, "to": t, "kind": "app", "app": "X", "note": null});
    assert_eq!(c.put("/api/days/2026-10-24", bad(json!([one(600.0, 600.0)]))).await.0, StatusCode::BAD_REQUEST, "ohne Dauer");
    assert_eq!(c.put("/api/days/2026-10-24", bad(json!([one(600.0, 700.0), one(650.0, 720.0)]))).await.0, StatusCode::BAD_REQUEST, "Überschneidung");
    assert_eq!(c.put("/api/days/2026-10-24", bad(json!([one(1400.0, 1500.0)]))).await.0, StatusCode::BAD_REQUEST, "nach 24:00");
    assert_eq!(c.put("/api/days/2026-10-27", bad(json!([one(900.0, 960.0)]))).await.0, StatusCode::BAD_REQUEST, "Zukunft (heute 14:32)");
    assert_eq!(c.put("/api/days/2026-11-02", bad(json!([one(600.0, 660.0)]))).await.0, StatusCode::BAD_REQUEST, "Tag liegt in der Zukunft");
    assert_eq!(c.put("/api/days/2025-12-01", bad(json!([one(600.0, 660.0)]))).await.0, StatusCode::BAD_REQUEST, "vor Beginn der Erfassung");
    // Das Löschen aller Abschnitte leert den Tag
    assert_eq!(c.put("/api/days/2026-10-24", bad(json!([]))).await.0, StatusCode::OK);
    assert_eq!(day(&c.get("/api/snapshot?now=872").await.1, "2026-10-24")["has"], false);

    // Heute: der laufende Abschnitt bleibt; davor lässt sich ergänzen (z. B. vergessene Zeit), danach nicht
    let t = c.get("/api/today?now=872").await.1;
    let open_from = t["day"]["segs"].as_array().unwrap().last().unwrap()["from"].as_f64().unwrap();
    let mut today_segs: Vec<Value> = t["day"]["segs"].as_array().unwrap().iter().filter(|g| g["to"] != Value::Null && g["kind"] != "off").map(seg_json).collect();
    let mut with_open_overlap = today_segs.clone();
    with_open_overlap.push(one(open_from - 5.0, open_from + 5.0));
    assert_eq!(c.put("/api/days/2026-10-26", bad(json!(with_open_overlap))).await.0, StatusCode::BAD_REQUEST);
    let mut extra = today_segs.clone();
    extra.insert(0, one(420.0, 470.0));
    assert_eq!(c.put("/api/days/2026-10-26", bad(json!(extra))).await.0, StatusCode::OK);
    let t = c.get("/api/today?now=872").await.1;
    assert!(t["day"]["segs"].as_array().unwrap().last().unwrap()["to"].is_null(), "der laufende Abschnitt läuft weiter");
    assert_eq!(t["day"]["tracking"], true);
    assert_eq!(t["day"]["segs"][0]["from"], 420.0);
    today_segs.clear();

    // Protokoll
    let log = c.get("/api/admin/log?kind=admin").await.1;
    assert!(log["entries"].as_array().unwrap().iter().any(|e| e["kind"] == "edit" && e["detail"].as_str().unwrap().contains("2026-10-20")));

    // Administrator kann es ausschalten oder zeitlich begrenzen
    let mut reg = c.get("/api/snapshot?now=872").await.1["reg"].clone();
    reg["editDaysBack"] = json!(3);
    assert_eq!(c.put("/api/admin/registration", reg.clone()).await.0, StatusCode::OK);
    assert_eq!(c.put("/api/days/2026-10-20", bad(json!([one(600.0, 660.0)]))).await.0, StatusCode::BAD_REQUEST, "mehr als 3 Tage zurück");
    assert_eq!(c.put("/api/days/2026-10-25", bad(json!([one(600.0, 660.0)]))).await.0, StatusCode::OK);
    reg["editDaysBack"] = json!(0);
    assert_eq!(c.put("/api/admin/registration", reg).await.0, StatusCode::OK);
    assert_eq!(c.put("/api/days/2026-10-25", bad(json!([one(600.0, 660.0)]))).await.0, StatusCode::FORBIDDEN);
    assert_eq!(c.get("/api/snapshot?now=872").await.1["limits"]["editDaysBack"], 0);
}

#[tokio::test]
async fn backups_go_to_a_separate_encrypted_directory_on_schedule_with_retention() {
    let dir = tempfile::tempdir().unwrap();
    let mut c = Client::custom(Some("ein-ausreichend-langes-test-geheimnis"), Some(dir.path().to_path_buf())).await;
    c.login_andre().await;
    let pol = c.get("/api/admin/backup-policy").await.1;
    assert_eq!((pol["encrypted"].clone(), pol["dir"]["ok"].clone()), (json!(true), json!(true)));
    assert_eq!(pol["policy"]["time"], "02:00");
    // Einstellungen prüfen
    let put = |t: &str, keep: u32, to_dir: bool| json!({"enabled": true, "time": t, "keep": keep, "toDir": to_dir});
    assert_eq!(c.put("/api/admin/backup-policy", put("25:99", 2, true)).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(c.put("/api/admin/backup-policy", put("00:00", 0, true)).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(c.put("/api/admin/backup-policy", put("00:00", 2, true)).await.0, StatusCode::OK);

    // Zeitplan: um 00:00 fällig, einmal pro Tag; Ergebnis liegt verschlüsselt im Verzeichnis
    let id = tickly_api::backup::run_due(&c.app, chrono::Utc::now()).await.expect("fällig");
    assert!(id.starts_with("file:tickly-auto-") && id.ends_with(".json.gz.enc"), "{id}");
    assert!(tickly_api::backup::run_due(&c.app, chrono::Utc::now()).await.is_none(), "heute schon gesichert");
    let file = dir.path().join(id.trim_start_matches("file:"));
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(&bytes[..4], b"TKY1");
    assert!(!String::from_utf8_lossy(&bytes).contains("andre@example.de") && !bytes.starts_with(&[0x1f, 0x8b]));
    // Eine Zeit in der Zukunft löst nichts aus
    let later = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Berlin) + chrono::Duration::hours(2);
    let mut p2 = put("00:00", 2, true);
    p2["time"] = json!(later.format("%H:%M").to_string());
    c.put("/api/admin/backup-policy", p2).await;
    sqlx::query("DELETE FROM app_settings WHERE key = 'backup_state'").execute(&c.app.pool).await.unwrap();
    if later.date_naive() == chrono::Utc::now().with_timezone(&chrono_tz::Europe::Berlin).date_naive() {
        assert!(tickly_api::backup::run_due(&c.app, chrono::Utc::now()).await.is_none(), "Uhrzeit noch nicht erreicht");
    }

    // Aufbewahrung: nur die neuesten 2 manuellen Sicherungen bleiben
    for _ in 0..3 {
        assert_eq!(c.post("/api/admin/backups", json!({})).await.0, StatusCode::OK);
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }
    let list = c.get("/api/admin/backups").await.1;
    let manual: Vec<_> = list.as_array().unwrap().iter().filter(|b| b["kind"] == "manual").collect();
    assert_eq!(manual.len(), 2, "{list}");
    assert!(manual.iter().all(|b| b["file"] == true));
    assert_eq!(list.as_array().unwrap().iter().filter(|b| b["kind"] == "auto").count(), 1, "die automatische Sicherung bleibt (andere Art)");
    // Ungültige Namen werden abgewiesen
    assert!(matches!(c.raw(Method::GET, "/api/admin/backups/file:..%2F..%2Fetc%2Fpasswd", vec![]).await.0, StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND));
    assert_eq!(c.raw(Method::GET, "/api/admin/backups/file:geheim.txt", vec![]).await.0, StatusCode::BAD_REQUEST);

    // Wiederherstellen aus der verschlüsselten Datei und aus einem Download
    let first = manual[0]["id"].as_str().unwrap().to_string();
    assert_eq!(c.call(Method::DELETE, "/api/admin/users/u9", None).await.0, StatusCode::OK);
    assert_eq!(c.post(&format!("/api/admin/backups/{first}/restore"), json!({})).await.0, StatusCode::OK);
    assert_eq!(c.get("/api/snapshot?now=872").await.1["users"].as_array().unwrap().len(), 9);
    let (_, dl) = c.raw(Method::GET, "/api/admin/backup", vec![]).await;
    assert_eq!(&dl[..4], b"TKY1");
    assert_eq!(c.raw(Method::POST, "/api/admin/restore", dl).await.0, StatusCode::OK, "verschlüsselter Upload");
    // Ein Server ohne Geheimnis kann die Datei nicht lesen
    let mut other = Client::custom(None, Some(dir.path().to_path_buf())).await;
    other.login_andre().await;
    let (st, r) = other.raw(Method::POST, "/api/admin/restore", std::fs::read(&file).unwrap()).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{}", String::from_utf8_lossy(&r));
    assert!(String::from_utf8_lossy(&r).contains("TICKLY_SECRET"));

    // TOTP-Geheimnis liegt verschlüsselt in der Datenbank und funktioniert trotzdem
    let secret = c.post("/api/totp/setup", json!({})).await.1["secret"].as_str().unwrap().to_string();
    assert_eq!(c.post("/api/totp/enable", json!({"code": totp_code(&secret, 0)})).await.0, StatusCode::OK);
    let stored: String = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = 'u1'").fetch_one(&c.app.pool).await.unwrap();
    assert!(stored.starts_with("enc1:") && !stored.contains(&secret));
    let t = c.fresh().post("/api/auth/login", json!({"email": "andre@example.de", "password": "demo-passwort"})).await.1["ticket"].as_str().unwrap().to_string();
    assert_eq!(c.fresh().post("/api/auth/totp", json!({"ticket": t, "code": totp_code(&secret, 1)})).await.0, StatusCode::OK);
}

#[tokio::test]
async fn startup_check_detects_a_wrong_or_missing_secret() {
    use tickly_api::secrets::{verify_and_migrate, Secrets};
    let c = Client::new().await;
    let pool = &c.app.pool;
    sqlx::query("UPDATE users SET totp_secret = 'JBSWY3DPEHPK3PXP' WHERE id = 'u2'").execute(pool).await.unwrap();
    let a = Secrets::with("geheimnis-nummer-eins-ist-lang-genug");
    assert!(verify_and_migrate(pool, &Secrets::default()).await.is_ok(), "ohne Geheimnis und ohne Prüfwert: nichts zu tun");
    assert!(verify_and_migrate(pool, &a).await.is_ok());
    let stored: String = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = 'u2'").fetch_one(pool).await.unwrap();
    assert!(stored.starts_with("enc1:"), "Altbestand wurde verschlüsselt");
    assert!(verify_and_migrate(pool, &a).await.is_ok());
    assert!(verify_and_migrate(pool, &Secrets::with("geheimnis-nummer-zwei-ist-auch-lang-genug")).await.unwrap_err().contains("passt nicht"));
    assert!(verify_and_migrate(pool, &Secrets::default()).await.unwrap_err().contains("nicht gesetzt"));
}

/// Minimaler SMTP-Server für Tests: nimmt Nachrichten an und sammelt sie.
async fn fake_smtp() -> (u16, Arc<tokio::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let inbox: Arc<tokio::sync::Mutex<Vec<String>>> = Default::default();
    let store = inbox.clone();
    tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else { return };
            let store = store.clone();
            tokio::spawn(async move {
                let (r, mut w) = sock.into_split();
                let mut r = BufReader::new(r);
                let _ = w.write_all(b"220 test ESMTP\r\n").await;
                let (mut line, mut data, mut in_data) = (String::new(), String::new(), false);
                loop {
                    line.clear();
                    if r.read_line(&mut line).await.unwrap_or(0) == 0 { return }
                    if in_data {
                        if line == ".\r\n" { in_data = false; store.lock().await.push(std::mem::take(&mut data)); let _ = w.write_all(b"250 queued\r\n").await; } else { data.push_str(&line); }
                        continue;
                    }
                    let up = line.to_uppercase();
                    let reply: &[u8] = if up.starts_with("EHLO") || up.starts_with("HELO") { b"250 test\r\n" } else if up.starts_with("DATA") { in_data = true; b"354 go\r\n" } else if up.starts_with("QUIT") { let _ = w.write_all(b"221 bye\r\n").await; return } else { b"250 OK\r\n" };
                    let _ = w.write_all(reply).await;
                }
            });
        }
    });
    (port, inbox)
}

/// Quoted-printable zurück in Klartext (Mails werden so übertragen).
fn qp(s: &str) -> String {
    let s = s.replace("=\r\n", "");
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'=' && i + 2 < b.len() + 0 && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else { out.push(b[i]); i += 1; }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// RFC-2047-Kopfzeilen („=?utf-8?b?…?=“) lesbar machen.
fn decode_words(s: &str) -> String {
    use base64::Engine;
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("=?") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        let mut parts = tail.splitn(3, '?');
        let (Some(_charset), Some(enc), Some(more)) = (parts.next(), parts.next(), parts.next()) else { out.push_str("=?"); rest = tail; continue };
        let Some((text, after)) = more.split_once("?=") else { out.push_str("=?"); rest = tail; continue };
        let decoded = if enc.eq_ignore_ascii_case("b") { base64::engine::general_purpose::STANDARD.decode(text).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default() } else { qp(&text.replace('_', " ")) };
        out.push_str(&decoded);
        rest = after;
        let t = rest.trim_start_matches([' ', '\r', '\n', '\t']);
        if t.starts_with("=?") { rest = t; }
    }
    out.push_str(rest);
    out
}

async fn wait_mails(inbox: &Arc<tokio::sync::Mutex<Vec<String>>>, n: usize) -> Vec<String> {
    for _ in 0..60 {
        if inbox.lock().await.len() >= n { break }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    inbox.lock().await.iter().map(|m| qp(&decode_words(m))).collect()
}

fn token_in(mail: &str) -> String {
    let i = mail.find("token=").expect("Link in der Mail") + 6;
    mail[i..].chars().take_while(|c| c.is_ascii_hexdigit()).collect()
}

async fn smtp_settings(admin: &mut Client, port: u16, enabled: bool) -> StatusCode {
    admin.put("/api/admin/smtp", json!({"enabled": enabled, "host": "127.0.0.1", "port": port, "security": "none", "username": "", "newPassword": null,
        "fromEmail": "tickly@example.de", "fromName": "Tickly", "notifyAdminRegistration": true, "notifyAdminBlock": true, "notifyUserApproved": true})).await.0
}

#[tokio::test]
async fn smtp_is_configured_in_the_gui_and_test_mail_arrives() {
    let (port, inbox) = fake_smtp().await;
    let mut admin = Client::new().await;
    admin.login_andre().await;
    assert_eq!(admin.get("/api/admin/smtp").await.1["config"]["enabled"], false);
    // ungültige Eingaben
    let bad = |host: &str, from: &str| json!({"enabled": true, "host": host, "port": 25, "security": "none", "username": "", "fromEmail": from, "fromName": "T"});
    assert_eq!(admin.put("/api/admin/smtp", bad("smtp://x", "a@b.de")).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(admin.put("/api/admin/smtp", bad("mail.example.de", "keine-mail")).await.0, StatusCode::BAD_REQUEST);
    // Passwort: wird nie ausgegeben, bleibt bei leerer Eingabe erhalten
    let mut c = bad("127.0.0.1", "tickly@example.de");
    c["port"] = json!(port);
    c["username"] = json!("benutzer");
    c["newPassword"] = json!("geheim-123");
    assert_eq!(admin.put("/api/admin/smtp", c.clone()).await.0, StatusCode::OK);
    let got = admin.get("/api/admin/smtp").await.1;
    assert_eq!((got["config"]["passwordSet"].clone(), got["config"].get("password").is_none()), (json!(true), true));
    c["newPassword"] = json!("");
    assert_eq!(admin.put("/api/admin/smtp", c).await.0, StatusCode::OK);
    assert_eq!(admin.get("/api/admin/smtp").await.1["config"]["passwordSet"], true);
    // Normale Benutzer kommen nicht dran
    let mut lena = admin.fresh();
    lena.login("lena.hoffmann@example.de", "demo-passwort").await;
    assert_eq!(lena.get("/api/admin/smtp").await.0, StatusCode::FORBIDDEN);

    // Test-Mail (ohne Benutzername/Passwort, der Testserver verlangt keine Anmeldung)
    assert_eq!(smtp_settings(&mut admin, port, true).await, StatusCode::OK);
    let (st, r) = admin.post("/api/admin/smtp/test", json!({})).await;
    assert_eq!((st, r["ok"].clone()), (StatusCode::OK, json!(true)), "{r}");
    let mails = wait_mails(&inbox, 1).await;
    assert!(mails[0].contains("Subject: Tickly: Test-Nachricht") && mails[0].contains("andre@example.de") && mails[0].contains("funktioniert der E-Mail-Versand"), "{}", mails[0]);
    // Fehler werden verständlich gemeldet statt zu hängen
    assert_eq!(smtp_settings(&mut admin, 1, true).await, StatusCode::OK);
    let r = admin.post("/api/admin/smtp/test", json!({})).await.1;
    assert_eq!(r["ok"], false);
    assert!(r["error"].as_str().unwrap().len() > 5);
    // Protokoll
    let log = admin.get("/api/admin/log?q=Test-Nachricht").await.1;
    assert!(log["entries"].as_array().unwrap().iter().any(|e| e["kind"] == "mail" && e["ok"] == true));
    assert!(log["entries"].as_array().unwrap().iter().any(|e| e["kind"] == "mail" && e["ok"] == false));
}

#[tokio::test]
async fn registration_requires_confirmed_email_and_mails_flow() {
    let (port, inbox) = fake_smtp().await;
    let base = Client::new().await;
    let mut admin = base.fresh().from_ip("203.0.113.10");
    admin.login_andre().await;
    // Bestätigungspflicht geht erst mit eingerichtetem SMTP
    let mut reg = admin.get("/api/snapshot?now=872").await.1["reg"].clone();
    reg["verifyEmail"] = json!(true);
    reg["approval"] = json!(true);
    assert_eq!(admin.put("/api/admin/registration", reg.clone()).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(smtp_settings(&mut admin, port, true).await, StatusCode::OK);
    assert_eq!(admin.put("/api/admin/registration", reg.clone()).await.0, StatusCode::OK);
    assert!(admin.get("/api/snapshot?now=872").await.1["reg"]["verifyEmailSince"].is_string());
    // …und SMTP lässt sich nicht abschalten, solange sie verlangt wird
    assert_eq!(smtp_settings(&mut admin, port, false).await, StatusCode::BAD_REQUEST);
    // Bestehende Konten sind nicht betroffen
    assert_eq!(base.fresh().login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::OK);

    // Registrierung: keine Sitzung, Mail mit Link
    let mut anon = base.fresh().from_ip("198.51.100.20");
    let (st, r) = anon.post("/api/auth/register", json!({"name": "Neu Person", "email": "neu.person@example.de", "password": "Ein-Langes-Passwort-42", "startBal": 3.5, "questions": qs()})).await;
    assert_eq!((st, r["status"].clone()), (StatusCode::OK, json!("verify")));
    assert_eq!(anon.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    let mails = wait_mails(&inbox, 1).await;
    assert!(mails[0].contains("Subject: Bitte bestätige deine E-Mail-Adresse") && mails[0].contains("neu.person@example.de"), "{}", &mails[0][..mails[0].len().min(900)]);
    let t1 = token_in(&mails[0]);
    // Anmelden geht erst nach der Bestätigung
    let (st, r) = anon.post("/api/auth/login", json!({"email": "neu.person@example.de", "password": "Ein-Langes-Passwort-42"})).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(r["error"].as_str().unwrap().contains("bestätige"));
    // Erneut senden: der neue Link ersetzt den alten
    assert_eq!(anon.post("/api/auth/resend-verification", json!({"email": "neu.person@example.de"})).await.0, StatusCode::OK);
    let mails = wait_mails(&inbox, 2).await;
    let t2 = token_in(&mails[1]);
    assert_ne!(t1, t2);
    assert_eq!(anon.post("/api/auth/verify", json!({"token": t1})).await.0, StatusCode::BAD_REQUEST, "alter Link ungültig");
    let (st, r) = anon.post("/api/auth/verify", json!({"token": t2})).await;
    assert_eq!((st, r["status"].clone()), (StatusCode::OK, json!("pending")), "Freigabe durch Administrator steht noch aus");
    assert_eq!(anon.post("/api/auth/verify", json!({"token": t2})).await.0, StatusCode::BAD_REQUEST, "Link gilt einmal");
    // Erst jetzt erfahren die Administratoren von der Registrierung
    // (zwei Administratoren in den Demo-Daten, also zwei Mails)
    let mails = wait_mails(&inbox, 4).await;
    assert!(mails[2].contains("Neue Registrierung wartet auf Freigabe") && mails[2].contains("neu.person@example.de"), "{}", mails[2]);
    assert!(mails[2..4].iter().any(|m| m.contains("To: andre@example.de")) && mails[2..4].iter().any(|m| m.contains("To: sophie.wagner@example.de")));
    // Freigabe: der Benutzer bekommt eine Mail, danach klappt die Anmeldung; der Übertrag wurde übernommen
    let uid = admin.get("/api/snapshot?now=872").await.1["users"].as_array().unwrap().iter().find(|u| u["email"] == "neu.person@example.de").unwrap()["id"].as_str().unwrap().to_string();
    assert_eq!(admin.post(&format!("/api/admin/users/{uid}/status"), json!({"status": "active"})).await.0, StatusCode::OK);
    let mails = wait_mails(&inbox, 5).await;
    assert!(mails[4].contains("Subject: Dein Tickly-Konto wurde freigeschaltet") && mails[4].contains("To: neu.person@example.de"), "{}", mails.iter().map(|m| m.lines().nth(2).unwrap_or("").to_string()).collect::<Vec<_>>().join(" | "));
    assert_eq!(anon.login("neu.person@example.de", "Ein-Langes-Passwort-42").await, StatusCode::OK);
    assert_eq!(anon.get("/api/snapshot?now=872").await.1["settings"]["startBal"], 3.5);

    // Einladung per Mail
    let (st, r) = admin.post("/api/admin/users", json!({"name": "Eingeladene Person", "email": "eingeladen@example.de", "role": "user", "week": 40})).await;
    assert_eq!((st, r["mailed"].clone()), (StatusCode::OK, json!(true)));
    let mails = wait_mails(&inbox, 6).await;
    assert!(mails[5].contains("Subject: Einladung zu Tickly") && mails[5].contains("eingeladen@example.de"));

    // Gesperrte IP: Administratoren bekommen eine Mail
    let mut bad = base.fresh().from_ip("203.0.113.99");
    for _ in 0..5 { bad.login("andre@example.de", "falsch").await; }
    let mails = wait_mails(&inbox, 8).await;
    assert!(mails[6].contains("IP-Adresse gesperrt") && mails[6].contains("203.0.113.99"), "{}", mails[6]);
}

#[tokio::test]
async fn forgotten_passwords_are_reset_by_one_time_link_and_changed_when_logged_in() {
    let (port, inbox) = fake_smtp().await;
    let base = Client::new().await;
    let mut admin = base.fresh().from_ip("203.0.113.10");
    admin.login_andre().await;
    assert_eq!(smtp_settings(&mut admin, port, true).await, StatusCode::OK);
    let mut lena = base.fresh().from_ip("198.51.100.30");
    lena.login("lena.hoffmann@example.de", "demo-passwort").await;
    // Unbekannte Adressen bekommen dieselbe Antwort, aber keine Mail
    let mut anon = base.fresh().from_ip("198.51.100.31");
    assert_eq!(anon.post("/api/auth/forgot", json!({"email": "niemand@example.de"})).await.1["ok"], true);
    assert_eq!(anon.post("/api/auth/forgot", json!({"email": "lena.hoffmann@example.de"})).await.1["ok"], true);
    let mails = wait_mails(&inbox, 1).await;
    assert_eq!(mails.len(), 1);
    assert!(mails[0].contains("Subject: Passwort zurücksetzen") && mails[0].contains("lena.hoffmann@example.de"), "{}", &mails[0][..mails[0].len().min(600)]);
    let t = token_in(&mails[0]);
    assert_eq!(anon.post("/api/auth/reset", json!({"token": t, "password": "zu-schwach"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(anon.post("/api/auth/reset", json!({"token": "0".repeat(64), "password": "Neues-Passwort-4242!"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(anon.post("/api/auth/reset", json!({"token": t, "password": "Neues-Passwort-4242!"})).await.0, StatusCode::OK);
    assert_eq!(anon.post("/api/auth/reset", json!({"token": t, "password": "Noch-Eins-Anderes-42!"})).await.0, StatusCode::BAD_REQUEST, "Link gilt einmal");
    // Alle bisherigen Sitzungen sind beendet, das alte Passwort gilt nicht mehr
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(base.fresh().login("lena.hoffmann@example.de", "demo-passwort").await, StatusCode::UNAUTHORIZED);
    assert_eq!(lena.login("lena.hoffmann@example.de", "Neues-Passwort-4242!").await, StatusCode::OK);

    // Angemeldet ändern: braucht das aktuelle Passwort, beendet andere Sitzungen
    let mut other = base.fresh().from_ip("198.51.100.32");
    other.login("lena.hoffmann@example.de", "Neues-Passwort-4242!").await;
    assert_eq!(lena.post("/api/me/password", json!({"current": "falsch", "password": "Wieder-Ein-Neues-42!"})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(lena.post("/api/me/password", json!({"current": "Neues-Passwort-4242!", "password": "kurz"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(lena.post("/api/me/password", json!({"current": "Neues-Passwort-4242!", "password": "Wieder-Ein-Neues-42!"})).await.0, StatusCode::OK);
    assert_eq!(lena.get("/api/snapshot?now=872").await.0, StatusCode::OK, "die aktuelle Sitzung bleibt");
    assert_eq!(other.get("/api/snapshot?now=872").await.0, StatusCode::UNAUTHORIZED);
    // Ohne SMTP gibt es keine Mails, die Antwort verrät das nur über das Feld „mail“
    assert_eq!(smtp_settings(&mut admin, port, false).await, StatusCode::OK);
    assert_eq!(anon.post("/api/auth/forgot", json!({"email": "lena.hoffmann@example.de"})).await.1["mail"], false);
}

#[tokio::test]
async fn first_run_setup_in_browser() {
    let mut c = Client::first_run().await;
    let code = c.app.setup_code.lock().unwrap().clone().expect("Einrichtungscode wird erzeugt");
    assert_eq!(c.get("/api/config").await.1["setup"], true);
    // Bis zur Einrichtung ist die Registrierung zu, sonst gäbe es nie einen Administrator
    let (st, _) = c.post("/api/auth/register", json!({"name": "Vorwitzig", "email": "v@example.de", "password": "Sehr-Langes-Passwort-42"})).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // Falscher Code, schwaches Passwort
    let good = json!({"code": code, "name": "Chefin", "email": "chefin@example.de", "password": "Sehr-Langes-Passwort-42", "questions": qs()});
    let mut wrong = good.clone(); wrong["code"] = json!("falsch");
    assert_eq!(c.post("/api/setup", wrong).await.0, StatusCode::FORBIDDEN);
    let mut weak = good.clone(); weak["password"] = json!("kurz");
    assert_eq!(c.post("/api/setup", weak).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(c.get("/api/config").await.1["setup"], true);
    // Eine leere Installation wird nicht gesichert
    let noon = chrono::Utc::now().date_naive().and_hms_opt(12, 0, 0).unwrap().and_utc();
    assert!(tickly_api::backup::run_due(&c.app, noon).await.is_none());
    // Richtig: Administrator ist angelegt und angemeldet
    let (st, _) = c.post("/api/setup", good.clone()).await;
    assert_eq!(st, StatusCode::OK);
    let ses = c.get("/api/session").await.1;
    assert_eq!(ses["loggedIn"], true);
    assert_eq!(ses["me"]["role"], "admin");
    assert_eq!(c.get("/api/config").await.1["setup"], false);
    assert!(c.app.setup_code.lock().unwrap().is_none());
    // Nur einmal möglich
    assert_eq!(c.fresh().post("/api/setup", good).await.0, StatusCode::CONFLICT);
    assert_eq!(c.fresh().login("chefin@example.de", "Sehr-Langes-Passwort-42").await, StatusCode::OK);
    // Jetzt wird gesichert
    assert!(tickly_api::backup::run_due(&c.app, noon).await.is_some());
}

/// Konto per Registrierung anlegen (Freigabe aus) und die Anmeldung wieder verlassen.
async fn register_user(admin: &mut Client, email: &str, pw: &str) -> String {
    assert_eq!(admin.put("/api/admin/registration", json!({"open": true, "approval": false, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 14})).await.0, StatusCode::OK);
    let mut anon = admin.fresh();
    let (st, r) = anon.post("/api/auth/register", json!({"name": "Test Person", "email": email, "password": pw, "questions": qs()})).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let snap = admin.get("/api/snapshot").await.1;
    snap["users"].as_array().unwrap().iter().find(|u| u["email"] == email).unwrap()["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn password_reset_with_security_questions() {
    let mut admin = Client::new().await;
    admin.login_andre().await;
    // Registrierung verlangt drei verschiedene Fragen und brauchbare Antworten
    let mut anon = admin.fresh();
    assert_eq!(admin.put("/api/admin/registration", json!({"open": true, "approval": false, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 14})).await.0, StatusCode::OK);
    let reg = |q: Value| json!({"name": "T", "email": "t@example.de", "password": "Ein-Langes-Passwort-42", "questions": q});
    assert_eq!(anon.post("/api/auth/register", json!({"name": "T", "email": "t@example.de", "password": "Ein-Langes-Passwort-42"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(anon.post("/api/auth/register", reg(json!([{"question": "Wie hieß dein erstes Haustier?", "answer": "Bello"}, {"question": "Wie hieß dein erstes Haustier?", "answer": "Rex"}, {"question": "Eigene Frage nach etwas Geheimem?", "answer": "xyz"}]))).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(anon.post("/api/auth/register", reg(json!([{"question": "Wie hieß dein erstes Haustier?", "answer": "Bello"}, {"question": "Wie lautete dein Spitzname in der Kindheit?", "answer": "ab"}, {"question": "Eigene Frage nach etwas Geheimem?", "answer": "xyz"}]))).await.0, StatusCode::BAD_REQUEST);
    let id = register_user(&mut admin, "sq@example.de", "Ein-Langes-Passwort-42").await;
    assert!(!id.is_empty());

    // Fragen: echte für das Konto, stabile Scheinfragen für unbekannte Adressen (gleiche Form)
    let mut pub_c = admin.fresh();
    pub_c.cookie = None;
    let (st, q) = pub_c.post("/api/auth/security-questions", json!({"email": "SQ@example.de"})).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(q["questions"], json!(["Wie hieß dein erstes Haustier?", "Wie heißt die Straße, in der du aufgewachsen bist?", "Wie lautete dein Spitzname in der Kindheit?"]));
    let (_, f1) = pub_c.post("/api/auth/security-questions", json!({"email": "gibt-es-nicht@example.de"})).await;
    let (_, f2) = pub_c.post("/api/auth/security-questions", json!({"email": "gibt-es-nicht@example.de"})).await;
    assert_eq!(f1, f2);
    assert_eq!(f1["questions"].as_array().unwrap().len(), 3);

    // Ein Backup behält die Antworten
    let (st, b) = admin.post("/api/admin/backups", json!({})).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let bid = b["id"].as_str().unwrap().to_string();
    assert_eq!(admin.post(&format!("/api/admin/backups/{bid}/restore"), json!({})).await.0, StatusCode::OK);

    // Falsche Antworten: zuerst abgewiesen, nach fünf Fehlversuchen ist das Konto für diesen Weg gesperrt
    let wrong = json!({"email": "sq@example.de", "answers": ["Bello", "Lindenweg", "falsch"], "password": "Neues-Langes-Passwort-77!"});
    // (von fünf verschiedenen Adressen, damit die Sperre des Kontos und nicht die der Adresse greift)
    for i in 0..5 { assert_eq!(admin.fresh().from_ip(&format!("198.51.100.{}", 10 + i)).post("/api/auth/reset-questions", wrong.clone()).await.0, StatusCode::BAD_REQUEST); }
    let right = json!({"email": "sq@example.de", "answers": ["  bello! ", "LINDEN-WEG", "mäuschen"], "password": "Neues-Langes-Passwort-77!"});
    let (st, e) = admin.fresh().from_ip("198.51.100.99").post("/api/auth/reset-questions", right.clone()).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(e["error"].as_str().unwrap().contains("zu viele falsche Antworten"), "{e}");
    // Unbekanntes Konto: dieselbe Abweisung wie bei falschen Antworten, kein Hinweis auf die Existenz
    let (st, r) = admin.fresh().from_ip("198.51.100.98").post("/api/auth/reset-questions", json!({"email": "gibt-es-nicht@example.de", "answers": ["a1", "b2", "c3"], "password": "Neues-Langes-Passwort-77!"})).await;
    assert_eq!((st, r["error"].as_str()), (StatusCode::BAD_REQUEST, Some("Die Antworten stimmen nicht.")));
    // Wiederholtes Raten von derselben Adresse sperrt die Adresse
    let mut guesser = admin.fresh().from_ip("198.51.100.77");
    for i in 0..5 { assert_eq!(guesser.post("/api/auth/reset-questions", json!({"email": format!("x{i}@example.de"), "answers": ["a1", "b2", "c3"], "password": "Neues-Langes-Passwort-77!"})).await.0, StatusCode::BAD_REQUEST); }
    assert_eq!(guesser.post("/api/auth/security-questions", json!({"email": "sq@example.de"})).await.0, StatusCode::FORBIDDEN);

    // Nach Ablauf der Sperre klappt es: alle drei Antworten richtig (Schreibweise egal), Sitzungen enden
    sqlx::query("UPDATE audit_log SET at = now() - interval '2 hours' WHERE kind = 'reset_q'").execute(&admin.app.pool).await.unwrap();
    let mut user = admin.fresh();
    assert_eq!(user.login("sq@example.de", "Ein-Langes-Passwort-42").await, StatusCode::OK);
    let mut fresh_ip = admin.fresh().from_ip("198.51.100.55");
    assert_eq!(fresh_ip.post("/api/auth/reset-questions", json!({"email": "sq@example.de", "answers": ["Bello", "Lindenweg"], "password": "Neues-Langes-Passwort-77!"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(fresh_ip.post("/api/auth/reset-questions", right).await.0, StatusCode::OK);
    assert_eq!(user.get("/api/snapshot").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(admin.fresh().login("sq@example.de", "Ein-Langes-Passwort-42").await, StatusCode::UNAUTHORIZED);
    assert_eq!(admin.fresh().login("sq@example.de", "Neues-Langes-Passwort-77!").await, StatusCode::OK);

    // Ausgeschaltet: weder Fragen noch Zurücksetzen
    assert_eq!(admin.put("/api/admin/registration", json!({"open": true, "approval": false, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 14, "secQuestions": false})).await.0, StatusCode::OK);
    assert_eq!(pub_c.post("/api/auth/security-questions", json!({"email": "sq@example.de"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(pub_c.post("/api/auth/reset-questions", json!({"email": "sq@example.de", "answers": ["a", "b", "c"], "password": "Neues-Langes-Passwort-77!"})).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn own_security_questions_can_be_changed() {
    let mut admin = Client::new().await;
    admin.login_andre().await;
    register_user(&mut admin, "own@example.de", "Ein-Langes-Passwort-42").await;
    let mut u = admin.fresh();
    assert_eq!(u.login("own@example.de", "Ein-Langes-Passwort-42").await, StatusCode::OK);
    let st = u.get("/api/me/security-questions").await.1;
    assert_eq!((st["set"].clone(), st["enabled"].clone()), (json!(true), json!(true)));
    let newq = json!([{"question": "Wie hieß dein Kuscheltier als Kind?", "answer": "Teddy"}, {"question": "Welches war dein erstes Handy (Marke und Modell)?", "answer": "Nokia 3310"}, {"question": "Eine eigene Frage, die nur ich kenne?", "answer": "Zebra"}]);
    assert_eq!(u.put("/api/me/security-questions", json!({"current": "falsch", "questions": newq})).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(u.put("/api/me/security-questions", json!({"current": "Ein-Langes-Passwort-42", "questions": newq})).await.0, StatusCode::OK);
    let st = u.get("/api/me/security-questions").await.1;
    assert_eq!(st["questions"][2], "Eine eigene Frage, die nur ich kenne?");
    // Die neuen Antworten gelten (Leerzeichen und Satzzeichen egal)
    let mut anon = admin.fresh();
    anon.cookie = None;
    assert_eq!(anon.post("/api/auth/reset-questions", json!({"email": "own@example.de", "answers": ["TEDDY", "nokia-3310", "zebra"], "password": "Neues-Langes-Passwort-77!"})).await.0, StatusCode::OK);
}

#[tokio::test]
async fn admin_initial_password_must_be_changed_at_next_login() {
    let mut admin = Client::new().await;
    admin.login_andre().await;
    let id = register_user(&mut admin, "temp@example.de", "Ein-Langes-Passwort-42").await;
    let mut user = admin.fresh();
    assert_eq!(user.login("temp@example.de", "Ein-Langes-Passwort-42").await, StatusCode::OK);

    // Nur Administratoren, nicht für sich selbst
    assert_eq!(user.post(&format!("/api/admin/users/{id}/password"), json!({})).await.0, StatusCode::FORBIDDEN);
    let me = admin.get("/api/session").await.1["me"]["id"].as_str().unwrap().to_string();
    assert_eq!(admin.post(&format!("/api/admin/users/{me}/password"), json!({})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(admin.post(&format!("/api/admin/users/{id}/password"), json!({"password": "zu kurz"})).await.0, StatusCode::BAD_REQUEST);

    // Erzeugtes Passwort: erfüllt die Richtlinie, beendet die Sitzungen des Benutzers
    let (st, r) = admin.post(&format!("/api/admin/users/{id}/password"), json!({})).await;
    assert_eq!(st, StatusCode::OK);
    let temp = r["password"].as_str().unwrap().to_string();
    assert!(temp.len() >= 18 && tickly_api::app::password_problem(&temp, 14).is_none(), "{temp}");
    assert_eq!(user.get("/api/snapshot").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(admin.fresh().login("temp@example.de", "Ein-Langes-Passwort-42").await, StatusCode::UNAUTHORIZED);

    // Anmeldung klappt, aber alles ist gesperrt, bis das Passwort geändert ist
    let mut u = admin.fresh();
    assert_eq!(u.login("temp@example.de", &temp).await, StatusCode::OK);
    let ses = u.get("/api/session").await.1;
    assert_eq!((ses["limited"].clone(), ses["reason"].clone()), (json!(true), json!("password")));
    let (st, e) = u.get("/api/snapshot").await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(e["error"].as_str().unwrap().contains("neues Passwort"), "{e}");
    assert_eq!(u.post("/api/me/password", json!({"current": &temp, "password": "Neues-Langes-Passwort-77!"})).await.0, StatusCode::FORBIDDEN);
    // Das neue Passwort muss anders und gültig sein
    assert_eq!(u.post("/api/me/password/required", json!({"password": &temp})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(u.post("/api/me/password/required", json!({"password": "kurz"})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(u.post("/api/me/password/required", json!({"password": "Neues-Langes-Passwort-77!"})).await.0, StatusCode::OK);
    assert_eq!(u.get("/api/snapshot").await.0, StatusCode::OK);
    assert_eq!(u.get("/api/session").await.1["limited"], false);
    // Ohne erzwungene Änderung lässt sich der Weg nicht missbrauchen
    assert_eq!(u.post("/api/me/password/required", json!({"password": "Noch-Ein-Passwort-88!x"})).await.0, StatusCode::BAD_REQUEST);
    // Das Initialpasswort gilt nicht mehr, das neue schon
    assert_eq!(admin.fresh().login("temp@example.de", &temp).await, StatusCode::UNAUTHORIZED);
    let mut again = admin.fresh();
    assert_eq!(again.login("temp@example.de", "Neues-Langes-Passwort-77!").await, StatusCode::OK);
    assert_eq!(again.get("/api/snapshot").await.0, StatusCode::OK);

    // Eigenes Initialpasswort des Administrators
    let (st, r) = admin.post(&format!("/api/admin/users/{id}/password"), json!({"password": "Vom-Admin-Gewaehlt-99!"})).await;
    assert_eq!((st, r["password"].clone()), (StatusCode::OK, json!("Vom-Admin-Gewaehlt-99!")));
}

#[tokio::test]
async fn web_only_tracking_start_stop_and_preview() {
    let mut c = Client::real().await;
    assert_eq!(c.login("chef@example.de", "Sehr-Langes-Passwort-42").await, StatusCode::OK);
    // Ohne laufendes Tracking gibt es nichts zu beenden; Systemereignisse meldet nur der Companion
    assert_eq!(c.post("/api/companion", json!({"kind": "stop"})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "stopat", "until": 100})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "lock"})).await.0, StatusCode::FORBIDDEN);
    assert_eq!(c.post("/api/tracking/stop-preview", json!({})).await.0, StatusCode::CONFLICT);
    // Start im Web (ohne Companion), nicht doppelt
    assert_eq!(c.post("/api/companion", json!({"kind": "start"})).await.0, StatusCode::OK);
    assert_eq!(c.post("/api/companion", json!({"kind": "start"})).await.0, StatusCode::CONFLICT);
    let t = c.get("/api/today").await.1;
    assert_eq!(t["day"]["tracking"], true);
    assert!(t["day"]["events"][0]["txt"].as_str().unwrap().contains("im Web gestartet"));
    // Vorschau: Zeitpunkt, Brutto, Pause, Netto; ein Ende vor dem Beginn geht nicht (die Uhr läuft sekundengenau)
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let (st, p) = c.post("/api/tracking/stop-preview", json!({})).await;
    assert_eq!(st, StatusCode::OK, "{p}");
    assert!(p["gross"].is_number() && p["net"].is_number() && p["pause"].is_number() && p["at"].is_number());
    assert_eq!(c.post("/api/tracking/stop-preview", json!({"at": 0})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "stopat"})).await.0, StatusCode::BAD_REQUEST);
    // Beenden: danach läuft nichts mehr, Fortsetzen geht (Lücke zählt nicht)
    assert_eq!(c.post("/api/companion", json!({"kind": "stop"})).await.0, StatusCode::OK);
    let t = c.get("/api/today").await.1;
    assert_eq!((t["day"]["tracking"].clone(), t["day"]["events"].as_array().unwrap().last().unwrap()["k"].clone()), (json!(false), json!("stop")));
    assert_eq!(c.post("/api/companion", json!({"kind": "stop"})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/companion", json!({"kind": "start"})).await.0, StatusCode::OK);

    // Ein gekoppelter Companion darf nicht „beenden“ melden (das ist Sache der Weboberfläche)
    let code = c.post("/api/devices/pair-code", json!({})).await.1["code"].as_str().unwrap().to_string();
    let mut dev = c.fresh();
    dev.cookie = None;
    let token = dev.post("/api/device/pair", json!({"code": code, "name": "Mac", "os": "macos", "version": "1.0.0"})).await.1["token"].as_str().unwrap().to_string();
    let at = chrono::Utc::now().to_rfc3339();
    let (st, r) = dev.call_h(Method::POST, "/api/device/events", Some(json!({"events": [{"seq": 1, "kind": "stop", "at": at}]})), Some(&token)).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["results"][0]["ok"], false);
    assert_eq!(c.get("/api/today").await.1["day"]["tracking"], true);
}

#[tokio::test]
async fn web_stop_at_manual_time_with_preview() {
    let mut c = Client::new().await;
    c.login_andre().await;
    // Demo-Uhr: 15:00; Vorschau für ein Ende um 13:00 (Brutto/Netto bis dahin), Zukunft und vor Beginn gehen nicht
    let now = 900;
    let (st, p) = c.post("/api/tracking/stop-preview", json!({"at": 780, "now": now})).await;
    assert_eq!(st, StatusCode::OK, "{p}");
    assert_eq!(p["at"], 780.0);
    assert!(p["gross"].as_f64().unwrap() > 0.0 && p["net"].as_f64().unwrap() <= p["gross"].as_f64().unwrap());
    let later = c.post("/api/tracking/stop-preview", json!({"at": 840, "now": now})).await.1["gross"].as_f64().unwrap();
    assert!(later > p["gross"].as_f64().unwrap());
    assert_eq!(c.post("/api/tracking/stop-preview", json!({"at": 950, "now": now})).await.0, StatusCode::CONFLICT);
    assert_eq!(c.post("/api/tracking/stop-preview", json!({"at": 100, "now": now})).await.0, StatusCode::CONFLICT);
    // Die Vorschau ändert nichts
    assert_eq!(c.get(&format!("/api/today?now={now}")).await.1["day"]["tracking"], true);
    // Beenden mit gewünschtem Ende: alles danach entfällt, Zeit läuft nicht weiter
    assert_eq!(c.post("/api/companion", json!({"kind": "stopat", "until": 780, "now": now})).await.0, StatusCode::OK);
    let a = c.get(&format!("/api/today?now={now}")).await.1;
    let b = c.get("/api/today?now=1100").await.1;
    assert_eq!((a["day"]["tracking"].clone(), a["day"]["ended"].clone()), (json!(false), json!(780.0)));
    assert_eq!(a["today"]["counted"], b["today"]["counted"]);
    assert_eq!(a["today"]["gross"].as_f64().unwrap(), p["gross"].as_f64().unwrap());
    assert!(a["day"]["segs"].as_array().unwrap().iter().all(|s| s["to"].as_f64().unwrap() <= 780.0));
}

#[tokio::test]
async fn long_tracking_mail_and_midnight_carry_over() {
    let (port, inbox) = fake_smtp().await;
    let mut c = Client::real().await;
    assert_eq!(c.login("chef@example.de", "Sehr-Langes-Passwort-42").await, StatusCode::OK);
    assert_eq!(smtp_settings(&mut c, port, true).await, StatusCode::OK);
    let app = c.app.clone();
    let (today, _) = app.clock(None);
    // Tracking seit 08:00, jetzt 20:00 (12 h): Hinweis genau einmal, mit Link und Brutto/Netto
    let mut day = tickly_core::DayState::empty(480.0);
    day.web_start(480.0).unwrap();
    let uid = c.get("/api/session").await.1["me"]["id"].as_str().unwrap().to_string();
    { let mut conn = app.pool.acquire().await.unwrap(); db::put_day(&mut conn, &uid, today, &day).await.map_err(|e| e.1).unwrap(); }
    assert_eq!(tickly_api::tracking::tick_at(&app, today, 9.0 * 60.0).await.unwrap(), 0, "erst 1 h: kein Hinweis");
    assert_eq!(tickly_api::tracking::tick_at(&app, today, 20.0 * 60.0).await.unwrap(), 1);
    assert_eq!(tickly_api::tracking::tick_at(&app, today, 20.5 * 60.0).await.unwrap(), 0, "höchstens einmal am Tag");
    let mails = wait_mails(&inbox, 1).await;
    let m = &mails[0];
    assert!(m.contains("chef@example.de") && m.contains("Läuft dein Tracking noch?") && m.contains("08:00") && m.contains("#/dashboard"), "{}", &m[..m.len().min(1200)]);
    assert!(m.contains("12:00 Stunden Brutto"), "{}", &m[..m.len().min(1200)]);
    // Schwelle 0 = aus
    let mut reg = c.get("/api/snapshot").await.1["reg"].clone();
    reg["longTrackHours"] = json!(0);
    assert_eq!(c.put("/api/admin/registration", reg).await.0, StatusCode::OK);
    sqlx::query("DELETE FROM track_alerts").execute(&app.pool).await.unwrap();
    assert_eq!(tickly_api::tracking::tick_at(&app, today, 20.0 * 60.0).await.unwrap(), 0);

    // Ohne Companion läuft das Tracking über Mitternacht weiter; mit gekoppeltem Companion macht das dessen Rechner
    let tomorrow = today + chrono::Duration::days(1);
    assert_eq!(tickly_api::tracking::tick_at(&app, tomorrow, 5.0).await.unwrap(), 0);
    let mut conn = app.pool.acquire().await.unwrap();
    let next = db::get_day(&mut conn, &uid, tomorrow).await.map_err(|e| e.1).unwrap().expect("Fortsetzung");
    assert!(next.tracking && next.segs[0].from == 0.0);
    let prev = db::get_day(&mut conn, &uid, today).await.map_err(|e| e.1).unwrap().unwrap();
    assert_eq!(prev.segs.last().unwrap().to, Some(1440.0));
}

#[tokio::test]
async fn admin_created_user_sets_password_and_security_questions_at_first_login() {
    let mut admin = Client::new().await;
    admin.login_andre().await;
    // Direkt anlegen: Initialpasswort wird erzeugt (oder gewählt, dann geprüft); nur für Administratoren
    let new = |extra: Value| { let mut v = json!({"name": "Neu Angelegt", "email": "angelegt@example.de", "role": "user", "week": 40, "direct": true}); v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone()); v };
    assert_eq!(admin.post("/api/admin/users", new(json!({"password": "zu kurz"}))).await.0, StatusCode::BAD_REQUEST);
    let (st, r) = admin.post("/api/admin/users", new(json!({}))).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let temp = r["password"].as_str().unwrap().to_string();
    assert!(tickly_api::app::password_problem(&temp, 14).is_none());
    // Das Konto ist sofort aktiv (keine Einladung, keine Mail-Bestätigung nötig)
    let snap = admin.get("/api/snapshot?now=872").await.1;
    let u = snap["users"].as_array().unwrap().iter().find(|u| u["email"] == "angelegt@example.de").unwrap();
    assert_eq!((u["status"].clone(), u["mustChangePw"].clone()), (json!("active"), json!(true)));
    // Registrieren geht nicht mehr (Konto existiert bereits)
    assert_eq!(admin.fresh().post("/api/auth/register", json!({"name": "X", "email": "angelegt@example.de", "password": "Ein-Langes-Passwort-42", "questions": qs()})).await.0, StatusCode::CONFLICT);

    // Erste Anmeldung: gesperrt, die Oberfläche erfährt, dass auch Sicherheitsfragen fehlen
    let mut u = admin.fresh();
    assert_eq!(u.login("angelegt@example.de", &temp).await, StatusCode::OK);
    let ses = u.get("/api/session").await.1;
    assert_eq!((ses["reason"].clone(), ses["needsQuestions"].clone()), (json!("password"), json!(true)));
    let pw = "Eigenes-Neues-Passwort-77!";
    // Ohne gültige Fragen wird nichts geändert
    assert_eq!(u.post("/api/me/password/required", json!({"password": pw})).await.0, StatusCode::BAD_REQUEST);
    let same = json!([{"question": "Wie hieß dein erstes Haustier?", "answer": "Bello"}, {"question": "Wie hieß dein erstes Haustier?", "answer": "Rex"}, {"question": "Wie heißt die Straße, in der du aufgewachsen bist?", "answer": "Lindenweg"}]);
    assert_eq!(u.post("/api/me/password/required", json!({"password": pw, "questions": same})).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(u.get("/api/session").await.1["reason"], "password", "nichts wurde geändert");
    assert_eq!(u.post("/api/me/password/required", json!({"password": pw, "questions": qs()})).await.0, StatusCode::OK);
    assert_eq!(u.get("/api/snapshot?now=872").await.0, StatusCode::OK);
    assert_eq!(u.get("/api/me/security-questions").await.1["set"], true);
    // Die Fragen funktionieren: Zurücksetzen per Antworten
    let mut anon = admin.fresh();
    anon.cookie = None;
    assert_eq!(anon.post("/api/auth/reset-questions", json!({"email": "angelegt@example.de", "answers": ["bello", "lindenweg", "mäuschen"], "password": "Wieder-Ein-Neues-Passwort-88!"})).await.0, StatusCode::OK);

    // Wer schon Fragen hat (Konto mit Registrierung), wird bei einem Initialpasswort nicht erneut gefragt
    let id = register_user(&mut admin, "hat.fragen@example.de", "Ein-Langes-Passwort-42").await;
    assert_eq!(admin.post(&format!("/api/admin/users/{id}/password"), json!({})).await.0, StatusCode::OK);
    let tmp = admin.post(&format!("/api/admin/users/{id}/password"), json!({"password": "Vom-Admin-Gewaehlt-99!"})).await.1["password"].as_str().unwrap().to_string();
    let mut v = admin.fresh();
    assert_eq!(v.login("hat.fragen@example.de", &tmp).await, StatusCode::OK);
    assert_eq!(v.get("/api/session").await.1["needsQuestions"], false);
    assert_eq!(v.post("/api/me/password/required", json!({"password": "Mein-Neues-Passwort-55!"})).await.0, StatusCode::OK);
    // Funktion ausgeschaltet: keine Fragen verlangt
    assert_eq!(admin.put("/api/admin/registration", json!({"open": true, "approval": false, "domains": "", "weekly": 40, "passkeyPrompt": true, "minPw": 14, "secQuestions": false})).await.0, StatusCode::OK);
    let (_, r2) = admin.post("/api/admin/users", new(json!({"email": "ohne@example.de"}))).await;
    let mut w = admin.fresh();
    assert_eq!(w.login("ohne@example.de", r2["password"].as_str().unwrap()).await, StatusCode::OK);
    assert_eq!(w.get("/api/session").await.1["needsQuestions"], false);
    assert_eq!(w.post("/api/me/password/required", json!({"password": "Mein-Neues-Passwort-55!"})).await.0, StatusCode::OK);
}

#[tokio::test]
async fn origin_secret_is_required_and_violations_are_logged() {
    const SECRET: &str = "ein-geheimer-origin-wert-mit-genug-laenge-1234";
    let name = fresh_db(true).await;
    let url = url_for(&name);
    let pool = db::connect(&url).await.unwrap();
    let mut cfg = config(url, true);
    cfg.origin_secret = Some(SECRET.into());
    let app = Arc::new(App::new(cfg, pool));
    let base = Client { router: api::router(app.clone()), cookie: None, app, headers: vec![], peer: None };

    // Ohne und mit falschem Header: 403, auch für die Weboberfläche; mit richtigem Header läuft alles normal
    let mut anon = base.fresh().from_ip("203.0.113.5");
    assert_eq!(anon.get("/api/config").await.0, StatusCode::FORBIDDEN);
    assert_eq!(anon.get("/").await.0, StatusCode::FORBIDDEN);
    anon.headers.push(("x-origin-secret".into(), "falsch".into()));
    assert_eq!(anon.get("/api/config").await.0, StatusCode::FORBIDDEN);
    let mut ok = base.fresh().from_ip("203.0.113.6");
    ok.headers.push(("x-origin-secret".into(), SECRET.into()));
    assert_eq!(ok.get("/api/config").await.0, StatusCode::OK);
    ok.login_andre().await;

    // Healthcheck: nur von Loopback ohne Header; von einer anderen Adresse nicht
    assert_eq!(base.fresh().direct("127.0.0.1", &[]).get("/api/health").await.0, StatusCode::OK);
    assert_eq!(base.fresh().direct("172.18.0.5", &[]).get("/api/health").await.0, StatusCode::FORBIDDEN);
    assert_eq!(base.fresh().direct("127.0.0.1", &[]).get("/api/config").await.0, StatusCode::FORBIDDEN, "nur der Healthcheck ist ausgenommen");

    // Protokoll: ein Eintrag je Adresse und Minute (nicht je Anfrage), ohne den Wert des Headers; später mit der Zahl der weiteren Anfragen
    let log = ok.get("/api/admin/log?kind=origin").await.1;
    let entries = log["entries"].as_array().unwrap();
    let from = |ip: &str| entries.iter().filter(|e| e["ip"] == ip).collect::<Vec<_>>();
    assert_eq!(from("203.0.113.5").len(), 1, "3 Anfragen, 1 Eintrag: {entries:?}");
    let e = from("203.0.113.5")[0];
    assert_eq!(e["ok"], false);
    let d = e["detail"].as_str().unwrap();
    assert!(d.contains("X-Origin-Secret fehlt") && d.contains("GET /api/config") && d.contains("Verbindung von"), "{d}");
    assert!(!entries.iter().any(|e| e["detail"].as_str().unwrap().contains("falsch\"") || e["detail"].as_str().unwrap().contains(SECRET)));
    assert!(entries.iter().any(|e| e["ip"] == "172.18.0.5"));
    // Auch unter „Fehlversuche“ sichtbar
    assert!(ok.get("/api/admin/log?kind=fail").await.1["entries"].as_array().unwrap().iter().any(|e| e["kind"] == "origin"));
    // Nach Ablauf der Minute steht die Zahl der unterdrückten Anfragen im nächsten Eintrag
    for _ in 0..4 { anon.get("/api/config").await; }
    for v in ok.app.origin_hits.lock().unwrap().values_mut() { v.0 = Some(std::time::Instant::now() - std::time::Duration::from_secs(120)); }
    assert_eq!(anon.get("/api/config").await.0, StatusCode::FORBIDDEN);
    let log = ok.get("/api/admin/log?kind=origin&q=weitere").await.1;
    assert!(log["entries"][0]["detail"].as_str().unwrap().contains("+"), "{log}");
    // Die Administration zeigt, dass die Prüfung aktiv ist
    assert_eq!(ok.get("/api/admin/security").await.1["originSecret"], true);
}

#[tokio::test]
async fn without_origin_secret_everything_works_as_before() {
    let mut c = Client::new().await;
    assert_eq!(c.get("/api/config").await.0, StatusCode::OK);
    c.login_andre().await;
    assert_eq!(c.get("/api/admin/security").await.1["originSecret"], false);
}
