//! Deterministische Demo-Historie (identisch zum JS-Prototyp, gleicher Zufallsgenerator).
use crate::calc::wd;
use crate::model::*;
use chrono::NaiveDate;

/// mulberry32 mit String-Seed, bitgleich zur JS-Variante.
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: &str) -> Rng {
        let mut h: u32 = 1779033703 ^ (seed.chars().map(|c| c.len_utf16()).sum::<usize>() as u32);
        for c in seed.encode_utf16() {
            h = (h ^ c as u32).wrapping_mul(3432918353);
            h = h.rotate_left(13);
        }
        Rng(h)
    }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let a = self.0;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
}

/// Mittlere Brutto-Tageslänge (Minuten) des Hauptbenutzers bei 8 h Soll.
pub const DEFAULT_BASE: f64 = 541.0;

/// Passende Brutto-Länge zu einem Tagessoll (Stunden): Soll + 55 Minuten (ergibt leichte Überstunden).
pub fn base_for_hours(hours: f64) -> f64 {
    hours * 60.0 + 55.0
}

struct Rec {
    start: f64,
    end: f64,
    lunch_at: f64,
    lunch_len: f64,
    short: Vec<(f64, f64)>,
}

fn special_saturday() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 5, 16).unwrap()
}

fn gen_rec(prefix: &str, date: NaiveDate, base: f64) -> Rec {
    if date == special_saturday() {
        return Rec { start: 600.0, end: 820.0, lunch_at: 0.0, lunch_len: 0.0, short: vec![] };
    }
    let mut r = Rng::new(&format!("rec{prefix}{date}"));
    let d = wd(date);
    let start = 445.0 + (r.next() * 100.0).floor();
    let mut gross = base + ((r.next() + r.next() - 1.0) * 90.0 + 0.5).floor();
    if d == 4 {
        gross -= 25.0;
    }
    gross = gross.max(340.0);
    let end = start + gross;
    let lunch_len = 30.0 + (r.next() * 28.0).floor();
    let lunch_at = (start + 200.0).max((end - 120.0).min(700.0 + (r.next() * 70.0).floor()));
    let mut short = vec![];
    if r.next() < 0.5 {
        short.push((start + 90.0 + (r.next() * 60.0).floor(), 4.0 + (r.next() * 8.0).floor()));
    }
    Rec { start, end, lunch_at, lunch_len, short }
}

fn pick_app(r: &mut Rng, prev: Option<&str>, first: bool) -> &'static str {
    let w: &[(&'static str, f64)] = if first {
        &[("mail", 5.0), ("teams", 2.0), ("code", 2.0), ("web", 1.0)]
    } else {
        &[("code", 34.0), ("web", 18.0), ("mail", 10.0), ("teams", 16.0), ("figma", 8.0), ("excel", 8.0), ("other", 6.0)]
    };
    let tot: f64 = w.iter().map(|x| x.1).sum();
    for _ in 0..6 {
        let mut x = r.next() * tot;
        let mut pick = w[0].0;
        for (k, wt) in w {
            x -= wt;
            if x <= 0.0 {
                pick = k;
                break;
            }
        }
        if Some(pick) != prev {
            return pick;
        }
    }
    if prev == Some("code") { "web" } else { "code" }
}

fn build_segs(prefix: &str, date: NaiveDate, base: f64) -> Vec<Seg> {
    let rec = gen_rec(prefix, date, base);
    let mut r = Rng::new(&format!("seg{prefix}{date}"));
    let mut locks: Vec<(f64, f64)> = vec![];
    if rec.lunch_len > 0.0 {
        locks.push((rec.lunch_at, rec.lunch_at + rec.lunch_len));
    }
    for (a, l) in &rec.short {
        locks.push((*a, a + l));
    }
    locks.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut segs: Vec<Seg> = vec![];
    let (mut prev, mut first): (Option<&'static str>, bool) = (None, true);
    let mut fill = |segs: &mut Vec<Seg>, a: f64, b: f64, r: &mut Rng| {
        let mut t = a;
        while b - t > 0.0 {
            let mut len = 10.0 + (r.next() * 60.0).floor();
            if b - t - len < 15.0 {
                len = b - t;
            }
            let app = pick_app(r, prev, first);
            first = false;
            prev = Some(app);
            segs.push(Seg::app(t, t + len, app));
            t += len;
        }
    };
    let mut cur = rec.start;
    for (a, b) in locks {
        fill(&mut segs, cur, a, &mut r);
        segs.push(Seg::locked(a, b));
        cur = b;
    }
    fill(&mut segs, cur, rec.end, &mut r);
    segs
}

/// Historie-Funktion für Demo-Benutzer. `prefix` unterscheidet die Benutzer ("" = Hauptbenutzer),
/// `base` ist die mittlere Brutto-Tageslänge in Minuten (Standard: [`DEFAULT_BASE`]).
pub fn history(prefix: String, base: f64, track_start: NaiveDate, today: NaiveDate) -> impl Fn(NaiveDate) -> Option<Vec<Seg>> {
    move |date| {
        if date < track_start || date >= today {
            return None;
        }
        if wd(date) >= 5 && date != special_saturday() {
            return None;
        }
        Some(build_segs(&prefix, date, base))
    }
}

/// Demo-Historie wie im Prototyp: An Feiertagen, Urlaub, Krank und Zeitausgleich wird nicht gearbeitet.
pub fn history_for(prefix: String, base: f64, track_start: NaiveDate, today: NaiveDate, settings: Settings, absences: Vec<Absence>) -> impl Fn(NaiveDate) -> Option<Vec<Seg>> {
    let h = history(prefix, base, track_start, today);
    move |date| if crate::day_info(date, &settings, &absences).off { None } else { h(date) }
}

/// Beispiel-Arbeitstag des Hauptbenutzers (Montag, 07:52 bis 14:32).
pub fn example_day() -> DayState {
    let o = |f: f64, t: f64, a: &str| Seg::app(f, t, a);
    DayState {
        start: 472.0,
        tracking: true,
        ended: None,
        stop: None,
        declined: None,
        last_beat: None,
        edited: false,
        carried: false,
        night_ok: false,
        planned_end: None,
        away: None,
        last_app: "code".into(),
        segs: vec![
            o(472.0, 485.0, "mail"), o(485.0, 500.0, "teams"), o(500.0, 580.0, "code"), o(580.0, 600.0, "web"),
            o(600.0, 630.0, "teams"), o(630.0, 685.0, "code"), o(685.0, 700.0, "web"), o(700.0, 725.0, "excel"),
            Seg::locked(725.0, 768.0), o(768.0, 785.0, "mail"), o(785.0, 830.0, "code"), Seg::locked(830.0, 842.0),
            Seg { from: 842.0, to: None, kind: SegKind::App, app: Some("code".into()), note: None },
        ],
        events: vec![
            Event { t: 472.0, k: "start".into(), txt: "Windows gestartet · Companion automatisch gestartet · Arbeitstag beginnt".into() },
            Event { t: 725.0, k: "lock".into(), txt: "Bildschirm gesperrt".into() },
            Event { t: 768.0, k: "unlock".into(), txt: "Entsperrt (43 Min. gesperrt)".into() },
            Event { t: 830.0, k: "lock".into(), txt: "Bildschirm gesperrt".into() },
            Event { t: 842.0, k: "unlock".into(), txt: "Entsperrt (12 Min. gesperrt)".into() },
        ],
    }
}

/// Die Abwesenheiten des Demo-Hauptbenutzers.
pub fn example_absences() -> Vec<Absence> {
    let d = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
    let a = |id, kind, f: &str, t: &str, note: &str| Absence { id, kind, from: d(f), to: d(t), note: note.into() };
    vec![
        a(1, AbsType::Vacation, "2026-02-16", "2026-02-20", "Skiurlaub"),
        a(2, AbsType::Sick, "2026-03-10", "2026-03-11", ""),
        a(3, AbsType::Vacation, "2026-04-07", "2026-04-10", "Ostern"),
        a(4, AbsType::Comp, "2026-06-05", "2026-06-05", "Brückentag"),
        a(5, AbsType::Vacation, "2026-07-27", "2026-08-07", "Sommerurlaub"),
        a(6, AbsType::Sick, "2026-09-14", "2026-09-15", ""),
        a(7, AbsType::Vacation, "2026-10-05", "2026-10-09", "Herbstferien"),
        a(8, AbsType::Vacation, "2026-12-28", "2026-12-31", "Zwischen den Jahren"),
    ]
}
