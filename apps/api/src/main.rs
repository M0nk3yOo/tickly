use std::sync::Arc;
use tickly_api::{api, app::{App, Config}, db};
use tracing_subscriber::EnvFilter;

/// `tickly-api healthcheck`: für Docker (das schlanke Image hat kein curl).
fn healthcheck() -> ! {
    use std::io::{Read, Write};
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let ok = std::net::TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), std::time::Duration::from_secs(2))
        .and_then(|mut s| {
            s.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
            s.write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
            let mut out = String::new();
            s.read_to_string(&mut out)?;
            Ok(out.starts_with("HTTP/1.1 200"))
        })
        .unwrap_or(false);
    std::process::exit(if ok { 0 } else { 1 })
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        healthcheck();
    }
    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=warn".into())).init();
    let cfg = Config::from_env();
    let bind = cfg.bind.clone();
    tracing::info!("Tickly API startet auf {bind} · PUBLIC_URL={} · RP_ID={} · Web={} · Zeitzone={} · Demo={}", cfg.public_url, cfg.rp_id, cfg.web_dir.display(), cfg.tz, cfg.demo);
    let pool = db::connect(&cfg.database_url).await.unwrap_or_else(|e| panic!("Datenbank nicht erreichbar oder Migration fehlgeschlagen: {e}"));
    if let Err(e) = tickly_api::secrets::verify_and_migrate(&pool, &cfg.secrets).await { panic!("{e}"); }
    if !cfg.secrets.active() { tracing::warn!("TICKLY_SECRET ist nicht gesetzt: Sicherungen, TOTP-Geheimnisse und das SMTP-Passwort werden unverschlüsselt gespeichert"); }
    if cfg.origin_secret.is_some() { tracing::info!("Origin-Prüfung aktiv: Anfragen ohne passenden Header X-Origin-Secret werden abgelehnt und protokolliert"); }
    else if cfg.public_url.scheme() == "https" { tracing::warn!("ORIGIN_SECRET ist nicht gesetzt: Der Server nimmt Anfragen von jedem an, der den Port erreicht (nicht nur vom Proxy)"); }
    let app = Arc::new(App::new(cfg, pool));
    db::seed_if_empty(&app).await.unwrap_or_else(|e| panic!("Erste Daten konnten nicht angelegt werden: {}", e.1));
    tickly_api::backup::spawn_scheduler(app.clone());
    tickly_api::tracking::spawn_watch(app.clone());
    tickly_api::netsec::spawn_housekeeping(app.pool.clone(), app.cfg.log_retention_days);
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap_or_else(|e| panic!("Port {bind} nicht verfügbar: {e}"));
    axum::serve(listener, api::router(app).into_make_service_with_connect_info::<std::net::SocketAddr>()).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await.unwrap();
}
