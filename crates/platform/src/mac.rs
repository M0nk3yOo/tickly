//! macOS: aktive App über `NSWorkspace`, Sperre über die Sitzungsdaten, Herunterfahren über die Workspace-Benachrichtigung.
use crate::{raise_shutdown, Platform};
use block2::RcBlock;
use core_foundation::{
    base::TCFType,
    boolean::{CFBoolean, CFBooleanRef},
    dictionary::{CFDictionary, CFDictionaryRef},
    string::CFString,
};
use objc2_app_kit::{NSWorkspace, NSWorkspaceWillPowerOffNotification};
use objc2_foundation::NSNotification;
use std::{ffi::c_void, ptr::NonNull};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

pub struct Mac;

impl Mac {
    pub fn new() -> Mac {
        register_power_off();
        Mac
    }
}

impl Platform for Mac {
    fn os(&self) -> &'static str {
        "macos"
    }

    fn foreground_app(&mut self) -> Option<String> {
        let ws = NSWorkspace::sharedWorkspace();
        let app = ws.frontmostApplication()?;
        app.localizedName().map(|n| n.to_string())
    }

    fn is_locked(&mut self) -> bool {
        // Ohne Sitzung (z. B. Anmeldefenster) liefert das System kein Wörterbuch: gilt als gesperrt.
        unsafe {
            let d = CGSessionCopyCurrentDictionary();
            if d.is_null() {
                return true;
            }
            let dict: CFDictionary<*const c_void, *const c_void> = CFDictionary::wrap_under_create_rule(d);
            let key = CFString::from_static_string("CGSSessionScreenIsLocked");
            match dict.find(key.as_concrete_TypeRef() as *const c_void) {
                Some(v) => bool::from(CFBoolean::wrap_under_get_rule(*v as CFBooleanRef)),
                None => false,
            }
        }
    }
}

/// „Wird heruntergefahren / neu gestartet / abgemeldet“.
fn register_power_off() {
    unsafe {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let block = RcBlock::new(|_n: NonNull<NSNotification>| raise_shutdown());
        let token = center.addObserverForName_object_queue_usingBlock(Some(NSWorkspaceWillPowerOffNotification), None, None, &block);
        // Die Anmeldung gilt für die gesamte Laufzeit des Prozesses.
        std::mem::forget(token);
        std::mem::forget(block);
    }
}
