// Prüft den Rechenkern von „Zeiten bearbeiten“ (web/dayedit.js). Aufruf: node scripts/test-dayedit.mjs
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
const DE = createRequire(import.meta.url)('../web/dayedit.js');
const h = (a, b = 0) => a * 60 + b;
const app = (from, to, a, note) => ({ from, to, kind: 'app', app: a, ...(note ? { note } : {}) });
const lock = (from, to) => ({ from, to, kind: 'locked' });
const rows = segs => DE.blocks(segs).map(r => `${r.kind === 'app' ? 'A' : 'P'} ${r.from}-${r.to}`);
const total = segs => DE.sums(segs);
let n = 0;
const test = (name, f) => { f(); n++; console.log('ok  ' + name); };

// Ein Arbeitstag mit vielen Programmwechseln ist ein einziger Eintrag
const day = () => DE.init([app(h(7), h(9), 'code'), app(h(9), h(10, 30), 'chrome'), app(h(10, 30), h(12), 'code'), app(h(12), h(16), 'teams'), { from: h(16), to: h(17), kind: 'off' }]);

test('Programmwechsel bilden keinen eigenen Eintrag', () => {
  assert.deepEqual(rows(day()), ['A 420-960']);
  const [r] = DE.blocks(day());
  assert.deepEqual(r.apps.map(a => a.app), ['teams', 'code', 'chrome']);
});

test('Sperre und Lücken trennen Einträge', () => {
  const s = DE.init([app(h(7), h(10), 'code'), lock(h(10), h(10, 20)), app(h(10, 20), h(12), 'code'), app(h(13), h(15), 'code')]);
  assert.deepEqual(rows(s), ['A 420-600', 'P 600-620', 'A 620-720', 'A 780-900']);
  assert.deepEqual(DE.gaps(DE.blocks(s)), [{ from: h(12), to: h(13) }]);
});

test('eine Pause mitten im Tag teilt den Eintrag', () => {
  const { segs } = DE.add(day(), { from: h(12), to: h(12, 30), kind: 'locked' });
  assert.deepEqual(rows(segs), ['A 420-720', 'P 720-750', 'A 750-960']);
  assert.deepEqual(total(segs), { work: 540 - 30, pause: 30 });
  assert.deepEqual(DE.gaps(DE.blocks(segs)), []);
  // die Programme bleiben dort erhalten, wo sie waren
  const [a, , b] = DE.blocks(segs);
  assert.deepEqual(a.apps.map(x => x.app).sort(), ['chrome', 'code']);
  assert.deepEqual(b.apps.map(x => [x.app, x.min]), [['teams', 210]]);
});

test('ein Programmabschnitt wird mitten durchtrennt', () => {
  const { segs } = DE.add(day(), { from: h(8), to: h(8, 30), kind: 'app', app: 'Manuell', note: 'Termin' });
  assert.deepEqual(rows(segs), ['A 420-480', 'A 480-510', 'A 510-960']);
  assert.deepEqual(DE.out(segs).slice(0, 3).map(s => [s.from, s.to, s.app]), [[420, 480, 'code'], [480, 510, 'Manuell'], [510, 540, 'code']]);
  assert.equal(DE.blocks(segs)[1].note, 'Termin');
});

test('ein Eintrag über mehrere andere ersetzt sie ganz', () => {
  const s = DE.init([app(h(8), h(9), 'a'), lock(h(9), h(9, 15)), app(h(9, 15), h(10), 'b')]);
  const r = DE.add(s, { from: h(8, 30), to: h(9, 45), kind: 'app', app: 'Manuell' });
  assert.deepEqual(rows(r.segs), ['A 480-510', 'A 510-585', 'A 585-600']);
  assert.equal(total(r.segs).work, 120); // 8:00–10:00 lückenlos
});

test('Löschen hinterlässt eine Lücke, Füllen schließt sie', () => {
  let { segs } = DE.add(day(), { from: h(12), to: h(12, 30), kind: 'locked' });
  const pause = DE.blocks(segs).find(r => r.kind === 'locked');
  segs = DE.remove(segs, pause.b);
  const g = DE.gaps(DE.blocks(segs));
  assert.deepEqual(g, [{ from: h(12), to: h(12, 30) }]);
  const f = DE.add(segs, { ...g[0], kind: 'app', app: 'Manuell' });
  assert.deepEqual(rows(f.segs), ['A 420-720', 'A 720-750', 'A 750-960']);
  assert.deepEqual(DE.gaps(DE.blocks(f.segs)), []);
});

test('Lücke bis zum laufenden Abschnitt', () => {
  const s = DE.init([app(h(8), h(10), 'a')]);
  assert.deepEqual(DE.gaps(DE.blocks(s), h(11)), [{ from: h(10), to: h(11) }]);
  assert.deepEqual(DE.gaps(DE.blocks(s), h(10, 0.5)), []);
});

test('Ende verlängern ersetzt den Nachbarn teilweise, der neue Teil heißt „Manuell“', () => {
  const s = DE.init([app(h(8), h(10), 'a'), lock(h(10), h(10, 30)), app(h(10, 30), h(12), 'b')]);
  const first = DE.blocks(s)[0];
  const r = DE.setRange(s, first.b, h(8), h(10, 15));
  assert.deepEqual(rows(r), ['A 480-615', 'P 615-630', 'A 630-720']);
  assert.deepEqual(DE.blocks(r)[0].apps.map(a => a.app), ['a', 'Manuell']);
  assert.deepEqual(DE.blocks(r)[0].apps.map(a => a.min), [120, 15]);
});

test('Eintrag kürzen belässt die Programme darin', () => {
  const s = day();
  const b = DE.blocks(s)[0].b;
  const r = DE.setRange(s, b, h(8), h(15));
  assert.deepEqual(rows(r), ['A 480-900']);
  assert.deepEqual(DE.blocks(r)[0].apps.map(a => a.app).sort(), ['chrome', 'code', 'teams']);
  assert.deepEqual(DE.gaps(DE.blocks(r), h(17)), [{ from: h(15), to: h(17) }]);
});

test('ungültige Zeiten ändern nichts', () => {
  const s = day();
  assert.equal(DE.setRange(s, DE.blocks(s)[0].b, h(12), h(12)), s);
});

test('Art und Programm eines Eintrags ändern', () => {
  const s = day();
  const b = DE.blocks(s)[0].b;
  const a = DE.setAttrs(s, b, { app: 'Manuell', note: 'Kunde' });
  assert.deepEqual(DE.out(a), [{ from: 420, to: 960, kind: 'app', app: 'Manuell', note: 'Kunde' }]);
  const p = DE.setAttrs(s, b, { kind: 'locked' });
  assert.deepEqual(DE.out(p), [{ from: 420, to: 960, kind: 'locked', app: null, note: '' }]);
  const back = DE.setAttrs(p, b, { kind: 'app' });
  assert.deepEqual(DE.out(back), [{ from: 420, to: 960, kind: 'app', app: 'Manuell', note: '' }]);
});

test('Ergebnis ist sortiert und überschneidungsfrei', () => {
  let segs = day();
  for (const e of [[h(9), h(9, 10), 'locked'], [h(8, 55), h(9, 5), 'app'], [h(15), h(18), 'app'], [h(6), h(7, 30), 'locked'], [h(11), h(11, 5), 'locked']]) segs = DE.add(segs, { from: e[0], to: e[1], kind: e[2] }).segs;
  const o = DE.out(segs);
  for (let i = 1; i < o.length; i++) assert.ok(o[i].from >= o[i - 1].to - 1e-9, JSON.stringify(o));
  o.forEach(s => assert.ok(s.to - s.from > 0));
});

test('zwei Teile desselben Eintrags werden zu zwei Einträgen', () => {
  const { segs } = DE.add(day(), { from: h(10), to: h(11), kind: 'locked' });
  const r = DE.blocks(segs);
  assert.equal(new Set(r.map(x => x.b)).size, 3);
});

test('Speichern erzeugt dieselben Abschnitte wie vorher, wenn nichts geändert wurde', () => {
  const raw = [app(h(7), h(9), 'code'), app(h(9), h(10), 'chrome'), lock(h(10), h(10, 20)), app(h(10, 20), h(12), 'code')];
  const o = DE.out(DE.init(raw));
  assert.deepEqual(o.map(s => [s.from, s.to, s.kind, s.app]), raw.map(s => [s.from, s.to, s.kind, s.app ?? null]));
});

test('Vorschlag für eine Pause', () => {
  assert.deepEqual(DE.suggestPause(DE.blocks(day())), { from: 720, to: 750 });
  const s = DE.init([app(h(7), h(9), 'a')]);
  assert.deepEqual(DE.suggestPause(DE.blocks(s)), { from: 465, to: 495 });
  assert.equal(DE.suggestPause([]), null);
});

console.log(`\n${n} Tests bestanden`);
