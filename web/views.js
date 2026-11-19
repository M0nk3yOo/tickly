'use strict';
/* Tickly – Prototyp · Ansichten (liefern HTML-Strings) */

const ICONS = {
  dash: '<rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/>',
  day: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  chart: '<path d="M3 3v18h18"/><path d="M7 15l4-4 3 3 5-6"/>',
  cal: '<rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/>',
  umb: '<path d="M12 3a9 9 0 019 9H3a9 9 0 019-9zM12 12v6a2 2 0 004 0"/>',
  monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
  sliders: '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0"/><circle cx="16" cy="6" r="2"/><circle cx="10" cy="12" r="2"/><circle cx="18" cy="18" r="2"/>',
  users: '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20c0-3.5 3-6 6.5-6s6.5 2.5 6.5 6"/><circle cx="17" cy="9" r="2.5"/><path d="M17 14c2.5 0 4.5 1.8 4.5 4.5"/>',
  lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 018 0v3"/>',
  unlock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 017.5-2"/>',
  key: '<circle cx="8" cy="15" r="4"/><path d="M11 12l9-9M16 7l3 3M14 9l2 2"/>',
  logout: '<path d="M9 21H5a2 2 0 01-2-2V5a2 2 0 012-2h4M16 17l5-5-5-5M21 12H9"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  moon: '<path d="M21 12.8A9 9 0 1111.2 3a7 7 0 009.8 9.8z"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
  edit: '<path d="M4 20h4L19 9l-4-4L4 16v4zM13 7l4 4"/>',
  x: '<path d="M6 6l12 12M18 6L6 18"/>',
  chevL: '<path d="M15 5l-7 7 7 7"/>', chevR: '<path d="M9 5l7 7-7 7"/>',
  shield: '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6l8-3z"/>',
  download: '<path d="M12 3v12M7 11l5 5 5-5M4 20h16"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>',
  user: '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 3.5-7 8-7s8 3 8 7"/>',
  power: '<path d="M12 3v9M6.4 6.4a8 8 0 1011.2 0"/>',
  coffee: '<path d="M4 8h13v6a5 5 0 01-5 5H9a5 5 0 01-5-5V8zM17 10h2a2 2 0 010 4h-2M8 2v3M12 2v3"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>',
  bolt: '<path d="M13 2L4 14h7l-1 8 9-12h-7l1-8z"/>',
  mail: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>',
  play: '<path d="M7 4l13 8-13 8V4z"/>',
  flask: '<path d="M9 3h6M10 3v6l-5 9a2 2 0 002 3h10a2 2 0 002-3l-5-9V3"/>',
  fp: '<path d="M12 11v3a6 6 0 01-1 3M8 14a4 4 0 018 0v1a10 10 0 01-1.5 5M5 12a7 7 0 0114 0c0 2-.2 3.5-.6 5M3 16c.5-1 1-2.2 1-4a8 8 0 0116 0"/>',
  pause: '<rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/>',
  tag: '<path d="M3 12V4h8l10 10-8 8L3 12z"/><circle cx="8" cy="8" r="1.2"/>'
};
const ic = (n, s = 18) => `<svg class="ic" width="${s}" height="${s}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${ICONS[n] || ''}</svg>`;
const sgn = (m, c = '') => `<span class="${Math.round(m) > 0 ? 'pos' : Math.round(m) < 0 ? 'neg' : 'zero'} ${c}">${sdur(m)}</span>`;
const AVC = ['--s1', '--s7', '--s3', '--s2', '--s5', '--s8'];
const initials = n => n.split(/\s+/).map(x => x[0]).slice(0, 2).join('').toUpperCase();
const avatar = (n, cls = '') => { let h = 0; for (const c of n) h = (h * 31 + c.charCodeAt(0)) >>> 0; return `<span class="av ${cls}" style="background:var(${AVC[h % AVC.length]})">${esc(initials(n))}</span>`; };
const isAdmin = () => S.me.role === 'admin';
const OSN = () => S.os === 'windows' ? 'Windows' : 'macOS';

/* Datum und Uhrzeit immer im deutschen Format; der Kalender-Button öffnet die Browser-Auswahl */
const dateField = (path, iso, { max, rm, cls = '' } = {}) => `<span class="dfield ${cls}"><input type="text" class="dtxt" inputmode="numeric" placeholder="TT.MM.JJJJ" maxlength="10" autocomplete="off" aria-label="Datum" value="${dMid(iso)}" data-in="${path}" data-fmt="date"${max ? ` data-max="${max}"` : ''}${rm ? ' data-rm="1"' : ''}><button type="button" class="ibtn dpick" data-a="pickDate" title="Kalender öffnen" tabindex="-1">${ic('cal', 16)}</button><input type="date" class="dnative" tabindex="-1" aria-hidden="true" value="${iso}"${max ? ` max="${max}"` : ''} data-in="${path}"${rm ? ' data-rm="1"' : ''}></span>`;
const timeField = (path, min) => `<input type="text" class="ttxt" inputmode="numeric" placeholder="HH:MM" maxlength="5" autocomplete="off" aria-label="Uhrzeit" value="${hm(min)}" data-in="${path}" data-fmt="time">`;

const EVICON = { app: 'monitor', start: 'power', lock: 'lock', unlock: 'unlock', quit: 'power', shutdown: 'power', pause: 'pause', ask: 'info', stop: 'power' };
const trkTone = ts => ts.key === 'running' ? 'good' : ['locked', 'paused', 'ask'].includes(ts.key) ? 'warn' : 'bad';
/** Rückfrage „Arbeitest du gerade?“ – erscheint, wenn der Rechner außerhalb der Zeitfenster startet. */
function askBox() {
  const fresh = !S.day.segs.length;
  return `<div class="askbox"><h4>${ic('info', 19)}Arbeitest du gerade?</h4>
    <p class="ink2 sm">${fresh ? `Der Rechner wurde um <b>${hm(S.day.start)}</b> gestartet – außerhalb deiner automatischen Zeitfenster.` : `Der Companion läuft wieder – außerhalb deiner automatischen Zeitfenster.`} Soll die Zeiterfassung jetzt laufen?</p>
    <div class="row"><button class="btn primary" data-a="cAnswer" data-v="work">${ic('play', 14)} Ja, Tracking starten</button>${fresh ? `<button class="btn" data-a="cAnswer" data-v="boot">Ja, ab Rechnerstart (${hm(S.day.start)})</button>` : ''}<button class="btn" data-a="cAnswer" data-v="no">Nein, gerade nicht</button></div></div>`;
}

/* Piktogramme je App (eigene, schlichte Symbole). Die echte Companion-App kann später die Original-Icons der Programme aus dem System lesen. */
const APP_GLYPH = {
  code: '<path d="M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16"/>',
  web: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3.4 3.2 3.4 14.8 0 18M12 3c-3.4 3.2-3.4 14.8 0 18"/>',
  mail: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>',
  teams: '<path d="M4 5h16v11H10l-5 4v-4H4V5z"/><path d="M8 9.5h8M8 12.5h5"/>',
  figma: '<path d="M12 21l-2.3-6.2L12 3l2.3 11.8L12 21zM9.7 14.8h4.6"/>',
  excel: '<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M4 9.5h16M4 14.5h16M10 4v16"/>',
  other: '<circle cx="6" cy="12" r="1.4"/><circle cx="12" cy="12" r="1.4"/><circle cx="18" cy="12" r="1.4"/>'
};
const _ink = {};
/** Schriftfarbe (hell/dunkel) passend zur Farbe der App, damit das Symbol immer lesbar ist. */
function appInk(k) {
  const key = S.theme + appOf(k).c; if (_ink[key]) return _ink[key];
  let h = getComputedStyle(document.documentElement).getPropertyValue(appOf(k).c).trim().replace('#', '');
  if (h.length === 3) h = h.split('').map(x => x + x).join('');
  const n = parseInt(h, 16), [r, g, b] = [n >> 16 & 255, n >> 8 & 255, n & 255].map(c => { c /= 255; return c <= .03928 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4; });
  return _ink[key] = .2126 * r + .7152 * g + .0722 * b > .3 ? '#14141a' : '#fff';
}
const appGlyph = (k, size = 14) => `<svg class="ag" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="${appInk(k)}" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${APP_GLYPH[appOf(k).g] || APP_GLYPH.other}</svg>`;
const appIcon = (k, size = 20) => `<span class="aic" style="width:${size}px;height:${size}px;background:var(${appOf(k).c})">${appGlyph(k, Math.round(size * .66))}</span>`;

let LINES = [];
/** „07:58–16:40 Uhr · “ für den Tooltip eines Tages (leer, wenn nichts erfasst ist) */
const span = s => s.has && s.start != null ? (s.live || s.end == null ? `ab ${hm(s.start)} Uhr (läuft) · ` : `${hm(s.start)}–${hm(s.end)} Uhr · `) : '';
const tipAttr = (title, sub) => `data-tip="${esc(title)}${sub ? '|' + esc(sub) : ''}"`;

/* ---------- Diagramm-Bausteine ---------- */
function niceStep(max) { for (const s of [30, 60, 120, 240, 480, 600, 960, 1200, 2400, 4800]) if (max / s <= 5) return s; return 4800; }
const yLab = m => `${Math.round(m / 60)} h`;

function barChart(items, { h = 220, gap = 6, labelEvery = 1, minTop = 0 } = {}) {
  const max = Math.max(minTop, ...items.map(i => Math.max(i.ist || 0, i.soll || 0)));
  const step = niceStep(max), top = Math.max(step, Math.ceil(max / step) * step);
  const ticks = []; for (let v = 0; v <= top; v += step) ticks.push(v);
  const cols = items.map((it, i) => {
    const bh = Math.min(it.ist, it.soll) / top * 100, xh = Math.max(0, it.ist - it.soll) / top * 100;
    let inner = '';
    if (it.off) inner = `<span class="mk" style="background:var(${KINDS[it.off].c})" title=""></span>`;
    else if (it.ist > 0) {
      const base = it.soll > 0 ? Math.min(it.ist, it.soll) : 0, extra = it.soll > 0 ? Math.max(0, it.ist - it.soll) : it.ist;
      inner = `<div class="bar" style="height:${(it.ist / top * 100).toFixed(2)}%">${base > 0 ? `<i style="height:${(base / it.ist * 100).toFixed(2)}%;flex:none"></i>` : ''}${extra > 0 ? `<i class="x" style="height:${(extra / it.ist * 100).toFixed(2)}%;flex:none"></i>` : ''}</div>`;
    }
    const soll = (it.soll > 0 && !it.off) || it.kind === 'comp' ? `<b class="soll" style="bottom:calc(${(it.soll / top * 100).toFixed(2)}% - 1px)"></b>` : '';
    return `<div class="col ${it.today ? 'today' : ''} ${it.wk ? 'wk' : ''}" ${tipAttr(it.tip[0], it.tip[1])}><div class="track"><span class="hit"></span>${inner}${soll}</div><div class="lab">${i % labelEvery === 0 ? esc(it.lab) : ''}</div></div>`;
  }).join('');
  return `<div class="bchart" style="--h:${h}px;--gap:${gap}px">
    <div class="yax">${ticks.map(v => `<span style="bottom:${v / top * 100}%">${yLab(v)}</span>`).join('')}</div>
    <div class="plot"><div class="gridl">${ticks.map(v => `<i class="${v === 0 ? 'z' : ''}" style="bottom:${v / top * 100}%"></i>`).join('')}</div><div class="cols">${cols}</div></div></div>`;
}

function lineChart(pts, { h = 200, xt = [] } = {}) {
  const vals = pts.map(p => p.v), lo = Math.min(0, ...vals), hi = Math.max(0, ...vals);
  const step = niceStep((hi - lo) * 1.1), mn = Math.floor(lo / step) * step, mx = Math.max(step, Math.ceil(hi / step) * step);
  const n = pts.length, X = i => n === 1 ? 50 : i / (n - 1) * 100, Y = v => (1 - (v - mn) / (mx - mn)) * 100;
  const path = pts.map((p, i) => `${i ? 'L' : 'M'}${(X(i) * 10).toFixed(1)},${(Y(p.v)).toFixed(2)}`).join('');
  const area = `${path}L${X(n - 1) * 10},${Y(Math.max(mn, 0)).toFixed(2)}L0,${Y(Math.max(mn, 0)).toFixed(2)}Z`;
  const idx = LINES.push({ pts: pts.map((p, i) => ({ x: X(i), y: Y(p.v), t: p.t, s: p.s })) }) - 1;
  const grid = []; for (let v = mn; v <= mx; v += step) grid.push(v);
  return `<div class="lc-wrap"><div class="lc" style="--h:${h}px" data-line="${idx}">
    ${grid.map(v => `<div class="gl ${v === 0 ? 'z' : ''}" style="top:${Y(v)}%"><span>${v === 0 ? '0' : (v > 0 ? '+' : '−') + Math.abs(Math.round(v / 60))} h</span></div>`).join('')}
    <svg viewBox="0 0 1000 100" preserveAspectRatio="none"><path d="${area}" fill="var(--s1)" opacity=".11"/><path d="${path}" fill="none" stroke="var(--s1)" stroke-width="2.2" stroke-linejoin="round" vector-effect="non-scaling-stroke"/></svg>
    ${xt.map(t => `<span class="xl" style="left:${X(t.i)}%;transform:translateX(${t.i === 0 ? 0 : t.i === n - 1 ? -100 : -50}%)">${esc(t.l)}</span>`).join('')}
    <i class="cx"></i><i class="cd"></i></div></div>`;
}

/* ---------- Zeitleiste ---------- */
function tlAxis(from, to) { const ls = []; const stepH = (to - from) > 12 * 60 ? 2 : 1; for (let m = Math.ceil(from / 60) * 60; m <= to; m += stepH * 60) ls.push(m); return ls; }
function mergeStatus(segs) {
  const out = [];
  segs.forEach(s => {
    const k = s.kind === 'app' ? 'active' : s.kind;
    const l = out[out.length - 1];
    if (l && l.k === k && l.to === s.from) l.to = s.to; else out.push({ k, from: s.from, to: s.to });
  });
  return out;
}
function timeline(segs, from, to, { lanes = true, now = null, mini = false, icons = !mini } = {}) {
  const W = to - from, L = m => ((m - from) / W * 100).toFixed(3), Wd = (a, b) => ((b - a) / W * 100).toFixed(3);
  const ticks = tlAxis(from, to);
  const appLane = segs.map(s => {
    if (s.kind === 'app') return `<i class="sg ${icons && (s.to - s.from) / W > .045 ? 'ic' : ''}" style="left:${L(s.from)}%;width:${Wd(s.from, s.to)}%;background:var(${appOf(s.app).c})" ${tipAttr(dur(s.to - s.from), `${appName(s.app)} · ${hm(s.from)}–${hm(s.to)}${s.note ? ' · ' + s.note : ''}`)}>${icons && (s.to - s.from) / W > .045 ? appGlyph(s.app, 16) : ''}</i>`;
    if (s.kind === 'locked') return `<i class="sg locked" style="left:${L(s.from)}%;width:${Wd(s.from, s.to)}%" ${tipAttr(dur(s.to - s.from), `Rechner gesperrt · ${hm(s.from)}–${hm(s.to)}`)}></i>`;
    return `<i class="sg off" style="left:${L(s.from)}%;width:${Wd(s.from, s.to)}%" ${tipAttr(dur(s.to - s.from), `Companion nicht aktiv · ${hm(s.from)}–${hm(s.to)} · nicht gezählt`)}></i>`;
  }).join('');
  const stLane = mergeStatus(segs).map(s => {
    const nm = s.k === 'active' ? 'Aktiv am Rechner' : s.k === 'locked' ? 'Rechner gesperrt' : 'Tracker aus';
    const st = s.k === 'active' ? `background:var(--ink-2)` : '';
    return `<i class="sg ${s.k === 'active' ? '' : s.k}" style="left:${L(s.from)}%;width:${Wd(s.from, s.to)}%;${st}" ${tipAttr(dur(s.to - s.from), `${nm} · ${hm(s.from)}–${hm(s.to)}`)}></i>`;
  }).join('');
  return `<div class="tl ${mini ? 'mini' : ''}">
    <div class="tl-labels">${lanes ? '<span>Status</span><span>Aktive App</span>' : ''}</div>
    <div class="tl-plot">
      <div class="tl-grid">${ticks.map(m => `<i style="left:${L(m)}%"></i>`).join('')}</div>
      ${lanes ? `<div class="tl-track thin" style="background:transparent">${stLane}</div>` : ''}
      <div class="tl-track">${appLane}</div>
      <div class="tl-axis">${ticks.map(m => `<span style="left:${L(m)}%">${pad(m / 60)}</span>`).join('')}</div>
      ${now != null ? `<i class="tl-now" style="left:${L(now)}%"></i>` : ''}
    </div></div>`;
}
function appLegend() {
  return `<div class="legend">${Object.keys(APPS).map(k => `<span>${appIcon(k, 18)}${appOf(k).n}</span>`).join('')}<span><i class="hatch"></i>Gesperrt</span></div>`;
}
function appRank(list, total, max = 7) {
  if (!list.length) return '<div class="hint">Noch keine App-Daten.</div>';
  const top = list[0].min;
  return list.slice(0, max).map(a => `<div class="approw" ${tipAttr(dur(a.min), `${appOf(a.app).n} · ${(a.min / total * 100).toFixed(1).replace('.', ',')} % der aktiven Zeit`)}>
    <span class="nm">${appIcon(a.app, 20)}${appOf(a.app).n}</span>
    <span class="bar"><b style="width:${a.min / top * 100}%;background:var(${appOf(a.app).c})"></b></span>
    <span class="tm">${durN(a.min)}</span><span class="pc">${Math.round(a.min / total * 100)} %</span></div>`).join('');
}

/* ---------- Auth ---------- */
function authArt() {
  return `<div class="auth-art"><div class="brand" style="color:#fff;padding:0"><img class="logo" src="icon.svg" width="32" height="32" alt="" style="border-radius:7px">Tickly</div>
  <div><h2>Arbeitszeit erfassen, ohne daran zu denken.</h2></div>
  <p>Die Companion-App erkennt deinen Arbeitsbeginn automatisch, zählt Pausen sauber ab und zeigt dir jederzeit deinen Überstundenstand.</p>
  <div class="pts"><div>${ic('power', 20)}<span>Startet mit Windows &amp; macOS und beendet das Tracking beim Beenden</span></div><div>${ic('chart', 20)}<span>Tages-, Wochen- und Monatsstatistiken mit App-Verlauf</span></div><div>${ic('fp', 20)}<span>Anmeldung per Passkey – ohne Passwort</span></div></div>
  <div class="mock"><div class="row between sm" style="opacity:.8"><span>Heute · Montag</span><span>Arbeitsbeginn 07:52</span></div><div style="font-size:34px;font-weight:650;letter-spacing:-.02em;margin:6px 0 8px">5:57 <span style="font-size:14px;opacity:.7;font-weight:500">von 8:00 h</span></div><div style="height:8px;border-radius:5px;background:rgba(255,255,255,.2)"><div style="width:74%;height:100%;border-radius:5px;background:#fff"></div></div></div></div>`;
}
/* Pflicht zu Passkey/Authenticator-App: Hinweis, Erinnerung beim Anmelden und Sperrseite nach Ablauf der Karenzzeit */
const daysText = n => n <= 0 ? 'heute ist der letzte Tag' : `noch ${n} Tag${n === 1 ? '' : 'e'}`;
const mfaDate = m => m && m.deadline ? dMid(fmtD(new Date(m.deadline))) : '';
/* Überstunden-Stand zu Beginn (Registrierung, Ersteinrichtung, einmalig nach der Anmeldung). Auslesen: regStartBal() */
function balForm() {
  return `<div class="f"><span style="font-size:12.5px;color:var(--ink-2);font-weight:550">Überstunden zu Beginn</span>
      <label class="chk"><input type="radio" name="rg-bal" value="fresh" checked>Keine Überstunden – mit 0 Stunden starten</label>
      <div class="row" style="gap:8px"><label class="chk"><input type="radio" name="rg-bal" value="carry">Stand mitnehmen:</label><input type="text" id="rg-carry" inputmode="decimal" placeholder="z. B. 12,5 oder −3" style="width:150px"><span class="muted">Stunden</span></div>
      <span class="hint">Positiv = Guthaben, negativ = Minusstunden. Das legst du einmal fest, danach kann es nur noch ein Administrator ändern.</span></div>`;
}
function startBalBanner() {
  if (S.demo || S.startBalSet !== false) return '';
  return `<div class="mfa-banner">${ic('bolt', 20)}<div class="grow"><b>Lege deinen Überstunden-Stand fest.</b> Das geht einmalig, danach kann ihn nur noch ein Administrator ändern. Hast du keine Überstunden, trage 0 ein.</div><button class="btn sm primary" data-a="startBalOpen">Jetzt festlegen</button></div>`;
}
/* Sicherheitsfragen: drei Fragen mit Antworten (Registrierung, Einrichtung, Einstellungen) */
function secQForm(prefix) {
  const list = S.questionList || [], start = [0, 4, 8];
  return `<div class="f"><span style="font-size:12.5px;color:var(--ink-2);font-weight:550">Sicherheitsfragen</span>
    <span class="hint">Damit kannst du dein Passwort selbst zurücksetzen, wenn du es vergisst. Wähle drei verschiedene Fragen, die nur du beantworten kannst. Groß-/Kleinschreibung und Satzzeichen spielen keine Rolle.</span>
    ${[1, 2, 3].map(i => `<div class="sq"><select id="${prefix}-q${i}" data-sq="1">${list.map((q, k) => `<option value="${esc(q)}" ${k === start[i - 1] ? 'selected' : ''} ${start.includes(k) && k !== start[i - 1] ? 'disabled' : ''}>${esc(q)}</option>`).join('')}<option value="__own">Eigene Frage …</option></select>
      <input type="text" id="${prefix}-c${i}" placeholder="Eigene Frage (8 bis 120 Zeichen)" maxlength="120" style="display:none" autocomplete="off">
      <input type="text" id="${prefix}-a${i}" placeholder="Antwort" maxlength="100" autocomplete="off"></div>`).join('')}</div>`;
}
/* Jede Frage nur einmal: Was in einer Auswahl gewählt ist, ist in den anderen gesperrt */
function syncSecQ(prefix) {
  const sels = [1, 2, 3].map(i => $(`#${prefix}-q${i}`)).filter(Boolean), taken = sels.map(s => s.value);
  sels.forEach((s, i) => [...s.options].forEach(o => { o.disabled = o.value !== '__own' && taken.some((v, j) => j !== i && v === o.value); }));
}
function readSecQ(prefix) {
  return [1, 2, 3].map(i => { let q = $(`#${prefix}-q${i}`).value; if (q === '__own') q = $(`#${prefix}-c${i}`).value.trim(); return { question: q, answer: $(`#${prefix}-a${i}`).value.trim() }; });
}
function secQProblem(list) {
  if (list.some(x => !x.question)) return 'Bitte alle drei Fragen wählen';
  if (new Set(list.map(x => x.question.toLowerCase())).size < 3) return 'Bitte drei verschiedene Fragen wählen';
  if (list.some(x => x.answer.replace(/[^\p{L}\p{N}]/gu, '').length < 3)) return 'Jede Antwort braucht mindestens 3 Buchstaben oder Ziffern';
  if (new Set(list.map(x => x.answer.toLowerCase().replace(/[^\p{L}\p{N}]/gu, ''))).size < 3) return 'Bitte drei verschiedene Antworten geben';
  return '';
}
function secqBanner() {
  const q = S.secQ; if (S.demo || !q || !q.enabled || !q.hasPassword || q.set) return '';
  return `<div class="mfa-banner">${ic('shield', 20)}<div class="grow"><b>Lege Sicherheitsfragen fest.</b> Damit kannst du dein Passwort selbst zurücksetzen, falls du es vergisst.</div><button class="btn sm primary" data-a="secqOpen">Jetzt festlegen</button></div>`;
}
function vPwGate() {
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Neues Passwort festlegen</h1><p class="muted" style="margin-top:4px">Ein Administrator hat dein Passwort gesetzt. Lege jetzt ein eigenes fest${S.pwGate && S.pwGate.needsQuestions ? ' und wähle Sicherheitsfragen, mit denen du es später selbst zurücksetzen kannst' : ''}, dann geht es sofort weiter.</p></div>
    <label class="f">Neues Passwort<input type="password" id="rs-pw" autocomplete="new-password" autofocus></label><div class="pwrules" id="pw-rules">${pwRulesHtml('')}</div>
    <label class="f">Passwort wiederholen<input type="password" id="rs-pw2" autocomplete="new-password"></label>
    ${S.pwGate && S.pwGate.needsQuestions ? secQForm('pg') : ''}
    <button class="btn primary lg block" data-a="pwRequired">${S.pwGate && S.pwGate.needsQuestions ? 'Speichern und weiter' : 'Passwort speichern'}</button>
    <p class="sm" style="text-align:center"><a href="#/login" data-a="logout" style="color:var(--brand);font-weight:600">Abmelden</a></p>
  </div></div></div>`;
}
/* Zeiterfassung im Web: Beenden */
const startViaWeb = () => { const f = S.day.events && S.day.events[0]; return !!f && f.k === 'start' && /im Web/.test(f.txt); };
const endedByWeb = () => { const l = S.day.events && S.day.events[S.day.events.length - 1]; return !!l && l.k === 'stop'; };
function longTrackBanner(t, ts) {
  const h = S.limits && S.limits.longTrackHours;
  if (!h || !S.day.tracking || t.gross < h * 60) return '';
  return `<div class="callout warn" style="margin-top:14px">${ic('day')}<div class="grow"><b>Läuft dein Tracking noch?</b> Es läuft schon seit <b>${durN(t.gross)} Stunden</b> (Brutto, seit ${t.start != null ? hm(t.start) : '–'}). Hast du vergessen, es zu beenden?</div><button class="btn sm primary" data-a="stopOpen" data-m="now">Jetzt beenden</button><button class="btn sm" data-a="stopOpen" data-m="custom">Ende festlegen …</button></div>`;
}
/** Vorab festgelegtes Ende: Hinweis mit Ändern/Aufheben */
function plannedEndBanner() {
  const pe = S.day.plannedEnd;
  if (pe == null || !(S.day.tracking || S.day.stop === 'pause')) return '';
  return `<div class="callout info" style="margin-top:14px">${ic('day')}<div class="grow"><b>Ende vorab festgelegt:</b> Das Tracking endet um <b>${hm(pe)} Uhr</b> von selbst${S.now >= pe ? '' : ` (in ${dur(pe - S.now).replace(' h', '')} h)`}. Bis dahin läuft die Zeit normal weiter.</div><button class="btn sm" data-a="stopOpen" data-m="custom" data-planned="1">Ändern …</button><button class="btn sm" data-a="planClear">Aufheben</button></div>`;
}
/** Plausibilitätsprüfung: das Tracking läuft über Nacht und passt nicht zu den bisherigen Zeiten */
function nightBanner() {
  const n = S.night;
  if (!n || !S.day.tracking) return '';
  return `<div class="callout warn" style="margin-top:14px">${ic('day')}<div class="grow"><b>Läuft dein Tracking noch?</b> Es läuft seit gestern <b>${hm(n.since)} Uhr</b> ohne Unterbrechung, inzwischen <b>${durN(n.hours * 60)} Stunden</b>. Das passt nicht zu deinen bisherigen Zeiten. Hast du vergessen, es zu beenden?</div><button class="btn sm primary" data-a="nightOpen">Ende festlegen …</button><button class="btn sm" data-a="nightOk" title="Die Zeit zählt weiter, du wirst heute nicht mehr gefragt">Ich arbeite wirklich</button></div>`;
}
/** Zwei Companions melden gleichzeitig, und der zweite hat das Tracking gestartet */
function clashBanner() {
  const c = S.clash;
  if (!c || !S.day.tracking) return '';
  return `<div class="callout warn" style="margin-top:14px">${ic('day')}<div class="grow"><b>Tracking läuft auf zwei Geräten</b> Auf <b>${esc(c.starter)}</b> wurde das Tracking gestartet, während <b>${esc(c.other)}</b> noch lief. Beide melden Zeiten und Apps für denselben Tag und verfälschen sich gegenseitig. Beende den Companion auf einem der Geräte.</div><button class="btn sm" data-a="clashOk" title="Die Warnung verschwindet; sie kommt erst bei einem neuen Start wieder">Verstanden</button></div>`;
}
function nightSummary(m) {
  if (m.err) return `<div class="callout warn">${ic('info')}<span>${esc(m.err)}</span></div>`;
  const p = m.prev; if (!p) return `<p class="muted sm">Berechne …</p>`;
  return `<div class="stopsum"><div class="kv"><span>Tag</span><b class="mono">${dMid(p.date)}</b></div><div class="kv"><span>Arbeitsbeginn</span><b class="mono">${hm(p.start)}</b></div><div class="kv"><span>${p.planned ? 'Ende (vorab festgelegt)' : 'Ende'}</span><b class="mono">${hm(p.at)}</b></div>
    <div class="kv"><span>Brutto (Anwesenheit)</span><b class="mono">${dur(p.gross)}</b></div><div class="kv"><span>Pause (abgezogen)</span><b class="mono">${p.pause > 0 ? '−' : ''}${dur(p.pause)}</b></div>
    <div class="kv"><span><b>Netto</b></span><b class="mono">${dur(p.net)}</b></div></div>`;
}
function stopSummary(m) {
  if (m.err) return `<div class="callout warn">${ic('info')}<span>${esc(m.err)}</span></div>`;
  const p = m.prev; if (!p) return `<p class="muted sm">Berechne …</p>`;
  return `<div class="stopsum"><div class="kv"><span>Arbeitsbeginn</span><b class="mono">${hm(p.start)}</b></div><div class="kv"><span>Ende</span><b class="mono">${hm(p.at)}</b></div>
    <div class="kv"><span>Brutto (Anwesenheit)</span><b class="mono">${dur(p.gross)}</b></div><div class="kv"><span>Pause (abgezogen)</span><b class="mono">${p.pause > 0 ? '−' : ''}${dur(p.pause)}</b></div>
    <div class="kv"><span><b>Netto</b></span><b class="mono">${dur(p.net)}</b></div>${p.soll ? `<div class="kv"><span>Tagessoll</span><b class="mono">${dur(p.soll)}</b></div>` : ''}
    ${p.diff != null ? `<div class="kv"><span><b>Saldo des Tages</b></span><b class="mono">${sgn(p.diff)}</b></div>` : ''}
    ${p.balanceBefore != null ? `<div class="kv"><span>Überstundenkonto</span><b class="mono">${sgn(p.balanceBefore)} → ${sgn(p.balanceAfter)}</b></div>` : ''}</div>
    ${p.diff != null && p.diff < -0.5 ? `<div class="callout warn" style="margin-top:10px">${ic('info')}<span>${p.planned ? 'Du beendest' : 'Du beendest'} mit <b>${dur(-p.diff)} Minus</b> gegenüber dem Tagessoll. Das wird nach dem Beenden dem Überstundenkonto abgezogen.</span></div>` : ''}`;
}
function startSummary(m) {
  if (m.err) return `<div class="callout warn">${ic('info')}<span>${esc(m.err)}</span></div>`;
  const p = m.prev; if (!p) return `<p class="muted sm">Berechne …</p>`;
  const d = p.counted - p.before.counted, same = Math.abs(p.at - p.from) < .01;
  return `<div class="stopsum"><div class="kv"><span>Beginn des laufenden Trackings</span><b class="mono">${same ? hm(p.at) : `${hm(p.from)} → ${hm(p.at)}`}</b></div>
    <div class="kv"><span>Brutto (Anwesenheit) heute</span><b class="mono">${dur(p.before.gross)}${same ? '' : ` → ${dur(p.gross)}`}</b></div>
    <div class="kv"><span><b>${S.set.mode === 'net' ? 'Netto' : 'Gezählt'}</b></span><b class="mono">${dur(p.before.counted)}${same ? '' : ` → ${dur(p.counted)} <span class="${d >= 0 ? 'pos' : 'neg'}">(${d >= 0 ? '+' : '−'}${dur(Math.abs(d)).replace(' h', '')})</span>`}</b></div>
    ${p.soll ? `<div class="kv"><span>Tagessoll</span><b class="mono">${dur(p.soll)}</b></div>` : ''}</div>`;
}
function mfaBanner() {
  const m = S.mfa; if (!m || !m.required || m.compliant) return '';
  return `<div class="mfa-banner">${ic('shield', 20)}<div class="grow"><b>Bitte sichere dein Konto ab.</b> Dein Administrator verlangt einen Passkey oder eine Authenticator-App – ${daysText(m.daysLeft)} (bis ${mfaDate(m)}). Bis dahin gilt deine Anmeldung nur 24 Stunden.</div><button class="btn sm primary" data-a="mfaGo">Jetzt einrichten</button></div>`;
}
function vMfaGate() {
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Konto absichern</h1><p class="muted" style="margin-top:4px">Dein Administrator verlangt für alle Konten einen Passkey oder eine Authenticator-App. Die Frist ist abgelaufen – richte jetzt einen von beiden ein, dann geht es sofort weiter.</p></div>
    <button class="btn primary lg block" data-a="passkeyAdd">${ic('fp', 20)} Passkey einrichten</button>
    <div class="divider">oder</div>
    <button class="btn lg block" data-a="totpSetup">${ic('shield', 20)} Authenticator-App einrichten</button>
    <p class="hint">Passkey: Fingerabdruck, Gesicht, PIN oder Sicherheitsschlüssel. Authenticator-App: z. B. Google Authenticator, Microsoft Authenticator, Authy oder 1Password.</p>
    <p class="sm" style="text-align:center"><a href="#/login" data-a="logout" style="color:var(--brand);font-weight:600">Abmelden</a></p>
  </div></div></div>`;
}
function vLoginTotp() {
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Zweiter Schritt</h1><p class="muted" style="margin-top:4px">Gib den 6-stelligen Code aus deiner Authenticator-App ein. Ohne Handy geht auch ein Wiederherstellungscode.</p></div>
    <label class="f">Code<input type="text" id="mfa-code" class="mfa-in" inputmode="text" autocomplete="one-time-code" autocapitalize="characters" maxlength="12" placeholder="123 456" autofocus></label>
    <button class="btn primary lg block" data-a="totpLogin">Bestätigen</button>
    <p class="sm" style="text-align:center"><a href="#/login" data-a="mfaBack" style="color:var(--brand);font-weight:600">${ic('chevL', 14)} Zurück zur Anmeldung</a></p>
  </div></div></div>`;
}
function vForgot() {
  const q = S.fq;
  const back = `<p class="sm" style="text-align:center"><a href="#/login" style="color:var(--brand);font-weight:600">${ic('chevL', 14)} Zurück zur Anmeldung</a></p>`;
  if (q) return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Sicherheitsfragen</h1><p class="muted" style="margin-top:4px">Beantworte alle drei Fragen so, wie du sie bei der Registrierung angegeben hast, und lege dann ein neues Passwort fest.</p></div>
    ${q.questions.map((t, i) => `<label class="f">${esc(t)}<input type="text" id="fq-a${i + 1}" autocomplete="off" maxlength="100"></label>`).join('')}
    <label class="f">Neues Passwort<input type="password" id="rs-pw" autocomplete="new-password"></label><div class="pwrules" id="pw-rules">${pwRulesHtml('')}</div>
    <button class="btn primary lg block" data-a="fqSave">Passwort ändern</button>
    <p class="sm" style="text-align:center"><a href="#/forgot" data-a="fqBack" style="color:var(--brand);font-weight:600">${ic('chevL', 14)} Zurück</a></p></div></div></div>`;
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Passwort vergessen?</h1><p class="muted" style="margin-top:4px">Gib deine E-Mail-Adresse ein. Wir schicken dir einen Link, mit dem du ein neues Passwort festlegst.${S.reg.secQuestions ? ' Oder beantworte deine Sicherheitsfragen.' : ''}</p></div>
    ${S.forgotSent ? `<div class="callout info">${ic('mail')}<span>${S.forgotMail === false ? 'Der E-Mail-Versand ist auf diesem Server nicht eingerichtet. Bitte wende dich an einen Administrator.' : 'Wenn es zu dieser Adresse ein Konto gibt, ist eine Mail unterwegs. Der Link ist eine Stunde gültig.'}</span></div>` : `<label class="f">E-Mail<input type="email" id="fg-mail" autocomplete="username" autofocus></label><button class="btn primary lg block" data-a="forgotSend">Link senden</button>`}
    ${S.reg.secQuestions && !S.forgotSent ? `<div class="divider">oder</div><button class="btn lg block" data-a="fqStart">${ic('shield', 18)} Mit Sicherheitsfragen zurücksetzen</button>` : ''}
    ${back}</div></div></div>`;
}
function vReset() {
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Neues Passwort festlegen</h1><p class="muted" style="margin-top:4px">Danach bist du überall abgemeldet und meldest dich mit dem neuen Passwort an.</p></div>
    <label class="f">Neues Passwort<input type="password" id="rs-pw" autocomplete="new-password" autofocus></label><div class="pwrules" id="pw-rules">${pwRulesHtml('')}</div>
    <button class="btn primary lg block" data-a="resetSave">Passwort speichern</button>
    <p class="sm" style="text-align:center"><a href="#/login" style="color:var(--brand);font-weight:600">Zur Anmeldung</a></p></div></div></div>`;
}
function vVerify() {
  const v = S.verify || { state: 'work' };
  const body = v.state === 'work' ? `<div class="spin" style="margin:18px auto"></div><h1 style="font-size:22px;text-align:center">Einen Moment …</h1>`
    : v.state === 'email' ? `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--good) 16%,var(--surface));color:var(--good-ink)">${ic('check', 26)}</div><h1 style="font-size:22px">E-Mail-Adresse geändert</h1><p class="muted">Du meldest dich ab sofort mit <b>${esc(v.email)}</b> an.</p></div><a class="btn primary block" href="${S.loggedIn ? '#/settings' : '#/login'}">${S.loggedIn ? 'Weiter zu Tickly' : 'Zur Anmeldung'}</a>`
    : v.state === 'pending' ? `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--good) 16%,var(--surface));color:var(--good-ink)">${ic('check', 26)}</div><h1 style="font-size:22px">E-Mail-Adresse bestätigt</h1><p class="muted">Dein Konto wartet noch auf die Freigabe durch einen Administrator. Du bekommst eine Mail, sobald es soweit ist.</p></div><a class="btn block" href="#/login">Zur Anmeldung</a>`
    : `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--bad) 12%,var(--surface));color:var(--bad-ink)">${ic('info', 26)}</div><h1 style="font-size:22px">Link ungültig</h1><p class="muted">${esc(v.msg || 'Der Link ist ungültig oder abgelaufen.')}</p></div><a class="btn block" href="#/login">Zur Anmeldung</a>`;
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">${body}</div></div></div>`;
}
function vLogin() {
  if (S.mfa) return vLoginTotp();
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Willkommen zurück</h1><p class="muted" style="margin-top:4px">Melde dich an, um deine Zeiten zu sehen.</p></div>
    <button class="btn lg block" data-a="passkeyLogin" style="border-color:var(--brand);color:var(--brand)">${ic('fp', 20)} Mit Passkey anmelden</button>
    <div class="divider">oder mit Passwort</div>
    <label class="f">E-Mail<input type="email" id="lg-mail" value="${S.demoHint ? 'andre@example.de' : ''}" autocomplete="username webauthn"></label>
    <label class="f">Passwort<input type="password" id="lg-pw" value="${S.demoHint ? 'demo-passwort' : ''}" autocomplete="current-password"></label>
    <div class="row between"><label class="chk"><input type="checkbox" checked>Angemeldet bleiben</label><a href="#/login" data-a="forgot" class="sm" style="color:var(--brand)">Passwort vergessen?</a></div>
    ${S.needsVerify ? `<div class="callout warn">${ic('mail')}<div class="grow">Bitte bestätige zuerst deine E-Mail-Adresse.</div><button class="btn sm" data-a="resendVerify">Mail erneut senden</button></div>` : ''}
    <button class="btn primary lg block" data-a="login">Anmelden</button>
    <p class="sm muted" style="text-align:center">${S.reg.open ? `Noch kein Konto? <a href="#/register" style="color:var(--brand);font-weight:600">Jetzt registrieren</a>` : `Die Registrierung ist deaktiviert – Zugang nur per Einladung durch einen Administrator.`}</p>
    ${S.demoHint ? `<p class="xs muted" style="text-align:center">Demo-Zugang vorausgefüllt (Administrator). Normaler Benutzer: lena.hoffmann@example.de · gleiches Passwort.</p>` : ''}
  </div></div></div>`;
}
function vSetup() {
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">
    <div><h1>Willkommen bei Tickly</h1><p class="muted" style="margin-top:4px">Lege jetzt das Konto des ersten Administrators an.</p></div>
    <label class="f">Einrichtungscode<input type="text" id="su-code" autocomplete="off" spellcheck="false" placeholder="aus dem Server-Protokoll" class="mono">
      <span class="hint">Der Code steht im Protokoll des Servers: <span class="mono">docker compose logs api</span> (Zeile „Einrichtungscode“). Er schützt davor, dass ein Fremder die Einrichtung zuerst abschließt.</span></label>
    <label class="f">Vollständiger Name<input type="text" id="su-name" placeholder="Max Mustermann" autocomplete="name"></label>
    <label class="f">E-Mail<input type="email" id="su-mail" placeholder="max@firma.de" autocomplete="username"></label>
    <label class="f">Passwort<input type="password" id="rg-pw" autocomplete="new-password" placeholder="Mindestens ${S.reg.minPw} Zeichen"></label>
    <div class="pwrules" id="pw-rules">${pwRulesHtml('')}</div>
    <label class="f">Passwort wiederholen<input type="password" id="su-pw2" autocomplete="new-password"></label>
    ${S.reg.secQuestions ? secQForm('sq') : ''}
    ${balForm()}
    <div class="callout info">${ic('info')}<span>Lange Passwörter sind besser. Ein Passwortmanager erzeugt und speichert sie für dich. Danach kannst du in den Einstellungen einen Passkey oder TOTP einrichten.</span></div>
    <button class="btn primary lg block" data-a="setupSave">Administrator anlegen</button>
  </div></div></div>`;
}
function vRegister() {
  const back = `<a href="#/login" class="sm" style="color:var(--brand);font-weight:600">${ic('chevL', 14)} Zurück zur Anmeldung</a>`;
  let body;
  if (!S.reg.open) body = `<div><h1>Registrierung geschlossen</h1><p class="muted" style="margin-top:6px">Ein Administrator hat die offene Registrierung deaktiviert. Bitte fordere eine Einladung an.</p></div><div class="callout info">${ic('info')}<span>Administratoren können die offene Registrierung unter <b>Serververwaltung → Registrierung</b> aktivieren.</span></div>`;
  else if (S.regStep === 'pending') body = `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--warn) 20%,var(--surface));color:var(--warn-ink)">${ic('clock' in ICONS ? 'clock' : 'day', 26)}</div><h1 style="font-size:22px">Fast geschafft</h1><p class="muted">Dein Konto wurde angelegt und wartet auf die Freigabe durch einen Administrator.${S.regPasskey ? ' Dein Passkey ist bereits eingerichtet – nach der Freigabe kannst du dich damit anmelden.' : ' Du kannst dich anmelden, sobald es freigegeben ist.'}</p></div><button class="btn block" data-a="regReset">Weitere Registrierung simulieren</button>`;
  else if (S.regStep === 'pknew') body = `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--good) 16%,var(--surface));color:var(--good-ink)">${ic('check', 26)}</div><h1 style="font-size:22px">E-Mail-Adresse bestätigt</h1><p class="muted">Richte jetzt deinen Passkey ein. Ohne ihn kannst du dich nicht anmelden.</p></div><button class="btn primary lg block" data-a="regPasskeyTry">${ic('fp', 20)} Passkey einrichten</button>`;
  else if (S.regStep === 'pkretry') body = `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--warn) 20%,var(--surface));color:var(--warn-ink)">${ic('info', 26)}</div><h1 style="font-size:22px">Passkey nicht eingerichtet</h1><p class="muted">Dein Konto ist angelegt, aber es wurde noch kein Passkey gespeichert. Ohne Passkey (oder Passwort) kannst du dich später nicht anmelden.</p></div><button class="btn primary lg block" data-a="regPasskeyTry">${ic('fp', 20)} Passkey erneut einrichten</button><p class="hint" style="text-align:center">Klappt es nicht? Du kannst später über „Passwort vergessen“ ein Passwort festlegen oder einen Administrator darum bitten.</p>`;
  else if (S.regStep === 'verify') body = `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--brand) 16%,var(--surface));color:var(--brand)">${ic('mail', 26)}</div><h1 style="font-size:22px">Bitte bestätige deine E-Mail-Adresse</h1><p class="muted">Wir haben dir einen Link geschickt. Klicke darauf, um dein Konto zu bestätigen – danach geht es weiter.${S.regPkFirst ? ' Danach richtest du deinen Passkey ein.' : ''}</p></div><button class="btn block" data-a="resendVerify">Mail erneut senden</button><a href="#/login" class="sm" style="text-align:center;color:var(--brand);font-weight:600">Zur Anmeldung</a>`;
  else if (S.regStep === 'done') body = `<div class="empty" style="padding:10px 0"><div class="big" style="background:color-mix(in oklab,var(--good) 16%,var(--surface));color:var(--good-ink)">${ic('check', 26)}</div><h1 style="font-size:22px">Konto erstellt</h1><p class="muted">Richte jetzt einen Passkey ein, um dich künftig mit Fingerabdruck, Gesicht oder PIN anzumelden.</p></div><button class="btn primary lg block" data-a="passkeyReg">${ic('fp', 20)} Passkey einrichten</button><button class="btn block" data-a="regContinue">Später – weiter zur App</button>`;
  else body = `<div><h1>Konto erstellen</h1><p class="muted" style="margin-top:4px">Starte in einer Minute mit der Zeiterfassung.</p></div>
    <label class="f">Vollständiger Name<input type="text" id="rg-name" placeholder="Max Mustermann"></label>
    <label class="f">E-Mail${S.reg.domains ? ` <span class="muted" style="font-weight:400">(nur ${esc(S.reg.domains)})</span>` : ''}<input type="email" id="rg-mail" placeholder="max@firma.de"></label>
    <label class="f">Passwort<input type="password" id="rg-pw" autocomplete="new-password" placeholder="Mindestens ${S.reg.minPw} Zeichen"></label>
    <div class="pwrules" id="pw-rules">${pwRulesHtml('')}</div>
    ${S.reg.secQuestions ? secQForm('sq') : ''}
    ${balForm()}
    <label class="chk"><input type="checkbox" id="rg-agb" checked>Ich stimme den Nutzungsbedingungen und der Datenschutzerklärung zu.</label>
    ${S.reg.approval ? `<div class="callout info">${ic('info')}<span>Neue Konten werden von einem Administrator <b>freigegeben</b>, bevor du dich anmelden kannst.</span></div>` : ''}
    <button class="btn primary lg block" data-a="register">Konto erstellen</button>
    <div class="divider">oder</div>
    <button class="btn block" data-a="passkeyRegNew">${ic('fp', 18)} Direkt mit Passkey registrieren</button>`;
  return `<div class="auth">${authArt()}<div class="auth-form"><div class="auth-box">${back}${body}</div></div></div>`;
}

/* ---------- Dashboard ---------- */
function ring(frac, over) {
  const r = 74, c = 2 * Math.PI * r, f = Math.min(1, frac);
  return `<svg width="176" height="176" viewBox="0 0 176 176"><circle cx="88" cy="88" r="${r}" fill="none" stroke="var(--surface-2)" stroke-width="13"/><circle cx="88" cy="88" r="${r}" fill="none" stroke="var(${over ? '--s2' : '--s1'})" stroke-width="13" stroke-linecap="round" stroke-dasharray="${(c * f).toFixed(1)} ${c.toFixed(1)}" transform="rotate(-90 88 88)"/></svg>`;
}
function vDashboard() {
  const t = dayStat(TODAY), ts = trackState(), wk = rangeStats(weekStart(TODAY), TODAY), mo = rangeStats(monthStart(TODAY), TODAY);
  const pw = rangeStats(addDays(weekStart(TODAY), -7), addDays(weekStart(TODAY), -1));
  const bal = totalBalance(), fe = forecastEnd(t), rest = Math.max(0, t.soll - t.counted), over = t.counted > t.soll;
  const trkBadge = `<span class="badge ${trkTone(ts)}"><span class="dot ${ts.cls}"></span>${ts.label}</span>`;
  const first = S.me.name.split(' ')[0];
  let msg;
  const noWork = !S.day.segs.length, dayEnded = dayClosed(), longTrack = clashBanner() + (nightBanner() || longTrackBanner(t, ts)) + plannedEndBanner(), st0 = t.start != null ? t.start : S.now - 30;
  if (ts.key === 'ask') msg = `Der Rechner läuft seit <b>${hm(S.day.start)}</b>, aber das Tracking wurde noch nicht gestartet.`;
  else if (noWork) msg = `Heute wird noch nicht getrackt. Starte das Tracking im Companion, hier im Web oder lege Zeitfenster für den automatischen Start fest.`;
  else if (ts.key === 'paused') msg = `Das Tracking ist seit <b>${hm(S.day.ended)}</b> pausiert. Die Pause wird nicht gezählt – der Tag bleibt offen.`;
  else if (S.day.stop === 'shutdown') msg = `Der Rechner wurde um <b>${hm(S.day.ended)}</b> heruntergefahren. Nach dem nächsten Start läuft das Tracking von selbst weiter.`;
  else if (dayEnded) msg = `Das Tracking endete um <b>${hm(S.day.ended)}</b>${S.day.stop === 'quit' && !endedByWeb() ? ' (Companion beendet)' : ''}. Das war dein bisheriges Arbeitsende.`;
  else if (over) msg = `Tagessoll erreicht – seit <b>${dur(t.counted - t.soll)}</b> sammelst du Überstunden.`;
  else msg = `Du hast heute um <b>${hm(t.start)}</b> begonnen. Noch <b>${dur(rest)}</b> bis zum Tagessoll.`;
  // Letzte 2 Wochen
  const items = []; let d = addDays(weekStart(TODAY), -14);
  while (d <= TODAY) {
    if (dow(d) < 5) {
      const s = dayStat(d), k = s.info.kind;
      const tip = s.info.off && k !== 'comp' ? [KINDS[k].n + (s.info.holiday ? ': ' + s.info.holiday : ''), `${dLong(d)} · Soll entfällt – kein Minus`] : [dur(s.counted), `${dLong(d)} · ${span(s)}Soll ${dur(s.soll)} · ${sdur(s.diffLive)}`];
      items.push({ lab: DOW_S[dow(d)] + ' ' + parseD(d).getDate() + '.', ist: s.counted, soll: s.soll, off: k && k !== 'comp' && !s.has ? k : null, kind: k, today: d === TODAY, tip });
    }
    d = addDays(d, 1);
  }
  const hist = balanceSeries().filter((_, i, a) => i >= a.length - 90);
  const wb = hist.map(p => ({ v: p.bal, t: sdur(p.bal), s: `Überstundenkonto · ${dLong(p.date)}` }));
  // Beschriftungen nur an vorhandenen Punkten (neue Konten haben weniger als 90 Tage Verlauf)
  const xt = [...new Set([0, 30, 60, hist.length - 1].filter(i => i >= 0 && i < hist.length))].map(i => ({ i, l: dShort(hist[i].date) }));
  const app = appUsage(TODAY, TODAY), up = nextHolidays(3);
  const vac = S.absences.filter(a => a.from > TODAY).sort((a, b) => a.from.localeCompare(b.from));
  return `<div class="g-hero">
    <div class="card"><div class="row between wrap" style="margin-bottom:6px"><div><h3 style="font-size:20px;letter-spacing:-.01em">Guten Tag, ${esc(first)}</h3><p class="muted" style="margin-top:2px">${dLong(TODAY)} · KW ${isoWeek(TODAY)}</p></div><div class="row gap-s" style="gap:10px">${ts.key === 'running' ? `<button class="btn sm" data-a="cAct" data-k="pause" title="Zeiterfassung anhalten – die Pause zählt nicht, der Tag bleibt offen">${ic('pause', 14)} Pause</button>` : ''}${trkBadge}</div></div>
      <p class="ink2" style="margin:8px 0 16px">${msg}</p>
      <div class="row wrap gap-l" style="align-items:center">
        <div class="ring">${ring(t.soll ? t.counted / t.soll : 0, over)}<div class="mid"><div class="n mono sec" id="ring-n" data-base="${t.counted}" data-run="${ts.key === 'running' ? 1 : 0}" data-at="${S.now}">${durS(t.counted)}</div><div class="u">von ${durN(t.soll)} h</div></div></div>
        <div class="grow" style="min-width:230px">
          <div class="kv"><span>Arbeitsbeginn</span><b>${t.start != null ? hm(t.start) : '–'}${t.start != null ? ` <span class="badge brand" style="margin-left:4px">${ic('power', 12)} ${startViaWeb() ? 'Web' : S.os === 'windows' ? 'Windows' : 'macOS'}-Start</span>` : ''}</b></div>
          <div class="kv"><span>Brutto (Anwesenheit)</span><b class="mono">${durN(t.gross)} h</b></div>
          <div class="kv"><span>${S.set.mode === 'net' ? 'Pause abgezogen' : 'Pause (nicht abgezogen)'}</span><b class="mono">${S.set.mode === 'net' ? '−' : ''}${durN(S.set.mode === 'net' ? t.brk : Math.max(t.rule, t.lockedBreak))} h</b></div>
          <div class="kv"><span>${dayEnded ? 'Arbeitsende' : 'Voraussichtliches Ende'}</span><b class="mono">${dayEnded ? hm(S.day.ended) : fe ? hmDay(fe) : over ? 'Soll erreicht' : '–'}</b></div>
          <div class="kv"><span>Erfassungsmodus</span><b>${S.set.mode === 'net' ? 'Netto' : 'Brutto'}</b></div>
        </div></div>
      ${longTrack}
      ${(S.day.tracking || ts.key === 'paused') && !longTrack ? `<div class="row wrap" style="margin-top:14px;gap:8px"><button class="btn" data-a="stopOpen" data-m="now" title="Tracking jetzt beenden – zeigt vorher Zeitpunkt, Brutto und Netto">${ic('power', 15)} Tracking beenden</button>${S.day.tracking && S.limits.startChange ? `<button class="btn" data-a="startOpen" title="Beginn des laufenden Trackings ändern, z. B. wenn du früher angefangen hast">${ic('edit', 15)} Beginn ändern …</button>` : ''}<button class="btn" data-a="stopOpen" data-m="custom" title="Beenden mit einem eigenen Ende, z. B. wenn du es vergessen hast">${ic('day', 15)} Ende festlegen …</button></div>` : ''}
      ${ts.key === 'ask' ? `<div style="margin-top:14px">${askBox()}</div>` : ''}
      ${ts.key === 'paused' ? `<div class="callout warn" style="margin-top:14px">${ic('pause')}<div class="grow"><b>Tracking pausiert.</b> Zeit wird nicht gezählt, bis du das Tracking fortsetzt.</div><button class="btn sm primary" data-a="cAct" data-k="start">${ic('play', 13)} Tracking fortsetzen</button></div>` : ''}
      ${ts.key === 'idle' ? `<div class="callout warn" style="margin-top:14px">${ic('info')}<div class="grow"><b>Tracking ist aus.</b> Es startet automatisch im nächsten Zeitfenster oder auf Knopfdruck.</div><button class="btn sm primary" data-a="cAct" data-k="start">${ic('play', 13)} Tracking starten</button></div>` : ''}
      ${ts.key === 'stopped' ? `<div class="callout warn" style="margin-top:14px">${ic('power')}<div class="grow"><b>Tracking beendet.</b> Es wird keine Zeit gezählt, bis das Tracking wieder gestartet wird – im Companion oder hier im Web.</div><button class="btn sm primary" data-a="cAct" data-k="start">${ic('play', 13)} Tracking starten</button></div>` : ''}
    </div>
    <div class="stack">
      <div class="tile"><div class="l">${ic('cal', 15)}Wochensaldo · KW ${isoWeek(TODAY)}</div><div class="v mono">${sgn(wk.diff)}</div><div class="s">Vorwoche KW ${isoWeek(TODAY) - 1}: ${sgn(pw.diff)}${t.diffLive < 0 && !dayEnded ? ` · heute noch ${dur(-t.diffLive)} offen` : ''}</div></div>
      <div class="tile"><div class="l">${ic('cal', 15)}Monatssaldo · ${MONTHS[parseD(TODAY).getMonth()]}</div><div class="v mono">${sgn(mo.diff)}</div><div class="s">Ist ${dur(mo.counted)} · Soll ${dur(mo.soll)}${mo.off.vacation + mo.off.holiday ? ` (${mo.off.vacation} Urlaubs-, ${mo.off.holiday} Feiertag${mo.off.holiday === 1 ? '' : 'e'} nicht gewertet)` : ''}</div></div>
      <div class="tile"><div class="l">${ic('bolt', 15)}Überstundenkonto gesamt</div><div class="v mono">${sgn(bal)}</div><div class="s">seit ${dMid(TRACK_START)} inkl. Übertrag ${sdur(S.set.startBal * 60)}</div></div>
    </div></div>
  <div class="g-2-1">
    <div class="card"><div class="ch"><h3>Letzten zwei Wochen</h3><span class="sub">Arbeitszeit je Werktag</span><span class="right legend"><span><i style="background:var(--s1)"></i>Arbeitszeit</span><span><i style="background:var(--s2)"></i>Überstunden</span><span><i class="ln"></i>Soll</span></span></div>
      ${barChart(items, { h: 190, gap: 8, minTop: 480 })}
      <div class="legend" style="margin-top:6px"><span><i style="background:var(--s7)"></i>Feiertag</span><span><i style="background:var(--s3)"></i>Urlaub</span><span><i style="background:var(--s8)"></i>Krank</span><span class="muted">· Abwesenheit senkt das Soll auf 0 und erzeugt kein Minus.</span></div></div>
    <div class="card"><div class="ch"><h3>Heute im Verlauf</h3><a class="right sm" style="color:var(--brand);font-weight:600" href="#/day" data-a="goToday">Tagesansicht ${ic('chevR', 13)}</a></div>
      <div style="padding-top:18px"></div>${timeline(todaySegs(), Math.floor((st0 - 30) / 60) * 60, Math.max(Math.ceil((S.now + 30) / 60) * 60, Math.floor((st0 - 30) / 60) * 60 + 540), { lanes: false, now: S.day.tracking ? S.now : null, mini: true, icons: true })}
      <div style="margin-top:14px">${appRank(app.list, app.total, 4)}</div></div></div>
  <div class="g-2-1">
    <div class="card"><div class="ch"><h3>Überstundenkonto</h3><span class="sub">letzte 90 Tage</span><span class="right">${sgn(bal)}</span></div>${lineChart(wb, { h: 170, xt })}</div>
    <div class="card"><div class="ch"><h3>Als Nächstes</h3></div>
      ${vac.slice(0, 2).map(a => `<div class="kv"><span style="display:flex;gap:8px;align-items:center"><span class="kd" style="--tc:var(${KINDS[a.type].c})"></span>${KINDS[a.type].n}${a.note ? ' · ' + esc(a.note) : ''}</span><b>${dShort(a.from)}–${dShort(a.to)}</b></div>`).join('')}
      ${up.map(h => `<div class="kv"><span style="display:flex;gap:8px;align-items:center"><span class="kd" style="--tc:var(--s7)"></span>${esc(h.name)}</span><b>${DOW_S[dow(h.date)]}, ${dShort(h.date)}</b></div>`).join('')}
      <p class="hint" style="margin-top:10px">Feiertage für ${STATES[S.set.state]}.</p></div></div>`;
}

/* ---------- Tagesansicht ---------- */
/* Nachträgliches Bearbeiten: erlaubt für Tage bis zur vom Administrator eingestellten Zahl zurück */
const canEditDay = d => S.limits.editDaysBack > 0 && d <= TODAY && d >= TRACK_START && diffDays(d, TODAY) <= S.limits.editDaysBack;
/* Zeitleiste im Bearbeiten-Fenster: Arbeit nach App gefärbt, Pausen schraffiert, Lücken gestrichelt */
function editTimeline(segs, gaps, openFrom) {
  const all = [...segs, ...gaps];
  if (openFrom != null) all.push({ from: openFrom, to: openFrom + 1 });
  const lo = Math.min(...all.map(x => x.from)), hi = Math.max(...all.map(x => x.to));
  const from = Math.max(0, Math.floor((lo - 30) / 60) * 60), to = Math.min(1440, Math.max(Math.ceil((hi + 30) / 60) * 60, from + 6 * 60)), W = to - from;
  const L = m => ((m - from) / W * 100).toFixed(3), Wd = (a, b) => ((b - a) / W * 100).toFixed(3), ticks = tlAxis(from, to);
  const bars = segs.map(g => g.kind === 'app'
    ? `<i class="sg" style="left:${L(g.from)}%;width:${Wd(g.from, g.to)}%;background:var(${appOf(g.app).c})" ${tipAttr(dur(g.to - g.from), `${appName(g.app)} · ${hm(g.from)}–${hm(g.to)}${g.note ? ' · ' + g.note : ''}`)}></i>`
    : `<i class="sg locked" style="left:${L(g.from)}%;width:${Wd(g.from, g.to)}%" ${tipAttr(dur(g.to - g.from), `Pause · ${hm(g.from)}–${hm(g.to)}`)}></i>`).join('')
    + gaps.map(g => `<i class="sg gap" style="left:${L(g.from)}%;width:${Wd(g.from, g.to)}%" ${tipAttr(dur(g.to - g.from), `Lücke · ${hm(g.from)}–${hm(g.to)} · wird nicht gezählt`)}></i>`).join('');
  return `<div class="tl mini de-tl"><div class="tl-labels"></div><div class="tl-plot"><div class="tl-grid">${ticks.map(x => `<i style="left:${L(x)}%"></i>`).join('')}</div>
    <div class="tl-track">${bars}${openFrom != null ? `<i class="sg run" style="left:${L(openFrom)}%;right:0" ${tipAttr('Läuft noch', `ab ${hm(openFrom)} · wird vom Companion geführt`)}></i>` : ''}</div>
    <div class="tl-axis">${ticks.map(x => `<span style="left:${L(x)}%">${pad(x / 60)}</span>`).join('')}</div></div></div>`;
}
const deTime = (b, f, v) => `<input type="text" class="ttxt" inputmode="numeric" placeholder="HH:MM" maxlength="5" autocomplete="off" aria-label="${f === 'from' ? 'Von' : 'Bis'}" value="${hm(v)}" data-de="${f}" data-b="${b}">`;
/* Formular „Neuer Eintrag“ im Bearbeiten-Fenster: Art (Arbeitszeit oder Pause), Zeiten, Bezeichnung, Notiz */
function daInfo(m) {
  const a = m.add, err = deLimit(m, a.from, a.to);
  if (err) return { err, html: esc(err) };
  const over = deOverlap(m.segs, a.from, a.to, -1);
  return { err: null, html: over >= 1 ? `${ic('info', 14)} Ersetzt ${dur(over)} bestehender Zeit – ein Eintrag, der dort liegt, wird geteilt oder gekürzt.` : `${ic('check', 14)} Passt in freie Zeit, nichts wird ersetzt.` };
}
function daForm(m) {
  const a = m.add, info = daInfo(m), pause = a.kind === 'locked';
  const tf = (f, v) => `<span class="tfield"><input type="text" class="ttxt" inputmode="numeric" placeholder="HH:MM" maxlength="5" autocomplete="off" aria-label="${f === 'from' ? 'Von' : 'Bis'}" value="${hm(v)}" data-da="${f}"><button type="button" class="ibtn tpick ${m.tp === f ? 'on' : ''}" data-a="daPick" data-f="${f}" title="Uhrzeit wählen" aria-label="Uhrzeit wählen" aria-expanded="${m.tp === f}">${ic('day', 16)}</button></span>`;
  const panel = () => { const f = m.tp, v = a[f], h = Math.floor(v / 60), mi = Math.round(v % 60);
    return `<div class="tpanel"><div class="tp-h"><b>${f === 'from' ? 'Von' : 'Bis'}</b><span class="mono">${hm(v)}</span><button class="ibtn right" data-a="daPick" data-f="${f}" title="Schließen">${ic('x', 14)}</button></div>
      <div class="tp-l">Stunde</div><div class="tp-g h">${Array.from({ length: 24 }, (_, i) => `<button type="button" class="${i === h ? 'on' : ''}" data-a="daTime" data-p="h" data-v="${i}">${pad(i)}</button>`).join('')}</div>
      <div class="tp-l">Minute</div><div class="tp-g m">${Array.from({ length: 12 }, (_, i) => i * 5).map(i => `<button type="button" class="${i === mi ? 'on' : ''}" data-a="daTime" data-p="m" data-v="${i}">${pad(i)}</button>`).join('')}</div>
      <div class="tp-n">Minutengenau: die Zeit oben im Feld eintippen.</div></div>`; };
  return `<div class="de-add"><div class="row between"><b>Neuer Eintrag</b><button class="ibtn" data-a="daClose" title="Schließen">${ic('x', 15)}</button></div>
    <div class="de-addgrid">
      <div class="f"><span>Art</span><div class="seg"><button data-a="daKind" data-v="app" class="${pause ? '' : 'on'}">Arbeitszeit</button><button data-a="daKind" data-v="locked" class="${pause ? 'on' : ''}">${ic('coffee', 13)} Pause</button></div></div>
      <label class="f"><span>Von</span>${tf('from', a.from)}</label>
      <label class="f"><span>Bis</span>${tf('to', a.to)}</label>
      <span class="de-dur mono muted" id="da-dur">${a.to > a.from ? dur(a.to - a.from) : ''}</span>
      ${pause ? `<div class="f grow"><span>&nbsp;</span><span class="muted xs">${!S.set.lockAsBreak ? 'Zählt nicht als Pause (Einstellung)' : `Ab ${S.set.chunk} Minuten wird sie als Pause abgezogen.`}</span></div>`
        : `<label class="f grow"><span>App / Bezeichnung</span><input type="text" list="app-names" value="${esc(a.app)}" placeholder="Manuell" maxlength="64" data-da="app"></label>
      <label class="f grow"><span>Notiz</span><input type="text" value="${esc(a.note)}" placeholder="z. B. Kundentermin" maxlength="200" data-da="note"></label>`}
    </div>
    ${m.tp ? panel() : ''}
    <div class="de-addfoot"><span class="de-hint ${info.err ? 'bad' : ''}" id="da-hint">${info.html}</span><button class="btn primary sm" id="da-ok" data-a="daSubmit" ${info.err ? 'disabled' : ''}>${ic('plus', 14)} Eintrag einfügen</button></div></div>`;
}
function dayEditModal(m) {
  const rows = DayEdit.blocks(m.segs), gaps = DayEdit.gaps(rows, m.open), sum = DayEdit.sums(m.segs), gapMin = gaps.reduce((a, g) => a + g.to - g.from, 0);
  const dirty = JSON.stringify(DayEdit.out(m.segs)) !== m.orig;
  // Lücken stehen vor dem Eintrag, der nach ihnen beginnt; die letzte (bis zum laufenden Abschnitt) am Ende
  const items = [];
  rows.forEach(r => { const g = gaps.find(x => Math.abs(x.to - r.from) < DayEdit.EPS); if (g) items.push({ gap: g }); items.push({ r }); });
  gaps.filter(g => !items.some(x => x.gap === g)).forEach(g => items.push({ gap: g }));
  const row = r => {
    const pause = r.kind === 'locked', one = r.apps.length === 1, l = r.to - r.from;
    const what = pause ? `<span class="muted">${ic('coffee', 15)} Pause</span>`
      : one ? `<input type="text" list="app-names" value="${esc(appName(r.apps[0].app))}" placeholder="Manuell" data-de="app" data-b="${r.b}" maxlength="64" aria-label="App oder Bezeichnung">`
      : `<div class="de-apps" title="Mehrere Programme in diesem Eintrag – die Zeiten oben ändern nur Beginn und Ende">${r.apps.slice(0, 2).map(a => `<span class="de-app">${appIcon(a.app, 16)}${esc(appName(a.app))}<small>${durN(a.min)}</small></span>`).join('')}${r.apps.length > 2 ? `<span class="muted xs">+${r.apps.length - 2}</span>` : ''}</div>`;
    const note = pause ? `<span class="muted xs">${!S.set.lockAsBreak ? 'zählt nicht als Pause (Einstellung)' : l < S.set.chunk ? `kürzer als ${S.set.chunk} Min. – zählt nicht als Pause` : 'wird als Pause abgezogen'}</span>`
      : `<input type="text" value="${esc(r.note || '')}" placeholder="z. B. Kundentermin" data-de="note" data-b="${r.b}" maxlength="200" aria-label="Notiz">`;
    return `<tr class="${pause ? 'de-pause' : 'de-work'}${m.flash === r.b ? ' flash' : ''}"><td>${deTime(r.b, 'from', r.from)}</td><td>${deTime(r.b, 'to', r.to)}</td><td class="mono muted">${dur(l)}</td>
      <td><select data-de="kind" data-b="${r.b}" aria-label="Art"><option value="app" ${pause ? '' : 'selected'}>Arbeitszeit</option><option value="locked" ${pause ? 'selected' : ''}>Pause</option></select></td>
      <td>${what}</td><td>${note}</td><td><button class="ibtn" data-a="deDel" data-b="${r.b}" title="Eintrag entfernen">${ic('trash', 16)}</button></td></tr>`;
  };
  const gap = g => `<tr class="de-gap"><td colspan="7"><div class="de-gapbox">${ic('day', 16)}<span class="grow"><b>Lücke ${hm(g.from)}–${hm(g.to)}</b> · ${dur(g.to - g.from)} werden nicht gezählt<small>Kein Tracking in dieser Zeit, z. B. Rechner aus oder ein Termin vor Ort</small></span><button class="btn sm primary" data-a="deBook" data-from="${g.from}" data-to="${g.to}" title="Eintrag für diese Zeit anlegen: Art, Bezeichnung und Notiz wählst du selbst">${ic('edit', 13)} Nachbuchen</button><button class="btn sm" data-a="deFill" data-from="${g.from}" data-to="${g.to}" title="Die Lücke sofort als Arbeitszeit „Manuell“ füllen">${ic('plus', 13)} Als Arbeitszeit füllen</button></div></td></tr>`;
  const body = items.map(x => x.gap ? gap(x.gap) : row(x.r)).join('') + (m.open != null ? `<tr class="muted"><td>${hm(m.open)}</td><td colspan="6">läuft noch – wird vom Companion geführt</td></tr>` : '');
  return `<div class="ov"><div class="modal de" style="width:min(940px,100%)"><div class="mh"><h3>Zeiten bearbeiten · ${dLong(m.date)}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p class="hint">Jeder Eintrag fasst die Zeit zwischen zwei Unterbrechungen zusammen – Programmwechsel darin bleiben erhalten. Ein neuer Eintrag ersetzt, was an seiner Stelle steht: eine Pause mitten im Arbeitstag teilt ihn in zwei Teile. Lücken zählen nicht als Arbeitszeit.</p>
      ${rows.length ? `<div class="de-tlbox">${editTimeline(m.segs, gaps, m.open)}<div class="legend"><span><i style="background:var(--brand)"></i>Arbeitszeit (Farbe = Programm)</span><span><i class="hatch"></i>Pause</span><span><i class="dash"></i>Lücke</span></div></div>` : ''}
      <div class="tbwrap"><table class="tb edit-tb"><thead><tr><th>Von</th><th>Bis</th><th>Dauer</th><th>Art</th><th>App / Bezeichnung</th><th>Notiz</th><th></th></tr></thead><tbody>
      ${body || '<tr><td colspan="7" class="muted" style="text-align:center;padding:18px">Keine Einträge – mit „Eintrag hinzufügen“ Zeit ergänzen.</td></tr>'}
      </tbody></table></div>
      <datalist id="app-names">${Object.values(APPS).filter(a => a.n !== 'Sonstiges').map(a => `<option value="${esc(a.n)}">`).join('')}<option value="Manuell"></datalist>
      ${m.add ? daForm(m) : `<div class="row wrap"><button class="btn sm" data-a="deAdd">${ic('plus', 14)} Eintrag hinzufügen</button><span class="muted xs">Arbeitszeit, Termin ohne Rechner, Pause …</span></div>`}</div>
    <div class="mf"><div class="de-sum"><span>Arbeitszeit <b>${dur(sum.work)}</b></span><span>Pausen <b>${dur(sum.pause)}</b></span>${gapMin >= 1 ? `<span class="warn">Lücken <b>${dur(gapMin)}</b></span>` : ''}</div>
      <div class="row"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" id="de-save" data-a="dayEditSave" ${dirty ? '' : 'disabled'}>Speichern</button></div></div></div></div>`;
}
function vDay() {
  const date = S.dayDate, s = dayStat(date), info = s.info, isToday = date === TODAY, liveDay = isToday && !dayClosed();
  const nav = `<div class="row wrap"><div class="row gap-s"><button class="ibtn bd" data-a="dayNav" data-n="-1" title="Vorheriger Tag">${ic('chevL')}</button><button class="ibtn bd" data-a="dayNav" data-n="1" title="Nächster Tag" ${date >= TODAY ? 'disabled style="opacity:.4"' : ''}>${ic('chevR')}</button></div>
    ${dateField('dayDate', date, { max: TODAY, cls: 'inl' })}
    <h2 style="font-size:18px;font-weight:650">${dLong(date)}</h2>${isToday ? '<span class="badge brand">Heute</span>' : `<button class="btn sm" data-a="dayNav" data-to="${TODAY}">Zu heute</button>`}${liveDay && S.day.tracking && S.limits.startChange ? `<button class="btn sm right" data-a="startOpen" title="Beginn des laufenden Trackings ändern">${ic('edit', 14)} Beginn ändern</button>` : ''}${canEditDay(date) ? `<button class="btn sm ${liveDay && S.day.tracking && S.limits.startChange ? '' : 'right'}" data-a="dayEdit">${ic('edit', 14)} ${s.has ? 'Zeiten bearbeiten' : 'Zeit eintragen'}</button>` : ''}</div>`;
  if (s.future) return `<div class="stack">${nav}<div class="card empty"><div class="big">${ic('cal', 26)}</div><h3>Dieser Tag liegt in der Zukunft</h3><p>${info.holiday ? esc(info.holiday) + ' – ' : ''}Soll: ${dur(info.soll)}</p></div></div>`;
  if (!s.has) {
    const k = info.kind, label = k ? KINDS[k].n + (info.holiday ? ': ' + info.holiday : '') : info.weekend ? 'Wochenende' : 'Keine Arbeitszeit erfasst';
    return `<div class="stack">${nav}<div class="card empty"><div class="big">${ic(k === 'sick' ? 'shield' : k ? 'umb' : 'cal', 26)}</div><h3>${esc(label)}</h3>
      <p>${k && k !== 'comp' ? `Tagessoll <b>0:00 h</b> – dieser Tag wird nicht als „nicht geleistete Arbeit“ gewertet und belastet dein Überstundenkonto nicht.` : k === 'comp' ? `Zeitausgleich: Das Tagessoll von <b>${dur(info.soll)}</b> wird bewusst vom Überstundenkonto abgezogen.` : info.weekend ? 'Soll 0:00 h – keine Erfassung nötig.' : isToday ? 'Heute wurde noch nicht getrackt.' : 'Für diesen Tag liegen keine Tracking-Daten vor.'}</p>
      ${isToday && S.day.stop === 'ask' ? askBox() : ''}
      ${!k && !info.weekend ? `<button class="btn" data-a="absNewDate" data-d="${date}">${ic('umb', 16)} Abwesenheit eintragen</button>` : ''}</div></div>`;
  }
  const from = Math.max(0, Math.floor((s.start - 30) / 60) * 60), toRaw = Math.ceil(((liveDay ? S.now : s.end) + 30) / 60) * 60, to = Math.min(1440, Math.max(toRaw, from + 9 * 60));
  const segs = s.segs, app = appUsage(date, date), ended = isToday ? S.day.ended : s.end;
  const hoursCols = []; for (let h = Math.floor(from / 60); h < Math.ceil(to / 60); h++) {
    const by = {}; segs.forEach(g => { if (g.kind !== 'app') return; const a = Math.max(g.from, h * 60), b = Math.min(g.to, h * 60 + 60); if (b > a) { const k = ak(g.app); by[k] = (by[k] || 0) + (b - a); } });
    const tot = Object.values(by).reduce((x, y) => x + y, 0);
    hoursCols.push(`<div class="hr-col"><div class="stk">${Object.keys(by).map(k => `<i style="height:${by[k] / 60 * 100}%;background:var(${appOf(k).c})" ${tipAttr(Math.round(by[k]) + ' Min.', `${appOf(k).n} · ${pad(h)}:00–${pad(h + 1)}:00`)}>${by[k] >= 8 ? appGlyph(k, by[k] >= 14 ? 18 : 14) : ''}</i>`).join('')}</div><div class="lb">${pad(h)}</div></div>`);
  }
  const baseEvents = isToday ? S.day.events : histEvents(date);
  // Programmwechsel aus dem Zeitverlauf, auf Wunsch eingeblendet
  const switches = S.showSwitches ? (segs || []).filter(g => g.kind === 'app').map(g => ({ t: g.from, k: 'app', txt: `Wechsel zu ${appName(g.app)}${g.to != null ? ` · ${dur(g.to - g.from)}` : ''}` })) : [];
  const events = baseEvents.concat(switches).sort((a, b) => a.t - b.t).reverse(); // neueste zuerst (bei gleicher Minute bleibt die Reihenfolge der Ereignisse, umgekehrt)
  const iconFor = EVICON;
  const dayDiff = s.diffLive;
  return `<div class="stack">${nav}
    ${info.holiday || info.kind ? `<div class="callout warn">${ic('info')}<span>Auf diesen Tag ${info.holiday ? `fällt der Feiertag <b>${esc(info.holiday)}</b>` : `entfällt ${KINDS[info.kind].n}`}. Es wurde trotzdem gearbeitet – die gesamte Zeit zählt als Überstunden.</span></div>` : ''}
    <div class="g4" style="grid-template-columns:repeat(auto-fit,minmax(150px,1fr))">
      <div class="tile sm"><div class="l">Arbeitsbeginn</div><div class="v mono">${hm(s.start)}</div><div class="s">automatisch erkannt</div></div>
      <div class="tile sm"><div class="l">${liveDay ? 'Anwesenheit (Brutto)' : 'Arbeitsende'}</div><div class="v mono">${liveDay ? durN(s.gross) + '<small>h</small>' : hm(ended)}</div><div class="s">${liveDay ? (S.day.stop === 'pause' ? 'Tracking pausiert' : 'seit Arbeitsbeginn') + (S.day.plannedEnd != null ? ` · endet um ${hm(S.day.plannedEnd)}` : '') : (isToday && S.day.stop === 'shutdown' ? 'Rechner aus' : 'Companion beendet') + ' · Brutto ' + dur(s.gross)}</div></div>
      <div class="tile sm"><div class="l">Pausen</div><div class="v mono">${durN(S.set.mode === 'net' ? s.brk : Math.max(s.rule, s.lockedBreak))}<small>h</small></div><div class="s">${S.set.mode === 'net' ? 'abgezogen' : 'nicht abgezogen (Brutto)'}</div></div>
      <div class="tile sm"><div class="l">${S.set.mode === 'net' ? 'Netto-Arbeitszeit' : 'Brutto-Arbeitszeit'}</div><div class="v mono">${durN(s.counted)}<small>h</small></div><div class="s">Soll ${dur(s.soll)}</div></div>
      <div class="tile sm"><div class="l">Tagessaldo</div><div class="v mono">${sgn(dayDiff)}</div><div class="s">${liveDay ? 'Tag läuft noch' : 'Tag abgeschlossen'}</div></div>
    </div>
    <div class="card"><div class="ch"><h3>Zeitverlauf</h3><span class="sub">Aktive App und Sperrzeiten</span></div>
      ${timeline(segs, from, to, { now: isToday && S.day.tracking ? S.now : null })}
      <div style="margin-top:10px">${appLegend()}</div></div>
    <div class="g-2-1">
      <div class="card"><div class="ch"><h3>Aktivität pro Stunde</h3><span class="sub">Minuten je App</span></div><div class="hours">${hoursCols.join('')}</div></div>
      <div class="card"><div class="ch"><h3>Meistgenutzte Apps</h3><span class="right muted sm">${dur(app.total)} aktiv</span></div>${appRank(app.list, app.total)}</div></div>
    <div class="g-1-1">
      <div class="card"><div class="ch"><h3>Wie die Zeit berechnet wird</h3></div>
        <div class="kv"><span>Brutto (Start bis ${liveDay ? 'jetzt' : 'Ende'})</span><b class="mono">${dur(s.gross)}</b></div>
        <div class="kv"><span>Gesperrt gesamt</span><b class="mono">${dur(s.locked)}</b></div>
        <div class="kv"><span>Als Pause gewertet (Sperre ≥ ${S.set.chunk} Min.)</span><b class="mono">${S.set.lockAsBreak ? dur(s.lockedBreak) : 'aus'}</b></div>
        <div class="kv"><span>Gesetzliche Mindestpause</span><b class="mono">${dur(s.rule)}</b></div>
        <div class="kv"><span>Abgezogen (${S.set.mode === 'net' ? 'größerer Wert' : 'Brutto-Modus: nichts'})</span><b class="mono">${S.set.mode === 'net' ? '−' + dur(s.brk) : '0:00 h'}</b></div>
        <div class="kv"><span><b>Gezählte Arbeitszeit</b></span><b class="mono">${dur(s.counted)}</b></div></div>
      <div class="card"><div class="ch"><h3>Ereignisse</h3><span class="right row" style="gap:8px"><span class="muted xs">Programmwechsel</span><button class="switch ${S.showSwitches ? 'on' : ''}" data-a="switchesTog" role="switch" aria-checked="${S.showSwitches}" title="Programmwechsel in der Liste anzeigen" style="width:34px;height:20px"></button></span></div><div class="evl">${events.map(e => `<div><time>${hm(e.t)}</time><span class="muted">${ic(iconFor[e.k] || 'info', 15)}</span><span>${esc(e.txt)}</span></div>`).join('')}</div></div></div>
  </div>`;
}

/* ---------- Statistik ---------- */
function statRange() {
  const r = S.statRef;
  if (S.statMode === 'week') { const a = weekStart(r); return { from: a, to: addDays(a, 6), title: `KW ${isoWeek(a)} · ${dShort(a)} – ${dShort(addDays(a, 6))} ${a.slice(0, 4)}`, name: `Wochensaldo KW ${isoWeek(a)}` }; }
  if (S.statMode === 'month') { const a = monthStart(r); return { from: a, to: monthEnd(a), title: `${MONTHS[parseD(a).getMonth()]} ${a.slice(0, 4)}`, name: `Monatssaldo ${MONTHS[parseD(a).getMonth()]}` }; }
  const y = r.slice(0, 4); return { from: `${y}-01-01`, to: `${y}-12-31`, title: y, name: `Jahressaldo ${y}` };
}
function vStats() {
  const R = statRange(), st = rangeStats(R.from, R.to), mode = S.statMode;
  const hasData = st.days.length > 0;
  const head = `<div class="row wrap"><div class="seg">${[['week', 'Woche'], ['month', 'Monat'], ['year', 'Jahr']].map(([k, l]) => `<button class="${mode === k ? 'on' : ''}" data-a="statMode" data-v="${k}">${l}</button>`).join('')}</div>
    <div class="row gap-s"><button class="ibtn bd" data-a="statNav" data-n="-1">${ic('chevL')}</button><button class="ibtn bd" data-a="statNav" data-n="1">${ic('chevR')}</button></div>
    <h2 style="font-size:18px;font-weight:650">${R.title}</h2><button class="btn sm" data-a="statNav" data-to="${TODAY}">Aktuell</button>
    <button class="btn sm right" data-a="exportCsv">${ic('download', 15)} Export</button></div>`;
  if (!hasData) return `<div class="stack">${head}<div class="card empty"><div class="big">${ic('chart', 26)}</div><h3>Keine Daten für diesen Zeitraum</h3><p>Die Zeiterfassung beginnt am ${dMid(TRACK_START)}.</p></div></div>`;
  // Balken: Tage (Woche/Monat) oder Wochen (Jahr)
  let items = [], labelEvery = 1, gap = 6;
  if (mode !== 'year') {
    const end = R.to > TODAY ? TODAY : R.to; let d = R.from;
    if (mode === 'month') { gap = 4; labelEvery = 2; }
    while (d <= R.to) {
      const s = dayStat(d), k = s.info.kind;
      const offKind = k && !s.has && s.info.base > 0 ? k : null;
      let tip;
      if (d > TODAY) tip = [dLong(d), 'Noch nicht erfasst'];
      else if (!s.has && s.info.weekend) tip = [s.info.holiday ? s.info.holiday + ' (Wochenende)' : 'Wochenende', dLong(d)];
      else if (offKind && offKind !== 'comp') tip = [KINDS[offKind].n + (s.info.holiday ? ': ' + s.info.holiday : ''), `${dLong(d)} · Soll entfällt`];
      else if (offKind === 'comp') tip = ['Zeitausgleich', `${dLong(d)} · ${sdur(-s.soll)} Überstundenabbau`];
      else if (!s.has && s.info.weekend) tip = ['Wochenende', dLong(d)];
      else tip = [dur(s.counted), `${dLong(d)} · ${span(s)}Soll ${dur(s.soll)} · ${sdur(s.diffLive)}`];
      if (!(mode === 'month' && s.info.weekend && !s.has && d <= TODAY) || true) items.push({ lab: mode === 'week' ? DOW_S[dow(d)] + ' ' + parseD(d).getDate() + '.' : String(parseD(d).getDate()), ist: s.counted, soll: s.soll, off: offKind && offKind !== 'comp' ? offKind : null, kind: k, today: d === TODAY, wk: s.info.weekend, tip });
      d = addDays(d, 1);
    }
  } else {
    gap = 3; labelEvery = 4;
    let w = weekStart(R.from < TRACK_START ? TRACK_START : R.from);
    while (w <= R.to && w <= TODAY) {
      const r = rangeStats(w, addDays(w, 6));
      items.push({ lab: 'KW' + isoWeek(w), ist: r.counted, soll: r.soll, today: weekStart(TODAY) === w, tip: [dur(r.counted), `KW ${isoWeek(w)} · Soll ${dur(r.soll)} · ${sdur(r.diff)}`] });
      w = addDays(w, 7);
    }
  }
  // Saldo-Linie (Zeitraum, kumuliert ab Start)
  const bs = balanceSeries(), inR = bs.filter(p => p.date >= R.from && p.date <= R.to);
  const short = mode !== 'year' && inR.length < 7, lineSrc = mode === 'year' ? bs : short ? bs.slice(-28) : inR;
  const lp = lineSrc.map(p => ({ v: p.bal, t: sdur(p.bal), s: `Überstundenkonto · ${dLong(p.date)}` }));
  const nL = lp.length, xt = nL > 1 ? [0, Math.floor(nL / 3), Math.floor(nL * 2 / 3), nL - 1].map(i => ({ i, l: dShort(lineSrc[i].date) })) : [];
  const app = appUsage(R.from, R.to);
  const offTotal = st.off.holiday + st.off.vacation + st.off.sick;
  const kpi = `<div class="g4" style="grid-template-columns:repeat(auto-fit,minmax(170px,1fr))">
    <div class="tile"><div class="l">${R.name}</div><div class="v mono">${sgn(st.diff)}</div><div class="s">Ist ${dur(st.counted)} · Soll ${dur(st.soll)}</div></div>
    <div class="tile"><div class="l">Überstundenkonto gesamt</div><div class="v mono">${sgn(totalBalance())}</div><div class="s">inkl. Übertrag ${sdur(S.set.startBal * 60)}</div></div>
    <div class="tile"><div class="l">Ø Arbeitsbeginn</div><div class="v mono">${st.avgStart ? hm(st.avgStart) : '–'}</div><div class="s">an ${st.worked} Arbeitstagen</div></div>
    <div class="tile"><div class="l">Ø Arbeitszeit je Tag</div><div class="v mono">${durN(st.avgDay)}<small>h</small></div><div class="s">${st.longest ? `Längster Tag: ${dur(st.longest.counted)} (${dShort(st.longest.date)})` : ''}</div></div>
    <div class="tile"><div class="l">Nicht gewertet</div><div class="v mono">${offTotal}<small>Tage</small></div><div class="s">${st.off.vacation} Urlaub · ${st.off.holiday} Feiertag${st.off.holiday === 1 ? '' : 'e'} · ${st.off.sick} Krank${st.off.comp ? ` · ${st.off.comp} Zeitausgleich (zählt)` : ''}</div></div></div>`;
  // Heatmap oder Wochen-Zeitleisten
  let third;
  if (mode === 'week') {
    let rows = '', ax = [1440, 0], sumNet = 0, sumBrk = 0, sumGross = 0;
    st.days.forEach(s => { if (s.has) { ax[0] = Math.min(ax[0], s.start); ax[1] = Math.max(ax[1], s.end); } });
    const from = Math.floor((ax[0] - 30) / 60) * 60, to = Math.ceil((ax[1] + 30) / 60) * 60;
    let dd = R.from; while (dd <= R.to) {
      const s = dayStat(dd);
      rows += `<div class="week-row"><span class="sm ${dd === TODAY ? '' : 'ink2'}" style="font-weight:${dd === TODAY ? 700 : 500}">${DOW_S[dow(dd)]} ${parseD(dd).getDate()}.</span>${s.has ? timeline(s.segs, from, to, { lanes: false, mini: true }) : `<div class="tl-track" style="opacity:.6;background:var(--surface-2);display:flex;align-items:center;padding-left:10px;font-size:12px;color:var(--muted)">${s.info.kind ? KINDS[s.info.kind].n + (s.info.holiday ? ' · ' + esc(s.info.holiday) : '') : dd > TODAY ? '' : s.info.weekend ? 'Wochenende' : '–'}</div>`}<span class="wk-t" ${s.has ? tipAttr('Arbeitszeit ' + dur(s.net), `Brutto ${dur(s.gross)} · Pause ${dur(s.brk)}`) : ''}>${s.has ? `<b class="mono">${dur(s.net)}</b><small>${s.brk > 0 ? 'Pause ' + durN(s.brk) : 'ohne Pause'}</small>` : ''}</span></div>`;
      if (s.has) { sumNet += s.net; sumBrk += s.brk; sumGross += s.gross; }
      dd = addDays(dd, 1);
    }
    third = `<div class="card"><div class="ch"><h3>Arbeitszeiten der Woche</h3><span class="sub">App-Verlauf und Arbeitszeit je Tag (ohne Pausen)</span></div><div class="week-rows">${rows}</div><div class="row between" style="margin-top:8px;padding-top:10px;border-top:1px solid var(--line)"><span class="muted sm">Summe der Woche · Brutto ${dur(sumGross)} · Pausen ${dur(sumBrk)}</span><span class="mono" style="font-weight:700">${dur(sumNet)} <span class="muted" style="font-weight:500">netto</span></span></div><div style="margin-top:10px">${appLegend()}</div></div>`;
  } else {
    const cs = mode === 'month' ? 34 : 15, cols = []; let w = weekStart(R.from < TRACK_START ? R.from : R.from);
    w = weekStart(R.from);
    while (w <= R.to) {
      const cells = []; let lbl = '';
      for (let i = 0; i < 7; i++) {
        const d = addDays(w, i); if (d < R.from || d > R.to) { cells.push('<span></span>'); continue; }
        if (parseD(d).getDate() <= 7 && i === 0 && mode === 'year') lbl = MON_S[parseD(d).getMonth()];
        const s = dayStat(d); let cls, tip;
        if (d > TODAY || d < TRACK_START) { cls = 'none'; tip = [dLong(d), d > TODAY ? 'Noch nicht erfasst' : 'Vor Start der Erfassung']; }
        else if (!s.has && s.info.weekend) { cls = 'none'; tip = [s.info.holiday ? s.info.holiday + ' (Wochenende)' : 'Wochenende', dLong(d)]; }
        else if (s.info.kind && !s.has && s.info.kind !== 'comp') { cls = 'off'; tip = [KINDS[s.info.kind].n + (s.info.holiday ? ': ' + s.info.holiday : ''), `${dLong(d)} · nicht gewertet`]; }
        else if (!s.has && s.info.weekend) { cls = 'none'; tip = ['Wochenende', dLong(d)]; }
        else { const v = s.live && !dayClosed() ? s.diff : s.diffLive, a = Math.abs(v); const lvl = a < 10 ? 0 : a < 40 ? 1 : a < 80 ? 2 : a < 120 ? 3 : 4; cls = lvl === 0 ? 'z0' : (v > 0 ? 'p' : 'n') + lvl; tip = [sdur(v), `${dLong(d)} · ${dur(s.counted)} von ${dur(s.soll)}${s.info.kind === 'comp' ? ' · Zeitausgleich' : ''}`]; }
        cells.push(`<i class="cell ${cls}" ${tipAttr(tip[0], tip[1])}>${mode === 'month' ? `<span style="font-size:10.5px;color:${cls.match(/[pn][34]|^n4|^p4/) ? '#fff' : 'var(--ink-2)'};display:grid;place-items:center;height:100%;font-style:normal">${parseD(d).getDate()}</span>` : ''}</i>`);
      }
      cols.push(`<div class="hc">${mode === 'year' ? `<span class="mo">${lbl}</span>` : ''}${cells.join('')}</div>`);
      w = addDays(w, 7);
    }
    const lab = `<div class="hd">${mode === 'year' ? '<span class="mo" style="height:14px"></span>' : ''}${DOW_S.map(x => `<span>${x}</span>`).join('')}</div>`;
    const heatCard = `<div class="card"><div class="ch"><h3>Abweichung vom Tagessoll</h3><span class="sub">schraffiert = nicht gewertet</span></div>
      <div class="heat" style="--cs:${cs}px">${lab}${cols.join('')}</div>
      <div class="row wrap" style="margin-top:12px;gap:18px"><span class="scale"><span>−2 h</span>${['n4', 'n3', 'n2', 'n1', 'z0', 'p1', 'p2', 'p3', 'p4'].map(c => `<i class="cell ${c}" style="width:18px;height:12px"></i>`).join('')}<span>+2 h</span></span><span class="legend"><span><i class="hatch"></i>Urlaub / Feiertag / Krank</span></span></div></div>`;
    const bmap = {}; bs.forEach(p => bmap[p.date] = p.bal);
    const rows = [];
    if (mode === 'month') {
      let w = weekStart(R.from);
      while (w <= R.to && w <= TODAY) { const a = w < R.from ? R.from : w, b = addDays(w, 6) > R.to ? R.to : addDays(w, 6), e = b > TODAY ? TODAY : b, r = rangeStats(a, e); rows.push({ name: 'KW ' + isoWeek(w), sub: dShort(a) + ' – ' + dShort(b), ist: r.counted, soll: r.soll, diff: r.diff, bal: bmap[e] }); w = addDays(w, 7); }
    } else {
      for (let mi = 0; mi < 12; mi++) { const a = `${R.from.slice(0, 4)}-${pad(mi + 1)}-01`; if (a > TODAY || monthEnd(a) < TRACK_START) continue; const e = monthEnd(a) > TODAY ? TODAY : monthEnd(a), r = rangeStats(a, e); rows.push({ name: MONTHS[mi], sub: dShort(a < TRACK_START ? TRACK_START : a) + ' – ' + dShort(e), ist: r.counted, soll: r.soll, diff: r.diff, bal: bmap[e] }); }
    }
    const tbl = `<div class="card"><div class="ch"><h3>${mode === 'month' ? 'Wochensalden' : 'Monatssalden'}</h3><span class="sub">Überstunden je ${mode === 'month' ? 'Kalenderwoche' : 'Monat'} – Abwesenheit nicht als Minus gewertet</span></div><div class="tbwrap"><table class="tb"><thead><tr><th>Zeitraum</th><th class="num">Ist</th><th class="num">Soll</th><th class="num">Saldo</th><th class="num">Konto danach</th></tr></thead><tbody>${rows.map(r => `<tr><td><b>${r.name}</b> <span class="xs muted">${r.sub}</span></td><td class="num">${dur(r.ist)}</td><td class="num">${dur(r.soll)}</td><td class="num">${sgn(r.diff)}</td><td class="num">${sdur(r.bal)}</td></tr>`).join('')}</tbody></table></div></div>`;
    third = mode === 'month' ? `<div class="g-1-1">${heatCard}${tbl}</div>` : `${heatCard}${tbl}`;
  }
  return `<div class="stack">${head}${kpi}
    <div class="card"><div class="ch"><h3>Ist vs. Soll</h3><span class="sub">${mode === 'year' ? 'je Kalenderwoche' : 'je Tag'}</span><span class="right legend"><span><i style="background:var(--s1)"></i>Arbeitszeit</span><span><i style="background:var(--s2)"></i>Überstunden</span><span><i class="ln"></i>Soll</span></span></div>
      ${barChart(items, { h: 220, gap, labelEvery, minTop: mode === 'year' ? 0 : 480 })}
      <div class="legend" style="margin-top:6px"><span><i style="background:var(--s7)"></i>Feiertag</span><span><i style="background:var(--s3)"></i>Urlaub</span><span><i style="background:var(--s8)"></i>Krank</span><span class="muted">· Kein Balken und kein Minus an freien Tagen.</span></div></div>
    <div class="g-2-1">
      <div class="card"><div class="ch"><h3>Überstundenkonto</h3><span class="sub">${mode === 'year' ? 'seit Beginn' : short ? 'letzte 4 Wochen' : 'Verlauf im Zeitraum'}</span><span class="right">${sgn(inR.length ? inR[inR.length - 1].bal : totalBalance())}</span></div>${lineChart(lp, { h: 190, xt })}</div>
      <div class="card"><div class="ch"><h3>App-Nutzung</h3><span class="right muted sm">${dur(app.total)} aktiv</span></div>${appRank(app.list, app.total)}</div></div>
    ${third}</div>`;
}

/* ---------- Abwesenheiten ---------- */
function vAbsences() {
  const [y, m] = S.calMonth.split('-').map(Number), first = `${y}-${pad(m)}-01`, last = monthEnd(first);
  let d = weekStart(first); const cells = [];
  while (d <= last || dow(d) !== 0) {
    const s = dayStat(d), inM = d.slice(0, 7) === S.calMonth, k = s.info.kind;
    const tags = [];
    if (s.info.holiday) tags.push(`<span class="tag" style="--tc:var(--s7)" ${tipAttr(s.info.holiday, dLong(d) + ' · Feiertag')}>${esc(s.info.holiday)}</span>`);
    if (s.info.absence && !s.info.holiday) tags.push(`<span class="tag" style="--tc:var(${KINDS[s.info.absence.type].c})">${KINDS[s.info.absence.type].n}</span>`);
    const ft = d > TODAY ? (s.soll ? `Soll ${durN(s.soll)}` : '') : s.has ? `${durN(s.counted)} · ${sdur(s.diffLive).replace(' h', '')}` : s.info.kind === 'comp' ? sdur(-s.soll).replace(' h', '') : '';
    cells.push(`<button class="cc ${inM ? '' : 'oth'} ${s.info.weekend ? 'we' : ''} ${d === TODAY ? 'today' : ''}" data-a="calDay" data-d="${d}"><span class="n">${parseD(d).getDate()}</span>${tags.join('')}<span class="ft">${ft}</span></button>`);
    d = addDays(d, 1);
  }
  const vs = vacationSummary(), list = S.absences.slice().sort((a, b) => a.from.localeCompare(b.from));
  return `<div class="g-2-1">
    <div class="card"><div class="ch"><div class="row gap-s"><button class="ibtn bd" data-a="calNav" data-n="-1">${ic('chevL')}</button><button class="ibtn bd" data-a="calNav" data-n="1">${ic('chevR')}</button></div><h2 style="font-size:18px;font-weight:650">${MONTHS[m - 1]} ${y}</h2><button class="btn sm" data-a="calNav" data-to="${TODAY.slice(0, 7)}">Heute</button>
      <button class="btn primary right" data-a="absNew">${ic('plus', 16)} Abwesenheit eintragen</button></div>
      <div class="cal">${DOW_S.map(x => `<div class="dh">${x}</div>`).join('')}${cells.join('')}</div>
      <div class="legend" style="margin-top:14px">${Object.keys(KINDS).map(k => `<span><i style="background:var(${KINDS[k].c})"></i>${KINDS[k].n}</span>`).join('')}<span class="muted">· Klick auf einen Tag trägt eine Abwesenheit ein.</span></div></div>
    <div class="stack">
      <div class="card"><div class="ch"><h3>Urlaubskonto ${y}</h3></div>
        <div class="row between" style="align-items:flex-end"><div style="font-size:34px;font-weight:650;letter-spacing:-.02em">${vs.rest}<span class="muted" style="font-size:15px;font-weight:500"> von ${vs.total} Tagen frei</span></div></div>
        <div class="meter" style="margin:12px 0"><i style="width:${vs.taken / vs.total * 100}%;background:var(--s3)"></i><i style="width:${vs.planned / vs.total * 100}%;background:var(--s3);opacity:.45"></i></div>
        <div class="legend"><span><i style="background:var(--s3)"></i>Genommen ${vs.taken}</span><span><i style="background:var(--s3);opacity:.45"></i>Geplant ${vs.planned}</span><span><i style="background:var(--surface-2);border:1px solid var(--line-2)"></i>Rest ${vs.rest}</span></div></div>
      <div class="card"><div class="ch"><h3>Eintragungen</h3></div>
        <div style="display:flex;flex-direction:column">${list.map(a => `<div class="kv" style="align-items:center"><span style="display:flex;gap:10px;align-items:center;min-width:0"><span class="kd" style="--tc:var(${KINDS[a.type].c})"></span><span><b style="color:var(--ink)">${KINDS[a.type].n}</b>${a.note ? ' · ' + esc(a.note) : ''}<br><span class="xs muted">${a.from === a.to ? dMid(a.from) : dMid(a.from) + ' – ' + dMid(a.to)} · ${absWorkdays(a)} Arbeitstag${absWorkdays(a) === 1 ? '' : 'e'}</span></span></span><span class="row gap-s"><button class="ibtn" data-a="absEdit" data-id="${a.id}" title="Bearbeiten">${ic('edit', 16)}</button><button class="ibtn" data-a="absDel" data-id="${a.id}" title="Löschen">${ic('trash', 16)}</button></span></div>`).join('')}</div></div>
      <div class="callout info">${ic('shield')}<span><b>Fair gerechnet:</b> Feiertage, Urlaub und Krankheit setzen das Tagessoll auf 0 – dein Überstundenkonto bleibt unverfälscht. Nur <b>Zeitausgleich</b> baut bewusst Überstunden ab.</span></div></div></div>`;
}

/* ---------- Companion ---------- */
function flyout() {
  const t = dayStat(TODAY), ts = trackState(), win = S.os === 'windows';
  const app = ts.key === 'running' ? appOf(ts.app) : null;
  return `<div class="flyout ${win ? '' : 'mac'}">
    <div class="fh"><img class="logo" src="icon.svg" width="24" height="24" alt="" style="border-radius:6px">Tickly<span class="right badge ${trkTone(ts)}"><span class="dot ${ts.cls}"></span>${{ running: 'Läuft', locked: 'Gesperrt', paused: 'Pausiert', ask: 'Rückfrage', idle: 'Aus' }[ts.key] || 'Beendet'}</span></div>
    <div class="fb">
      <div><div class="muted xs">Heute · ${S.set.mode === 'net' ? 'Netto' : 'Brutto'}</div><div class="big mono">${durN(t.counted)} <span class="muted" style="font-size:14px;font-weight:500">/ ${durN(t.soll)} h</span></div></div>
      <div class="pbar"><i style="width:${t.soll ? Math.min(100, t.counted / t.soll * 100) : 0}%"></i></div>
      <div><div class="kv"><span>Arbeitsbeginn</span><b>${t.start != null ? hm(t.start) : '–'}</b></div><div class="kv"><span>Aktive App</span><b>${app ? `<span style="display:inline-flex;align-items:center;gap:6px">${appIcon(ts.app, 18)}${app.n}</span>` : ts.key === 'locked' ? 'Gesperrt' : '–'}</b></div><div class="kv"><span>Pause</span><b>${durN(S.set.mode === 'net' ? t.brk : 0)} h</b></div><div class="kv"><span>Überstunden gesamt</span><b>${sgn(totalBalance())}</b></div></div>
      ${ts.key === 'ask' ? askBox() : `<div class="row"><button class="btn sm grow" data-a="goDash">Dashboard öffnen</button>${S.day.tracking ? `<button class="btn sm" data-a="cAct" data-k="pause" title="Tracking pausieren – die App läuft weiter">${ic('pause', 14)} Pausieren</button>` : `<button class="btn sm primary" data-a="cAct" data-k="start">${ic('play', 14)} ${ts.key === 'paused' ? 'Fortsetzen' : 'Starten'}</button>`}${ts.key !== 'stopped' ? `<button class="ibtn bd" data-a="cAct" data-k="quit" title="Companion beenden">${ic('power', 15)}</button>` : ''}</div>`}
      <label class="chk"><span class="switch ${S.set.autostart ? 'on' : ''}" data-a="tog" data-k="set.autostart" style="width:34px;height:20px"></span>Mit ${OSN()} starten</label>
    </div></div>`;
}
function vCompanionReal() {
  const left = S.pairCode ? Math.max(0, Math.round((S.pairCode.until - Date.now()) / 60000)) : 0;
  const code = S.pairCode && left > 0 ? S.pairCode.code : null;
  const dev = S.devices.length ? S.devices.map(d => `<div class="kv" style="align-items:center"><span class="row" style="gap:10px"><span class="pk" style="padding:8px;border:0">${ic('monitor', 20)}</span><span><b style="color:var(--ink)">${esc(d.name)}</b><br><span class="xs muted">${d.os === 'macos' ? 'macOS' : 'Windows'}${d.version ? ' · Companion ' + esc(d.version) : ''} · zuletzt ${esc(d.lastSeen)}</span></span></span><span class="row gap-s"><span class="badge ${d.online ? 'good' : ''}">${d.online ? 'Online' : 'Offline'}</span><button class="ibtn" data-a="devRevoke" data-id="${d.id}" title="Kopplung aufheben">${ic('trash', 16)}</button></span></div>
      <div class="setrow" style="padding:4px 0 10px 38px"><div class="tx"><b>Tracking automatisch starten</b><span>Startet das Tracking auf diesem Gerät von selbst, wenn der Rechner in einem Zeitfenster hochfährt. Aus: Du startest es selbst.</span></div><button class="switch ${d.autoTrack ? 'on' : ''}" data-a="devAuto" data-id="${d.id}" role="switch" aria-checked="${!!d.autoTrack}"></button></div>`).join('') : `<div class="hint">Noch kein Gerät gekoppelt. Ohne Companion werden keine Zeiten erfasst.</div>`;
  return `<div class="g-1-1">
    <div class="card"><div class="ch"><h3>Verbundene Geräte</h3></div>${dev}
      <div class="callout info" style="margin-top:12px">${ic('info')}<span>Der Companion meldet Rechnerstart, Sperre, aktive App und Beenden. Bricht die Verbindung ab, sammelt er die Ereignisse und sendet sie später nach.</span></div></div>
    <div class="card"><div class="ch"><h3>Neues Gerät koppeln</h3></div>${downloadCard()}
      <ol class="steps"><li>Companion auf dem Rechner installieren und öffnen.</li><li>Server-Adresse eintragen: <b class="mono">${esc(location.origin)}</b></li><li>Hier einen Code erzeugen und im Companion eingeben.</li></ol>
      ${code ? `<div class="paircode"><span class="mono">${esc(code)}</span><button class="btn sm" data-a="copyCode">Kopieren</button></div><p class="hint">Gilt noch etwa ${left} Min. und nur für eine Kopplung.</p>` : ''}
      <button class="btn primary" data-a="pairCode">${ic('plus', 15)} ${code ? 'Neuen Code erzeugen' : 'Kopplungscode erzeugen'}</button></div></div>`;
}
function vCompanion() {
  if (!S.demo) return vCompanionReal();
  const win = S.os === 'windows', ts = trackState();
  const stage = win ? `<div class="stage win"><div class="bar-os"><span>Windows 11</span><span class="muted" style="color:rgba(255,255,255,.7)">DESKTOP-ANDRE</span></div>
      <div style="display:flex;gap:18px;flex-wrap:wrap;margin-top:22px;align-items:flex-start">
        ${S.cwin === 'open' ? `<div class="winframe"><div class="tb2"><img class="logo" src="icon.svg" width="18" height="18" alt="" style="border-radius:4px">Tickly Companion<span class="right row" style="gap:0"><button class="wb">–</button><button class="wb">☐</button><button class="wb cl" data-a="closeWin" title="Fenster schließen">✕</button></span></div><div style="padding:16px">${windowBody()}</div></div>` : `<div class="callout info" style="max-width:380px;background:rgba(255,255,255,.88);color:#222"><div>${ic('info')}</div><span>Fenster geschlossen – <b>Companion läuft weiter</b> im Infobereich. <a href="#/companion" data-a="openWin" style="color:var(--brand);font-weight:700">Fenster öffnen</a></span></div>`}
        <div style="margin-left:auto">${flyout()}</div></div>
      <div class="taskbar" style="margin-top:auto"><span class="ti">${'⊞'}</span><span class="ti">${ic('search', 18)}</span><span class="ti" style="background:var(--brand);color:#fff">T</span><div class="tray"><span class="row gap-s" style="gap:6px"><span class="dot ${ts.cls}"></span>Tickly</span><span>${hm(S.now)}<br><span class="xs">${dMid(TODAY)}</span></span></div></div></div>`
    : `<div class="stage mac"><div class="menubar"><span></span><b>Finder</b><span>Ablage</span><span>Bearbeiten</span><span class="right row" style="gap:12px"><span class="row gap-s" style="gap:6px;background:rgba(255,255,255,.25);padding:2px 9px;border-radius:6px"><span class="dot ${ts.cls}"></span>${durN(dayStat(TODAY).counted)}</span><span>Mo 26. Okt ${hm(S.now)}</span></span></div>
      <div style="display:flex;justify-content:flex-end;margin-top:8px">${flyout()}</div>
      <div style="margin-top:20px">${S.cwin === 'open' ? `<div class="winframe" style="border-radius:12px"><div class="tb2"><span class="mac-dots"><i style="background:#ff5f57;cursor:pointer" data-a="closeWin"></i><i style="background:#febc2e"></i><i style="background:#28c840"></i></span><span style="margin:0 auto">Tickly Companion</span></div><div style="padding:16px">${windowBody()}</div></div>` : `<div class="callout info" style="max-width:380px;background:rgba(255,255,255,.9);color:#222"><div>${ic('info')}</div><span>Fenster geschlossen – <b>Companion läuft weiter</b> in der Menüleiste. <a href="#/companion" data-a="openWin" style="color:var(--brand);font-weight:700">Fenster öffnen</a></span></div>`}</div></div>`;
  const dis = k => ({ switchApp: ts.key !== 'running', lock: ts.key !== 'running', unlock: ts.key !== 'locked', pause: ts.key !== 'running', start: S.day.tracking, quit: ts.key === 'stopped', shutdown: ts.key === 'stopped' })[k] ? 'disabled' : '';
  return `<div class="g-2-1" style="grid-template-columns:minmax(0,1.5fr) minmax(0,1fr)">
    <div class="stack"><div class="row wrap"><div class="seg"><button class="${win ? 'on' : ''}" data-a="os" data-v="windows">Windows</button><button class="${!win ? 'on' : ''}" data-a="os" data-v="mac">macOS</button></div><span class="muted sm">Vorschau der Companion-App · Tray-Panel und Hauptfenster</span></div>${stage}
      <div class="g-1-1"><div class="card flat"><div class="ch"><h3>Verbundene Geräte</h3></div>
        <div class="kv" style="align-items:center"><span class="row" style="gap:10px"><span class="pk" style="padding:8px;border:0">${ic('monitor', 20)}</span><span><b style="color:var(--ink)">DESKTOP-ANDRE</b><br><span class="xs muted">Windows 11 · Companion 1.4.2</span></span></span><span class="badge ${ts.key === 'stopped' ? '' : 'good'}">${ts.key === 'stopped' ? 'Offline' : 'Online · vor 3 s'}</span></div>
        <div class="kv" style="align-items:center"><span class="row" style="gap:10px"><span class="pk" style="padding:8px;border:0">${ic('monitor', 20)}</span><span><b style="color:var(--ink)">MacBook Pro</b><br><span class="xs muted">macOS 15 · Companion 1.4.2</span></span></span><span class="badge">zuletzt gestern</span></div></div>
        <div class="card flat"><div class="ch"><h3>Companion herunterladen</h3></div><div class="stack gap-s"><button class="btn block" data-a="toast" data-m="Download-Link kopiert (Prototyp)">${ic('download', 16)} Windows 10/11 (.msi)</button><button class="btn block" data-a="toast" data-m="Download-Link kopiert (Prototyp)">${ic('download', 16)} macOS 13+ (.dmg)</button><p class="hint">Nach der Installation mit deinem Konto oder per Passkey koppeln.</p></div></div></div></div>
    <div class="stack">
      ${ts.key === 'ask' ? askBox() : ''}
      <div class="card"><div class="ch"><h3>${ic('flask', 17)} Systemereignisse simulieren</h3></div>
        <p class="hint" style="margin-bottom:12px">Der echte Companion meldet diese Ereignisse automatisch. Hier kannst du sie auslösen und siehst die Wirkung live im Dashboard.</p>
        <div class="stack gap-s">
          <button class="sim-btn" data-a="cAct" data-k="newday">${ic('power', 20)}<span><b>Neuer Arbeitstag: ${OSN()}-Start am Morgen</b><small>Setzt den Tag zurück (07:58). Im Zeitfenster startet das Tracking von selbst.</small></span></button>
          <button class="sim-btn" data-a="cAct" data-k="lock" ${dis('lock')}>${ic('lock', 20)}<span><b>Bildschirm sperren</b><small>Zeitverlauf zeigt „Gesperrt“</small></span></button>
          <button class="sim-btn" data-a="cAct" data-k="unlock" ${dis('unlock')}>${ic('unlock', 20)}<span><b>Entsperren</b><small>Tracking der aktiven App geht weiter</small></span></button>
          <button class="sim-btn" data-a="cAct" data-k="pause" ${dis('pause')}>${ic('pause', 20)}<span><b>Tracking im Companion pausieren</b><small>Die App läuft weiter, die Pause zählt nicht, der Tag bleibt offen</small></span></button>
          <button class="sim-btn" data-a="cAct" data-k="start" ${dis('start')}>${ic('play', 20)}<span><b>Tracking starten / fortsetzen</b><small>Die Lücke wird nicht als Arbeitszeit gezählt</small></span></button>
          <button class="sim-btn dng" data-a="cAct" data-k="quit" ${dis('quit')}>${ic('power', 20)}<span><b>Companion beenden</b><small>Richtig beenden – das Tracking endet, das Arbeitsende wird gesetzt</small></span></button>
          <button class="sim-btn dng" data-a="cAct" data-k="shutdown" ${dis('shutdown')}>${ic('power', 20)}<span><b>Rechner neu starten / herunterfahren</b><small>Lief das Tracking, wird es nach dem Start fortgesetzt</small></span></button>
          <button class="sim-btn" data-a="cAct" data-k="boot">${ic('monitor', 20)}<span><b>Rechner wieder starten</b><small>Fortsetzen, automatisch starten (Zeitfenster) oder nachfragen</small></span></button></div></div>
      <div class="card"><div class="ch"><h3>Aktive App wechseln</h3></div><div class="chips">${Object.keys(APPS).map(k => `<button class="chip" data-a="cApp" data-k="${k}" ${dis('switchApp')} style="${ts.key === 'running' && ts.app === k ? 'border-color:var(' + appOf(k).c + ');box-shadow:inset 0 0 0 1px var(' + appOf(k).c + ')' : ''}">${appIcon(k, 18)}${appOf(k).n}</button>`).join('')}</div></div>
      <div class="card"><div class="ch"><h3>Zeit</h3><span class="sub right mono">${hm(S.now)} Uhr</span></div><div class="seg">${[[1, '1×'], [60, '60×'], [300, '300×']].map(([v, l]) => `<button class="${S.speed === v ? 'on' : ''}" data-a="speed" data-v="${v}">${l}</button>`).join('')}</div><p class="hint" style="margin-top:8px">Zeitraffer: 60× = eine Simulations-Minute pro Sekunde.</p></div>
      <div class="card"><div class="ch"><h3>Ereignisprotokoll</h3></div><div class="evl">${S.day.events.slice().reverse().map(e => `<div><time>${hm(e.t)}</time><span class="muted">${ic(EVICON[e.k] || 'info', 15)}</span><span>${esc(e.txt)}</span></div>`).join('')}</div></div></div></div>`;
}
function windowBody() {
  const t = dayStat(TODAY), ts = trackState();
  return `<div class="stack" style="gap:12px"><div class="row between"><b>Einstellungen</b><span class="badge ${trkTone(ts)}">${ts.label}</span></div>
    ${ts.key === 'ask' ? askBox() : ''}
    <div class="setrow" style="padding:8px 0"><div class="tx"><b>Mit ${OSN()} starten</b><span>Arbeitsbeginn automatisch erkennen</span></div><button class="switch ${S.set.autostart ? 'on' : ''}" data-a="tog" data-k="set.autostart"></button></div>
    <div class="setrow" style="padding:8px 0"><div class="tx"><b>Zeitfenster für Auto-Start</b><span>${S.set.autoTrack.enabled ? S.set.autoTrack.windows.map(w => `${w.days.map((d, i) => d ? DOW_S[i] : '').filter(Boolean).join(',') } ${hm(w.from)}–${hm(w.to)}`).join(' · ') || 'Kein Zeitfenster' : 'Aus'}</span></div><button class="switch ${S.set.autoTrack.enabled ? 'on' : ''}" data-a="tog" data-k="set.autoTrack.enabled"></button></div>
    <div class="setrow" style="padding:8px 0"><div class="tx"><b>Aktive App erfassen</b><span>Nur App-Name, keine Inhalte</span></div><button class="switch ${S.set.trackApps ? 'on' : ''}" data-a="tog" data-k="set.trackApps"></button></div>
    <div class="setrow" style="padding:8px 0"><div class="tx"><b>Sperrbildschirm erfassen</b><span>Pausen im Zeitverlauf zeigen</span></div><button class="switch ${S.set.trackLock ? 'on' : ''}" data-a="tog" data-k="set.trackLock"></button></div>
    <div class="callout">${ic('info')}<span class="xs"><b>Fenster schließen</b> (✕) minimiert den Companion nur – das Tracking läuft weiter. <b>Beenden</b> beendet das Tracking.</span></div>
    <div class="row">${S.day.tracking ? `<button class="btn grow" data-a="cAct" data-k="pause">${ic('pause', 15)} Tracking pausieren</button>` : ts.key !== 'ask' ? `<button class="btn primary grow" data-a="cAct" data-k="start">${ic('play', 15)} ${ts.key === 'paused' ? 'Tracking fortsetzen' : 'Tracking starten'}</button>` : ''}<button class="btn grow" data-a="cAct" data-k="end" ${S.day.tracking || ts.key === 'paused' ? '' : 'disabled'}>${ic('power', 15)} Tracking beenden</button><button class="btn danger grow" data-a="cAct" data-k="quit" ${ts.key === 'stopped' ? 'disabled' : ''}>${ic('power', 15)} Tickly beenden</button></div></div>`;
}

/* ---------- Einstellungen ---------- */
const toggle = (k, v) => `<button class="switch ${v ? 'on' : ''}" data-a="tog" data-k="${k}" role="switch" aria-checked="${!!v}"></button>`;
function setRow(t, s, ctl) { return `<div class="setrow"><div class="tx"><b>${t}</b><span>${s}</span></div>${ctl}</div>`; }
function vSettings() {
  const tabs = [['work', 'Arbeitszeit'], ['rec', 'Zeiterfassung'], ['hol', 'Feiertage'], ['trk', 'Tracking & Datenschutz'], ['sec', 'Sicherheit & Passkeys'], ['prof', 'Profil']];
  const head = `<div class="tabs">${tabs.map(([k, l]) => `<button class="${S.setTab === k ? 'on' : ''}" data-a="setTab" data-v="${k}">${l}</button>`).join('')}</div>`;
  let body = '';
  const t = dayStat(TODAY);
  if (S.setTab === 'work') {
    const wk = S.set.hours.reduce((a, b) => a + b, 0);
    body = `<div class="g-1-1"><div class="card"><div class="ch"><h3>Tagesarbeitszeit (Soll)</h3><span class="sub">je Wochentag</span></div>
      ${S.set.hours.map((h, i) => `<div class="setrow" style="padding:9px 0"><div class="tx"><b>${DOW_L[i]}</b></div><span class="muted sm mono">${h > 0 ? durN(h * 60) + ' h' : 'frei'}</span><input type="number" min="0" max="12" step="0.25" value="${h}" data-in="set.hours.${i}" data-type="num"><span class="muted">h</span></div>`).join('')}
      <div class="kv" style="margin-top:6px"><span><b style="color:var(--ink)">Wochensoll</b></span><b class="mono">${hdec(wk)} h</b></div></div>
      <div class="stack"><div class="card"><div class="ch"><h3>Vorlagen</h3></div><div class="stack gap-s">
        <button class="btn" data-a="preset" data-v="8,8,8,8,8,0,0">Vollzeit · 5 × 8 h = 40 h</button>
        <button class="btn" data-a="preset" data-v="6,6,6,6,6,0,0">Teilzeit · 5 × 6 h = 30 h</button>
        <button class="btn" data-a="preset" data-v="10,10,10,10,0,0,0">4-Tage-Woche · 4 × 10 h = 40 h</button>
        <button class="btn" data-a="preset" data-v="4,4,4,4,4,0,0">Teilzeit · 5 × 4 h = 20 h</button></div></div>
</div></div>`;
  } else if (S.setTab === 'rec') {
    const net = S.set.mode === 'net';
    body = `<div class="g-1-1"><div class="stack"><div class="card"><div class="ch"><h3>Erfassungsmodus</h3></div><div class="stack gap-s">
      <button class="radio-card ${net ? 'on' : ''}" data-a="pick" data-k="set.mode" data-v="net"><span class="rd"></span><span><b>Netto</b><br><span class="muted sm">Pausen werden automatisch abgezogen. Gezählt wird die reine Arbeitszeit.</span></span></button>
      <button class="radio-card ${!net ? 'on' : ''}" data-a="pick" data-k="set.mode" data-v="gross"><span class="rd"></span><span><b>Brutto</b><br><span class="muted sm">Die gesamte Anwesenheit vom Start bis zum Ende zählt – ohne Pausenabzug.</span></span></button></div></div>
      <div class="card" style="${net ? '' : 'opacity:.5;pointer-events:none'}"><div class="ch"><h3>Automatischer Pausenabzug</h3><span class="sub">gilt im Netto-Modus</span></div>
        ${S.set.rules.map((r, i) => `<div class="rule" style="margin-bottom:8px"><span class="muted">Ab</span><input type="number" min="1" max="14" step="0.5" value="${r.after}" data-in="set.rules.${i}.after" data-type="num"><span class="muted">h Arbeitszeit</span><input type="number" min="5" max="120" step="5" value="${r.min}" data-in="set.rules.${i}.min" data-type="num"><span class="muted">Min.</span><button class="ibtn" data-a="ruleDel" data-i="${i}" title="Entfernen">${ic('trash', 16)}</button></div>`).join('')}
        <button class="btn sm" data-a="ruleAdd">${ic('plus', 14)} Regel hinzufügen</button><p class="hint" style="margin-top:10px">Standard nach Arbeitszeitgesetz: ab 6 h 30 Min., ab 9 h 45 Min.</p>
        <div class="hr" style="margin:14px 0 4px"></div>
        ${setRow('Gesperrte Zeit als Pause werten', 'Echte Pausen (Bildschirm gesperrt) ersetzen den automatischen Abzug, wenn sie länger sind.', toggle('set.lockAsBreak', S.set.lockAsBreak))}
        ${S.set.lockAsBreak ? setRow('Mindestdauer einer Sperre', 'Kürzere Sperren zählen nicht als Pause.', `<span class="row gap-s"><input type="number" min="1" max="60" value="${S.set.chunk}" data-in="set.chunk" data-type="num" style="width:80px"><span class="muted">Min.</span></span>`) : ''}</div>
      <div class="card"><div class="ch"><h3>Rüstzeit</h3><span class="sub">Hochfahren und Herunterfahren</span></div>
        ${setRow('Nach dem Rechnerstart', 'Zusatzzeit, die das erste Tracking nach dem Start früher beginnen lässt (Anmelden, Programme öffnen). 0 = aus.', `<span class="row gap-s"><input type="number" min="0" max="120" step="1" value="${S.set.setupBefore || 0}" data-in="set.setupBefore" data-type="num" style="width:80px"><span class="muted">Min.</span></span>`)}
        ${setRow('Beim Herunterfahren', 'Zusatzzeit, die nach dem Herunterfahren noch zur Arbeitszeit zählt (Sichern, Aufräumen). 0 = aus.', `<span class="row gap-s"><input type="number" min="0" max="120" step="1" value="${S.set.setupAfter || 0}" data-in="set.setupAfter" data-type="num" style="width:80px"><span class="muted">Min.</span></span>`)}
        <p class="hint" style="margin-top:6px">Gilt für künftige Starts und Herunterfahren; vergangene Tage ändern sich nicht. Beim Start zählt sie nur, wenn das Tracking mit dem Rechnerstart beginnt (automatisch oder mit „ab Rechnerstart“), und nie in die Zeit davor. Ein Neustart unter der Mindestdauer einer Sperre läuft ohne Rüstzeit durch und zählt als Arbeitszeit.</p></div></div>
      <div class="card"><div class="ch"><h3>Live-Beispiel · heute</h3></div>
        <div class="kv"><span>Brutto (${hm(t.start)} bis jetzt)</span><b class="mono">${dur(t.gross)}</b></div>
        <div class="kv"><span>Gesperrt (Sperren ≥ ${S.set.chunk} Min.)</span><b class="mono">${S.set.lockAsBreak ? dur(t.lockedBreak) : '–'}</b></div>
        <div class="kv"><span>Automatische Mindestpause</span><b class="mono">${dur(t.rule)}</b></div>
        <div class="kv"><span>Abzug</span><b class="mono">${net ? '−' + dur(t.brk) : 'keiner (Brutto)'}</b></div>
        <div class="kv"><span><b style="color:var(--ink)">Gezählte Arbeitszeit</b></span><b class="mono">${dur(t.counted)}</b></div>
        <div class="callout info" style="margin-top:12px">${ic('info')}<span>${net ? `Es gilt der <b>größere</b> Wert aus gesperrter Zeit (${dur(t.lockedBreak)}) und Mindestpause (${dur(t.rule)}): ${dur(t.brk)} werden abgezogen.` : 'Im Brutto-Modus wird nichts abgezogen. Überstunden entstehen früher.'}</span></div>
        <p class="hint" style="margin-top:10px">Der Modus wirkt sich auf alle Auswertungen aus – auch rückwirkend. Überstundenkonto: ${sgn(totalBalance())}</p></div></div>`;
  } else if (S.setTab === 'hol') {
    const hl = DB.holidays;
    const xd = S.set.extraDays;
    body = `<div class="g-1-1"><div class="stack"><div class="card"><div class="ch"><h3>Bundesland</h3></div><label class="f">Feiertage berechnen für<select data-in="set.state">${Object.keys(STATES).map(k => `<option value="${k}" ${S.set.state === k ? 'selected' : ''}>${STATES[k]}</option>`).join('')}</select></label>
      <p class="hint" style="margin-top:10px">Gesetzliche Feiertage setzen das Tagessoll auf 0. Wird an einem Feiertag gearbeitet, zählt die gesamte Zeit als Überstunden.</p></div>
      <div class="card"><div class="ch"><h3>Weitere freie Tage</h3><span class="sub">z. B. betriebsfrei</span></div>
        ${setRow('Rosenmontag', 'Beweglich, 48 Tage vor Ostersonntag.', toggle('set.extraDays.rosenmontag', xd.rosenmontag))}
        ${setRow('Heiligabend (24.12.)', 'Ganzer Tag frei, Soll 0.', toggle('set.extraDays.heiligabend', xd.heiligabend))}
        ${setRow('Silvester (31.12.)', 'Ganzer Tag frei, Soll 0.', toggle('set.extraDays.silvester', xd.silvester))}
        ${xd.custom.map((c, i) => `<div class="setrow"><div class="tx"><b>${esc(c.name)}</b><span>jedes Jahr am ${pad(c.day)}.${pad(c.month)}.</span></div><button class="ibtn" data-a="xdDel" data-i="${i}" title="Entfernen">${ic('trash', 16)}</button></div>`).join('')}
        <div class="row wrap" style="margin-top:12px"><input type="text" id="xd-name" placeholder="Name, z. B. Brückentag" style="flex:1;min-width:150px" maxlength="40"><input type="text" id="xd-date" class="ttxt" placeholder="TT.MM." maxlength="6" style="width:90px !important"><button class="btn" data-a="xdAdd">${ic('plus', 15)} Hinzufügen</button></div>
        <p class="hint" style="margin-top:10px">Diese Tage werden wie Feiertage behandelt: Soll 0, kein Minus, kein Urlaubstag. Halbe Tage gibt es noch nicht.</p></div></div>
      <div class="card"><div class="ch"><h3>Freie Tage ${TODAY.slice(0, 4)}</h3><span class="sub right">${hl.length} Tage</span></div>${hl.map(h => `<div class="kv"><span>${esc(h.name)}${h.extra ? ' <span class="badge" style="margin-left:4px">frei</span>' : ''}</span><b class="${dow(h.date) >= 5 ? 'muted' : ''}">${DOW_S[dow(h.date)]}, ${dMid(h.date)}${dow(h.date) >= 5 ? ' · Wochenende' : ''}</b></div>`).join('')}</div></div>`;
  } else if (S.setTab === 'trk') {
    body = `<div class="g-1-1"><div class="card"><div class="ch"><h3>Companion</h3></div>
      ${setRow('Aktive App erfassen', 'Zeigt, welche App wann und wie lange im Vordergrund war.', toggle('set.trackApps', S.set.trackApps))}
      ${setRow('Fenstertitel speichern', 'Nicht empfohlen – Titel können sensible Inhalte enthalten.', toggle('set.trackTitles', S.set.trackTitles))}
      ${setRow('Sperrbildschirm erfassen', 'Gesperrte Zeit erscheint im Zeitverlauf und kann als Pause zählen.', toggle('set.trackLock', S.set.trackLock))}
      ${setRow('Inaktivität erkennen', 'Ohne Eingabe wird die Zeit als Leerlauf markiert.', `<select data-in="set.idle" data-type="num" style="width:120px">${[[0, 'Aus'], [5, '5 Min.'], [10, '10 Min.'], [15, '15 Min.']].map(([v, l]) => `<option value="${v}" ${S.set.idle === v ? 'selected' : ''}>${l}</option>`).join('')}</select>`)}</div>
      <div class="stack"><div class="card"><div class="ch"><h3>Automatischer Tracking-Start</h3></div>
        ${setRow('Zeitfenster nutzen', 'Läuft der Rechner in einem dieser Zeiträume an, startet das Tracking von selbst. Nicht an Feiertagen und Abwesenheitstagen.', toggle('set.autoTrack.enabled', S.set.autoTrack.enabled))}
        ${S.set.autoTrack.enabled ? `<div class="stack gap-s" style="margin:6px 0 8px">${S.set.autoTrack.windows.map((w, i) => `<div class="awin"><div class="daychips">${DOW_S.map((d, j) => `<button class="${w.days[j] ? 'on' : ''}" data-a="tog" data-k="set.autoTrack.windows.${i}.days.${j}" title="${DOW_L[j]}">${d}</button>`).join('')}</div>
          <div class="tt">von ${timeField(`set.autoTrack.windows.${i}.from`, w.from)} bis ${timeField(`set.autoTrack.windows.${i}.to`, w.to)} Uhr</div><button class="ibtn right" data-a="winDel" data-i="${i}" title="Zeitfenster entfernen">${ic('trash', 16)}</button></div>`).join('') || '<div class="hint">Noch kein Zeitfenster – das Tracking startet dann nie von selbst.</div>'}
          <div><button class="btn sm" data-a="winAdd">${ic('plus', 14)} Zeitfenster hinzufügen</button></div></div>` : ''}
        ${setRow('Außerhalb nachfragen', 'Startet der Rechner zu anderen Zeiten, fragt der Companion: „Arbeitest du gerade?“ Sonst bleibt das Tracking aus, bis du es startest.', toggle('set.autoTrack.ask', S.set.autoTrack.ask))}
        <div class="callout info" style="margin-top:12px">${ic('info')}<span>Ob das Tracking auf einem Rechner von selbst startet, stellst du je Gerät ein: im Companion selbst oder unter <a href="#/companion" style="color:var(--brand);font-weight:600">Companion-App</a>. Dort schaltest du es für einen Rechner aus, der nicht zur Arbeit gehört. Ebenso legst du im Companion fest, ob er mit dem Rechner startet.<br>Lief das Tracking, als der Rechner neu gestartet oder heruntergefahren wurde, geht es nach dem Start <b>immer</b> von selbst weiter – die Zeit dazwischen zählt nicht. Ein bewusst pausiertes Tracking wird vom Zeitfenster nie überstimmt.</span></div></div>
      <div class="card"><div class="ch"><h3>Ausgeschlossene Apps</h3></div><p class="hint" style="margin-bottom:10px">Diese Apps werden nie namentlich erfasst (z. B. Passwortmanager).</p>
        <div class="chips">${S.set.excluded.map((a, i) => `<span class="chip">${esc(a)}<span class="x" data-a="exclDel" data-i="${i}" style="cursor:pointer">${ic('x', 12)}</span></span>`).join('')}</div>
        <div class="row" style="margin-top:12px"><input type="text" id="excl-in" placeholder="App-Name hinzufügen" style="max-width:220px"><button class="btn" data-a="exclAdd">${ic('plus', 15)} Hinzufügen</button></div></div>
      <div class="card"><div class="ch"><h3>Deine Daten</h3></div><div class="row wrap"><button class="btn" data-a="exportCsv">${ic('download', 15)} Export (CSV)</button><button class="btn danger" data-a="wipeApps">${ic('trash', 15)} App-Verlauf löschen</button></div>
        <div class="callout info" style="margin-top:12px">${ic('shield')}<span>Es werden nur App-Name und Zeitstempel gespeichert – keine Tastatureingaben, Screenshots oder Dokumentinhalte.</span></div></div></div></div>`;
  } else if (S.setTab === 'sec') {
    body = `<div class="g-1-1"><div class="card"><div class="ch"><h3>Passkeys</h3><button class="btn primary sm right" data-a="passkeyAdd">${ic('plus', 15)} Passkey hinzufügen</button></div>
      <p class="hint" style="margin-bottom:12px">Melde dich mit Fingerabdruck, Gesicht oder Geräte-PIN an – schneller und sicherer als ein Passwort.</p>
      <div class="stack gap-s">${S.passkeys.map(p => `<div class="pk"><span class="pi">${ic('fp', 22)}</span><div class="grow"><b>${esc(p.name)}</b><br><span class="xs muted">Erstellt ${p.created} · zuletzt genutzt ${p.last}</span></div><button class="ibtn" data-a="pkRename" data-id="${p.id}" title="Umbenennen">${ic('edit', 16)}</button><button class="ibtn" data-a="pkDel" data-id="${p.id}" title="Entfernen">${ic('trash', 16)}</button></div>`).join('') || '<div class="hint">Noch keine Passkeys eingerichtet.</div>'}</div></div>
      <div class="stack"><div class="card"><div class="ch"><h3>Passwort</h3></div>${setRow('Anmeldung mit Passwort erlauben', S.passkeys.length ? 'Kann deaktiviert werden, wenn Passkeys eingerichtet sind.' : 'Richte zuerst einen Passkey ein.', `<button class="switch ${S.set.pwLogin ? 'on' : ''}" data-a="tog" data-k="set.pwLogin" ${S.passkeys.length ? '' : 'disabled style="opacity:.5"'}></button>`)}
        <div class="row" style="margin-top:8px"><button class="btn" data-a="pwOpen">Passwort ändern</button></div></div>
        ${S.secQ && S.secQ.enabled && S.secQ.hasPassword ? `<div class="card"><div class="ch"><h3>Sicherheitsfragen</h3><span class="badge ${S.secQ.set ? 'good' : 'warn'} right">${S.secQ.set ? 'Eingerichtet' : 'Fehlt'}</span></div>
          <p class="hint" style="margin-bottom:12px">Damit setzt du dein Passwort selbst zurück, wenn du es vergisst. Alle drei Antworten müssen stimmen.</p>
          ${S.secQ.set ? `<div class="stack gap-s" style="margin-bottom:12px">${S.secQ.questions.map(t => `<div class="kv"><span>${esc(t)}</span><b class="muted">••••••</b></div>`).join('')}</div>` : ''}
          <button class="btn ${S.secQ.set ? '' : 'primary'}" data-a="secqOpen">${S.secQ.set ? 'Fragen ändern' : 'Fragen festlegen'}</button></div>` : ''}
        <div class="card"><div class="ch"><h3>Authenticator-App (TOTP)</h3><span class="badge ${S.totp.enabled ? 'good' : ''} right">${S.totp.enabled ? 'Aktiv' : 'Aus'}</span></div>
          ${S.totp.enabled ? `<p class="hint" style="margin-bottom:12px">Bei der Anmeldung mit Passwort wird zusätzlich ein Code aus deiner Authenticator-App verlangt. Mit Passkey brauchst du ihn nicht.</p>
            <div class="kv"><span>Wiederherstellungscodes</span><b class="${S.totp.recoveryLeft <= 2 ? 'neg' : ''}">${S.totp.recoveryLeft} übrig</b></div>
            <div class="row wrap" style="margin-top:12px"><button class="btn sm" data-a="totpAsk" data-v="recovery">Neue Wiederherstellungscodes</button><button class="btn sm danger" data-a="totpAsk" data-v="disable">Ausschalten</button></div>`
          : `<p class="hint" style="margin-bottom:12px">${S.passkeys.length ? 'Zusätzlich zum Passkey kannst du' : 'Ohne Passkey kannst du dein Konto'} mit einer Authenticator-App (Google Authenticator, Microsoft Authenticator, Authy, 1Password …) absichern: Beim Anmelden mit Passwort wird dann ein Code aus der App verlangt.</p>
            <button class="btn primary" data-a="totpSetup">${ic('shield', 15)} Einrichten</button>`}</div>
        <div class="card"><div class="ch"><h3>Angemeldete Geräte & Browser</h3></div>
          ${S.mySessions.map(x => `<div class="kv" style="align-items:center"><span><b style="color:var(--ink)">${esc(shortUa(x.userAgent))}</b>${x.current ? ' <span class="badge good">Dieses Gerät</span>' : ''}<br><span class="xs muted">${place(x.ip, x.country)} · aktiv ${fmtAgo(x.lastSeenAt)}</span></span>${x.current ? '' : `<button class="btn sm" data-a="mySessEnd" data-id="${x.id}">Abmelden</button>`}</div>`).join('') || '<div class="hint">Keine Daten.</div>'}</div>
        <div class="card"><div class="ch"><h3>Sitzung</h3></div><div class="kv"><span>Dieser Browser</span><b class="pos">Aktiv</b></div><p class="hint" style="margin-top:8px">Die Sitzung läuft 30 Tage. Mit „Abmelden“ wird sie auf dem Server beendet.</p></div></div></div>`;
  } else {
    body = `<div class="g-1-1"><div class="card"><div class="ch"><h3>Profil</h3></div><div class="row" style="margin-bottom:16px">${avatar(S.me.name)}<div><b>${esc(S.me.name)}</b><br><span class="muted sm">${S.me.role === 'admin' ? 'Administrator' : 'Benutzer'}</span></div></div>
      <div class="stack"><label class="f">Name<input type="text" value="${esc(S.me.name)}" data-in="me.name" data-nr="1"></label><label class="f">E-Mail<input type="email" value="${esc(S.me.email)}" data-in="me.email" data-nr="1">${S.me.pendingEmail ? `<span class="hint" style="font-weight:400">Die Änderung auf <b>${esc(S.me.pendingEmail)}</b> wartet auf die Bestätigung per Mail. Zurücknehmen: bisherige Adresse eintragen und speichern.</span>` : '<span class="hint" style="font-weight:400">Eine neue Adresse gilt erst, wenn du den Link in der Mail an die neue Adresse öffnest.</span>'}</label>
      <label class="f">Sprache<select><option>Deutsch</option><option>English</option></select></label><label class="f">Zeitzone<select><option>Europe/Berlin (MEZ)</option></select></label>
      <div class="f" style="gap:6px"><span style="font-size:12.5px;color:var(--ink-2);font-weight:550">Darstellung</span><div class="seg" style="align-self:flex-start">${[['system', 'System', 'monitor'], ['light', 'Hell', 'sun'], ['dark', 'Dunkel', 'moon']].map(([v, l, i]) => `<button class="${S.themePref === v ? 'on' : ''}" data-a="themePick" data-v="${v}" style="display:inline-flex;gap:6px;align-items:center">${ic(i, 15)}${l}</button>`).join('')}</div><span class="hint">${S.themePref === 'system' ? `Folgt dem Betriebssystem (gerade ${S.theme === 'dark' ? 'dunkel' : 'hell'}).` : S.themePref === 'dark' ? 'Immer dunkel.' : 'Immer hell.'} Gilt auch für die Companion-App.</span></div>
      <div><button class="btn primary" data-a="saveMe">Speichern</button></div></div></div>
      <div class="card"><div class="ch"><h3>Konto</h3></div><div class="kv"><span>Überstunden-Stand zu Beginn</span>${S.startBalSet === false ? `<button class="btn sm primary" data-a="startBalOpen">Jetzt festlegen</button>` : `<b class="mono">${sgn(S.set.startBal * 60)}</b>`}</div><p class="hint" style="margin:-4px 0 6px">${S.startBalSet === false ? 'Noch nicht festgelegt. Das geht einmalig; danach nur durch einen Administrator.' : 'Einmalig festgelegt; Änderungen nur durch einen Administrator.'}</p><div class="kv"><span>Mitglied seit</span><b>${dMid(S.memberSince || TRACK_START)}</b></div><div class="kv"><span>Zeiterfassung ab</span><span><b>${dMid(TRACK_START)}</b>${S.demo ? '' : ' <button class="btn sm" data-a="trackStartOpen">Ändern</button>'}</span></div><p class="hint" style="margin:-4px 0 6px">Ab diesem Tag rechnet Tickly Arbeitszeit und Überstundenkonto. Frühere Tage zählen nicht, zum Beispiel Testtage.</p><div class="kv"><span>Rolle</span><b>${S.me.role === 'admin' ? 'Administrator' : 'Benutzer'}</b></div>
      <div class="hr" style="margin:14px 0"></div><button class="btn danger" data-a="logout">${ic('logout', 15)} Abmelden</button></div></div>`;
  }
  return `<div class="stack">${head}${body}</div>`;
}

/* ---------- Admin ---------- */
const STAT = { active: ['Aktiv', 'good'], pending: ['Wartet auf Freigabe', 'warn'], blocked: ['Gesperrt', 'bad'], invited: ['Eingeladen', 'brand'] };
const TODAYL = { tracking: ['Tracking läuft', 'run'], locked: ['Gesperrt', 'lock'], paused: ['Pausiert', 'lock'], asking: ['Rückfrage offen', 'lock'], vacation: ['Urlaub', ''], none: ['–', ''] };
const fmtSize = b => b < 1048576 ? Math.max(1, Math.round(b / 1024)) + ' KB' : (b / 1048576).toFixed(1).replace('.', ',') + ' MB';
const fmtStamp = iso => { const d = new Date(iso); return `${pad(d.getDate())}.${pad(d.getMonth() + 1)}.${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`; };
const KIND_LABEL = { edit: 'Zeiten bearbeitet', login: 'Anmeldung', totp: 'Zweiter Faktor', passkey: 'Passkey', register: 'Registrierung', setup: 'Einrichtung', origin: 'Origin-Prüfung', device_pair: 'Gerät', block: 'IP-Sperre', unblock: 'Entsperrt', admin: 'Verwaltung', backup: 'Sicherung', restore: 'Wiederherstellung' };
const shortUa = ua => { if (!ua) return '–'; const b = /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : /tickly-companion/.test(ua) ? 'Companion' : ua.slice(0, 18); const o = /Windows/.test(ua) ? 'Windows' : /Mac OS/.test(ua) ? 'macOS' : /Android/.test(ua) ? 'Android' : /iPhone|iPad/.test(ua) ? 'iOS' : /Linux/.test(ua) ? 'Linux' : ''; return o ? `${b} · ${o}` : b; };
const place = (ip, cc) => `<span class="mono sm">${esc(ip || '–')}</span>${cc ? ` <span title="${esc(countryName(cc))}">${flag(cc)} <span class="muted xs">${esc(cc)}</span></span>` : ''}`;
function vMail() {
  const M = S.smtp; if (!M) return '<div class="card empty"><p>Wird geladen …</p></div>';
  const c = M.config, T = S.smtpTest;
  return `<div class="g-1-1"><div class="stack"><div class="card"><div class="ch"><h3>SMTP-Server</h3><span class="badge ${c.enabled ? 'good' : ''} right">${c.enabled ? 'Aktiv' : 'Aus'}</span></div>
      ${setRow('E-Mail-Versand einschalten', 'Für Bestätigung der Registrierung, Passwort zurücksetzen, Einladungen und Hinweise an Administratoren.', `<button class="switch ${c.enabled ? 'on' : ''}" data-a="smtpTog" role="switch" aria-checked="${c.enabled}"></button>`)}
      <div class="row wrap" style="gap:10px;margin-top:6px"><label class="f grow" style="min-width:200px">Server<input type="text" id="sm-host" value="${esc(c.host)}" placeholder="smtp.example.com" autocomplete="off"></label>
        <label class="f" style="width:96px">Port<input type="number" id="sm-port" value="${c.port}" min="1" max="65535" style="width:100%;text-align:left"></label>
        <label class="f" style="width:190px">Verschlüsselung<select id="sm-sec"><option value="starttls" ${c.security === 'starttls' ? 'selected' : ''}>STARTTLS (587)</option><option value="tls" ${c.security === 'tls' ? 'selected' : ''}>SSL/TLS (465)</option><option value="none" ${c.security === 'none' ? 'selected' : ''}>keine (25)</option></select></label></div>
      <div class="row wrap" style="gap:10px;margin-top:10px"><label class="f grow" style="min-width:200px">Benutzername<input type="text" id="sm-user" value="${esc(c.username)}" placeholder="optional" autocomplete="off"></label>
        <label class="f grow" style="min-width:200px">Passwort<input type="password" id="sm-pw" placeholder="${c.passwordSet ? '•••••••• (unverändert)' : 'optional'}" autocomplete="new-password"></label></div>
      ${c.passwordSet ? '<label class="chk" style="margin-top:6px"><input type="checkbox" id="sm-clear">Gespeichertes Passwort entfernen</label>' : ''}
      <div class="row wrap" style="gap:10px;margin-top:10px"><label class="f grow" style="min-width:200px">Absenderadresse<input type="email" id="sm-from" value="${esc(c.fromEmail)}" placeholder="zeit@example.com"></label>
        <label class="f grow" style="min-width:160px">Absendername<input type="text" id="sm-name" value="${esc(c.fromName)}" placeholder="Tickly"></label></div>
      <div class="callout ${M.encrypted ? 'info' : 'warn'}" style="margin-top:12px">${ic('shield')}<span>${M.encrypted ? 'Das Passwort wird mit <code>TICKLY_SECRET</code> verschlüsselt gespeichert und nie wieder angezeigt.' : 'Das Passwort wird <b>unverschlüsselt</b> in der Datenbank gespeichert. Mit gesetztem <code>TICKLY_SECRET</code> wird es verschlüsselt.'}</span></div>
      <div class="row" style="margin-top:12px"><button class="btn primary" data-a="smtpSave">Speichern</button></div></div></div>
    <div class="stack"><div class="card"><div class="ch"><h3>Test-Nachricht</h3></div>
      <p class="hint" style="margin-bottom:10px">Schickt eine Nachricht mit den <b>gespeicherten</b> Einstellungen. Fehler (Verbindung, Anmeldung, Zertifikat) werden hier genau angezeigt.</p>
      <div class="row wrap"><input type="email" id="sm-to" placeholder="${esc(S.me.email)}" style="flex:1;min-width:200px"><button class="btn" data-a="smtpTest">${ic('mail', 15)} Test senden</button></div>
      ${T ? `<div class="callout ${T.ok ? 'info' : 'bad'}" style="margin-top:12px">${ic(T.ok ? 'check' : 'info')}<span>${T.ok ? `Nachricht an <b>${esc(T.to)}</b> abgeschickt. Sieh im Postfach nach.` : `<b>Fehlgeschlagen:</b> ${esc(T.error)}`}</span></div>` : ''}</div>
      <div class="card"><div class="ch"><h3>Benachrichtigungen</h3></div>
        ${setRow('Neue Registrierung wartet auf Freigabe', 'Mail an alle Administratoren.', `<button class="switch ${c.notifyAdminRegistration ? 'on' : ''}" data-a="smtpNotify" data-k="notifyAdminRegistration" role="switch"></button>`)}
        ${setRow('IP-Adresse wurde gesperrt', 'Mail an alle Administratoren.', `<button class="switch ${c.notifyAdminBlock ? 'on' : ''}" data-a="smtpNotify" data-k="notifyAdminBlock" role="switch"></button>`)}
        ${setRow('Konto wurde freigeschaltet', 'Mail an den Benutzer.', `<button class="switch ${c.notifyUserApproved ? 'on' : ''}" data-a="smtpNotify" data-k="notifyUserApproved" role="switch"></button>`)}
        <p class="hint" style="margin-top:10px">Immer verschickt, wenn eingeschaltet: Einladungen, Passwort zurücksetzen, Bestätigung der E-Mail-Adresse. Versand und Fehler stehen im Protokoll („Mail“).</p></div></div></div>`;
}
function vSessions() {
  const A = S.sessAdmin, q = S.sessQ.toLowerCase(), hit = (...v) => !q || v.some(x => String(x || '').toLowerCase().includes(q));
  const sess = A.sessions.filter(s => hit(s.name, s.email, s.ip, s.country, s.userAgent)), devs = A.devices.filter(d => hit(d.name, d.userName, d.email, d.version, d.os));
  return `<div class="card" style="padding:0"><div class="row wrap" style="padding:16px 18px"><h3>Aktive Sitzungen</h3><span class="muted sm">${sess.length} von ${A.sessions.length}</span>
      <span class="right row"><span style="position:relative"><span style="position:absolute;left:10px;top:9px;color:var(--muted)">${ic('search', 16)}</span><input type="search" id="sess-q" value="${esc(S.sessQ)}" placeholder="Benutzer, IP, Land …" style="padding-left:34px;width:230px"></span></span></div>
    <div class="tbwrap"><table class="tb"><thead><tr><th>Benutzer</th><th>IP-Adresse · Land</th><th>Browser</th><th>Angemeldet</th><th>Zuletzt aktiv</th><th>Läuft ab</th><th></th></tr></thead><tbody>
    ${sess.map(s => `<tr><td><b>${esc(s.name)}</b><br><span class="xs muted">${esc(s.email)}</span></td><td>${place(s.ip, s.country)}</td><td class="muted sm" title="${esc(s.userAgent)}">${esc(shortUa(s.userAgent))}</td><td class="sm">${fmtStamp(s.createdAt)}</td><td class="sm">${fmtAgo(s.lastSeenAt)}</td><td class="sm">${fmtStamp(s.expiresAt)}</td>
      <td>${s.current ? '<span class="badge good">Du</span>' : `<button class="btn sm" data-a="sessEnd" data-id="${s.id}" data-who="${esc(s.email)}">Beenden</button>`}</td></tr>`).join('') || '<tr><td colspan="7" class="muted" style="text-align:center;padding:20px">Keine Sitzungen.</td></tr>'}
    </tbody></table></div></div>
    <div class="card" style="padding:0"><div class="row wrap" style="padding:16px 18px"><h3>Companion-Geräte</h3><span class="muted sm">${devs.length} gekoppelt${A.latestVersion ? ` · neueste bekannte Version ${esc(A.latestVersion)}` : ''}</span></div>
    <div class="tbwrap"><table class="tb"><thead><tr><th>Gerät</th><th>Benutzer</th><th>System</th><th>Version</th><th>Zuletzt gesehen</th><th>Gekoppelt</th><th></th></tr></thead><tbody>
    ${devs.map(d => `<tr><td><b>${esc(d.name)}</b></td><td>${esc(d.userName)}<br><span class="xs muted">${esc(d.email)}</span></td><td>${d.os === 'macos' ? 'macOS' : 'Windows'}</td>
      <td class="mono sm">${esc(d.version || '–')} ${d.outdated ? `<span class="badge bad" title="Neueste bekannte Version: ${esc(A.latestVersion)}">veraltet</span>` : ''}</td>
      <td class="sm"><span class="row gap-s" style="gap:6px"><span class="dot ${d.online ? 'run' : 'stop'}"></span>${d.online ? 'online' : fmtAgo(d.lastSeenAt)}</span></td><td class="sm">${fmtStamp(d.createdAt)}</td>
      <td><button class="btn sm danger" data-a="devEnd" data-id="${d.id}" data-who="${esc(d.name)}">Kopplung aufheben</button></td></tr>`).join('') || '<tr><td colspan="7" class="muted" style="text-align:center;padding:20px">Keine Geräte gekoppelt.</td></tr>'}
    </tbody></table></div></div>
    <p class="hint">Land und echte Adresse kennt der Server nur hinter Cloudflare bzw. einem vertrauenswürdigen Proxy (Header <code>CF-Connecting-IP</code>, <code>CF-IPCountry</code>). Das Beenden einer Sitzung meldet den Benutzer sofort ab.</p>`;
}
function vSecurity() {
  const sc = S.sec; if (!sc) return '<div class="card empty"><p>Wird geladen …</p></div>';
  const p = sc.policy, fmtUntil = t => t ? fmtStamp(t) : 'bis zur Entsperrung';
  return `<div class="g-1-1"><div class="stack">
    <div class="card"><div class="ch"><h3>Automatische Sperre</h3></div>
      ${setRow('IP-Adressen nach Fehlversuchen sperren', 'Falsche Passwörter, Codes, Passkeys und Kopplungscodes zählen.', `<button class="switch ${p.enabled ? 'on' : ''}" data-a="secTog" role="switch"></button>`)}
      <div class="row wrap" style="padding:12px 0;gap:8px;align-items:center;${p.enabled ? '' : 'opacity:.5;pointer-events:none'}">Sperren ab <input type="number" min="3" max="100" value="${p.maxFails}" data-in="sec.policy.maxFails" data-type="num" data-nr="1" style="width:64px"> Fehlversuchen innerhalb von <input type="number" min="1" max="1440" value="${p.windowMin}" data-in="sec.policy.windowMin" data-type="num" data-nr="1" style="width:74px"> Min.</div>
      <div class="row wrap" style="padding:0 0 12px;gap:8px;align-items:center;${p.enabled ? '' : 'opacity:.5;pointer-events:none'}">Sperrdauer <input type="number" min="0" max="43200" value="${p.blockMin}" data-in="sec.policy.blockMin" data-type="num" data-nr="1" style="width:84px"> Min. <span class="muted xs">(0 = bis ein Administrator entsperrt)</span></div>
      <label class="f">Nie sperren (Ausnahmen, durch Komma getrennt)<input type="text" id="sec-allow" value="${esc(p.allowlist.join(', '))}" placeholder="z. B. 203.0.113.7, 10.1.0.0/16"></label>
      <div class="row" style="margin-top:12px"><button class="btn primary" data-a="secSave">Speichern</button></div>
      <div class="callout ${sc.originSecret ? 'info' : 'warn'}" style="margin-top:12px">${ic('shield')}<span>${sc.originSecret ? '<b>Origin-Prüfung aktiv.</b> Anfragen ohne den Header <span class="mono">X-Origin-Secret</span> mit dem richtigen Wert werden abgelehnt und im Protokoll (Art „Origin-Prüfung“) vermerkt.' : '<b>Origin-Prüfung aus.</b> Der Server nimmt Anfragen von jedem an, der den Port erreicht. Mit <span class="mono">ORIGIN_SECRET</span> in der <span class="mono">.env</span> lassen sich nur Anfragen über deinen Proxy zu.'}</span></div>
      <div class="callout info" style="margin-top:12px">${ic('info')}<span>Deine aktuelle Adresse: <b class="mono">${esc(sc.yourIp)}</b>. Die Adresse des Besuchers wird auch hinter Cloudflare oder einem anderen Proxy richtig erkannt. IPv6 wird nach <b>/64-Netzen</b> gesperrt.</span></div></div></div>
    <div class="stack"><div class="card"><div class="ch"><h3>Gesperrte Adressen</h3><span class="sub right">${sc.blocked.length} aktiv</span></div>
      ${sc.blocked.length ? sc.blocked.map(b => `<div class="kv" style="align-items:center"><span><b class="mono" style="color:var(--ink)">${esc(b.ip)}</b>${b.manual ? ' <span class="badge">manuell</span>' : ''}<br><span class="xs muted">${esc(b.reason)} · seit ${fmtStamp(b.blockedAt)} · ${fmtUntil(b.until)}</span></span><button class="btn sm" data-a="ipUnblock" data-ip="${esc(b.ip)}">Entsperren</button></div>`).join('') : '<div class="hint">Zurzeit ist keine Adresse gesperrt.</div>'}
      <div class="hr" style="margin:14px 0"></div>
      <div class="row wrap"><input type="text" id="blk-ip" placeholder="IP-Adresse manuell sperren" style="flex:1;min-width:160px"><input type="number" id="blk-min" min="0" placeholder="Min. (0 = dauerhaft)" style="width:140px;text-align:left"><button class="btn danger" data-a="ipBlock">Sperren</button></div></div></div></div>`;
}
function vLog() {
  const L = S.log, kinds = [['all', 'Alle'], ['fail', 'Fehlversuche'], ['auth', 'Anmeldungen'], ['block', 'Sperren'], ['admin', 'Verwaltung'], ['device', 'Geräte'], ['origin', 'Origin-Prüfung']];
  const tone = e => e.kind === 'block' ? 'bad' : e.kind === 'unblock' ? 'good' : e.ok ? '' : 'warn';
  return `<div class="card" style="padding:0"><div class="row wrap" style="padding:16px 18px"><div class="chips">${kinds.map(([k, l]) => `<button class="chip ${S.logKind === k ? 'on' : ''}" data-a="logKind" data-v="${k}">${l}</button>`).join('')}</div>
    <span class="right row"><span style="position:relative"><span style="position:absolute;left:10px;top:9px;color:var(--muted)">${ic('search', 16)}</span><input type="search" id="log-q" value="${esc(S.logQ)}" placeholder="IP, E-Mail, Text …" style="padding-left:34px;width:230px"></span><button class="btn" data-a="logSearch">Suchen</button><button class="btn" data-a="logReload" title="Aktualisieren">${ic('day', 15)}</button></span></div>
    <div class="tbwrap"><table class="tb"><thead><tr><th>Zeit</th><th>Ereignis</th><th>Benutzer</th><th>IP-Adresse</th><th>Gerät</th><th>Details</th></tr></thead><tbody>
    ${L.entries.map(e => `<tr><td class="mono sm" style="white-space:nowrap">${fmtStamp(e.at)}</td><td><span class="badge ${tone(e)}">${KIND_LABEL[e.kind] || esc(e.kind)}${e.ok ? '' : ' · fehlgeschlagen'}</span></td>
      <td class="sm">${e.email ? esc(e.email) : '<span class="muted">–</span>'}</td><td><a href="#/admin" class="mono sm" data-a="logIp" data-ip="${esc(e.ip)}" title="Nach dieser Adresse filtern">${esc(e.ip)}</a>${e.country ? ` <span title="${esc(countryName(e.country))}">${flag(e.country)}</span>` : ''}</td><td class="muted sm" title="${esc(e.userAgent)}">${esc(shortUa(e.userAgent))}</td><td class="sm" style="white-space:normal;min-width:220px">${esc(e.detail)}</td></tr>`).join('') || '<tr><td colspan="6" class="muted" style="text-align:center;padding:24px">Keine Einträge.</td></tr>'}
    </tbody></table></div>
    ${L.next ? `<div class="row" style="padding:14px 18px;justify-content:center"><button class="btn" data-a="logMore">Ältere Einträge laden</button></div>` : ''}</div>
    <p class="hint">Das Protokoll enthält IP-Adressen und damit personenbezogene Daten. Es ist nur für Administratoren sichtbar und wird nach der eingestellten Aufbewahrungszeit automatisch gelöscht (Standard 365 Tage, <code>LOG_RETENTION_DAYS</code>).</p>`;
}
function vBackup() {
  const list = S.backups || [], P = S.bkPol, pol = P && P.policy;
  const kindL = { auto: 'Automatisch', manual: 'Manuell', 'pre-restore': 'Vor Wiederherstellung' };
  const auto = !P ? '' : `<div class="card"><div class="ch"><h3>Automatische Sicherung</h3><button class="btn sm right" data-a="backupNow">${ic('plus', 14)} Jetzt sichern</button></div>
      ${setRow('Täglich automatisch sichern', 'Verpasste Zeiten (Server war aus) werden nachgeholt.', `<button class="switch ${pol.enabled ? 'on' : ''}" data-a="bkTog" role="switch" aria-checked="${pol.enabled}"></button>`)}
      <div class="row wrap" style="padding:12px 0;gap:10px;align-items:center;${pol.enabled ? '' : 'opacity:.5;pointer-events:none'}">Um <input type="text" id="bk-time" class="ttxt" value="${esc(pol.time)}" maxlength="5" placeholder="02:00"> Uhr · die letzten <input type="number" id="bk-keep" min="1" max="365" value="${pol.keep}" style="width:76px"> Sicherungen je Art behalten</div>
      <div class="stack gap-s" style="margin-bottom:10px">
        <label class="chk"><input type="radio" name="bk-target" value="dir" ${pol.toDir ? 'checked' : ''} ${P.dir.path ? '' : 'disabled'}>In das Sicherungsverzeichnis schreiben ${P.dir.path ? `<code>${esc(P.dir.path)}</code> ${P.dir.ok ? '<span class="badge good">beschreibbar</span>' : `<span class="badge bad" title="${esc(P.dir.error || '')}">nicht beschreibbar</span>`}` : '<span class="muted">(nicht eingerichtet – Umgebungsvariable <code>BACKUP_DIR</code>)</span>'}</label>
        <label class="chk"><input type="radio" name="bk-target" value="db" ${pol.toDir && P.dir.path ? '' : 'checked'}>In der Datenbank ablegen</label></div>
      <div class="callout ${P.encrypted ? 'info' : 'warn'}">${ic('shield')}<span>${P.encrypted ? '<b>Verschlüsselt:</b> Sicherungen (Dateien und Datenbank), TOTP-Geheimnisse und das SMTP-Passwort sind mit AES-256 geschützt. Der Schlüssel stammt aus <code>TICKLY_SECRET</code> auf dem Server und steht nicht in der Datenbank. Geht er verloren, sind verschlüsselte Sicherungen unlesbar – sicher aufbewahren.' : '<b>Nicht verschlüsselt.</b> Setze <code>TICKLY_SECRET</code> (z. B. <code>openssl rand -base64 32</code>) auf dem Server, damit Sicherungen, TOTP-Geheimnisse und das SMTP-Passwort verschlüsselt gespeichert werden.'}</span></div>
      <div class="row" style="margin-top:12px"><button class="btn primary" data-a="bkSave">Speichern</button></div></div>`;
  return `<div class="g-1-1"><div class="stack">${auto}
    <div class="card"><div class="ch"><h3>Sicherung herunterladen</h3></div>
      <p class="hint" style="margin-bottom:12px">Lädt eine Datei mit dem gesamten Datenbestand herunter: Benutzer, Zeiten, Abwesenheiten, Einstellungen, Passkeys und gekoppelte Geräte.${P && P.encrypted ? ' Die Datei ist verschlüsselt.' : ''}</p>
      <a class="btn primary" href="/api/admin/backup" download>${ic('download', 16)} Sicherung herunterladen</a>
      <div class="callout warn" style="margin-top:14px">${ic('shield')}<span>Die Datei enthält Passwort-Hashes und Passkeys${P && P.encrypted ? '' : ' (unverschlüsselt)'}. Sicher aufbewahren und nicht weitergeben.</span></div></div>
    <div class="card"><div class="ch"><h3>Wiederherstellen</h3></div>
      <p class="hint" style="margin-bottom:12px">Ersetzt <b>alle</b> aktuellen Daten durch den Stand der Sicherung. Vorher wird der jetzige Stand automatisch gesichert (siehe rechts). Du bleibst angemeldet, alle anderen müssen sich neu anmelden.</p>
      <div class="row wrap"><input type="file" id="bk-file" accept=".gz,.enc,.json,application/gzip,application/json" style="max-width:260px"><button class="btn danger" data-a="restoreFile">${ic('download', 16)} Wiederherstellen …</button></div></div></div>
    <div class="card"><div class="ch"><h3>Gespeicherte Sicherungen</h3></div>
      <p class="hint" style="margin-bottom:8px">Automatisch, manuell („Jetzt sichern“) und vor jeder Wiederherstellung. Je Art bleiben die neuesten erhalten${pol ? ` (automatisch/manuell: ${pol.keep}, vor Wiederherstellung: 5)` : ''}.</p>
      ${list.length ? list.map(b => `<div class="kv" style="align-items:center"><span><b style="color:var(--ink)">${fmtStamp(b.createdAt)}</b><br><span class="xs muted">${kindL[b.kind] || b.kind} · ${b.file ? 'Verzeichnis' : 'Datenbank'}${b.users != null ? ` · ${b.users} Benutzer` : ''} · ${fmtSize(b.size)}</span></span>
        <span class="row gap-s"><a class="ibtn" href="/api/admin/backups/${encodeURIComponent(b.id)}" download title="Herunterladen">${ic('download', 16)}</a><button class="btn sm" data-a="restoreSaved" data-id="${esc(b.id)}" data-at="${esc(fmtStamp(b.createdAt))}">Wiederherstellen</button></span></div>`).join('') : '<div class="hint">Noch keine gespeicherte Sicherung.</div>'}</div></div>`;
}
function vAdmin() {
  if (!isAdmin()) return `<div class="card empty"><div class="big">${ic('lock', 26)}</div><h3>Kein Zugriff</h3><p>Dieser Bereich ist Administratoren vorbehalten.</p></div>`;
  const U = S.users, q = S.adminQ.toLowerCase();
  const filtered = U.filter(u => (S.adminFilter === 'all' || u.status === S.adminFilter) && (!q || u.name.toLowerCase().includes(q) || u.email.toLowerCase().includes(q)));
  const cnt = k => U.filter(u => u.status === k).length, act = U.filter(u => u.status === 'active');
  const head = `<div class="tabs"><button class="${S.adminTab === 'users' ? 'on' : ''}" data-a="adminTab" data-v="users">Benutzer</button><button class="${S.adminTab === 'reg' ? 'on' : ''}" data-a="adminTab" data-v="reg">Registrierung & Richtlinien</button><button class="${S.adminTab === 'backup' ? 'on' : ''}" data-a="adminTab" data-v="sec">Sicherheit & Sperren${S.sec && S.sec.blocked.length ? ` <span class="badge bad" style="margin-left:4px">${S.sec.blocked.length}</span>` : ''}</button><button class="${S.adminTab === 'mail' ? 'on' : ''}" data-a="adminTab" data-v="mail">E-Mail</button><button class="${S.adminTab === 'sess' ? 'on' : ''}" data-a="adminTab" data-v="sess">Sitzungen & Geräte</button><button class="${S.adminTab === 'log' ? 'on' : ''}" data-a="adminTab" data-v="log">Protokoll</button><button class="${S.adminTab === 'system' ? 'on' : ''}" data-a="adminTab" data-v="system">System${S.update && S.update.enabled && S.update.newer ? ' <span class="badge warn" style="margin-left:4px">neu</span>' : ''}</button><button class="${S.adminTab === 'backup' ? 'on' : ''}" data-a="adminTab" data-v="backup">Sicherung</button></div>`;
  const kpi = `<div class="g4"><div class="tile"><div class="l">${ic('users', 15)}Benutzer</div><div class="v">${U.length}</div><div class="s">${cnt('active')} aktiv · ${cnt('blocked')} gesperrt</div></div>
    <div class="tile"><div class="l">${ic('bolt', 15)}Heute aktiv</div><div class="v">${U.filter(u => u.today === 'tracking').length}</div><div class="s">Companion verbunden</div></div>
    <div class="tile"><div class="l">${ic('clock' in ICONS ? 'clock' : 'day', 15)}Wartet auf Freigabe</div><div class="v">${cnt('pending')}</div><div class="s">${cnt('pending') ? 'Aktion erforderlich' : 'Alles erledigt'}</div></div>
    <div class="tile"><div class="l">${ic('fp', 15)}Passkey-Quote</div><div class="v">${Math.round(act.filter(u => u.passkeys > 0).length / Math.max(1, act.length) * 100)}<small>%</small></div><div class="s">${act.filter(u => u.passkeys > 0).length} von ${act.length} aktiven Nutzern</div></div></div>`;
  if (S.adminTab === 'backup') return `<div class="stack">${head}${vBackup()}</div>`;
  if (S.adminTab === 'sec') return `<div class="stack">${head}${vSecurity()}</div>`;
  if (S.adminTab === 'mail') return `<div class="stack">${head}${vMail()}</div>`;
  if (S.adminTab === 'sess') return `<div class="stack">${head}${vSessions()}</div>`;
  if (S.adminTab === 'log') return `<div class="stack">${head}${vLog()}</div>`;
  if (S.adminTab === 'system') return `<div class="stack">${head}${vUpdate()}</div>`;
  if (S.adminTab === 'reg') {
    return `<div class="stack">${head}<div class="g-1-1"><div class="card"><div class="ch"><h3>Registrierung</h3></div>
      ${setRow('Offene Registrierung', 'Jeder kann sich selbst ein Konto anlegen. Aus: nur per Einladung.', toggle('reg.open', S.reg.open))}
      ${setRow('Freigabe durch Administrator', 'Neue Konten bleiben gesperrt, bis ein Admin sie freigibt.', toggle('reg.approval', S.reg.approval))}
      ${setRow('Passkey-Einrichtung anbieten', 'Nach der Registrierung wird die Passkey-Einrichtung vorgeschlagen.', toggle('reg.passkeyPrompt', S.reg.passkeyPrompt))}
      <div class="setrow"><div class="tx"><b>Erlaubte E-Mail-Domain</b><span>Leer = alle Domains erlaubt.</span></div><input type="text" value="${esc(S.reg.domains)}" placeholder="@firma.de" data-in="reg.domains" data-nr="1" style="width:170px"></div>
      ${setRow('E-Mail-Adresse bestätigen', 'Neue Konten müssen ihre Adresse per Link bestätigen, bevor sie sich anmelden dürfen. Braucht einen eingerichteten SMTP-Server.', toggle('reg.verifyEmail', S.reg.verifyEmail))}
      ${setRow('Zurücksetzen per Sicherheitsfragen', 'Neue Konten mit Passwort wählen bei der Registrierung drei Sicherheitsfragen. Damit können sie ihr Passwort selbst zurücksetzen. Schwächer als ein Link per Mail, weil sich Antworten erraten lassen; Fehlversuche sperren das Konto für diesen Weg.', toggle('reg.secQuestions', S.reg.secQuestions))}
      <div class="row" style="margin-top:12px"><a class="btn" href="#/register">Registrierungsseite ansehen</a></div></div>
      <div class="card"><div class="ch"><h3>Standardwerte für neue Benutzer</h3></div>
        <div class="setrow"><div class="tx"><b>Wochenarbeitszeit</b><span>Als Vorgabe für das Tagessoll (Mo–Fr).</span></div><input type="number" value="${S.reg.weekly}" data-in="reg.weekly" data-type="num" style="width:90px"><span class="muted">h</span></div>
        <div class="setrow"><div class="tx"><b>Mindestlänge Passwort</b><span>Mindestens 14 Zeichen. Immer mit Groß- und Kleinbuchstaben, Zahl und Sonderzeichen.</span></div><select data-in="reg.minPw" data-type="num" style="width:90px">${[14, 16, 20, 24].map(v => `<option ${S.reg.minPw === v ? 'selected' : ''}>${v}</option>`).join('')}</select><span class="muted" style="visibility:hidden" aria-hidden="true">h</span></div>
        <div class="setrow"><div class="tx"><b>Warnung bei langem Tracking</b><span>Läuft das Tracking so viele Stunden (Brutto), bekommt der Benutzer einen Hinweis im Dashboard und eine Mail (mit SMTP), ob er es vergessen hat. 0 = aus.</span></div><input type="number" min="0" max="24" value="${S.reg.longTrackHours ?? 10}" data-in="reg.longTrackHours" data-type="num" style="width:90px"><span class="muted">h</span></div>
        <div class="callout info" style="margin-top:12px">${ic('info')}<span>Die Registrierungsseite reagiert sofort auf diese Einstellungen.</span></div></div></div>
      ${(() => {
        const need = S.users.filter(u => u.mfa && u.mfa.required && !u.mfa.compliant), late = need.filter(u => u.mfa.expired);
        return `<div class="card"><div class="ch"><h3>Anmeldung & Sitzungen</h3></div>
          ${setRow('Passkey oder Authenticator-App verpflichtend', 'Mindestens einer von beiden muss eingerichtet sein. Ein Passwort allein genügt dann nicht mehr.', toggle('reg.mfaRequired', S.reg.mfaRequired))}
          <div class="setrow"><div class="tx"><b>Karenzzeit</b><span>So viele Tage ab Einschalten (bzw. ab Anlage des Kontos) darf man noch ohne arbeiten. 0 = sofort einrichten. Währenddessen erscheint ein Hinweis, die Anmeldung gilt 24 Stunden und bei jeder Anmeldung folgt eine Erinnerung.</span></div><input type="number" min="0" max="365" value="${S.reg.mfaGraceDays}" data-in="reg.mfaGraceDays" data-type="num" style="width:84px" ${S.reg.mfaRequired ? '' : 'disabled'}><span class="muted">Tage</span></div>
          <div class="setrow"><div class="tx"><b>Sitzungsdauer</b><span>So lange bleibt man angemeldet. Konten ohne Passkey/Authenticator unter der Pflicht müssen sich täglich neu anmelden. Gilt für neue Anmeldungen.</span></div><input type="number" min="1" max="365" value="${S.reg.sessionDays}" data-in="reg.sessionDays" data-type="num" style="width:84px"><span class="muted">Tage</span></div>
          <div class="setrow"><div class="tx"><b>Nachträgliches Bearbeiten von Zeiten</b><span>So viele Tage rückwirkend dürfen Benutzer Zeiten korrigieren oder manuell ergänzen. 0 = nicht erlaubt. Jede Änderung steht im Protokoll.</span></div><input type="number" min="0" max="3650" value="${S.reg.editDaysBack}" data-in="reg.editDaysBack" data-type="num" style="width:84px"><span class="muted">Tage</span></div>
          ${setRow('Beginn eines laufenden Trackings ändern', 'Benutzer dürfen den Beginn des laufenden Trackings früher oder später setzen (Übersicht und Tagesansicht, Knopf „Beginn ändern“). Jede Änderung steht im Protokoll.', toggle('reg.allowStartChange', S.reg.allowStartChange !== false))}
          ${setRow('Ende vorab festlegen', 'Benutzer dürfen beim Beenden eine spätere Uhrzeit wählen: Das Tracking läuft weiter und endet zu dieser Zeit von selbst, auch ohne Companion. Ausgeschaltet muss das Ende jetzt oder früher liegen.', toggle('reg.allowPlannedEnd', S.reg.allowPlannedEnd !== false))}
          ${S.reg.mfaRequired ? `<div class="callout ${late.length ? 'warn' : 'info'}" style="margin-top:12px">${ic('shield')}<span>${need.length ? `<b>${need.length} Konto${need.length === 1 ? '' : 'en'} ohne Passkey/Authenticator</b>${late.length ? `, bei ${late.length} ist die Karenzzeit abgelaufen (nur Einrichtung möglich)` : ''}: ${need.map(u => esc(u.name)).join(', ')}.` : 'Alle aktiven Konten haben Passkey oder Authenticator-App eingerichtet.'}</span></div>` : ''}
          <p class="hint" style="margin-top:10px">Die Companion-App meldet weiter Zeiten, auch wenn die Frist abgelaufen ist, damit keine Arbeitszeit verloren geht. Sie weist den Benutzer ebenfalls auf die Einrichtung hin.</p></div>`;
      })()}</div>`;
  }
  const chips = [['all', 'Alle', U.length], ['active', 'Aktiv', cnt('active')], ['pending', 'Freigabe offen', cnt('pending')], ['invited', 'Eingeladen', cnt('invited')], ['blocked', 'Gesperrt', cnt('blocked')]];
  return `<div class="stack">${head}${kpi}
    <div class="card" style="padding:0"><div class="row wrap" style="padding:16px 18px"><div class="chips">${chips.map(([k, l, n]) => `<button class="chip ${S.adminFilter === k ? 'on' : ''}" data-a="adminFilter" data-v="${k}">${l} · ${n}</button>`).join('')}</div>
      <span class="right row"><span style="position:relative"><span style="position:absolute;left:10px;top:9px;color:var(--muted)">${ic('search', 16)}</span><input type="search" id="admin-q" value="${esc(S.adminQ)}" placeholder="Suchen…" data-in="adminQ" style="padding-left:34px;width:220px"></span><button class="btn primary" data-a="userInvite">${ic('plus', 16)} Benutzer einladen</button></span></div>
      <div class="tbwrap"><table class="tb"><thead><tr><th>Benutzer</th><th>Rolle</th><th>Status</th><th>Heute</th><th class="num">Wochensoll</th><th class="num">Überstunden</th><th>Passkeys</th><th>Zuletzt aktiv</th><th></th></tr></thead><tbody>
      ${filtered.map(u => `<tr><td><div class="row">${avatar(u.name, 'sm2')}<div><b>${esc(u.name)}</b><br><span class="xs muted">${esc(u.email)}</span></div></div></td>
        <td>${u.role === 'admin' ? '<span class="badge brand">Admin</span>' : 'Benutzer'}</td><td><span class="badge ${STAT[u.status][1]}">${STAT[u.status][0]}</span></td>
        <td>${u.today === 'none' ? '<span class="muted">–</span>' : `<span class="row gap-s" style="gap:6px">${TODAYL[u.today][1] ? `<span class="dot ${TODAYL[u.today][1]}"></span>` : ''}${TODAYL[u.today][0]}</span>`}</td>
        <td class="num">${u.week} h</td><td class="num">${u.status === 'active' ? sgn(u.bal) : '<span class="muted">–</span>'}</td>
        <td>${u.passkeys || u.totp ? `<span class="row gap-s" style="gap:8px">${u.passkeys ? `<span class="row gap-s" style="gap:5px">${ic('fp', 15)}${u.passkeys}</span>` : ''}${u.totp ? '<span class="badge">TOTP</span>' : ''}</span>` : u.mfa && u.mfa.required && !u.mfa.compliant ? `<span class="badge ${u.mfa.expired ? 'bad' : 'warn'}" title="Frist bis ${mfaDate(u.mfa)}">ohne MFA</span>` : '<span class="muted">–</span>'}</td><td class="muted sm" style="white-space:normal;min-width:120px">${esc(u.last)}</td>
        <td style="white-space:nowrap;text-align:right">${u.status === 'pending' ? `<button class="btn sm primary" data-a="userStatus" data-id="${u.id}" data-v="active">Freigeben</button> <button class="btn sm" data-a="userDel" data-id="${u.id}">Ablehnen</button>` : u.status === 'invited' ? `<button class="btn sm" data-a="toast" data-m="Einladung erneut gesendet an ${esc(u.email)}">Erneut senden</button> <button class="ibtn" data-a="userEdit" data-id="${u.id}">${ic('edit', 16)}</button>` : `<button class="ibtn" data-a="userEdit" data-id="${u.id}" title="Bearbeiten">${ic('edit', 16)}</button>`}</td></tr>`).join('') || `<tr><td colspan="9"><div class="empty"><p>Keine Benutzer gefunden.</p></div></td></tr>`}
      </tbody></table></div></div></div>`;
}

/* ---------- Modals ---------- */
function absForm(f) {
  const n = f.from && f.to && f.to >= f.from ? absWorkdays({ from: f.from, to: f.to }) : 0;
  return `<div class="ov" data-a="ovClose"><div class="modal"><div class="mh"><h3>${f.id ? 'Abwesenheit bearbeiten' : 'Abwesenheit eintragen'}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><div class="seg" style="align-self:flex-start">${Object.keys(KINDS).filter(k => k !== 'holiday').map(k => `<button class="${f.type === k ? 'on' : ''}" data-a="absType" data-v="${k}">${KINDS[k].n}</button>`).join('')}</div>
    <div class="row wrap"><label class="f grow">Von${dateField('modal.f.from', f.from, { rm: true })}</label><label class="f grow">Bis${dateField('modal.f.to', f.to, { rm: true })}</label></div>
    <label class="f">Notiz (optional)<input type="text" value="${esc(f.note || '')}" data-in="modal.f.note" data-nr="1" placeholder="z. B. Sommerurlaub"></label>
    <div class="callout ${f.type === 'comp' ? 'warn' : 'info'}">${ic('info')}<span>${f.type === 'comp' ? `<b>Zeitausgleich</b> baut Überstunden ab: ${n} Tag${n === 1 ? '' : 'e'} × Tagessoll werden vom Überstundenkonto abgezogen.` : f.type === 'sick' ? `<b>${n} Arbeitstag${n === 1 ? '' : 'e'}</b> Krankheit – Soll entfällt, kein Minus auf dem Konto.` : `<b>${n} Arbeitstag${n === 1 ? '' : 'e'}</b> Urlaub – Wochenenden und Feiertage werden nicht mitgezählt. Das Soll entfällt, es entsteht kein Minus.`}</span></div></div>
    <div class="mf">${f.id ? `<button class="btn danger" style="margin-right:auto" data-a="absDel" data-id="${f.id}">Löschen</button>` : ''}<button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="absSave" ${n ? '' : 'disabled'}>Speichern</button></div></div></div>`;
}
function renderModal() {
  const m = S.modal; if (!m) return '';
  if (m.type === 'abs') return absForm(m.f);
  if (m.type === 'dayedit') return dayEditModal(m);
  if (m.type === 'passkey') {
    const win = S.os === 'windows', reg = m.mode !== 'login';
    return `<div class="ov"><div class="modal" style="width:min(400px,100%)"><div class="mb" style="align-items:center;text-align:center;padding:30px 26px 10px;gap:12px">
      <div class="spin" style="margin:12px 0"></div><h3 style="font-size:18px">${reg ? 'Passkey einrichten' : 'Mit Passkey anmelden'}</h3>
      <p class="muted">Bestätige im Dialog deines Geräts (${win ? 'Windows Hello, PIN' : 'Touch ID'}, Sicherheitsschlüssel oder Smartphone).</p></div>
      <div class="mf" style="justify-content:center"><button class="btn" data-a="pkCancel">Abbrechen</button></div></div></div>`;
  }
  if (m.type === 'user') {
    const u = m.u, self = u.id === S.me.id;
    return `<div class="ov" data-a="ovClose"><div class="modal"><div class="mh"><h3>${u.isNew ? 'Benutzer einladen' : 'Benutzer bearbeiten'}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div><div class="mb">
      ${u.isNew ? '' : `<div class="row">${avatar(u.name)}<div><b>${esc(u.name)}</b><br><span class="xs muted">${esc(u.last)}</span></div></div>`}
      <label class="f">Name<input type="text" value="${esc(u.name)}" data-in="modal.u.name" data-nr="1" placeholder="Vor- und Nachname"></label>
      <label class="f">E-Mail<input type="email" value="${esc(u.email)}" data-in="modal.u.email" data-nr="1" placeholder="name@firma.de">${u.isNew ? '' : u.pendingEmail ? `<span class="hint" style="font-weight:400">Die Änderung auf <b>${esc(u.pendingEmail)}</b> wartet auf die Bestätigung per Mail. Zurücknehmen: bisherige Adresse eintragen und speichern.</span>` : '<span class="hint" style="font-weight:400">Eine neue Adresse gilt erst, wenn der Benutzer den Link in der Mail an die neue Adresse öffnet.</span>'}</label>
      <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:12px;align-items:end"><label class="f">Rolle<select data-in="modal.u.role" data-nr="1" style="height:38px"><option value="user" ${u.role === 'user' ? 'selected' : ''}>Benutzer</option><option value="admin" ${u.role === 'admin' ? 'selected' : ''}>Administrator</option></select></label>
      <label class="f">Wochensoll (h)<input type="number" style="width:100%;text-align:left" value="${u.week}" data-in="modal.u.week" data-type="num" data-nr="1"></label>${u.isNew ? '' : `<label class="f" title="${u.startBalSet === false ? 'Noch nicht festgelegt' : ''}">Überstunden-Übertrag (h)${u.startBalSet === false ? ' *' : ''}<input type="number" step="0.25" style="width:100%;text-align:left" value="${u.startBal ?? 0}" data-in="modal.u.startBal" data-type="num" data-nr="1"></label>`}</div>${!u.isNew && u.startBalSet === false ? '<p class="hint" style="margin-top:-4px">* Der Überstunden-Übertrag ist noch nicht festgelegt.</p>' : ''}
      ${u.isNew ? `<label class="chk"><input type="checkbox" ${u.direct ? 'checked' : ''} data-a="inviteDirect">Direkt mit Initialpasswort anlegen (statt Einladung per Link)</label>
        ${u.direct ? `<label class="f">Initialpasswort (leer lassen = zufällig erzeugen)<input type="text" value="${esc(u.pw || '')}" data-in="modal.u.pw" data-nr="1" autocomplete="off" spellcheck="false" class="mono"></label>
          <p class="hint">Der Benutzer meldet sich mit seiner E-Mail und diesem Passwort an und legt dann ein eigenes Passwort und seine Sicherheitsfragen fest. Das Konto ist sofort aktiv.</p>` : `<p class="hint">Der Benutzer erhält einen Link zur Registrierung und kann sich anschließend per Passwort oder Passkey anmelden.</p>`}` : `<div class="chips">${self ? '' : `<button class="btn sm" data-a="pwAdminOpen">${ic('key', 14)} Neues Passwort setzen</button>`}<button class="btn sm" data-a="pkReset" ${u.passkeys ? '' : 'disabled'}>${ic('fp', 14)} Passkeys zurücksetzen (${u.passkeys})</button><button class="btn sm" data-a="endAllSess" data-id="${u.id}">${ic('logout', 14)} Alle Sitzungen beenden</button><button class="btn sm" data-a="totpReset" data-id="${u.id}" ${u.totp ? '' : 'disabled'}>${ic('shield', 14)} TOTP zurücksetzen</button></div>`}
      </div><div class="mf">${u.isNew ? '' : `<div class="row" style="margin-right:auto">${self ? '' : `<button class="btn danger" data-a="userDel" data-id="${u.id}">Löschen</button>${u.status === 'blocked' ? `<button class="btn" data-a="userStatus" data-id="${u.id}" data-v="active">Entsperren</button>` : `<button class="btn" data-a="userStatus" data-id="${u.id}" data-v="blocked">Sperren</button>`}`}</div>`}<button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="userSave">${u.isNew ? (u.direct ? 'Konto anlegen' : 'Einladung senden') : 'Speichern'}</button></div></div></div>`;
  }
  if (m.type === 'mfaRemind') return `<div class="ov"><div class="modal" style="width:min(450px,100%)"><div class="mh"><h3>Konto absichern</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><div class="callout warn">${ic('shield')}<span>Dein Administrator verlangt für alle Konten einen <b>Passkey</b> oder eine <b>Authenticator-App</b>.</span></div>
      <p>Du kannst noch bis <b>${mfaDate(S.mfa)}</b> ohne arbeiten (${daysText(S.mfa.daysLeft)}). Danach geht ohne Einrichtung nichts mehr.</p>
      <p class="hint">Solange nichts eingerichtet ist, bleibt deine Anmeldung 24 Stunden gültig, und du wirst bei jeder Anmeldung daran erinnert. Das Einrichten dauert etwa eine Minute.</p></div>
    <div class="mf"><button class="btn" data-a="modalClose">Später erinnern</button><button class="btn primary" data-a="mfaGo">Jetzt einrichten</button></div></div></div>`;
  if (m.type === 'stop') {
    const custom = m.mode === 'custom', at = m.prev ? m.prev.at : m.at, pl = custom && m.prev && m.prev.planned, can = S.limits.plannedEnd;
    return `<div class="ov"><div class="modal" style="width:min(460px,100%)"><div class="mh"><h3>${pl ? 'Ende vorab festlegen' : 'Tracking beenden'}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb">${custom
      ? `<label class="f">Ende (Uhrzeit)${timeField('modal.at', m.at)}<span class="hint">${can ? `Bis jetzt (${hm(S.now)}) beendet es das Tracking sofort, alles danach wird nicht gezählt. Eine <b>spätere Uhrzeit</b> legt das Ende vorab fest: Das Tracking läuft weiter und endet dann von selbst.` : `Zwischen Arbeitsbeginn und jetzt (${hm(S.now)}). Alles danach wird nicht gezählt.`}</span></label>`
      : `<p>Das Tracking endet <b>jetzt, um ${hm(m.prev ? m.prev.at : S.now)} Uhr</b>. Danach wird nichts mehr gezählt.</p>`}
      ${pl ? `<div class="callout info">${ic('day')}<span>Das Tracking läuft weiter und endet um <b>${hm(at)} Uhr</b> von selbst. Die Zahlen unten zeigen den Tag, wie er dann aussähe. Du kannst das Ende bis dahin ändern oder aufheben.</span></div>` : ''}
      ${stopSummary(m)}
      <p class="sm"><a href="#" data-a="stopMode" style="color:var(--brand);font-weight:600">${custom ? 'Stattdessen jetzt beenden' : (can ? 'Ende festlegen (auch für später) …' : 'Anderes Ende festlegen …')}</a></p></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="stopDo" ${m.prev ? '' : 'disabled'}>${pl ? `Ende für ${hm(at)} vormerken` : custom ? `Mit Ende ${hm(at)} beenden` : 'Tracking beenden'}</button></div></div></div>`;
  }
  if (m.type === 'startat') {
    const cur = m.prev ? m.prev.at : m.sat, changed = m.prev && Math.abs(m.prev.at - m.prev.from) >= .01;
    return `<div class="ov"><div class="modal" style="width:min(460px,100%)"><div class="mh"><h3>Beginn ändern</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p>Das laufende Tracking begann um <b>${m.from != null ? hm(m.from) : '…'} Uhr</b>. Setze den Beginn auf die Zeit, zu der du wirklich angefangen hast. Die Zeit wird sofort neu gerechnet, auch im Companion.</p>
      <label class="f">Beginn (Uhrzeit)${timeField('modal.sat', m.sat == null ? 0 : m.sat)}<span class="hint">Nicht später als jetzt (${hm(S.now)}) und nicht vor der Arbeitszeit, die davor schon eingetragen ist.</span></label>
      <div class="row wrap" style="gap:6px">${[[-30, '30 Min. früher'], [-5, '5 Min. früher'], [5, '5 Min. später']].map(([d, l]) => `<button class="chip" data-a="startShift" data-d="${d}">${l}</button>`).join('')}</div>
      ${startSummary(m)}</div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="startDo" ${changed ? '' : 'disabled'}>${m.prev ? `Beginn auf ${hm(cur)} setzen` : 'Beginn setzen'}</button></div></div></div>`;
  }
  if (m.type === 'night') {
    const prev = m.day === 'prev', n = S.night || {};
    return `<div class="ov"><div class="modal" style="width:min(460px,100%)"><div class="mh"><h3>Wann hast du aufgehört?</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p>Das Tracking läuft seit <b>${n.yesterday ? dMid(n.yesterday) : 'gestern'}, ${hm(n.since || 0)} Uhr</b>. Alles nach dem Ende wird nicht gezählt.</p>
      <div class="row" style="gap:8px;margin-bottom:10px"><button class="btn sm ${prev ? 'primary' : ''}" data-a="nightDay" data-v="prev">Gestern${n.yesterday ? ' (' + dMid(n.yesterday) + ')' : ''}</button><button class="btn sm ${prev ? '' : 'primary'}" data-a="nightDay" data-v="today">Heute (nach Mitternacht)</button></div>
      <label class="f">Ende (Uhrzeit)${timeField('modal.nat', m.at)}<span class="hint">${prev ? 'Zwischen Arbeitsbeginn und Mitternacht.' : `Zwischen 00:00 und jetzt (${hm(S.now)}).`}</span></label>
      ${nightSummary(m)}</div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="nightDo" ${m.prev ? '' : 'disabled'}>${m.prev ? `Mit Ende ${hm(m.prev.at)} beenden` : 'Beenden'}</button></div></div></div>`;
  }
  if (m.type === 'trackstart') return `<div class="ov"><div class="modal" style="width:min(480px,100%)"><div class="mh"><h3>Zeiterfassung ab</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p>Ab diesem Tag rechnet Tickly deine Arbeitszeit und das Überstundenkonto. <b>Tage davor zählen nicht</b>, es entstehen dort also keine Minusstunden. Das ist nützlich, wenn du erst ausprobierst und einzelne Tage nicht vollständig aufgezeichnet hast: Setze das Datum einfach auf den ersten Tag, ab dem es ernst ist.</p>
      <label class="f">Startdatum${dateField('modal.date', m.date)}</label>
      <p class="hint">Die Zeiten früherer Tage bleiben gespeichert, lassen sich aber nicht mehr bearbeiten. Setzt du das Datum später wieder früher, zählen sie wieder mit. Der Überstunden-Übertrag gilt ab dem Startdatum.</p></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="trackStartSave">Speichern</button></div></div></div>`;
  if (m.type === 'startbal') return `<div class="ov"><div class="modal" style="width:min(480px,100%)"><div class="mh"><h3>Überstunden zu Beginn</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p>Hast du Überstunden (oder Minusstunden), die Tickly von Anfang an mitrechnen soll? Das legst du <b>einmalig</b> fest, danach kann es nur noch ein Administrator ändern. Hast du keine, wähle „Keine Überstunden“.</p>
      ${balForm()}</div>
    <div class="mf"><button class="btn" data-a="modalClose">Später</button><button class="btn primary" data-a="startBalSave">Festlegen</button></div></div></div>`;
  if (m.type === 'secq') return `<div class="ov"><div class="modal" style="width:min(520px,100%)"><div class="mh"><h3>Sicherheitsfragen</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb">${secQForm('mq')}<label class="f">Zur Bestätigung: dein Passwort<input type="password" id="mq-cur" autocomplete="current-password"></label></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="secqSave">Speichern</button></div></div></div>`;
  if (m.type === 'adminpw') return `<div class="ov"><div class="modal" style="width:min(460px,100%)"><div class="mh"><h3>Neues Passwort für ${esc(m.u.name)}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p>Du vergibst ein <b>Initialpasswort</b>. ${esc(m.u.name)} wird überall abgemeldet und muss beim nächsten Anmelden ein eigenes Passwort festlegen.</p>
      <label class="f">Eigenes Initialpasswort (leer lassen = zufällig erzeugen)<input type="text" id="ap-pw" autocomplete="off" spellcheck="false" class="mono"></label>
      <p class="hint">Ein erzeugtes Passwort ist lang und zufällig. Gib es dem Benutzer auf einem sicheren Weg weiter (nicht per E-Mail im Klartext).</p></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="pwAdminSet">Passwort setzen</button></div></div></div>`;
  if (m.type === 'adminpwDone') return `<div class="ov"><div class="modal" style="width:min(460px,100%)"><div class="mh"><h3>${m.created ? 'Konto angelegt' : 'Initialpasswort gesetzt'}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p>${m.created ? 'Das Konto ist aktiv. ' : ''}Initialpasswort für <b>${esc(m.u.name)}</b>. Es wird nur jetzt angezeigt:</p>
      <div class="row" style="gap:8px"><input type="text" readonly value="${esc(m.password)}" class="mono grow" style="user-select:all" onclick="this.select()"><button class="btn" data-a="pwCopy">Kopieren</button></div>
      ${m.pwLogin ? '' : `<div class="callout warn">${ic('info')}<span>Die Anmeldung mit Passwort ist für dieses Konto abgeschaltet (Passkey). Das Passwort gilt erst, wenn der Benutzer sie wieder einschaltet.</span></div>`}
      <p class="hint">Der Benutzer muss es beim ${m.created ? 'ersten' : 'nächsten'} Anmelden ändern${m.created ? ' und Sicherheitsfragen festlegen' : ''}.</p></div>
    <div class="mf"><button class="btn primary" data-a="modalClose">Fertig</button></div></div></div>`;
  if (m.type === 'pwchange') return `<div class="ov"><div class="modal" style="width:min(420px,100%)"><div class="mh"><h3>Passwort ändern</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><label class="f">Aktuelles Passwort<input type="password" id="pw-cur" autocomplete="current-password"></label>
      <label class="f">Neues Passwort<input type="password" id="rg-pw" autocomplete="new-password"></label><div class="pwrules" id="pw-rules">${pwRulesHtml('')}</div>
      <p class="hint">Andere angemeldete Geräte werden abgemeldet.</p></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="pwSave">Speichern</button></div></div></div>`;
  if (m.type === 'totp') return `<div class="ov"><div class="modal" style="width:min(440px,100%)"><div class="mh"><h3>Authenticator-App einrichten</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb" style="align-items:center;text-align:center">
      <p class="muted">1. Scanne den QR-Code mit deiner Authenticator-App.</p>
      <img src="${m.qr}" alt="QR-Code" width="200" height="200" style="image-rendering:pixelated;background:#fff;border-radius:10px;padding:8px">
      <p class="xs muted">Kein Scan möglich? Schlüssel eingeben:<br><b class="mono" style="color:var(--ink);letter-spacing:.08em;user-select:all">${m.secret.replace(/(.{4})/g, '$1 ').trim()}</b></p>
      <p class="muted" style="margin-top:6px">2. Gib den angezeigten 6-stelligen Code ein.</p>
      <input type="text" id="totp-code" class="mfa-in" inputmode="numeric" autocomplete="one-time-code" maxlength="7" placeholder="123 456" style="max-width:200px"></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="totpEnable">Bestätigen und aktivieren</button></div></div></div>`;
  if (m.type === 'recovery') return `<div class="ov"><div class="modal" style="width:min(440px,100%)"><div class="mh"><h3>Wiederherstellungscodes</h3></div>
    <div class="mb"><div class="callout warn">${ic('shield')}<span><b>Jetzt sichern.</b> Diese Codes werden nur einmal angezeigt. Jeder gilt einmal – für den Fall, dass du dein Handy nicht hast.</span></div>
      <div class="rcodes">${m.codes.map(c => `<span>${esc(c)}</span>`).join('')}</div>
      <div class="row"><button class="btn sm" data-a="rcCopy">Kopieren</button><button class="btn sm" data-a="rcDownload">Als Datei speichern</button></div></div>
    <div class="mf"><button class="btn primary" data-a="rcDone">Ich habe sie gesichert</button></div></div></div>`;
  if (m.type === 'totpcode') return `<div class="ov" data-a="ovClose"><div class="modal" style="width:min(420px,100%)"><div class="mh"><h3>${m.action === 'disable' ? 'Authenticator-App ausschalten' : 'Neue Wiederherstellungscodes'}</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
    <div class="mb"><p class="muted">${m.action === 'disable' ? 'Zur Bestätigung den aktuellen Code aus der App oder einen Wiederherstellungscode eingeben.' : 'Zur Bestätigung den aktuellen Code eingeben. Die bisherigen Wiederherstellungscodes verfallen.'}</p>
      <input type="text" id="totp-code" class="mfa-in" autocomplete="one-time-code" maxlength="12" placeholder="123 456"></div>
    <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn ${m.action === 'disable' ? 'danger' : 'primary'}" data-a="totpDo">${m.action === 'disable' ? 'Ausschalten' : 'Neu erzeugen'}</button></div></div></div>`;
  if (m.type === 'confirm') return `<div class="ov" data-a="ovClose"><div class="modal" style="width:min(420px,100%)"><div class="mh"><h3>${esc(m.title)}</h3></div><div class="mb"><p class="ink2">${esc(m.text)}</p></div><div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn solid-danger" data-a="confirmOk">${esc(m.ok || 'Löschen')}</button></div></div></div>`;
  if (m.type === 'rename') return `<div class="ov" data-a="ovClose"><div class="modal" style="width:min(420px,100%)"><div class="mh"><h3>Passkey umbenennen</h3></div><div class="mb"><input type="text" id="rn-in" value="${esc(m.name)}"></div><div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="renameOk">Speichern</button></div></div></div>`;
  return '';
}

/* ---------- Shell ---------- */
const TITLES = {
  dashboard: ['Übersicht', 'Dein Arbeitstag auf einen Blick'], day: ['Tagesansicht', 'Wann welche App – und wann der Rechner gesperrt war'], stats: ['Statistik', 'Arbeitszeit, Überstunden und App-Nutzung'],
  absences: ['Abwesenheiten', 'Urlaub, Krankheit, Zeitausgleich und Feiertage'], companion: ['Companion-App', 'Läuft auf Windows und macOS im Hintergrund'], settings: ['Einstellungen', 'Arbeitszeit, Pausen, Datenschutz und Sicherheit'], admin: ['Serververwaltung', 'Benutzer, Richtlinien, Sicherheit, Sicherung und Protokoll']
};
function shell(view) {
  const nav = [['dashboard', 'dash', 'Übersicht'], ['day', 'day', 'Tagesansicht'], ['stats', 'chart', 'Statistik'], ['absences', 'umb', 'Abwesenheiten'], ['companion', 'monitor', 'Companion-App'], ['settings', 'sliders', 'Einstellungen']];
  const ts = trackState(), [h1, sub] = TITLES[S.route] || ['', ''];
  return `<div class="shell"><aside class="side">
    <a class="brand" href="#/dashboard" title="Zur Startseite" style="color:inherit;text-decoration:none"><img class="logo" src="icon.svg" width="32" height="32" alt="" style="border-radius:7px">Tickly</a>
    <nav class="nav">${nav.map(([r, i, l]) => `<a href="#/${r}" class="${S.route === r ? 'on' : ''}" ${r === 'day' ? 'data-a="goToday"' : ''}>${ic(i, 19)}${l}</a>`).join('')}
      ${isAdmin() ? `<div class="nav-h">Administration</div><a href="#/admin" class="${S.route === 'admin' ? 'on' : ''}">${ic('users', 19)}Serververwaltung${S.users.filter(u => u.status === 'pending').length ? `<span class="badge warn right" style="padding:1px 7px">${S.users.filter(u => u.status === 'pending').length}</span>` : ''}</a>` : ''}</nav>
    <div class="sp"></div>
    <a class="trk" href="#/companion" style="display:block"><div class="t1"><span class="dot ${ts.cls}"></span>${ts.label}</div><div class="t2">${ts.key === 'stopped' && S.day.ended != null ? `Arbeitsende ${hm(S.day.ended)}` : ts.sub} · ${S.os === 'windows' ? 'Windows' : 'macOS'}</div></a>
    <div class="me">${avatar(S.me.name)}<div class="grow"><div class="nm">${esc(S.me.name)}</div><div class="rl">${S.me.role === 'admin' ? 'Administrator' : 'Benutzer'}</div></div><button class="ibtn" data-a="logout" title="Abmelden">${ic('logout', 18)}</button></div>
    ${S.serverVersion ? `<div class="ver" title="Auf diesem Server läuft Tickly in Version ${esc(S.serverVersion)}">Server ${esc(S.serverVersion)}</div>` : ''}
  </aside>
  <main class="main"><div class="top"><div class="grow"><h1>${h1}</h1><div class="sub">${sub}</div></div>
    <span class="clockchip">${ic('day', 15)}${DOW_S[dow(TODAY)]}, ${dMid(TODAY).slice(0, 6)} · <b class="mono" style="color:var(--ink)">${hm(S.now)}</b>${S.speed > 1 ? ` · ${S.speed}×` : ''}</span>
    <button class="ibtn bd" data-a="theme" title="Darstellung: ${{ system: 'System', light: 'Hell', dark: 'Dunkel' }[S.themePref]} (Klick wechselt)">${ic({ system: 'monitor', light: 'sun', dark: 'moon' }[S.themePref], 18)}</button></div>
  <div class="content" id="view">${mfaBanner()}${startBalBanner()}${secqBanner()}${updateBanner()}${downloadBanner()}${view}</div></main></div>`;
}
function demoPanel() {
  const acc = [['andre@example.de', 'André Becker', 'Administrator'], ['lena.hoffmann@example.de', 'Lena Hoffmann', 'Benutzer'], ['sophie.wagner@example.de', 'Sophie Wagner', 'Administratorin']];
  return `<button class="demo-btn" data-a="demoToggle">${ic('flask', 16)} Demo</button>${S.demoOpen ? `<div class="demo-pan">
    <div><h4>Zeitraffer</h4><div class="seg" style="margin-top:6px">${[[1, '1×'], [60, '60×'], [300, '300×']].map(([v, l]) => `<button class="${S.speed === v ? 'on' : ''}" data-a="speed" data-v="${v}">${l}</button>`).join('')}</div></div>
    <div><h4>System</h4><div class="seg" style="margin-top:6px"><button class="${S.os === 'windows' ? 'on' : ''}" data-a="os" data-v="windows">Windows</button><button class="${S.os === 'mac' ? 'on' : ''}" data-a="os" data-v="mac">macOS</button></div></div>
    ${S.demoHint ? `<div><h4>Anmelden als</h4><div class="stack gap-s" style="margin-top:6px">${acc.map(([m, n, r]) => `<button class="btn sm ${S.me.email === m ? 'primary' : ''}" data-a="demoLogin" data-email="${m}" style="justify-content:space-between"><span>${n}</span><span style="opacity:.7;font-weight:500">${r}</span></button>`).join('')}</div></div>` : ''}
    <div class="row wrap"><button class="btn sm" data-a="cAct" data-k="newday">Neuer Arbeitstag</button><button class="btn sm" data-a="cAct" data-k="shutdown">Neustart</button><button class="btn sm" data-a="cAct" data-k="boot">Rechner starten</button></div>
  </div>` : ''}`;
}
