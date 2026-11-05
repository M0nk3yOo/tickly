'use strict';
/* Tickly – Zeiten bearbeiten: Rechenkern ohne Oberfläche (läuft auch unter Node, siehe scripts/test-dayedit.mjs)
 *
 * Der Server speichert die Zeit eines Tages feinkörnig: jeder Programmwechsel beginnt einen neuen Abschnitt.
 * Zum Bearbeiten wird das zu „Einträgen“ zusammengefasst: aneinandergrenzende Abschnitte gleicher Art (Arbeit oder Pause)
 * sind ein Eintrag. Die Einträge werden nur dort verändert, wo der Benutzer es will:
 *   - Zeiten eines Eintrags ändern kürzt oder verlängert ihn; Programme und Notizen darin bleiben erhalten.
 *   - Neue oder verschobene Einträge ersetzen, was an ihrer Stelle schon steht (eine Pause mitten in einem Arbeitstag
 *     teilt ihn in zwei Einträge).
 *   - Gelöschtes hinterlässt eine Lücke, die sich füllen lässt.
 * Ein Abschnitt hat { from, to, kind: 'app' | 'locked', app, note, b }; `b` ist die Nummer des Eintrags.
 */
const DayEdit = (() => {
  const EPS = 0.01; // Minuten; kürzere Reste werden verworfen
  const MANUAL = 'Manuell';
  let seq = 0;
  const nb = () => ++seq;
  const same = (a, b) => Math.abs(a - b) < EPS;
  const byTime = (a, b) => a.from - b.from || a.to - b.to;

  /** Abschnitte vom Server (ohne den laufenden) → Arbeitsliste mit Eintragsnummern */
  function init(raw) {
    const segs = raw.filter(g => g.kind !== 'off' && g.to != null && g.to - g.from > EPS)
      .map(g => ({ from: g.from, to: g.to, kind: g.kind, app: g.kind === 'app' ? (g.app || MANUAL) : null, note: g.kind === 'app' ? (g.note || '') : '', b: 0 }))
      .sort(byTime);
    let cur = 0, prev = null;
    segs.forEach(s => {
      if (!prev || !same(prev.to, s.from) || prev.kind !== s.kind || (s.kind === 'app' && prev.note !== s.note)) cur = nb();
      s.b = cur; prev = s;
    });
    return segs;
  }

  /** Entfernt [from, to) überall außer im Eintrag `keep`; Reste links und rechts bleiben. */
  function carve(segs, from, to, keep) {
    const out = [];
    segs.forEach(s => {
      if (s.b === keep || s.to <= from + EPS || s.from >= to - EPS) return out.push(s);
      if (s.from < from - EPS) out.push({ ...s, to: from });
      if (s.to > to + EPS) out.push({ ...s, from: to });
    });
    return out;
  }

  /** Sortiert, trennt zerrissene Einträge in getrennte Nummern und fasst gleiche, aneinandergrenzende Abschnitte zusammen. */
  function normalize(list) {
    const segs = list.map(s => ({ ...s })).sort(byTime), cur = new Map();
    segs.forEach(s => {
      const c = cur.get(s.b);
      if (!c) cur.set(s.b, { b: s.b, to: s.to });
      else if (same(c.to, s.from)) { const o = s.b; s.b = c.b; c.to = s.to; cur.set(o, c); }
      else { const id = nb(), o = s.b; s.b = id; cur.set(o, { b: id, to: s.to }); }
    });
    const out = [];
    segs.forEach(s => {
      const l = out[out.length - 1];
      if (l && l.b === s.b && l.kind === s.kind && l.app === s.app && (l.note || '') === (s.note || '') && same(l.to, s.from)) l.to = s.to; else out.push(s);
    });
    return out;
  }

  /** Die Einträge, nach Beginn sortiert, mit den Programmen darin (längstes zuerst). */
  function blocks(segs) {
    const m = new Map();
    segs.forEach(s => {
      let r = m.get(s.b);
      if (!r) m.set(s.b, r = { b: s.b, from: s.from, to: s.to, kind: s.kind, note: s.note || '', apps: new Map() });
      r.from = Math.min(r.from, s.from); r.to = Math.max(r.to, s.to);
      if (s.kind === 'app') r.apps.set(s.app, (r.apps.get(s.app) || 0) + s.to - s.from);
    });
    return [...m.values()].map(r => ({ ...r, apps: [...r.apps].map(([app, min]) => ({ app, min })).sort((a, b) => b.min - a.min) })).sort(byTime);
  }

  /** Lücken zwischen den Einträgen (und bis zum laufenden Abschnitt `openFrom`), mindestens `min` Minuten lang. */
  function gaps(rows, openFrom = null, min = 1) {
    const out = []; let end = null;
    rows.forEach(r => {
      if (end != null && r.from - end >= min) out.push({ from: end, to: r.from });
      end = end == null ? r.to : Math.max(end, r.to);
    });
    if (end != null && openFrom != null && openFrom - end >= min) out.push({ from: end, to: openFrom });
    return out;
  }

  /** Neuer Eintrag; ersetzt, was an seiner Stelle steht. Liefert die neue Liste und die Nummer des Eintrags. */
  function add(segs, e) {
    const b = nb();
    const s = { from: e.from, to: e.to, kind: e.kind, app: e.kind === 'app' ? (String(e.app || '').trim() || MANUAL) : null, note: e.kind === 'app' ? (e.note || '') : '', b };
    return { segs: normalize(carve([...segs, s], e.from, e.to, b)), b };
  }

  /** Beginn und Ende eines Eintrags ändern: kürzt ihn oder verlängert ihn (der neue Teil gilt als „Manuell“); anderes weicht. */
  function setRange(segs, b, from, to) {
    const own = segs.filter(s => s.b === b);
    if (!own.length || !(to - from > EPS)) return segs;
    const p = own[0], lo = Math.min(...own.map(s => s.from)), hi = Math.max(...own.map(s => s.to));
    const mine = [];
    own.forEach(s => { const a = Math.max(s.from, from), z = Math.min(s.to, to); if (z - a > EPS) mine.push({ ...s, from: a, to: z }); });
    const fill = (a, z) => ({ from: a, to: z, kind: p.kind, app: p.kind === 'app' ? MANUAL : null, note: p.kind === 'app' ? (p.note || '') : '', b });
    if (from < lo - EPS) mine.push(fill(from, Math.min(lo, to)));
    if (to > hi + EPS) mine.push(fill(Math.max(hi, from), to));
    return normalize(carve([...segs.filter(s => s.b !== b), ...mine], from, to, b));
  }

  /** Art, Programm oder Notiz eines ganzen Eintrags ändern (`p`: kind, app, note – nur was gesetzt ist). */
  function setAttrs(segs, b, p) {
    return normalize(segs.map(s => {
      if (s.b !== b) return s;
      const kind = p.kind || s.kind;
      if (kind === 'locked') return { ...s, kind, app: null, note: '' };
      const app = p.app != null ? (String(p.app).trim() || MANUAL) : (s.kind === 'locked' ? MANUAL : s.app);
      return { ...s, kind, app, note: p.note != null ? p.note : (s.kind === 'locked' ? '' : s.note) };
    }));
  }

  const remove = (segs, b) => segs.filter(s => s.b !== b);

  /** Für die Speicherung: ohne Eintragsnummern, nach Beginn sortiert. */
  const out = segs => segs.slice().sort(byTime).map(({ from, to, kind, app, note }) => ({ from, to, kind, app: kind === 'app' ? app : null, note: kind === 'app' ? (note || '') : '' }));

  /** Summen in Minuten: Arbeit, Pause */
  function sums(segs) {
    let work = 0, pause = 0;
    segs.forEach(s => { if (s.kind === 'app') work += s.to - s.from; else pause += s.to - s.from; });
    return { work, pause };
  }

  /** Vorschlag für eine Pause von `len` Minuten: um 12 Uhr, sonst in der Mitte des längsten Arbeitseintrags. */
  function suggestPause(rows, len = 30) {
    const work = rows.filter(r => r.kind === 'app' && r.to - r.from >= len + 2);
    if (!work.length) return null;
    if (work.some(r => r.from <= 720 && r.to >= 720 + len)) return { from: 720, to: 720 + len };
    const r = work.slice().sort((a, b) => (b.to - b.from) - (a.to - a.from))[0];
    const mid = Math.round((r.from + r.to) / 2 / 5) * 5, from = Math.max(r.from, Math.min(mid - len / 2, r.to - len));
    return { from, to: from + len };
  }

  return { MANUAL, EPS, init, carve, normalize, blocks, gaps, add, setRange, setAttrs, remove, out, sums, suggestPause };
})();
if (typeof module !== 'undefined' && module.exports) module.exports = DayEdit;
