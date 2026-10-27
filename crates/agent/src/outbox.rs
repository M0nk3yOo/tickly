//! Dauerhafte Warteschlange: Ereignisse überstehen Neustart und Netzausfall. `seq` steigt immer weiter.
use crate::protocol::QueuedEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, path::PathBuf};

#[derive(Default, Serialize, Deserialize)]
struct Disk {
    next_seq: i64,
    events: VecDeque<QueuedEvent>,
}

pub struct Outbox {
    path: Option<PathBuf>,
    d: Disk,
}

impl Outbox {
    /// Ohne Datei (Tests).
    pub fn in_memory() -> Outbox {
        Outbox { path: None, d: Disk { next_seq: 1, events: VecDeque::new() } }
    }

    /// Lädt die Datei oder beginnt leer. Eine beschädigte Datei wird nicht übernommen.
    pub fn open(path: PathBuf) -> Outbox {
        let d = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Disk>(&b).ok()).unwrap_or(Disk { next_seq: 1, events: VecDeque::new() });
        Outbox { path: Some(path), d: Disk { next_seq: d.next_seq.max(1), ..d } }
    }

    pub fn len(&self) -> usize {
        self.d.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.d.events.is_empty()
    }
    pub fn next_seq(&self) -> i64 {
        self.d.next_seq
    }

    /// Der Server kennt höhere Nummern als wir (z. B. nach verlorener Datei): nicht wieder unten anfangen.
    pub fn ensure_seq_after(&mut self, server_last: i64) {
        if self.d.next_seq <= server_last {
            self.d.next_seq = server_last + 1;
            self.save();
        }
    }

    pub fn push(&mut self, kind: &str, at: DateTime<Utc>, app: Option<String>, os: Option<String>, answer: Option<String>) {
        // Lebenszeichen ersetzen sich gegenseitig, damit nach langem Netzausfall nicht tausende anfallen.
        if kind == "beat" && self.d.events.back().is_some_and(|e| e.kind == "beat") {
            self.d.events.pop_back();
        }
        let seq = self.d.next_seq;
        self.d.next_seq += 1;
        self.d.events.push_back(QueuedEvent { seq, kind: kind.into(), at, app, os, answer });
        self.save();
    }

    pub fn batch(&self, n: usize) -> Vec<QueuedEvent> {
        self.d.events.iter().take(n).cloned().collect()
    }

    /// Alles bis einschließlich `last_seq` ist beim Server angekommen.
    pub fn ack(&mut self, last_seq: i64) {
        let before = self.d.events.len();
        while self.d.events.front().is_some_and(|e| e.seq <= last_seq) {
            self.d.events.pop_front();
        }
        if self.d.events.len() != before {
            self.save();
        }
    }

    fn save(&self) {
        let Some(p) = &self.path else { return };
        let tmp = p.with_extension("tmp");
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&tmp, serde_json::to_vec(&self.d).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, p);
        }
    }
}
