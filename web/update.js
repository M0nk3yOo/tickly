'use strict';
/* Tickly – Web-Oberfläche · Update-Hinweise und Companion-Download.
   Daten kommen mit dem Snapshot: S.update (nur Administratoren: Server-Version) und S.download (Installer des Companion). */

const RELEASES_URL = 'https://github.com/M0nk3yOo/tickly/releases/latest';
const UPD_KEY = 'tickly.updDismiss', DL_KEY = 'tickly.dlHint', DL_DAYS = 30;
const lsGet = k => { try { return localStorage.getItem(k); } catch (e) { return null; } };
const lsSet = (k, v) => { try { if (v == null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch (e) { /* Speicher gesperrt: der Hinweis kommt dann eben wieder */ } };
/** Adressen aus dem Server nur als http(s)-Link ausgeben */
const safeUrl = u => /^https?:\/\//i.test(u || '') ? esc(u) : esc(RELEASES_URL);
const browserOs = () => /Mac/.test(navigator.platform) ? 'mac' : /Win/.test(navigator.platform) ? 'win' : '';

/** Installer je System: Adresse, Beschriftung, Größe (ohne Angaben vom Server der Release-Seite) */
function dlInfo() {
  const d = S.download || {}, mk = (a, label, ext) => ({ url: a ? a.url : (d.url || RELEASES_URL), label, ext, size: a && a.size, exact: !!a });
  return { version: d.version || '', mac: mk(d.mac, 'macOS', '.dmg'), win: mk(d.win, 'Windows', d.win && /\.msi$/i.test(d.win.name) ? '.msi' : '.exe') };
}
const dlBtn = (o, key, primary) => `<a class="btn ${primary ? 'primary' : ''}" href="${safeUrl(o.url)}" target="_blank" rel="noopener">${ic('download', 15)} Für ${o.label} laden</a>`;

/* ---------- Hinweis für Administratoren: neue Serverversion ---------- */
function updateBanner() {
  const u = S.update; if (S.demo || !isAdmin() || !u || !u.enabled || !u.newer || lsGet(UPD_KEY) === u.latest) return '';
  return `<div class="mfa-banner" style="flex-wrap:wrap">${ic('download', 20)}<div class="grow"><b>Neue Version ${esc(u.latest)} verfügbar.</b> Auf diesem Server läuft ${esc(u.installed)}. Mit Docker: <span class="mono">docker compose pull &amp;&amp; docker compose up -d</span> (Anleitung unter Serververwaltung → System).</div>
    ${u.url ? `<a class="btn sm" href="${safeUrl(u.url)}" target="_blank" rel="noopener">Versionshinweise</a>` : ''}<a class="btn sm primary" href="#/admin" data-a="updGo">So aktualisierst du</a><button class="ibtn" data-a="updDismiss" title="Für Version ${esc(u.latest)} ausblenden" aria-label="Ausblenden">${ic('x', 16)}</button></div>`;
}

/* ---------- Hinweis für Benutzer ohne Companion-Gerät ---------- */
const dlDismissed = () => { const t = +lsGet(DL_KEY); return !!t && Date.now() - t < DL_DAYS * 864e5; };
function downloadBanner() {
  if (S.demo || !S.ready || S.devices.length || (S.route !== 'dashboard' && S.route !== 'companion') || dlDismissed()) return '';
  const d = dlInfo(), os = browserOs();
  return `<div class="mfa-banner" style="flex-wrap:wrap;border-color:var(--line);background:var(--surface-2)">${ic('monitor', 20)}<div class="grow"><b>Zeiterfassung automatisch:</b> Lade den Companion für macOS/Windows herunter und koppele dein Gerät.${d.version ? ` <span class="muted">Aktuelle Version ${esc(d.version)}.</span>` : ''}</div>
    ${dlBtn(d.mac, 'mac', os === 'mac')}${dlBtn(d.win, 'win', os === 'win')}${S.route === 'companion' ? '' : '<a class="btn sm" href="#/companion" data-a="dlPair">Gerät koppeln</a>'}<button class="ibtn" data-a="dlDismiss" title="Hinweis ausblenden" aria-label="Ausblenden">${ic('x', 16)}</button></div>`;
}

/** Download-Links auf der Companion-Seite (neben der Kopplung) */
function downloadCard() {
  const d = dlInfo(), os = browserOs(), line = o => `${o.label} (${o.ext})${d.version && o.exact ? ' · ' + esc(d.version) : ''}${o.size ? ' · ' + fmtSize(o.size) : ''}`;
  return `<div class="stack gap-s" style="margin:4px 0 14px"><b style="font-size:13px;color:var(--ink)">Companion herunterladen</b>
    <div class="row wrap" style="gap:8px"><a class="btn ${os === 'mac' ? 'primary' : ''}" href="${safeUrl(d.mac.url)}" target="_blank" rel="noopener">${ic('download', 15)} ${line(d.mac)}</a><a class="btn ${os === 'win' ? 'primary' : ''}" href="${safeUrl(d.win.url)}" target="_blank" rel="noopener">${ic('download', 15)} ${line(d.win)}</a></div>
    <span class="hint">Die Installer sind nicht signiert: Windows und macOS fragen beim ersten Start nach (siehe Anleitung). Alle Versionen: <a href="${safeUrl(RELEASES_URL)}" target="_blank" rel="noopener">GitHub</a>.${dlDismissed() ? ' <a href="#/companion" data-a="dlReset">Hinweis auf der Übersicht wieder zeigen</a>' : ''}</span></div>`;
}

/* ---------- Serververwaltung: Reiter „System“ ---------- */
function vUpdate() {
  const u = S.update || {}, stamp = u.checkedAt ? `${fmtStamp(u.checkedAt)} (${fmtAgo(u.checkedAt)})` : 'noch nie';
  let state;
  if (!u.enabled) state = '<span class="badge">Prüfung aus</span>';
  else if (!u.latest) state = '<span class="muted">unbekannt</span>';
  else state = u.newer ? '<span class="badge warn">Update verfügbar</span>' : '<span class="badge good">aktuell</span>';
  const off = u.demo ? 'Im Demo-Modus wird nie nach Updates gesucht.' : u.envOff ? 'Per Umgebungsvariable <span class="mono">TICKLY_UPDATE_CHECK=0</span> abgeschaltet (hat Vorrang, ändern in der .env).' : 'Fragt höchstens alle 6 Stunden bei github.com nach der neuesten Version. Es werden keine Benutzerdaten gesendet, nur die Anfrage selbst.';
  return `<div class="g-1-1"><div class="card"><div class="ch"><h3>Server-Version</h3></div>
      <div class="kv"><span>Installiert</span><b class="mono">${esc(u.installed || S.serverVersion || '–')}</b></div>
      <div class="kv"><span>Neueste Version</span><b class="mono">${u.latest ? esc(u.latest) : '–'} ${state}</b></div>
      <div class="kv"><span>Zuletzt geprüft</span><b>${stamp}</b></div>
      ${u.error && u.enabled ? `<div class="callout warn" style="margin-top:10px">${ic('info')}<span>Die letzte Abfrage ist fehlgeschlagen (${esc(u.error)}). Tickly versucht es später erneut.</span></div>` : ''}
      <div class="row" style="margin-top:12px"><button class="btn" data-a="updCheck" ${u.enabled ? '' : 'disabled'}>Jetzt prüfen</button>${u.url ? `<a class="btn" href="${safeUrl(u.url)}" target="_blank" rel="noopener">Versionshinweise</a>` : ''}</div>
      ${setRow('Nach Updates suchen', off, `<button class="switch ${u.setting ? 'on' : ''}" data-a="updToggle" role="switch" aria-checked="${!!u.setting}" ${u.demo || u.envOff ? 'disabled' : ''}></button>`)}
      ${u.notes && u.newer ? `<details style="margin-top:8px"><summary class="sm" style="cursor:pointer">Was ist neu in ${esc(u.latest)}?</summary><pre style="white-space:pre-wrap;font:inherit;font-size:12.5px;margin:8px 0 0;color:var(--ink-2)">${esc(u.notes)}</pre></details>` : ''}</div>
    <div class="card"><div class="ch"><h3>So aktualisierst du</h3></div>
      <ol class="steps"><li>Sicherung anlegen (Reiter „Sicherung“).</li><li>Im Ordner mit der <span class="mono">docker-compose.yml</span>: <span class="mono">docker compose pull &amp;&amp; docker compose up -d</span> (veröffentlichtes Image). Baust du aus dem Quellcode: <span class="mono">git pull &amp;&amp; docker compose up -d --build</span>.</li><li>Fertig. Datenbankänderungen spielt der Server beim Start selbst ein.</li></ol>
      <p class="hint">Details stehen in <span class="mono">docs/INSTALLATION.md</span>. Die Companion-Apps bekommen den Hinweis auf neue Versionen vom Server und laden Installer von GitHub herunter; der Download startet erst nach einem Klick.</p></div></div>`;
}

/* ---------- Aktionen (werden in main.js in H eingehängt) ---------- */
const UPD_ACTIONS = {
  updDismiss() { lsSet(UPD_KEY, S.update.latest); render(); },
  updGo() { S.adminTab = 'system'; go('admin'); loadAdminTab().then(render); },
  dlDismiss() { lsSet(DL_KEY, String(Date.now())); render(); },
  dlReset() { lsSet(DL_KEY, null); toast('Der Hinweis erscheint wieder auf der Übersicht'); render(); },
  dlPair() { go('companion'); },
  async updCheck() {
    S.update = await api('POST', '/admin/update/check', {});
    toast(S.update.error ? 'Abfrage fehlgeschlagen' : S.update.newer ? `Version ${S.update.latest} ist verfügbar` : 'Tickly ist auf dem neuesten Stand'); render();
  },
  async updToggle() { S.update = await api('PUT', '/admin/update', { enabled: !S.update.setting }); toast(S.update.setting ? 'Suche nach Updates eingeschaltet' : 'Suche nach Updates ausgeschaltet'); render(); }
};
