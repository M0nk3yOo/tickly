//! Berechnung von Tageswerten, Salden und Prognosen.
use crate::day::Policy;
use crate::holidays::{easter, holidays};
use std::collections::BTreeMap;
use crate::model::*;
use chrono::{Datelike, Duration, NaiveDate};
use serde::Serialize;

/// Alles, was für die Berechnung eines Benutzers gebraucht wird. Die Uhrzeit (`now`) kommt von außen.
pub struct Ctx<'a> {
    pub settings: &'a Settings,
    pub absences: &'a [Absence],
    pub today: NaiveDate,
    pub track_start: NaiveDate,
    /// Minuten seit Mitternacht (heute).
    pub now: f64,
    pub day: &'a DayState,
    /// Liefert die erfassten Abschnitte eines vergangenen Tages (None = keine Daten).
    pub history: &'a dyn Fn(NaiveDate) -> Option<Vec<Seg>>,
}

/// Wochentag, 0 = Montag.
pub fn wd(date: NaiveDate) -> usize {
    date.weekday().num_days_from_monday() as usize
}

pub fn rule_break(gross: f64, s: &Settings) -> f64 {
    let mut rules = s.rules.clone();
    rules.sort_by(|a, b| a.after.partial_cmp(&b.after).unwrap_or(std::cmp::Ordering::Equal));
    let mut b = 0.0;
    for r in rules {
        if gross >= r.after * 60.0 {
            b = r.min;
        }
    }
    b
}

/// Alle freien Tage eines Jahres: gesetzliche Feiertage des Bundeslands plus eigene freie Tage.
/// Der Eintrag ist `(Name, eigener_freier_Tag)`; ein gesetzlicher Feiertag hat Vorrang.
pub fn off_days(year: i32, s: &Settings) -> BTreeMap<NaiveDate, (String, bool)> {
    let mut m: BTreeMap<NaiveDate, (String, bool)> = BTreeMap::new();
    let x = &s.extra_days;
    let mut add = |d: Option<NaiveDate>, name: &str| {
        if let Some(d) = d {
            m.entry(d).or_insert_with(|| (name.to_string(), true));
        }
    };
    if x.rosenmontag { add(Some(easter(year) - Duration::days(48)), "Rosenmontag"); }
    if x.heiligabend { add(NaiveDate::from_ymd_opt(year, 12, 24), "Heiligabend"); }
    if x.silvester { add(NaiveDate::from_ymd_opt(year, 12, 31), "Silvester"); }
    for c in &x.custom {
        add(NaiveDate::from_ymd_opt(year, c.month, c.day), c.name.trim());
    }
    for (d, n) in holidays(year, &s.state) {
        m.insert(d, (n.to_string(), false));
    }
    m
}

pub fn off_day_name(date: NaiveDate, s: &Settings) -> Option<String> {
    off_days(date.year(), s).get(&date).map(|(n, _)| n.clone())
}

pub fn day_info(date: NaiveDate, s: &Settings, abs: &[Absence]) -> DayInfo {
    let hol = off_day_name(date, s);
    let ab = abs.iter().find(|a| date >= a.from && date <= a.to).cloned();
    let base = s.hours[wd(date)] * 60.0;
    let (mut soll, mut off, mut kind) = (base, false, None);
    if hol.is_some() {
        soll = 0.0;
        off = true;
        kind = Some(DayKind::Holiday);
    } else if let Some(a) = &ab {
        match a.kind {
            AbsType::Vacation => { soll = 0.0; off = true; kind = Some(DayKind::Vacation); }
            AbsType::Sick => { soll = 0.0; off = true; kind = Some(DayKind::Sick); }
            AbsType::Comp => { off = true; kind = Some(DayKind::Comp); }
        }
    }
    DayInfo { holiday: hol, absence: ab, soll, off, kind, base, weekend: wd(date) >= 5 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Calc {
    pub gross: f64,
    pub locked: f64,
    pub locked_break: f64,
    pub rule: f64,
    pub brk: f64,
    pub net: f64,
    pub counted: f64,
}

/// Offene Abschnitte bis `now` schließen.
pub fn resolve(segs: &[Seg], now: f64) -> Vec<Seg> {
    segs.iter()
        .map(|s| {
            let mut s = s.clone();
            if s.to.is_none() {
                s.to = Some(s.from.max(now));
            }
            s
        })
        .collect()
}

/// Brutto, Pausen und Netto aus aufgelösten Abschnitten.
pub fn calc_segs(segs: &[Seg], s: &Settings) -> Calc {
    let (mut gross, mut locked, mut locked_break) = (0.0, 0.0, 0.0);
    for g in segs {
        if g.kind == SegKind::Off {
            continue;
        }
        let l = g.to.unwrap_or(g.from) - g.from;
        gross += l;
        if g.kind == SegKind::Locked {
            locked += l;
            if l >= s.chunk {
                locked_break += l;
            }
        }
    }
    let rule = rule_break(gross, s);
    let brk = if s.lock_as_break { rule.max(locked_break) } else { rule };
    let net = gross - brk;
    Calc { gross, locked, locked_break, rule, brk, net, counted: if s.mode == Mode::Net { net } else { gross } }
}

pub fn day_stat(c: &Ctx, date: NaiveDate) -> DayStat {
    let info = day_info(date, c.settings, c.absences);
    let mut o = DayStat {
        date, soll: info.soll, info, has: false, live: false, future: date > c.today, start: None, end: None,
        gross: 0.0, locked: 0.0, rule: 0.0, locked_break: 0.0, brk: 0.0, net: 0.0, counted: 0.0,
        diff: Some(0.0), diff_live: Some(0.0), segs: None,
    };
    if o.future {
        o.diff = None;
        o.diff_live = None;
        return o;
    }
    // Gearbeitet wird gezählt, auch an freien Tagen: Das Soll ist dort 0, alles wird zu Mehrzeit.
    let segs: Option<Vec<Seg>> = if date == c.today {
        o.live = true;
        Some(resolve(&c.day.segs, c.now))
    } else {
        (c.history)(date)
    };
    if let Some(s) = segs.filter(|s| !s.is_empty()) {
        let k = calc_segs(&s, c.settings);
        o.gross = k.gross; o.locked = k.locked; o.rule = k.rule; o.locked_break = k.locked_break;
        o.brk = k.brk; o.net = k.net; o.counted = k.counted;
        o.has = true;
        o.start = Some(s[0].from);
        o.end = s.last().and_then(|x| x.to);
        o.segs = Some(s);
    }
    let live_diff = o.counted - o.soll;
    o.diff_live = Some(live_diff);
    // Heute zählt ein offenes Minus erst, wenn der Tag beendet ist.
    o.diff = Some(if o.live && !c.day.is_closed() { live_diff.max(0.0) } else { live_diff });
    o
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RangeStats {
    pub counted: f64,
    pub soll: f64,
    pub diff: f64,
    pub worked: u32,
    pub avg_start: Option<f64>,
    pub vacation: u32,
    pub holiday: u32,
    pub sick: u32,
    pub comp: u32,
}

pub fn range_stats(c: &Ctx, from: NaiveDate, to: NaiveDate) -> RangeStats {
    let mut r = RangeStats::default();
    let (mut sum_start, mut n) = (0.0, 0u32);
    let mut d = from.max(c.track_start);
    while d <= to && d <= c.today {
        let s = day_stat(c, d);
        r.counted += s.counted;
        r.soll += s.soll;
        r.diff += s.diff.unwrap_or(0.0);
        if s.has {
            r.worked += 1;
            sum_start += s.start.unwrap_or(0.0);
            n += 1;
        } else if s.info.base > 0.0 {
            match s.info.kind {
                Some(DayKind::Vacation) => r.vacation += 1,
                Some(DayKind::Holiday) => r.holiday += 1,
                Some(DayKind::Sick) => r.sick += 1,
                Some(DayKind::Comp) => r.comp += 1,
                None => {}
            }
        }
        d += Duration::days(1);
    }
    r.avg_start = if n > 0 { Some(sum_start / n as f64) } else { None };
    r
}

/// Kumulierter Saldo (Minuten) am Ende jedes Tages, beginnend mit dem Übertrag.
pub fn balance_series(c: &Ctx) -> Vec<(NaiveDate, f64)> {
    let mut bal = c.settings.start_bal * 60.0;
    let mut out = Vec::new();
    let mut d = c.track_start;
    while d <= c.today {
        bal += day_stat(c, d).diff.unwrap_or(0.0);
        out.push((d, bal));
        d += Duration::days(1);
    }
    out
}

pub fn total_balance(c: &Ctx) -> f64 {
    balance_series(c).last().map(|x| x.1).unwrap_or(c.settings.start_bal * 60.0)
}

/// Arbeitstage eines Zeitraums (ohne Wochenenden, freie Wochentage und Feiertage).
pub fn abs_workdays(from: NaiveDate, to: NaiveDate, s: &Settings) -> u32 {
    let mut n = 0;
    let mut d = from;
    while d <= to {
        if s.hours[wd(d)] > 0.0 && off_day_name(d, s).is_none() {
            n += 1;
        }
        d += Duration::days(1);
    }
    n
}

pub fn vacation_summary(c: &Ctx, total: u32) -> VacationSummary {
    let (mut taken, mut planned) = (0, 0);
    for a in c.absences.iter().filter(|a| a.kind == AbsType::Vacation && a.from.year() == c.today.year()) {
        let n = abs_workdays(a.from, a.to, c.settings);
        if a.to < c.today { taken += n } else { planned += n }
    }
    VacationSummary { total, taken, planned, rest: total as i32 - taken as i32 - planned as i32 }
}

pub fn next_holidays(c: &Ctx, n: usize) -> Vec<HolidayEntry> {
    let mut out = Vec::new();
    for y in [c.today.year(), c.today.year() + 1] {
        for (d, (name, extra)) in off_days(y, c.settings) {
            if d > c.today {
                out.push(HolidayEntry { date: d, name, extra });
            }
        }
    }
    out.truncate(n);
    out
}

pub fn year_holidays(year: i32, s: &Settings) -> Vec<HolidayEntry> {
    off_days(year, s).into_iter().map(|(date, (name, extra))| HolidayEntry { date, name, extra }).collect()
}

/// Voraussichtliches Tagesende (Minuten), wenn ab jetzt durchgehend gearbeitet wird.
pub fn forecast_end(c: &Ctx, st: &DayStat) -> Option<f64> {
    if !c.day.tracking || st.soll <= 0.0 || st.counted >= st.soll {
        return None;
    }
    let base = st.segs.clone()?;
    for m in 1..=720 {
        let mut segs = base.clone();
        segs.push(Seg::app(c.now, c.now + m as f64, "other"));
        if calc_segs(&segs, c.settings).counted >= st.soll {
            return Some(c.now + m as f64);
        }
    }
    None
}

/// Wie der Companion beim Start bzw. in der laufenden Prüfung entscheidet (Zeitfenster, Feiertage, Urlaub).
pub fn track_policy(s: &Settings, absences: &[Absence], date: NaiveDate, now: f64) -> Policy {
    let mut p = policy_for(&s.auto_track, day_info(date, s, absences).off, date, now);
    p.lead = s.setup_before.clamp(0.0, 120.0);
    p.tail = s.setup_after.clamp(0.0, 120.0);
    p.join = if s.lock_as_break { s.chunk.clamp(0.0, 240.0) } else { 0.0 };
    p
}

/// Wie [`track_policy`], wenn schon bekannt ist, ob der Tag frei ist (der Companion kennt keine Abwesenheiten).
pub fn policy_for(a: &AutoTrack, off: bool, date: NaiveDate, now: f64) -> Policy {
    let auto_from = if a.enabled && !off { a.window_start(date.weekday().num_days_from_monday() as usize, now) } else { None };
    Policy { auto_from, ask: a.ask, lead: 0.0, tail: 0.0, join: 0.0 }
}

/* ---------- Plausibilitätsprüfung nach Mitternacht ---------- */
/// So lange (Minuten nach Mitternacht) wird ein weiterlaufendes Tracking nicht hinterfragt: kurz über Mitternacht zu arbeiten ist normal.
pub const NIGHT_GRACE: f64 = 60.0;
/// So viele frühere Tage mit Arbeit bis zu dieser Uhrzeit machen eine Nachtschicht zum Muster.
pub const NIGHT_PATTERN_DAYS: usize = 3;
/// So weit (in Tagen) wird zurückgeschaut.
pub const NIGHT_LOOKBACK: i64 = 60;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NightCheck {
    /// Seit wann das Tracking ohne Unterbrechung läuft: Minuten seit Mitternacht des Vortags.
    pub since: f64,
    /// So viele Stunden läuft es schon ununterbrochen.
    pub hours: f64,
    /// Der Vortag
    pub yesterday: NaiveDate,
}

/// Läuft ein Tracking über Mitternacht weiter, länger als die Schonfrist, und passt das nicht zu den bisherigen Tagen
/// (weniger als `NIGHT_PATTERN_DAYS` Tage mit Arbeit ab Mitternacht bis mindestens zu dieser Uhrzeit), soll der Benutzer gefragt werden.
pub fn night_check(c: &Ctx) -> Option<NightCheck> {
    let d = c.day;
    if !d.tracking || !d.carried || d.night_ok || c.now < NIGHT_GRACE { return None }
    let usual = (2..=NIGHT_LOOKBACK)
        .filter_map(|k| (c.history)(c.today - Duration::days(k)))
        .filter(|segs| segs.iter().any(|g| g.kind != SegKind::Off && g.from <= 1.0 && g.to.unwrap_or(g.from) >= c.now - 5.0))
        .count();
    if usual >= NIGHT_PATTERN_DAYS { return None }
    let yesterday = c.today - Duration::days(1);
    // Beginn des ununterbrochenen Laufs: am Ende des Vortags rückwärts über lückenlos anschließende Abschnitte
    let mut since = 0.0;
    if let Some(segs) = (c.history)(yesterday) {
        let mut edge = 1440.0;
        for g in segs.iter().rev() {
            if g.kind == SegKind::Off || (g.to.unwrap_or(g.from) - edge).abs() > 1.0 { break }
            since = g.from;
            edge = g.from;
        }
    }
    Some(NightCheck { since, hours: (1440.0 - since + c.now) / 60.0, yesterday })
}
