//! Systemzugriff des Companion: aktive App, Bildschirmsperre und Herunterfahren.
//!
//! * macOS: `NSWorkspace` (aktive App und „WillPowerOff“), `CGSessionCopyCurrentDictionary` (Sperre)
//! * Windows: `GetForegroundWindow`, `OpenInputDesktop` (Sperre) und ein verstecktes Fenster für `WM_ENDSESSION`
//! * sonst: Platzhalter ohne Erkennung (für Tests und Entwicklung)
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod win;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysEvent {
    /// Der Rechner fährt herunter, startet neu oder die Sitzung wird beendet.
    Shutdown,
}

/// Was der Companion vom Betriebssystem wissen muss. Alle Abfragen sind günstig und blockieren nicht.
pub trait Platform: Send {
    fn os(&self) -> &'static str;
    /// Name der Anwendung im Vordergrund, falls ermittelbar.
    fn foreground_app(&mut self) -> Option<String>;
    /// Bildschirm gesperrt (Sperrbildschirm, Bildschirmschoner mit Kennwort).
    fn is_locked(&mut self) -> bool;
    /// Vom System gemeldete Ereignisse seit dem letzten Aufruf.
    fn drain_events(&mut self) -> Vec<SysEvent> {
        std::mem::take(&mut *QUEUE.lock().unwrap())
    }
}

static QUEUE: Mutex<Vec<SysEvent>> = Mutex::new(Vec::new());
static SHUTDOWN_HOOK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Wird beim Herunterfahren sofort und synchron aufgerufen. Das System lässt dafür nur wenige Sekunden,
/// deshalb sollte der Hook nur das Nötigste tun (Ereignis auf die Platte schreiben).
pub fn on_shutdown(f: impl Fn() + Send + Sync + 'static) {
    let _ = SHUTDOWN_HOOK.set(Box::new(f));
}

#[allow(dead_code)]
pub(crate) fn raise_shutdown() {
    QUEUE.lock().unwrap().push(SysEvent::Shutdown);
    if let Some(f) = SHUTDOWN_HOOK.get() {
        f();
    }
}

/// Die Plattform des laufenden Systems.
pub fn system() -> Box<dyn Platform> {
    #[cfg(target_os = "macos")]
    return Box::new(mac::Mac::new());
    #[cfg(windows)]
    return Box::new(win::Win::new());
    #[allow(unreachable_code)]
    Box::new(Unsupported)
}

/// Ohne Erkennung: nie gesperrt, keine App.
pub struct Unsupported;
impl Platform for Unsupported {
    fn os(&self) -> &'static str { "linux" }
    fn foreground_app(&mut self) -> Option<String> { None }
    fn is_locked(&mut self) -> bool { false }
}

/// Lesbarer App-Name aus dem Namen der ausführbaren Datei („msedge“ → „Edge“).
pub fn pretty_app_name(raw: &str) -> String {
    let stem = raw.rsplit(['\\', '/']).next().unwrap_or(raw);
    let stem = stem.strip_suffix(".exe").or_else(|| stem.strip_suffix(".EXE")).unwrap_or(stem);
    let l = stem.to_lowercase();
    let known = match l.as_str() {
        "code" | "code - insiders" => "VS Code",
        "chrome" => "Chrome",
        "msedge" => "Edge",
        "firefox" => "Firefox",
        "outlook" | "olk" => "Outlook",
        "ms-teams" | "teams" | "msteams" => "Teams",
        "excel" => "Excel",
        "winword" => "Word",
        "powerpnt" => "PowerPoint",
        "explorer" => "Explorer",
        "windowsterminal" | "wt" => "Terminal",
        "devenv" => "Visual Studio",
        "slack" => "Slack",
        "figma" => "Figma",
        _ => "",
    };
    if !known.is_empty() {
        return known.into();
    }
    let mut c = stem.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_prettified() {
        assert_eq!(pretty_app_name(r"C:\Program Files\Microsoft VS Code\Code.exe"), "VS Code");
        assert_eq!(pretty_app_name(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"), "Edge");
        assert_eq!(pretty_app_name("/Applications/Foo"), "Foo");
        assert_eq!(pretty_app_name("notepad++.exe"), "Notepad++");
    }
}
