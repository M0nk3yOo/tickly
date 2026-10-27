//! Windows: aktives Fenster, Sperrbildschirm (Secure Desktop) und `WM_ENDSESSION` über ein verstecktes Fenster.
use crate::{pretty_app_name, raise_shutdown, Platform};
use windows::{
    core::{w, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            StationsAndDesktops::{CloseDesktop, OpenInputDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP},
            Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION},
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId, RegisterClassW,
            TranslateMessage, MSG, WINDOW_EX_STYLE, WM_ENDSESSION, WM_QUERYENDSESSION, WNDCLASSW, WS_OVERLAPPED,
        },
    },
};

pub struct Win {
    /// Der Secure Desktop erscheint auch kurz bei Administratorabfragen: erst nach zwei Messungen in Folge als Sperre werten.
    secure_streak: u8,
}

impl Win {
    pub fn new() -> Win {
        spawn_session_window();
        Win { secure_streak: 0 }
    }
}

fn foreground_exe() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let res = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        res.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

impl Platform for Win {
    fn os(&self) -> &'static str {
        "windows"
    }

    fn foreground_app(&mut self) -> Option<String> {
        foreground_exe().map(|p| pretty_app_name(&p))
    }

    fn is_locked(&mut self) -> bool {
        // Auf dem Sperrbildschirm läuft die Eingabe auf dem Secure Desktop, den ein normaler Prozess nicht öffnen kann.
        let secure = unsafe {
            match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
                Ok(d) => {
                    let _ = CloseDesktop(d);
                    false
                }
                Err(_) => true,
            }
        };
        let lock_app = foreground_exe().is_some_and(|p| p.to_lowercase().ends_with("lockapp.exe"));
        self.secure_streak = if secure { self.secure_streak.saturating_add(1) } else { 0 };
        lock_app || self.secure_streak >= 2
    }
}

unsafe extern "system" fn wndproc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match m {
        WM_QUERYENDSESSION => LRESULT(1),
        WM_ENDSESSION => {
            if w.0 != 0 {
                raise_shutdown();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(h, m, w, l),
    }
}

/// Nur Fenster der obersten Ebene erhalten `WM_ENDSESSION`; dieses wird nie angezeigt.
fn spawn_session_window() {
    std::thread::Builder::new()
        .name("tickly-session".into())
        .spawn(|| unsafe {
            let Ok(hinst) = GetModuleHandleW(None) else { return };
            let class = w!("TicklySessionWatcher");
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: hinst.into(), lpszClassName: class, ..Default::default() };
            RegisterClassW(&wc);
            if CreateWindowExW(WINDOW_EX_STYLE(0), class, w!("Tickly"), WS_OVERLAPPED, 0, 0, 0, 0, None, None, Some(hinst.into()), None).is_err() {
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .ok();
}
