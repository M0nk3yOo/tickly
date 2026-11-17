'use strict';
/* Tickly – Web-Oberfläche · Hilfsfunktionen und Zustand.
   Alle Berechnungen (Soll/Ist, Pausen, Feiertage, Salden) macht der Server (Rust-Crate tickly-core);
   hier wird nur angezeigt und über den Zeitraum summiert. */

/* ---------- Datum & Format ---------- */
let TODAY = '2026-10-26';
let TRACK_START = '2026-01-05';
const pad = n => String(n).padStart(2, '0');
const parseD = s => { const [y, m, d] = s.split('-').map(Number); return new Date(y, m - 1, d, 12); };
const fmtD = d => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const addDays = (s, n) => { const d = parseD(s); d.setDate(d.getDate() + n); return fmtD(d); };
const dow = s => (parseD(s).getDay() + 6) % 7; // 0 = Montag
const diffDays = (a, b) => Math.round((parseD(b) - parseD(a)) / 864e5);
const MONTHS = ['Januar', 'Februar', 'März', 'April', 'Mai', 'Juni', 'Juli', 'August', 'September', 'Oktober', 'November', 'Dezember'];
const MON_S = ['Jan', 'Feb', 'Mär', 'Apr', 'Mai', 'Jun', 'Jul', 'Aug', 'Sep', 'Okt', 'Nov', 'Dez'];
const DOW_S = ['Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa', 'So'];
const DOW_L = ['Montag', 'Dienstag', 'Mittwoch', 'Donnerstag', 'Freitag', 'Samstag', 'Sonntag'];
const dShort = s => { const d = parseD(s); return `${d.getDate()}. ${MON_S[d.getMonth()]}`; };
const dMid = s => { const d = parseD(s); return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()}`; };
const dLong = s => { const d = parseD(s); return `${DOW_L[dow(s)]}, ${d.getDate()}. ${MONTHS[d.getMonth()]} ${d.getFullYear()}`; };
const isoWeek = s => {
  const d = parseD(s); const t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
  const n = t.getUTCDay() || 7; t.setUTCDate(t.getUTCDate() + 4 - n);
  const y0 = new Date(Date.UTC(t.getUTCFullYear(), 0, 1));
  return Math.ceil(((t - y0) / 864e5 + 1) / 7);
};
const weekStart = s => addDays(s, -dow(s));
const monthStart = s => s.slice(0, 8) + '01';
const monthEnd = s => { const d = parseD(s); return fmtD(new Date(d.getFullYear(), d.getMonth() + 1, 0, 12)); };
/* Eingaben im deutschen Format (TT.MM.JJJJ, HH:MM) – unabhängig von der Browser-Sprache */
function parseDE(v) {
  v = String(v).trim();
  if (/^\d{4}-\d{2}-\d{2}$/.test(v)) return fmtD(parseD(v)) === v ? v : null;
  const m = v.match(/^(\d{1,2})[.\-/ ](\d{1,2})[.\-/ ](\d{2}|\d{4})$/); if (!m) return null;
  const y = +m[3] < 100 ? 2000 + +m[3] : +m[3], iso = `${y}-${pad(m[2])}-${pad(m[1])}`;
  return fmtD(parseD(iso)) === iso ? iso : null;
}
function parseTime(v) {
  const m = String(v).trim().match(/^(\d{1,2})(?:[:.,h]?(\d{2}))?$/); if (!m) return null;
  const h = +m[1], mi = +(m[2] || 0); return h > 24 || (h === 24 && mi > 0) ? null : h * 60 + mi;
}
/* Passwortrichtlinie (der Server prüft dieselben Regeln) */
const pwChecks = pw => [
  [`Mindestens ${S.reg.minPw} Zeichen`, [...pw].length >= S.reg.minPw],
  ['Großbuchstabe', /\p{Lu}/u.test(pw)], ['Kleinbuchstabe', /\p{Ll}/u.test(pw)],
  ['Zahl', /[0-9]/.test(pw)], ['Sonderzeichen', /[^\p{L}\p{N}\s]/u.test(pw)]
];
const pwRulesHtml = pw => pwChecks(pw).map(([l, ok]) => `<span class="pwr ${ok ? 'ok' : ''}">${ic(ok ? 'check' : 'x', 12)}${l}</span>`).join('');
/* Land aus dem Cloudflare-Header: Flagge und deutscher Name */
const flag = cc => cc && /^[A-Z]{2}$/.test(cc) ? String.fromCodePoint(...[...cc].map(c => 0x1F1E6 + c.charCodeAt(0) - 65)) : '';
const countryName = cc => { try { return new Intl.DisplayNames(['de'], { type: 'region' }).of(cc) || cc; } catch (e) { return cc; } };
const fmtAgo = iso => { if (!iso) return 'nie'; const d = (Date.now() - new Date(iso)) / 1000; return d < 90 ? 'gerade eben' : d < 3600 ? `vor ${Math.round(d / 60)} Min.` : d < 86400 ? `vor ${Math.round(d / 3600)} Std.` : d < 172800 ? 'gestern' : `vor ${Math.round(d / 86400)} Tagen`; };
const hm = m => { m = Math.round(m); return `${pad(Math.floor(m / 60))}:${pad(m % 60)}`; };
/** Uhrzeit, auch jenseits von Mitternacht (z. B. Prognose bei spätem Beginn): 28:58 → „04:58 (morgen)“ */
const hmDay = m => m >= 1440 ? `${hm(m - 1440)} (morgen)` : hm(m);
const dur = m => { const s = Math.round(m) < 0 ? '−' : ''; m = Math.round(Math.abs(m)); return `${s}${Math.floor(m / 60)}:${pad(m % 60)} h`; };
/* Stunden:Minuten:Sekunden, z. B. 5:57:22 */
const durS = m => { let t = Math.round(Math.max(0, m) * 60); return `${Math.floor(t / 3600)}:${pad(Math.floor(t % 3600 / 60))}:${pad(t % 60)}`; };
const durN = m => dur(m).replace(' h', '');
const sdur = m => Math.round(m) === 0 ? '±0:00 h' : (m > 0 ? '+' : '−') + dur(Math.abs(m));
const hdec = h => String(Math.round(h * 100) / 100).replace('.', ',');
const esc = s => String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

const STATES = { BW: 'Baden-Württemberg', BY: 'Bayern', BE: 'Berlin', BB: 'Brandenburg', HB: 'Bremen', HH: 'Hamburg', HE: 'Hessen', MV: 'Mecklenburg-Vorpommern', NI: 'Niedersachsen', NW: 'Nordrhein-Westfalen', RP: 'Rheinland-Pfalz', SL: 'Saarland', SN: 'Sachsen', ST: 'Sachsen-Anhalt', SH: 'Schleswig-Holstein', TH: 'Thüringen' };
/* Apps kommen vom Server: die meistgenutzten haben eine eigene Farbe, der Rest läuft als „Sonstiges“. */
let APPS = { other: { n: 'Sonstiges', c: '--s0', g: 'other' } };
const appOf = k => APPS[k] || APPS.other;
const appName = k => APPS[k] ? APPS[k].n : k;
const ak = k => APPS[k] ? k : 'other';
function setApps(list) { APPS = {}; list.forEach(a => { APPS[a.k] = { n: a.n, c: a.c, g: a.g }; }); if (!APPS.other) APPS.other = { n: 'Sonstiges', c: '--s0', g: 'other' }; }
const KINDS = {
  holiday: { n: 'Feiertag', c: '--s7' }, vacation: { n: 'Urlaub', c: '--s3' },
  sick: { n: 'Krank', c: '--s8' }, comp: { n: 'Zeitausgleich', c: '--s2' }
};

/* ---------- Zustand ---------- */
const S = {
  // Oberfläche
  route: 'dashboard', os: /Mac/.test(navigator.platform) ? 'mac' : 'windows', theme: 'light', themePref: 'system', speed: 1, demoOpen: false,
  dayDate: TODAY, statMode: 'month', statRef: TODAY, calMonth: TODAY.slice(0, 7), setTab: 'work', adminTab: 'users',
  adminFilter: 'all', adminQ: '', regStep: 'form', modal: null, now: 14 * 60 + 32, cwin: 'open', busy: false,
  loggedIn: false, limited: false, ready: false, demoHint: true, demo: true, devices: [], pairCode: null, limits: { editDaysBack: 0 }, bkPol: null, smtp: null, smtpTest: null, forgotSent: false, verify: null, sessAdmin: { sessions: [], devices: [], latestVersion: null }, sessQ: '', mySessions: [], showSwitches: localStorage.getItem('tickly.switches') === '1', totp: { enabled: false, recoveryLeft: 0 }, mfa: null, mfaGate: null, sec: null, log: { entries: [], next: null }, logKind: 'all', logQ: '', backups: [],
  // Serverdaten (aus dem Snapshot)
  me: { id: '', name: '', email: '', role: 'user' },
  set: { hours: [8, 8, 8, 8, 8, 0, 0], mode: 'net', rules: [], lockAsBreak: true, chunk: 15, setupBefore: 0, setupAfter: 0, state: 'NW', startBal: 0, idle: 10, trackApps: true, trackTitles: false, trackLock: true, autostart: true, excluded: [], pwLogin: true, theme: 'system', autoTrack: { enabled: true, ask: true, windows: [] }, extraDays: { rosenmontag: false, heiligabend: false, silvester: false, custom: [] } },
  reg: { open: true, approval: true, domains: '', weekly: 40, passkeyPrompt: true, minPw: 14, mfaRequired: false, mfaGraceDays: 7, sessionDays: 30, editDaysBack: 90, verifyEmail: false, allowStartChange: true, allowPlannedEnd: true },
  passkeys: [], absences: [], users: [], day: { start: 0, tracking: false, ended: null, lastApp: 'mail', segs: [], events: [] }
};
const DB = { days: {}, forecastEnd: null, vacation: { total: 30, taken: 0, planned: 0, rest: 30 }, nextHolidays: [], holidays: [] };

/** Betriebssystem des Rechners, der heute gemeldet hat (steht im Tag); sonst das zuletzt aktive gekoppelte Gerät */
function syncOs() {
  if (S.demo) return;
  const o = S.day && S.day.os;
  if (o) { S.os = o === 'macOS' ? 'mac' : 'windows'; return; }
  const dev = (S.devices || []).find(x => x.online) || (S.devices || [])[0];
  if (dev) S.os = dev.os === 'macos' ? 'mac' : 'windows';
}

function applySnapshot(s) {
  TODAY = s.today; TRACK_START = s.trackStart; S.memberSince = s.memberSince;
  S.me = s.me; S.set = s.settings;
  if (!S.themeLocked && S.set.theme && S.set.theme !== S.themePref) { S.themePref = S.set.theme; applyTheme(); } S.absences = s.absences; S.passkeys = s.passkeys; S.day = s.day; S.night = s.nightCheck || null; S.clash = s.deviceClash || null;
  setApps(s.apps || []); S.devices = s.devices || []; S.totp = s.totp || { enabled: false, recoveryLeft: 0 }; S.mfa = s.mfa || null; S.limits = s.limits || { editDaysBack: 0 }; S.demo = !!s.demo; if (!S.demo) { S.now = s.now; syncOs(); }
  S.users = s.users || []; if (s.reg) S.reg = s.reg; S.secQ = s.secQ || null; S.startBalSet = s.startBalSet; S.serverVersion = s.serverVersion;
  DB.days = {}; s.days.forEach(d => { DB.days[d.date] = d; });
  DB.forecastEnd = s.forecastEnd; DB.vacation = s.vacation; DB.nextHolidays = s.nextHolidays; DB.holidays = s.holidays;
  S.ready = true;
}

/* ---------- Zugriff auf berechnete Werte ---------- */
function emptyStat(date) {
  const future = date > TODAY, weekend = dow(date) >= 5;
  return { date, info: { holiday: null, absence: null, soll: 0, off: false, kind: null, base: 0, weekend }, soll: 0, has: false, live: false, future, start: null, end: null, gross: 0, locked: 0, rule: 0, lockedBreak: 0, brk: 0, net: 0, counted: 0, diff: future ? null : 0, diffLive: future ? null : 0, segs: null };
}
const dayStat = date => DB.days[date] || emptyStat(date);
const todaySegs = () => S.day.segs.map(s => s.to == null ? { ...s, to: Math.max(s.from, S.now) } : s);
function trackState() {
  const d = S.day, ago = d.ended != null ? `seit ${hm(d.ended)}` : '';
  if (d.tracking) {
    const o = d.segs[d.segs.length - 1];
    if (!o) return { key: 'stopped', label: 'Tracking beendet', cls: 'stop', sub: '' };
    if (o.kind === 'locked') return { key: 'locked', label: 'Rechner gesperrt', cls: 'lock', sub: `seit ${hm(o.from)}` };
    return { key: 'running', label: 'Tracking läuft', cls: 'run', sub: `${appOf(o.app).n} seit ${hm(o.from)}`, app: o.app, since: o.from };
  }
  if (d.stop === 'pause') return { key: 'paused', label: 'Tracking pausiert', cls: 'lock', sub: `Pause ${ago}` };
  if (d.stop === 'ask') return { key: 'ask', label: 'Rückfrage offen', cls: 'lock', sub: 'Arbeitest du gerade?' };
  if (d.stop === 'idle') return { key: 'idle', label: 'Tracking aus', cls: 'stop', sub: 'Nicht gestartet' };
  const last = d.events && d.events[d.events.length - 1];
  return { key: 'stopped', label: 'Tracking beendet', cls: 'stop', sub: last && last.k === 'stop' ? 'Im Web beendet' : 'Companion beendet' };
}
/** Der Tag ist beendet (Companion beendet / Rechner aus), nicht nur pausiert. */
const dayClosed = () => !S.day.tracking && S.day.segs.length > 0 && S.day.stop !== 'pause';
const forecastEnd = () => DB.forecastEnd;
const vacationSummary = () => DB.vacation;
const nextHolidays = n => DB.nextHolidays.slice(0, n);
const absWorkdays = a => { let n = 0, d = a.from; while (d <= a.to) { const s = dayStat(d); if (s.info.base > 0 && !s.info.holiday) n++; d = addDays(d, 1); } return n; };
function histEvents(date) {
  const segs = dayStat(date).segs || [], ev = [];
  if (!segs.length) return ev;
  ev.push({ t: segs[0].from, k: 'start', txt: 'Windows gestartet · Arbeitstag beginnt' });
  segs.forEach(s => { if (s.kind === 'locked') { ev.push({ t: s.from, k: 'lock', txt: 'Bildschirm gesperrt' }); ev.push({ t: s.to, k: 'unlock', txt: `Entsperrt (${Math.round(s.to - s.from)} Min. gesperrt)` }); } });
  ev.push({ t: segs[segs.length - 1].to, k: 'quit', txt: 'Companion beendet · Arbeitsende' });
  return ev;
}

/* ---------- Summen über Zeiträume (reine Anzeige-Aggregation) ---------- */
function rangeStats(from, to) {
  const days = []; let d = from < TRACK_START ? TRACK_START : from;
  const sum = { counted: 0, soll: 0, diff: 0, worked: 0, sumStart: 0, nStart: 0, off: { holiday: 0, vacation: 0, sick: 0, comp: 0 }, longest: null };
  while (d <= to && d <= TODAY) {
    const s = dayStat(d); days.push(s);
    sum.counted += s.counted; sum.soll += s.soll; sum.diff += s.diff || 0;
    if (s.has) { sum.worked++; sum.sumStart += s.start; sum.nStart++; if (!sum.longest || s.counted > sum.longest.counted) sum.longest = s; }
    if (s.info.kind && !s.has && s.info.base > 0) sum.off[s.info.kind]++;
    d = addDays(d, 1);
  }
  sum.days = days; sum.avgStart = sum.nStart ? sum.sumStart / sum.nStart : null; sum.avgDay = sum.worked ? sum.counted / sum.worked : 0;
  return sum;
}
function balanceSeries() {
  let bal = S.set.startBal * 60; const out = []; let d = TRACK_START;
  while (d <= TODAY) { const s = dayStat(d); bal += s.diff || 0; out.push({ date: d, bal, st: s }); d = addDays(d, 1); }
  return out;
}
const totalBalance = () => { const b = balanceSeries(); return b.length ? b[b.length - 1].bal : S.set.startBal * 60; };
function appUsage(from, to) {
  const m = {}; let tot = 0, d = from < TRACK_START ? TRACK_START : from;
  while (d <= to && d <= TODAY) {
    const s = dayStat(d);
    if (s.has) s.segs.forEach(g => { if (g.kind === 'app') { const l = g.to - g.from; const a = ak(g.app); m[a] = (m[a] || 0) + l; tot += l; } });
    d = addDays(d, 1);
  }
  return { list: Object.keys(m).map(k => ({ app: k, min: m[k] })).sort((a, b) => b.min - a.min), total: tot };
}
