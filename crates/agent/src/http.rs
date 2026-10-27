use crate::protocol::{QueuedEvent, Reply, ServerState, Transport, TransportError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

/// Zugangsdaten eines gekoppelten Geräts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Credentials {
    pub server: String,
    pub token: String,
    pub device_id: String,
    pub device_name: String,
    pub user_name: String,
}

/// „zeit.example.com“ → „https://zeit.example.com“; „localhost:3000“ → „http://localhost:3000“.
pub fn normalize_server(s: &str) -> String {
    let s = s.trim().trim_end_matches('/');
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else if s.starts_with("localhost") || s.starts_with("127.") {
        format!("http://{s}")
    } else {
        format!("https://{s}")
    }
}

fn client(timeout: Duration) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder().timeout(timeout).connect_timeout(Duration::from_secs(5)).user_agent(concat!("tickly-companion/", env!("CARGO_PKG_VERSION"))).build().expect("HTTP-Client")
}

fn offline(e: reqwest::Error) -> TransportError {
    TransportError::Offline(if e.is_timeout() { "Zeitüberschreitung".into() } else if e.is_connect() { "Server nicht erreichbar".into() } else { e.to_string() })
}

fn read_error(r: reqwest::blocking::Response) -> TransportError {
    let status = r.status();
    let msg = r.json::<serde_json::Value>().ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_else(|| format!("Fehler {status}"));
    if status == reqwest::StatusCode::UNAUTHORIZED {
        TransportError::Unauthorized
    } else if status.is_server_error() {
        TransportError::Offline(msg)
    } else {
        TransportError::Server(msg)
    }
}

/// Gerät mit dem Einmalcode aus der Weboberfläche koppeln.
pub fn pair(server: &str, code: &str, device_name: &str, os: &str, version: &str) -> Result<Credentials, String> {
    let server = normalize_server(server);
    let r = client(Duration::from_secs(15))
        .post(format!("{server}/api/device/pair"))
        .json(&json!({ "code": code, "name": device_name, "os": os, "version": version }))
        .send()
        .map_err(|e| match offline(e) { TransportError::Offline(m) => m, _ => "Fehler".into() })?;
    if !r.status().is_success() {
        return Err(match read_error(r) { TransportError::Server(m) | TransportError::Offline(m) => m, TransportError::Unauthorized => "Nicht berechtigt".into() });
    }
    let v: serde_json::Value = r.json().map_err(|e| e.to_string())?;
    Ok(Credentials {
        server,
        token: v["token"].as_str().ok_or("Antwort ohne Token")?.to_string(),
        device_id: v["deviceId"].as_str().unwrap_or_default().to_string(),
        device_name: device_name.to_string(),
        user_name: v["user"]["name"].as_str().unwrap_or_default().to_string(),
    })
}

pub struct HttpTransport {
    base: String,
    token: String,
}

impl HttpTransport {
    pub fn new(c: &Credentials) -> HttpTransport {
        HttpTransport { base: normalize_server(&c.server), token: c.token.clone() }
    }
}

impl Transport for HttpTransport {
    fn send(&mut self, batch: &[QueuedEvent], timeout: Duration) -> Result<Reply, TransportError> {
        let r = client(timeout).post(format!("{}/api/device/events", self.base)).bearer_auth(&self.token).json(&json!({ "events": batch })).send().map_err(offline)?;
        if !r.status().is_success() {
            return Err(read_error(r));
        }
        r.json::<Reply>().map_err(|e| TransportError::Server(e.to_string()))
    }

    fn state(&mut self, timeout: Duration) -> Result<ServerState, TransportError> {
        let r = client(timeout).get(format!("{}/api/device/state", self.base)).bearer_auth(&self.token).send().map_err(offline)?;
        if !r.status().is_success() {
            return Err(read_error(r));
        }
        r.json::<ServerState>().map_err(|e| TransportError::Server(e.to_string()))
    }
}
