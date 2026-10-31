use crate::calc::*;
use crate::demo;
use crate::holidays::{easter, holidays};
use crate::model::*;
use chrono::NaiveDate;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

struct Fixture {
    settings: Settings,
    absences: Vec<Absence>,
    day: DayState,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture { settings: Settings::default(), absences: demo::example_absences(), day: demo::example_day() }
    }
    fn with<R>(&self, now: f64, f: impl FnOnce(&Ctx) -> R) -> R {
        self.with_day(&self.day, now, f)
    }
    fn with_day<R>(&self, day: &DayState, now: f64, f: impl FnOnce(&Ctx) -> R) -> R {
        let hist = demo::history_for(String::new(), demo::DEFAULT_BASE, d("2026-01-05"), d("2026-10-26"), self.settings.clone(), self.absences.clone());
        let c = Ctx { settings: &self.settings, absences: &self.absences, today: d("2026-10-26"), track_start: d("2026-01-05"), now, day, history: &hist };
        f(&c)
    }
}

fn hm(m: f64) -> String {
    let m = m.round().abs() as i64;
    format!("{}:{:02}", m / 60, m % 60)
}

#[test]
fn easter_dates() {
    assert_eq!(easter(2026), d("2026-04-05"));
    assert_eq!(easter(2025), d("2025-04-20"));
    assert_eq!(easter(2024), d("2024-03-31"));
}

#[test]
fn holidays_nrw_2026() {
    let h = holidays(2026, "NW");
    let got: Vec<String> = h.iter().map(|(d, n)| format!("{d} {n}")).collect();
    assert_eq!(got, vec![
        "2026-01-01 Neujahr", "2026-04-03 Karfreitag", "2026-04-06 Ostermontag", "2026-05-01 Tag der Arbeit",
        "2026-05-14 Christi Himmelfahrt", "2026-05-25 Pfingstmontag", "2026-06-04 Fronleichnam",
        "2026-10-03 Tag der Deutschen Einheit", "2026-11-01 Allerheiligen", "2026-12-25 1. Weihnachtstag", "2026-12-26 2. Weihnachtstag",
    ]);
}

#[test]
fn holidays_differ_by_state() {
    assert!(holidays(2026, "BY").contains_key(&d("2026-01-06")));
    assert!(!holidays(2026, "NW").contains_key(&d("2026-01-06")));
    assert!(holidays(2026, "BE").contains_key(&d("2026-03-08")));
    assert!(holidays(2026, "SN").contains_key(&d("2026-11-18"))); // Buß- und Bettag
    assert!(holidays(2026, "SN").contains_key(&d("2026-10-31")));
}

#[test]
fn rng_matches_javascript() {
    // Referenzwerte aus dem JS-Prototyp: rng('rec2026-03-02')
    let mut r = demo::Rng::new("rec2026-03-02");
    let v: Vec<String> = (0..3).map(|_| format!("{:.10}", r.next())).collect();
    assert_eq!(v, ["0.4014408563", "0.9100821293", "0.6344327389"]);
}

#[test]
fn today_net_calculation_matches_prototype() {
    let f = Fixture::new();
    f.with(872.0, |c| {
        let t = day_stat(c, c.today);
        assert_eq!(hm(t.gross), "6:40");
        assert_eq!(hm(t.locked), "0:55");
        assert_eq!(hm(t.locked_break), "0:43"); // die 12-Minuten-Sperre ist kürzer als 15 Min.
        assert_eq!(hm(t.rule), "0:30");
        assert_eq!(hm(t.brk), "0:43"); // echte Pause > Mindestpause
        assert_eq!(hm(t.counted), "5:57");
        assert!(t.live && t.has);
        // heute offen: kein Minus im Saldo
        assert_eq!(t.diff, Some(0.0));
        assert_eq!(t.diff_live.map(hm).unwrap(), "2:03");
        assert_eq!(forecast_end(c, &t).map(hm).unwrap(), "16:35");
    });
}

#[test]
fn gross_mode_does_not_deduct_breaks() {
    let mut f = Fixture::new();
    f.settings.mode = Mode::Gross;
    f.with(872.0, |c| {
        let t = day_stat(c, c.today);
        assert_eq!(hm(t.counted), "6:40");
        assert_eq!(hm(t.net), "5:57");
    });
}

#[test]
fn rule_break_applies_without_lock_data() {
    let s = Settings::default();
    assert_eq!(rule_break(5.0 * 60.0, &s), 0.0);
    assert_eq!(rule_break(6.0 * 60.0, &s), 30.0);
    assert_eq!(rule_break(9.0 * 60.0, &s), 45.0);
    let mut s2 = s.clone();
    s2.lock_as_break = false;
    let segs = vec![Seg::app(0.0, 300.0, "code"), Seg::locked(300.0, 360.0), Seg::app(360.0, 480.0, "code")];
    let k = calc_segs(&segs, &s2);
    assert_eq!(k.brk, 30.0); // nur die Regel, die echte Stunde Sperre zählt nicht
    let k = calc_segs(&segs, &s);
    assert_eq!(k.brk, 60.0); // echte Pause ersetzt die Regel
}

#[test]
fn off_gap_is_not_counted() {
    let segs = vec![Seg::app(0.0, 100.0, "code"), Seg { from: 100.0, to: Some(200.0), kind: SegKind::Off, app: None, note: None }, Seg::app(200.0, 260.0, "code")];
    let k = calc_segs(&segs, &Settings::default());
    assert_eq!(k.gross, 160.0);
}

#[test]
fn balances_match_prototype() {
    let f = Fixture::new();
    f.with(872.0, |c| {
        assert_eq!(hm(total_balance(c)), "19:53");
        assert!(total_balance(c) > 0.0);
        let week = range_stats(c, d("2026-10-19"), d("2026-10-25"));
        assert_eq!((week.diff).round() as i64, 24);
        let month = range_stats(c, d("2026-10-01"), d("2026-10-26"));
        assert_eq!(month.diff.round() as i64, 53);
        assert_eq!((month.vacation, month.holiday), (5, 0)); // 3.10. ist ein Samstag und zählt nicht
        let v = vacation_summary(c, 30);
        assert_eq!((v.taken, v.planned, v.rest), (24, 4, 2));
    });
}

#[test]
fn absences_do_not_distort_overtime() {
    let f = Fixture::new();
    f.with(872.0, |c| {
        // Urlaub 5.–9. Oktober: Soll 0, kein Minus
        for day in ["2026-10-05", "2026-10-07", "2026-10-09"] {
            let s = day_stat(c, d(day));
            assert_eq!((s.soll, s.counted, s.diff), (0.0, 0.0, Some(0.0)));
            assert!(!s.has);
        }
        // Feiertag auf Werktag
        let h = day_stat(c, d("2026-04-06"));
        assert_eq!(h.info.kind, Some(DayKind::Holiday));
        assert_eq!(h.diff, Some(0.0));
        // Krankheit
        assert_eq!(day_stat(c, d("2026-03-10")).diff, Some(0.0));
        // Zeitausgleich baut ab
        let z = day_stat(c, d("2026-06-05"));
        assert_eq!(z.info.kind, Some(DayKind::Comp));
        assert_eq!(z.diff, Some(-480.0));
    });
}

#[test]
fn working_on_a_holiday_counts_as_overtime() {
    // Samstag-Einsatz 16.5.: Soll 0 → alles ist Mehrzeit
    let f = Fixture::new();
    f.with(872.0, |c| {
        let s = day_stat(c, d("2026-05-16"));
        assert!(s.has && s.soll == 0.0);
        assert_eq!(s.diff, Some(s.counted));
        assert!(s.counted > 0.0);
    });
}

#[test]
fn day_events_behave_like_the_companion() {
    let mut day = DayState::boot(480.0, "Windows");
    assert_eq!(day.track(), crate::Track::Running);
    day.lock(500.0).unwrap();
    assert!(day.lock(501.0).is_err());
    assert!(day.switch_app("code", 502.0).is_err());
    day.unlock(530.0).unwrap();
    day.switch_app("code", 540.0).unwrap();
    day.quit(600.0).unwrap();
    assert!(day.quit(601.0).is_err());
    assert_eq!(day.ended, Some(600.0));
    day.start_tracking(660.0).unwrap();
    let off: Vec<_> = day.segs.iter().filter(|s| s.kind == SegKind::Off).collect();
    assert_eq!(off.len(), 1);
    assert_eq!((off[0].from, off[0].to), (600.0, Some(660.0)));
    let k = calc_segs(&crate::resolve(&day.segs, 700.0), &Settings::default());
    assert_eq!(k.gross, (600.0 - 480.0) + 40.0);
}

#[test]
fn settings_and_stats_roundtrip_json() {
    let s = Settings::default();
    let j = serde_json::to_string(&s).unwrap();
    assert!(j.contains("\"lockAsBreak\":true") && j.contains("\"mode\":\"net\""));
    let back: Settings = serde_json::from_str(&j).unwrap();
    assert_eq!(s, back);
    let a: Absence = serde_json::from_str(r#"{"type":"comp","from":"2026-11-02","to":"2026-11-02"}"#).unwrap();
    assert_eq!((a.id, a.kind), (0, AbsType::Comp));
}

fn pol(auto: Option<f64>, ask: bool) -> crate::Policy {
    crate::Policy { auto_from: auto, ask }
}

#[test]
fn ending_the_tracking_in_the_companion_closes_the_day_and_start_continues_it() {
    // Läuft: Arbeitsende jetzt
    let mut day = DayState::boot(480.0, "Windows");
    day.end_tracking(600.0).unwrap();
    assert_eq!(day.track(), crate::Track::Stopped);
    assert!(day.is_closed() && day.ended == Some(600.0));
    assert!(day.end_tracking(601.0).is_err(), "nichts mehr zu beenden");
    // Später wieder starten: die Lücke zählt nicht
    day.start_tracking(660.0).unwrap();
    assert!(day.tracking && day.events.last().unwrap().txt.contains("Lücke"));

    // Pausiert: das Arbeitsende ist der Beginn der Pause
    let mut day = DayState::boot(480.0, "Windows");
    day.pause(540.0).unwrap();
    day.end_tracking(600.0).unwrap();
    assert!(day.is_closed() && day.ended == Some(540.0), "{:?}", day.ended);
    assert_eq!(day.track(), crate::Track::Stopped);

    // Ohne laufendes Tracking (Rückfrage offen) gibt es nichts zu beenden
    let mut day = DayState::new_day(480.0, "Windows", &pol(None, true));
    assert!(day.end_tracking(500.0).is_err());
}

#[test]
fn pause_stops_counting_and_resumes_without_gap_time() {
    let mut day = DayState::boot(480.0, "Windows");
    day.pause(540.0).unwrap();
    assert_eq!(day.track(), crate::Track::Paused);
    assert!(!day.is_closed(), "eine Pause beendet den Tag nicht");
    assert!(day.pause(541.0).is_err() && day.lock(541.0).is_err());
    day.start_tracking(600.0).unwrap();
    assert_eq!(day.track(), crate::Track::Running);
    let k = calc_segs(&crate::resolve(&day.segs, 660.0), &Settings::default());
    assert_eq!(k.gross, 60.0 + 60.0, "die Stunde Pause zählt nicht");
}

#[test]
fn restart_of_the_pc_resumes_tracking_that_was_running() {
    let mut day = DayState::boot(480.0, "Windows");
    day.shutdown(720.0).unwrap();
    assert_eq!(day.track(), crate::Track::Stopped);
    // Auch außerhalb jedes Zeitfensters und ohne Rückfrage: Es lief vorher, also läuft es weiter.
    day.on_boot(735.0, "Windows", &pol(None, false));
    assert_eq!(day.track(), crate::Track::Running);
    assert_eq!(day.segs.iter().filter(|s| s.kind == SegKind::Off).count(), 1);
    let k = calc_segs(&crate::resolve(&day.segs, 800.0), &Settings::default());
    assert_eq!(k.gross, (720.0 - 480.0) + (800.0 - 735.0));
}

#[test]
fn boot_after_a_proper_quit_follows_the_policy() {
    let mk = || {
        let mut d = DayState::boot(480.0, "Windows");
        d.quit(720.0).unwrap();
        d
    };
    let mut d = mk();
    d.on_boot(800.0, "Windows", &pol(None, true));
    assert_eq!(d.track(), crate::Track::Asking);
    assert!(d.is_closed(), "bis zur Antwort gilt der Tag als beendet");
    let mut d = mk();
    d.on_boot(800.0, "Windows", &pol(None, false));
    assert_eq!(d.track(), crate::Track::Idle);
    let mut d = mk();
    d.on_boot(800.0, "Windows", &pol(Some(360.0), true));
    assert_eq!(d.track(), crate::Track::Running);
}

#[test]
fn new_day_asks_or_starts_automatically() {
    let d = DayState::new_day(478.0, "Windows", &pol(Some(360.0), true));
    assert_eq!(d.track(), crate::Track::Running);
    assert_eq!(d.segs[0].from, 478.0);
    let mut d = DayState::new_day(840.0, "Windows", &pol(None, true));
    assert_eq!(d.track(), crate::Track::Asking);
    assert!(d.segs.is_empty() && !d.is_closed());
    // „ab Rechnerstart“ rechnet rückwirkend
    d.answer(870.0, "boot").unwrap();
    assert_eq!(d.segs[0].from, 840.0);
    let mut d = DayState::new_day(840.0, "Windows", &pol(None, true));
    d.answer(870.0, "work").unwrap();
    assert_eq!(d.segs[0].from, 870.0);
    assert!(d.answer(871.0, "work").is_err());
}

#[test]
fn declining_the_question_is_respected_until_the_next_window_starts() {
    let mut d = DayState::new_day(840.0, "Windows", &pol(None, true));
    d.answer(850.0, "no").unwrap();
    assert_eq!(d.track(), crate::Track::Idle);
    // Fenster 14:00 begonnen, Antwort kam später: nicht überstimmen
    assert!(!d.auto_tick(851.0, &pol(Some(840.0), true)));
    // Ein späteres Fenster (z. B. 15:00) startet das Tracking
    assert!(d.auto_tick(900.0, &pol(Some(900.0), true)));
    assert_eq!(d.track(), crate::Track::Running);
    // Pausiert wird nie überstimmt
    d.pause(950.0).unwrap();
    assert!(!d.auto_tick(960.0, &pol(Some(900.0), true)));
}

#[test]
fn auto_windows_respect_weekdays_and_days_off() {
    let mut s = Settings::default();
    s.auto_track.windows[0].from = 420.0;
    s.auto_track.windows[0].to = 600.0;
    let abs = vec![Absence { id: 1, kind: AbsType::Vacation, from: d("2026-11-03"), to: d("2026-11-03"), note: String::new() }];
    let p = |date: &str, now: f64| track_policy(&s, &abs, d(date), now);
    assert_eq!(p("2026-11-02", 478.0).auto_from, Some(420.0));
    assert_eq!(p("2026-11-02", 600.0).auto_from, None, "Fenster endet exklusiv");
    assert_eq!(p("2026-11-02", 400.0).auto_from, None);
    assert_eq!(p("2026-11-07", 478.0).auto_from, None, "Samstag nicht im Fenster");
    assert_eq!(p("2026-11-03", 478.0).auto_from, None, "Urlaub: kein Auto-Start");
    s.auto_track.enabled = false;
    assert_eq!(track_policy(&s, &abs, d("2026-11-02"), 478.0).auto_from, None);
}

#[test]
fn old_settings_without_auto_track_still_load() {
    let mut v = serde_json::to_value(Settings::default()).unwrap();
    v.as_object_mut().unwrap().remove("autoTrack");
    let s: Settings = serde_json::from_value(v).unwrap();
    assert_eq!(s.auto_track, AutoTrack::default());
}

#[test]
fn paused_day_never_shows_a_minus_but_a_quit_day_does() {
    let f = Fixture::new();
    let mut day = DayState::boot(480.0, "Windows");
    day.pause(540.0).unwrap();
    f.with_day(&day, 900.0, |c| assert_eq!(day_stat(c, c.today).diff, Some(0.0)));
    day.start_tracking(560.0).unwrap();
    day.quit(600.0).unwrap();
    f.with_day(&day, 900.0, |c| assert!(day_stat(c, c.today).diff.unwrap() < 0.0));
}

#[test]
fn extra_days_off_do_not_create_minus_and_are_opt_in() {
    let mut s = Settings::default();
    let rosenmontag = d("2026-02-16");
    assert_eq!(day_info(rosenmontag, &s, &[]).soll, 480.0, "ohne Einstellung ein normaler Arbeitstag");
    s.extra_days.rosenmontag = true;
    s.extra_days.heiligabend = true;
    s.extra_days.custom.push(CustomDay { month: 6, day: 12, name: "Brückentag".into() });
    let i = day_info(rosenmontag, &s, &[]);
    assert_eq!((i.soll, i.off, i.kind, i.holiday.as_deref()), (0.0, true, Some(DayKind::Holiday), Some("Rosenmontag")));
    assert_eq!(day_info(d("2026-12-24"), &s, &[]).soll, 0.0);
    assert_eq!(day_info(d("2026-06-12"), &s, &[]).holiday.as_deref(), Some("Brückentag"));
    // Rosenmontag ist kein Urlaubstag: er wird bei Urlaub nicht mitgezählt
    assert_eq!(abs_workdays(d("2026-02-16"), d("2026-02-20"), &s), 4);
    // Zählt als freier Tag, taucht in der Liste als „eigener“ Tag auf
    let list = year_holidays(2026, &s);
    assert!(list.iter().any(|h| h.name == "Rosenmontag" && h.extra));
    assert!(list.iter().any(|h| h.name == "Neujahr" && !h.extra));
    // Ein gesetzlicher Feiertag hat Vorrang vor einem gleichen eigenen Tag
    s.extra_days.custom.push(CustomDay { month: 12, day: 25, name: "Eigener".into() });
    assert_eq!(day_info(d("2026-12-25"), &s, &[]).holiday.as_deref(), Some("1. Weihnachtstag"));
}

#[test]
fn crash_without_shutdown_ends_at_last_heartbeat() {
    let mut day = DayState::boot(480.0, "Windows");
    day.beat(600.0, &pol(None, true));
    day.beat(630.0, &pol(None, true));
    // Rechner stürzt ab, nach dem Start (14:00) kommt „boot“ ohne vorheriges „shutdown“
    assert_eq!(day.finalize().last().unwrap().to, Some(630.0), "offener Abschnitt endet beim letzten Lebenszeichen");
    day.on_boot(840.0, "Windows", &pol(None, true));
    assert_eq!(day.track(), crate::Track::Running);
    let off: Vec<_> = day.segs.iter().filter(|s| s.kind == SegKind::Off).collect();
    assert_eq!((off[0].from, off[0].to), (630.0, Some(840.0)));
    let k = calc_segs(&crate::resolve(&day.segs, 900.0), &Settings::default());
    assert_eq!(k.gross, (630.0 - 480.0) + 60.0);
}

#[test]
fn app_names_are_free_form_and_classified() {
    let mut day = DayState::boot(480.0, "Windows");
    day.switch_app("Google Chrome", 490.0).unwrap();
    assert!(day.switch_app("  ", 491.0).is_err());
    assert_eq!(crate::app_glyph("Google Chrome"), "web");
    assert_eq!(crate::app_glyph("Microsoft Teams"), "teams");
    assert_eq!(crate::app_glyph("Search Everything"), "other");
    assert_eq!(crate::app_label("code"), "VS Code");
    assert_eq!(crate::app_label("Notepad++"), "Notepad++");
}

fn web_day() -> DayState {
    let mut day = DayState::empty(8.0 * 60.0);
    day.web_start(8.0 * 60.0).unwrap();
    day
}

#[test]
fn web_tracking_starts_and_stops_without_companion() {
    let mut day = web_day();
    assert!(day.tracking && day.track() == crate::Track::Running);
    assert_eq!(day.web_start(500.0), Err("Tracking läuft bereits."));
    day.stop_web(17.0 * 60.0, 17.0 * 60.0).unwrap();
    assert!(!day.tracking && day.is_closed());
    assert_eq!(day.ended, Some(1020.0));
    assert_eq!(day.segs.len(), 1);
    assert_eq!(day.segs[0].to, Some(1020.0));
    assert_eq!(day.events.last().unwrap().k, "stop");
    // beendet: nichts mehr zu beenden, aber wieder startbar (Lücke zählt nicht)
    assert!(day.stop_web(1100.0, 1100.0).is_err());
    day.web_start(18.0 * 60.0).unwrap();
    assert_eq!(day.segs.len(), 3);
    assert_eq!(day.segs[1].kind, SegKind::Off);
}

#[test]
fn web_stop_with_manual_end_cuts_later_activity() {
    // 08:00 an, 12:00–12:30 Pause, 12:30 fortgesetzt, jetzt 18:30
    let mut day = web_day();
    day.pause(12.0 * 60.0).unwrap();
    day.web_start(12.5 * 60.0).unwrap();
    let now = 18.5 * 60.0;
    // Ende in der Lücke: Arbeitszeit endet am Beginn der Pause
    let mut a = day.clone();
    a.stop_web(now, 12.25 * 60.0).unwrap();
    assert_eq!(a.ended, Some(720.0));
    assert_eq!(a.segs.len(), 1);
    // Ende nach der Pause: der laufende Abschnitt wird gekürzt
    let mut b = day.clone();
    b.stop_web(now, 16.75 * 60.0).unwrap();
    assert_eq!(b.ended, Some(1005.0));
    assert_eq!(b.segs.last().unwrap().to, Some(1005.0));
    assert!(b.events.iter().all(|e| e.t <= 1005.0));
    assert!(b.events.last().unwrap().txt.contains("nachträglich auf 16:45"));
    // Vor dem Beginn oder in der Zukunft geht nicht
    assert!(day.clone().stop_web(now, 7.0 * 60.0).unwrap_err().contains("nach dem Arbeitsbeginn"));
    assert!(day.clone().stop_web(now, now + 5.0).unwrap_err().contains("Zukunft"));
    // Das Ergebnis rechnet normal: 08:00–16:45 = 8:45 brutto
    let fx = Fixture::new();
    let s = fx.with_day(&b, now, |c| day_stat(c, c.today));
    assert!((s.gross - (12.0 * 60.0 - 8.0 * 60.0 + (16.75 - 12.5) * 60.0)).abs() < 1e-6);
}

#[test]
fn web_stop_from_pause_ends_at_pause_start() {
    let mut day = web_day();
    day.pause(15.0 * 60.0).unwrap();
    day.stop_web(16.0 * 60.0, 16.0 * 60.0).unwrap();
    assert_eq!(day.ended, Some(900.0));
    assert!(day.is_closed());
    assert!(day.events.last().unwrap().txt.contains("Beginn der Pause"));
}

/// Tracking von gestern 14:00 bis jetzt; `usual` frühere Nächte mit Arbeit bis 02:00
fn night_ctx(usual: usize, now: f64, f: impl FnOnce(&Ctx) -> Option<NightCheck>) -> Option<NightCheck> {
    let s = Settings::default();
    let today = d("2026-10-26");
    let mut day = DayState::boot(14.0 * 60.0, "Windows").continued("Windows");
    day.carried = true;
    let hist = move |dt: NaiveDate| -> Option<Vec<Seg>> {
        let k = (today - dt).num_days();
        if k == 1 { return Some(vec![Seg::app(14.0 * 60.0, 1440.0, "code")]) }
        if (2..2 + usual as i64).contains(&k) { return Some(vec![Seg::app(0.0, 120.0, "code"), Seg::app(480.0, 900.0, "code")]) }
        None
    };
    let c = Ctx { settings: &s, absences: &[], today, track_start: d("2026-01-05"), now, day: &day, history: &hist };
    f(&c)
}

#[test]
fn night_check_asks_when_it_does_not_fit_the_pattern() {
    let q = night_ctx(0, 180.0, |c| night_check(c)).expect("fragt nach");
    assert_eq!(q.since, 14.0 * 60.0);
    assert_eq!(q.hours, 10.0 + 3.0);
    // bis zur Schonfrist wird nicht gefragt
    assert!(night_ctx(0, 59.0, |c| night_check(c)).is_none());
    // zwei frühere Nächte reichen nicht als Muster, drei schon
    assert!(night_ctx(2, 100.0, |c| night_check(c)).is_some());
    assert!(night_ctx(3, 100.0, |c| night_check(c)).is_none());
    // das Muster gilt nur bis zur gewohnten Uhrzeit: um 04:00 war früher nie jemand dabei
    assert!(night_ctx(3, 240.0, |c| night_check(c)).is_some());
}

#[test]
fn night_check_is_quiet_after_confirmation_or_without_carry_over() {
    let s = Settings::default();
    let today = d("2026-10-26");
    let none = |_: NaiveDate| -> Option<Vec<Seg>> { None };
    let mut day = DayState::boot(0.0, "Windows").continued("Windows");
    let ask = |day: &DayState| night_check(&Ctx { settings: &s, absences: &[], today, track_start: today, now: 200.0, day, history: &none });
    assert!(ask(&day).is_some());
    day.confirm_night(200.0).unwrap();
    assert!(ask(&day).is_none(), "bestätigt");
    let fresh = DayState::boot(0.0, "Windows");
    assert!(ask(&fresh).is_none(), "Rechnerstart nach Mitternacht ist keine Übernahme");
    let mut stopped = DayState::boot(0.0, "Windows").continued("Windows");
    stopped.quit(150.0).unwrap();
    assert!(ask(&stopped).is_none(), "beendet");
}
