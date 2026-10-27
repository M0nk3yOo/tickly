//! Gesetzliche Feiertage je Bundesland.
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::BTreeMap;

pub const STATES: [(&str, &str); 16] = [
    ("BW", "Baden-Württemberg"), ("BY", "Bayern"), ("BE", "Berlin"), ("BB", "Brandenburg"),
    ("HB", "Bremen"), ("HH", "Hamburg"), ("HE", "Hessen"), ("MV", "Mecklenburg-Vorpommern"),
    ("NI", "Niedersachsen"), ("NW", "Nordrhein-Westfalen"), ("RP", "Rheinland-Pfalz"), ("SL", "Saarland"),
    ("SN", "Sachsen"), ("ST", "Sachsen-Anhalt"), ("SH", "Schleswig-Holstein"), ("TH", "Thüringen"),
];

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("gültiges Datum")
}

/// Ostersonntag (Meeus/Jones/Butcher).
pub fn easter(y: i32) -> NaiveDate {
    let a = y % 19;
    let b = y / 100;
    let c = y % 100;
    let dd = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - dd - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    d(y, month as u32, day as u32)
}

pub fn holidays(y: i32, st: &str) -> BTreeMap<NaiveDate, &'static str> {
    let e = easter(y);
    let off = |n: i64| e + Duration::days(n);
    let mut h: BTreeMap<NaiveDate, &'static str> = BTreeMap::new();
    h.insert(d(y, 1, 1), "Neujahr");
    h.insert(off(-2), "Karfreitag");
    h.insert(off(1), "Ostermontag");
    h.insert(d(y, 5, 1), "Tag der Arbeit");
    h.insert(off(39), "Christi Himmelfahrt");
    h.insert(off(50), "Pfingstmontag");
    h.insert(d(y, 10, 3), "Tag der Deutschen Einheit");
    h.insert(d(y, 12, 25), "1. Weihnachtstag");
    h.insert(d(y, 12, 26), "2. Weihnachtstag");
    if matches!(st, "BW" | "BY" | "ST") { h.insert(d(y, 1, 6), "Heilige Drei Könige"); }
    if matches!(st, "BE" | "MV") { h.insert(d(y, 3, 8), "Internationaler Frauentag"); }
    if matches!(st, "BW" | "BY" | "HE" | "NW" | "RP" | "SL") { h.insert(off(60), "Fronleichnam"); }
    if st == "SL" { h.insert(d(y, 8, 15), "Mariä Himmelfahrt"); }
    if st == "TH" { h.insert(d(y, 9, 20), "Weltkindertag"); }
    if matches!(st, "BB" | "HB" | "HH" | "MV" | "NI" | "SN" | "ST" | "SH" | "TH") { h.insert(d(y, 10, 31), "Reformationstag"); }
    if matches!(st, "BW" | "BY" | "NW" | "RP" | "SL") { h.insert(d(y, 11, 1), "Allerheiligen"); }
    if st == "SN" {
        // Mittwoch vor dem 23. November
        let mut day = d(y, 11, 22);
        while day.weekday() != Weekday::Wed { day -= Duration::days(1); }
        h.insert(day, "Buß- und Bettag");
    }
    h
}

pub fn holiday_name(date: NaiveDate, st: &str) -> Option<&'static str> {
    holidays(date.year(), st).get(&date).copied()
}
