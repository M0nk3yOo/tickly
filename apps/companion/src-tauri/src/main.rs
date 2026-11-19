#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod store;

use chrono::Utc;
use serde::Serialize;
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};
use store::Store;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, RunEvent, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_notification::NotificationExt;
use tickly_agent::{Action, Credentials, Engine, HttpTransport, Outbox, View};

type Eng = Engine<HttpTransport>;

struct Shared {
    store: Store,
    creds: Mutex<Option<Credentials>>,
    engine: Arc<Mutex<Option<Eng>>>,
    /// Wurde der Companion bewusst beendet? Sonst gilt ein Ende als Herunterfahren des Rechners.
    quitting: Mutex<bool>,
    last_emit: Mutex<String>,
    asked: Mutex<bool>,
    /// Beginn der Sperre, zu der zuletzt nachgefragt wurde
    away_asked: Mutex<Option<f64>>,
    /// Startet der Companion mit dem Rechner? Das Betriebssystem ist maßgeblich, hier steht der zuletzt gelesene Stand.
    autostart: AtomicBool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiState {
    paired: bool,
    server: String,
    device_name: String,
    user_name: String,
    version: &'static str,
    os: &'static str,
    autostart: bool,
    view: Option<View>,
}

fn ui_state(sh: &Shared) -> UiState {
    let c = sh.creds.lock().unwrap().clone();
    let view = sh.engine.lock().unwrap().as_ref().map(|e| e.view_at(Utc::now()));
    UiState {
        paired: c.is_some(),
        server: c.as_ref().map(|c| c.server.clone()).unwrap_or_default(),
        device_name: c.as_ref().map(|c| c.device_name.clone()).unwrap_or_default(),
        user_name: c.as_ref().map(|c| c.user_name.clone()).unwrap_or_default(),
        version: env!("CARGO_PKG_VERSION"),
        os: tickly_platform::system().os(),
        autostart: sh.autostart.load(Ordering::Relaxed),
        view,
    }
}

fn start_engine(sh: &Shared, creds: &Credentials, manual: bool) {
    let mut e = Engine::new(tickly_platform::system(), HttpTransport::new(creds), Outbox::open(sh.store.outbox_path())).with_store(sh.store.agent_path());
    // Beenden/Herunterfahren der letzten Sitzung, das nicht mehr erfasst werden konnte: zuerst nachtragen, dann den Start melden
    if let Some((kind, at)) = sh.store.take_finish() {
        e.recover_finish(at, &kind);
    }
    e.start(Utc::now(), manual);
    *sh.engine.lock().unwrap() = Some(e);
}

/// Sendet beim Herunterfahren das Ereignis, bevor das System den Prozess beendet.
fn finish(sh: &Shared, kind: &str) {
    let at = Utc::now();
    // Zuerst den Marker sichern: Hält gerade ein Tick (Netzwerk) die Engine, kommt das Ereignis nicht mehr rechtzeitig hinein.
    // Der Marker trägt es beim nächsten Start nach (mit dem echten Zeitpunkt), das Tracking endet also trotzdem zur richtigen Zeit.
    sh.store.mark_finish(kind, at);
    for _ in 0..60 {
        if let Ok(mut g) = sh.engine.try_lock() {
            if let Some(e) = g.as_mut() {
                e.finish(at, kind);
            }
            sh.store.clear_finish();
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn hm(m: f64) -> String {
    let t = m.round() as i64;
    format!("{}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

fn status_text(v: &Option<View>) -> String {
    let Some(v) = v else { return "Nicht gekoppelt".into() };
    if v.unauthorized {
        return "Kopplung aufgehoben".into();
    }
    let t = match v.state.track.as_str() {
        "running" => "Tracking läuft",
        "locked" => "Rechner gesperrt",
        "paused" => "Tracking pausiert",
        "asking" => "Arbeitest du gerade?",
        "idle" => "Tracking aus",
        _ => "Tracking beendet",
    };
    format!("{t} · {} h von {} h{}", hm(v.state.counted), hm(v.state.soll), if v.connected { "" } else { " · offline" })
}

struct Tray {
    status: MenuItem<tauri::Wry>,
    toggle: MenuItem<tauri::Wry>,
    end: MenuItem<tauri::Wry>,
}

fn refresh_ui(app: &AppHandle, tray: &Tray) {
    let sh = app.state::<Arc<Shared>>();
    // „Mit dem Rechner starten“ gehört dem Betriebssystem (Anmeldeobjekte, Autostart): so wird auch eine Änderung von außen sichtbar
    if let Ok(on) = app.autolaunch().is_enabled() {
        sh.autostart.store(on, Ordering::Relaxed);
    }
    let st = ui_state(&sh);
    let json = serde_json::to_string(&st).unwrap_or_default();
    {
        let mut last = sh.last_emit.lock().unwrap();
        if *last == json {
            return;
        }
        *last = json;
    }
    let _ = app.emit("state", &st);
    let _ = tray.status.set_text(status_text(&st.view));
    let track = st.view.as_ref().map(|v| v.state.track.clone()).unwrap_or_default();
    let (label, enabled) = match track.as_str() {
        "running" | "locked" => ("Tracking pausieren", true),
        "paused" => ("Tracking fortsetzen", st.paired),
        "idle" | "stopped" => ("Tracking starten", st.paired),
        _ => ("Tracking starten", false),
    };
    let _ = tray.toggle.set_text(label);
    let _ = tray.toggle.set_enabled(enabled);
    let _ = tray.end.set_enabled(matches!(track.as_str(), "running" | "locked" | "paused"));
    if let Some(t) = app.tray_by_id("main") {
        let _ = t.set_tooltip(Some(status_text(&st.view)));
    }
    // Rückfrage: Fenster zeigen und benachrichtigen (einmal je Rückfrage)
    let mut asked = sh.asked.lock().unwrap();
    if track == "asking" {
        if !*asked {
            *asked = true;
            show_window(app);
            let _ = app.notification().builder().title("Tickly").body("Arbeitest du gerade? Soll die Zeiterfassung laufen?").show();
        }
    } else {
        *asked = false;
    }
    // Nach einer längeren Sperre: Pause oder Arbeitszeit? (einmal je Sperre)
    let away = st.view.as_ref().and_then(|v| v.state.away);
    let mut told = sh.away_asked.lock().unwrap();
    if let Some(a) = away {
        if *told != Some(a.from) {
            *told = Some(a.from);
            show_window(app);
            let _ = app.notification().builder().title("Tickly").body(format!("Du warst {} Min. weg ({}–{} Uhr). Pause oder Arbeitszeit?", (a.to - a.from).round(), clock(a.from), clock(a.to))).show();
        }
    } else {
        *told = None;
    }
}

fn clock(m: f64) -> String {
    let t = m.round() as i64;
    format!("{:02}:{:02}", t.div_euclid(60), t.rem_euclid(60))
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn get_view(sh: tauri::State<Arc<Shared>>) -> UiState {
    ui_state(&sh)
}

#[tauri::command]
fn pair(app: AppHandle, sh: tauri::State<Arc<Shared>>, server: String, code: String) -> Result<UiState, String> {
    let name = hostname();
    let os = tickly_platform::system().os();
    let creds = tickly_agent::pair(&server, &code, &name, os, env!("CARGO_PKG_VERSION"))?;
    sh.store.save(&creds)?;
    *sh.creds.lock().unwrap() = Some(creds.clone());
    start_engine(&sh, &creds, true);
    // Ein Zeiterfassungs-Companion soll den Arbeitsbeginn mitbekommen: zunächst mit dem Rechner starten (in der App abschaltbar)
    if app.autolaunch().enable().is_ok() {
        sh.autostart.store(true, Ordering::Relaxed);
    }
    let _ = app.emit("state", ui_state(&sh));
    Ok(ui_state(&sh))
}

#[tauri::command]
fn unpair(sh: tauri::State<Arc<Shared>>) -> UiState {
    *sh.engine.lock().unwrap() = None;
    *sh.creds.lock().unwrap() = None;
    sh.store.clear();
    ui_state(&sh)
}

#[tauri::command]
fn act(sh: tauri::State<Arc<Shared>>, kind: String, value: Option<String>) -> Result<UiState, String> {
    let a = match kind.as_str() {
        "pause" => Action::Pause,
        "end" => Action::End,
        "resume" => Action::Resume,
        "answer" => Action::Answer(value.unwrap_or_default()),
        "away" => Action::Away(value.unwrap_or_default()),
        _ => return Err("Unbekannte Aktion".into()),
    };
    if let Some(e) = sh.engine.lock().unwrap().as_mut() {
        e.action(Utc::now(), a);
    }
    Ok(ui_state(&sh))
}

#[tauri::command]
fn set_autostart(app: AppHandle, sh: tauri::State<Arc<Shared>>, on: bool) -> Result<UiState, String> {
    let al = app.autolaunch();
    if on { al.enable() } else { al.disable() }.map_err(|e| format!("Autostart ließ sich nicht ändern: {e}"))?;
    sh.autostart.store(al.is_enabled().unwrap_or(on), Ordering::Relaxed);
    Ok(ui_state(&sh))
}

#[tauri::command]
fn set_auto_track(sh: tauri::State<Arc<Shared>>, on: bool) -> Result<UiState, String> {
    match sh.engine.lock().unwrap().as_mut() {
        Some(e) => e.set_auto_track(Utc::now(), on)?,
        None => return Err("Das Gerät ist nicht gekoppelt".into()),
    }
    Ok(ui_state(&sh))
}

/// Öffnet die Weboberfläche im Browser, auf Wunsch gleich auf einer Seite („settings“ = Einstellungen).
fn open_web(app: &AppHandle, sh: &Shared, page: Option<&str>) {
    if let Some(c) = sh.creds.lock().unwrap().as_ref() {
        use tauri_plugin_opener::OpenerExt;
        let base = c.server.trim_end_matches('/');
        let url = match page {
            Some("settings") => format!("{base}/#/settings"),
            _ => base.to_string(),
        };
        let _ = app.opener().open_url(url, None::<&str>);
    }
}

#[tauri::command]
fn open_dashboard(app: AppHandle, sh: tauri::State<Arc<Shared>>, page: Option<String>) {
    open_web(&app, &sh, page.as_deref());
}

#[tauri::command]
fn quit_app(app: AppHandle, sh: tauri::State<Arc<Shared>>) {
    quit(&app, &sh);
}

fn quit(app: &AppHandle, sh: &Shared) {
    *sh.quitting.lock().unwrap() = true;
    finish(sh, "quit");
    app.exit(0);
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).ok().filter(|s| !s.is_empty()).or_else(|| {
        std::process::Command::new("hostname").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).filter(|s| !s.is_empty())
    }).unwrap_or_else(|| "Mein Rechner".into())
}

fn main() {
    let manual = !std::env::args().any(|a| a == "--autostart");
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![get_view, pair, unpair, act, open_dashboard, set_autostart, set_auto_track, quit_app])
        .setup(move |app| {
            let dir = app.path().app_data_dir()?;
            let store = Store::new(&dir);
            let mut creds = store.load();
            // Unbeaufsichtigte Einrichtung: TICKLY_PAIR="server|code" koppelt beim ersten Start ohne Eingabe
            if creds.is_none() {
                if let Some((server, code)) = std::env::var("TICKLY_PAIR").ok().and_then(|v| v.split_once('|').map(|(a, b)| (a.to_string(), b.to_string()))) {
                    match tickly_agent::pair(&server, &code, &hostname(), tickly_platform::system().os(), env!("CARGO_PKG_VERSION")) {
                        Ok(c) => { let _ = store.save(&c); creds = Some(c); let _ = app.autolaunch().enable(); }
                        Err(e) => eprintln!("Kopplung per TICKLY_PAIR fehlgeschlagen: {e}"),
                    }
                }
            }
            let shared = Arc::new(Shared {
                store,
                creds: Mutex::new(creds.clone()),
                engine: Arc::new(Mutex::new(None)),
                quitting: Mutex::new(false),
                last_emit: Mutex::new(String::new()),
                asked: Mutex::new(false),
                away_asked: Mutex::new(None),
                autostart: AtomicBool::new(app.autolaunch().is_enabled().unwrap_or(false)),
            });
            app.manage(shared.clone());
            if let Some(c) = &creds {
                start_engine(&shared, c, manual);
            }
            // Das System kündigt Herunterfahren an: Ereignis sofort sichern und senden
            let sh2 = shared.clone();
            tickly_platform::on_shutdown(move || finish(&sh2, "shutdown"));

            // Tray
            let status = MenuItem::with_id(app, "status", status_text(&None), false, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Tracking pausieren", false, None::<&str>)?;
            let end = MenuItem::with_id(app, "end", "Tracking beenden", false, None::<&str>)?;
            let open = MenuItem::with_id(app, "open", "Fenster öffnen", true, None::<&str>)?;
            let dash = MenuItem::with_id(app, "dash", "Dashboard im Browser", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Einstellungen im Browser", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Tickly beenden", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status, &PredefinedMenuItem::separator(app)?, &toggle, &end, &open, &dash, &settings, &PredefinedMenuItem::separator(app)?, &quit_i])?;
            // macOS: einfarbiges Symbol, das das System passend zur Menüleiste einfärbt (hell/dunkel); Windows: das farbige
            #[cfg(target_os = "macos")]
            let (tray_icon, as_template) = (tauri::include_image!("icons/tray-template.png"), true);
            #[cfg(not(target_os = "macos"))]
            let (tray_icon, as_template) = (app.default_window_icon().cloned().ok_or("Symbol fehlt")?, false);
            TrayIconBuilder::with_id("main")
                .icon(tray_icon)
                .icon_as_template(as_template)
                .tooltip("Tickly")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| {
                    let sh = app.state::<Arc<Shared>>();
                    match ev.id().as_ref() {
                        "toggle" => {
                            let track = sh.engine.lock().unwrap().as_ref().map(|e| e.view().state.track).unwrap_or_default();
                            if let Some(e) = sh.engine.lock().unwrap().as_mut() {
                                e.action(Utc::now(), if track == "running" || track == "locked" { Action::Pause } else { Action::Resume });
                            }
                        }
                        "end" => {
                            if let Some(e) = sh.engine.lock().unwrap().as_mut() {
                                e.action(Utc::now(), Action::End);
                            }
                        }
                        "open" => show_window(app),
                        "dash" => open_web(app, &sh, None),
                        "settings" => open_web(app, &sh, Some("settings")),
                        "quit" => quit(app, &sh),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_window(tray.app_handle());
                    }
                })
                .build(app)?;
            let tray = Tray { status, toggle, end };

            // Nur beim ersten Start oder manuellen Öffnen zeigen; Autostart läuft still im Hintergrund
            if creds.is_none() || manual {
                show_window(app.handle());
            }

            // Die Tracking-Schleife
            let handle = app.handle().clone();
            std::thread::Builder::new().name("tickly-engine".into()).spawn(move || loop {
                std::thread::sleep(Duration::from_secs(2));
                let sh = handle.state::<Arc<Shared>>();
                if let Some(e) = sh.engine.lock().unwrap().as_mut() {
                    e.tick(Utc::now());
                }
                refresh_ui(&handle, &tray);
            })?;
            Ok(())
        })
        .on_window_event(|w, ev| {
            // Fenster schließen heißt nur verstecken: der Companion läuft weiter
            if let WindowEvent::CloseRequested { api, .. } = ev {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("Tauri konnte nicht gestartet werden")
        .run(|app, ev| {
            if matches!(ev, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
                // Vom System, per Cmd+Q oder AppleScript beendet: ohne eigene Beenden-Aktion als sauberes Ende melden
                // (war es das Herunterfahren, hat der Hook das Ereignis schon gesendet und `finish` tut nichts mehr)
                let sh = app.state::<Arc<Shared>>();
                if !*sh.quitting.lock().unwrap() {
                    *sh.quitting.lock().unwrap() = true;
                    finish(&sh, "quit");
                }
            }
        });
}
