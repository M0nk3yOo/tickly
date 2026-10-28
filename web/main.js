'use strict';
/* Tickly – Web-Oberfläche · Router, Rendering, Events (alle Daten kommen von der Rust-API) */

const $ = s => document.querySelector(s);
const ROUTES = ['setup', 'login', 'register', 'forgot', 'reset', 'verify', 'dashboard', 'day', 'stats', 'absences', 'companion', 'settings', 'admin'];
const VIEWS = { dashboard: vDashboard, day: vDay, stats: vStats, absences: vAbsences, companion: vCompanion, settings: vSettings, admin: vAdmin };
const DEMO_PW = 'demo-passwort';

const getPath = p => p.split('.').reduce((o, k) => o[k], S);
function setPath(p, v) { const a = p.split('.'), l = a.pop(); a.reduce((o, k) => o[k], S)[l] = v; }

function render() {
  const y = window.scrollY; LINES = [];
  hideTip();
  $('#root').innerHTML = S.mfaGate ? vMfaGate() : S.route === 'forgot' ? vForgot() : S.route === 'reset' ? vReset() : S.route === 'verify' ? vVerify() : S.pwGate ? vPwGate() : S.route === 'setup' ? vSetup() : S.route === 'login' ? vLogin() : S.route === 'register' ? vRegister() : S.ready ? shell((VIEWS[S.route] || vDashboard)()) : '';
  renderModalOnly();
  $('#demo').innerHTML = S.loggedIn && S.ready && S.demo ? demoPanel() : '';
  window.scrollTo(0, y);
}
const logUrl = before => `/admin/log?limit=100&kind=${S.logKind}${S.logQ ? '&q=' + encodeURIComponent(S.logQ) : ''}${before ? '&before=' + before : ''}`;
async function loadLog() { S.log = await api('GET', logUrl()); }
/* Lädt, was der gewählte Administrationsreiter braucht */
async function loadAdminTab() {
  if (!isAdmin()) return;
  if (S.adminTab === 'backup') { S.backups = await api('GET', '/admin/backups'); S.bkPol = await api('GET', '/admin/backup-policy'); }
  if (S.adminTab === 'sec' || S.adminTab === 'users') S.sec = await api('GET', '/admin/security');
  if (S.adminTab === 'log') await loadLog();
  if (S.adminTab === 'mail') { S.smtp = await api('GET', '/admin/smtp'); }
  if (S.adminTab === 'sess') S.sessAdmin = await api('GET', '/admin/sessions');
}
function renderModalOnly() { $('#modal').innerHTML = renderModal(); }
async function route() {
  if (S.pwGate || S.mfaGate) { S.route = S.pwGate ? 'pwgate' : 'mfagate'; render(); return; }
  let r = (location.hash.replace(/^#\/?/, '') || 'dashboard').split('?')[0];
  if (!ROUTES.includes(r)) r = 'dashboard';
  if (S.loggedIn && !S.ready) { try { await reload(); } catch (e) { S.loggedIn = false; } }
  const open = ['register', 'forgot', 'reset', 'verify'];
  // Noch kein Benutzer: nur die Ersteinrichtung ist erreichbar
  if (S.setup && !S.loggedIn) r = 'setup';
  else if (r === 'setup') r = S.loggedIn ? 'dashboard' : 'login';
  else if (!S.loggedIn && !open.includes(r)) r = 'login';
  else if (S.loggedIn && (r === 'login' || r === 'forgot' || r === 'reset')) r = 'dashboard';
  // Link aus der Bestätigungsmail: Adresse bestätigen (einmalig) und – bei aktivem Konto – gleich anmelden
  if (r === 'verify' && !S.loggedIn) {
    const token = new URLSearchParams(location.hash.split('?')[1] || '').get('token');
    if (token && S.verifyToken !== token) {
      S.verifyToken = token; S.verify = { state: 'work' };
      api('POST', '/auth/verify', { token }).then(async r2 => { if (r2.status === 'active') { history.replaceState(null, '', '#/dashboard'); await enter(); } else { S.verify = { state: 'pending' }; render(); } }).catch(e => { S.verify = { state: 'error', msg: e.message }; render(); });
    } else if (!token) S.verify = { state: 'error', msg: 'Der Link ist unvollständig.' };
  }
  if (r === 'admin' && isAdmin()) { try { await loadAdminTab(); } catch (e) { /* Liste bleibt leer */ } }
  if (r === 'settings' && S.setTab === 'sec') { try { S.mySessions = await api('GET', '/sessions'); } catch (e) { /* egal */ } }
  S.route = r; if (!window.__keepModal) S.modal = null; window.__keepModal = false; render(); window.scrollTo(0, 0);
}
function go(r) { if (location.hash === '#/' + r) route(); else location.hash = '#/' + r; }

function toast(msg, isErr) {
  const t = document.createElement('div'); t.className = 'toast'; t.innerHTML = ic(isErr ? 'info' : 'check', 16); t.append(msg);
  if (isErr) t.style.background = 'var(--bad)';
  $('#toasts').append(t); setTimeout(() => { t.style.transition = '.3s'; t.style.opacity = 0; setTimeout(() => t.remove(), 300); }, isErr ? 4200 : 2600);
}
function confirmBox(title, text, ok, fn) { S.modal = { type: 'confirm', title, text, ok, fn }; renderModalOnly(); }
const closeModal = () => { S.modal = null; renderModalOnly(); };
/* Darstellung: „system“ folgt dem Betriebssystem (auch während der Laufzeit), sonst fest hell oder dunkel */
const sysDark = matchMedia('(prefers-color-scheme: dark)');
function applyTheme() {
  S.theme = S.themePref === 'system' ? (sysDark.matches ? 'dark' : 'light') : S.themePref;
  document.documentElement.dataset.theme = S.theme;
  localStorage.setItem('tickly.theme', S.themePref);
}
sysDark.addEventListener('change', () => { if (S.themePref === 'system') { applyTheme(); if (S.ready) render(); } });
async function setThemePref(v) {
  S.themePref = v; S.themeLocked = false; applyTheme();
  if (S.loggedIn && S.ready && S.set.theme !== v) { S.set.theme = v; await saveSet(); } else render();
}

/* Nach einer Zustandsänderung: Server-Stand holen und neu zeichnen. */
async function sync() { await reload(); render(); }
/* Überstunden-Stand aus dem Registrierungsformular (Stunden); „neu beginnen“ ergibt 0 */
function regStartBal() {
  const r = document.querySelector('input[name="rg-bal"]:checked');
  if (!r || r.value !== 'carry') return 0;
  const v = parseFloat(String($('#rg-carry').value).replace(',', '.').replace('−', '-'));
  if (isNaN(v)) throw new Error('Bitte den Überstunden-Stand als Zahl eingeben (z. B. 12,5)');
  return v;
}
async function enter() {
  S.loggedIn = true; S.limited = false;
  // Ist die Karenzzeit für Passkey/Authenticator-App abgelaufen, gibt es nur noch die Einrichtung
  const ses = await api('GET', '/session');
  if (ses.limited && ses.reason === 'password') { S.pwGate = ses; S.ready = false; return go('dashboard'); }
  if (ses.limited && ses.reason === 'mfa') { S.mfaGate = ses; S.ready = false; return go('dashboard'); }
  await reload();
  toast('Willkommen, ' + S.me.name.split(' ')[0]);
  // Erinnerung bei jeder Anmeldung, solange nichts eingerichtet ist
  // Überstunden-Stand noch nicht festgelegt: einmalig abfragen (hat Vorrang), sonst die Erinnerung an Passkey/Authenticator
  if (S.startBalSet === false && !S.demo) { S.modal = { type: 'startbal' }; window.__keepModal = true; }
  else if (S.mfa && S.mfa.required && !S.mfa.compliant) { S.modal = { type: 'mfaRemind' }; window.__keepModal = true; }
  go('dashboard');
}
/* Nach der Einrichtung im Sperr-Modus: Sitzung neu prüfen und in die App wechseln */
async function recheck() {
  const ses = await api('GET', '/session');
  if (ses.limited) return render();
  S.mfaGate = null; S.ready = false; await enter();
}
function expire() { S.loggedIn = false; S.ready = false; toast('Sitzung abgelaufen – bitte erneut anmelden', true); go('login'); }

/* ---------- Tooltip ---------- */
function showTip(text, x, y) {
  const t = $('#tip'), [a, b] = text.split('|');
  t.replaceChildren(); const s = document.createElement('b'); s.textContent = a; t.append(s);
  if (b) { const p = document.createElement('span'); p.textContent = b; t.append(p); }
  t.style.display = 'block';
  const w = t.offsetWidth, h = t.offsetHeight;
  t.style.left = Math.max(8, Math.min(x + 14, innerWidth - w - 8)) + 'px';
  t.style.top = (y + 18 + h > innerHeight ? y - h - 12 : y + 18) + 'px';
}
function hideTip() {
  const t = $('#tip'); if (t) t.style.display = 'none';
  document.querySelectorAll('.lc .cx,.lc .cd').forEach(e => e.style.display = 'none');
}
document.addEventListener('pointermove', e => {
  const tg = e.target.closest ? e.target : null; if (!tg) return;
  const lc = tg.closest('.lc');
  if (lc) {
    const r = lc.getBoundingClientRect(), pts = LINES[+lc.dataset.line].pts, n = pts.length;
    const f = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)), p = pts[Math.round(f * (n - 1))];
    const cx = lc.querySelector('.cx'), cd = lc.querySelector('.cd');
    cx.style.display = cd.style.display = 'block'; cx.style.left = p.x + '%'; cd.style.left = p.x + '%'; cd.style.top = p.y + '%';
    showTip(p.t + '|' + p.s, e.clientX, e.clientY); return;
  }
  const t = tg.closest('[data-tip]');
  if (t) showTip(t.dataset.tip, e.clientX, e.clientY); else hideTip();
});
document.addEventListener('pointerleave', hideTip, true);

/* ---------- Export ---------- */
function exportCsv() {
  const rows = [['Datum', 'Beginn', 'Ende', 'Brutto', 'Pause', 'Gezählt', 'Soll', 'Saldo']];
  let d = TRACK_START; while (d <= TODAY) { const s = dayStat(d); if (s.has || s.soll) rows.push([d, s.has ? hm(s.start) : '', s.has ? hm(s.end) : '', durN(s.gross), durN(s.brk), durN(s.counted), durN(s.soll), s.info.kind && !s.has && s.info.kind !== 'comp' ? 'nicht gewertet' : durN(s.diff)]); d = addDays(d, 1); }
  const url = URL.createObjectURL(new Blob(['﻿' + rows.map(r => r.join(';')).join('\n')], { type: 'text/csv' }));
  const a = document.createElement('a'); a.href = url; a.download = 'tickly-export.csv'; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
  toast('CSV-Export erstellt');
}

/* ---------- Passkey-Dialog ---------- */
async function withPasskeyDialog(mode, fn) {
  const m = { type: 'passkey', mode, step: 'wait' }; S.modal = m; renderModalOnly();
  try { await fn(); if (S.modal === m) closeModal(); return true; }
  catch (e) { if (S.modal === m) closeModal(); toast(friendlyPasskeyError(e), true); return false; }
}
async function saveSet() {
  try { await api('PUT', '/settings', S.set); await reload(); }
  catch (e) { toast(e.message, true); await reload().catch(() => { }); }
  render();
}
async function saveReg() {
  try { await api('PUT', '/admin/registration', S.reg); await reload(); }
  catch (e) { toast(e.message, true); await reload().catch(() => { }); }
  render();
}

/* ---------- Aktionen ---------- */
const H = {
  async login() {
    const mail = $('#lg-mail').value.trim(), pw = $('#lg-pw').value;
    if (!mail || !pw) return toast('Bitte E-Mail und Passwort eingeben', true);
    S.needsVerify = false;
    let r; try { r = await api('POST', '/auth/login', { email: mail, password: pw }); } catch (e) { if (e.status === 403 && /bestätige/.test(e.message)) { S.needsVerify = true; render(); } throw e; }
    if (r.needsTotp) { S.mfa = { ticket: r.ticket }; render(); setTimeout(() => $('#mfa-code') && $('#mfa-code').focus(), 50); return; }
    await enter();
  },
  async totpLogin() {
    const code = $('#mfa-code').value.trim(); if (!code) return toast('Bitte den Code eingeben', true);
    try { await api('POST', '/auth/totp', { ticket: S.mfa.ticket, code }); S.mfa = null; await enter(); }
    catch (e) { if (e.status === 401 && /abgelaufen|Zu viele Versuche/.test(e.message)) { S.mfa = null; render(); } throw e; }
  },
  mfaBack(el, e) { e.preventDefault(); S.mfa = null; render(); },
  /* Authenticator-App (TOTP) */
  async totpSetup() { const r = await api('POST', '/totp/setup', {}); S.modal = { type: 'totp', qr: r.qr, secret: r.secret }; renderModalOnly(); },
  async totpEnable() {
    const code = $('#totp-code').value.trim(); if (!code) return toast('Bitte den Code aus der App eingeben', true);
    const r = await api('POST', '/totp/enable', { code }); S.modal = { type: 'recovery', codes: r.recoveryCodes }; renderModalOnly(); if (!S.mfaGate) await reload();
  },
  totpAsk(el) { S.modal = { type: 'totpcode', action: el.dataset.v }; renderModalOnly(); setTimeout(() => $('#totp-code') && $('#totp-code').focus(), 50); },
  async totpDo() {
    const code = $('#totp-code').value.trim(), act = S.modal.action; if (!code) return toast('Bitte den Code eingeben', true);
    if (act === 'disable') { await api('POST', '/totp/disable', { code }); closeModal(); toast('Authenticator-App ausgeschaltet'); await sync(); }
    else { const r = await api('POST', '/totp/recovery', { code }); S.modal = { type: 'recovery', codes: r.recoveryCodes }; renderModalOnly(); await reload(); render(); }
  },
  rcCopy() { navigator.clipboard.writeText(S.modal.codes.join('\n')).then(() => toast('Codes kopiert'), () => toast('Kopieren nicht möglich – bitte abschreiben', true)); },
  rcDownload() {
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([`Tickly – Wiederherstellungscodes für ${S.me.email}\nJeder Code gilt einmal.\n\n${S.modal.codes.join('\n')}\n`], { type: 'text/plain' })); a.download = 'tickly-wiederherstellungscodes.txt'; a.click(); URL.revokeObjectURL(a.href);
  },
  async rcDone() { closeModal(); if (S.mfaGate) return recheck(); await reload(); render(); },
  totpReset(el) { const u = S.users.find(x => x.id === el.dataset.id); confirmBox('TOTP zurücksetzen?', `${u.name} kann sich danach wieder nur mit dem Passwort anmelden und die Authenticator-App neu einrichten.`, 'Zurücksetzen', async () => { await api('POST', `/admin/users/${u.id}/reset-totp`, {}); toast('TOTP zurückgesetzt'); await reload(); S.modal = null; render(); }); },
  async passkeyLogin() {
    const mail = $('#lg-mail').value.trim(); if (!mail) return toast('Bitte zuerst deine E-Mail-Adresse eingeben', true);
    const ok = await withPasskeyDialog('login', () => webauthnGet(mail)); if (ok) await enter();
  },
  async passkeyAdd() { const ok = await withPasskeyDialog('add', () => webauthnCreate()); if (ok) { toast('Passkey hinzugefügt'); if (S.mfaGate) return recheck(); await sync(); } },
  mfaGo() { S.modal = null; S.setTab = 'sec'; go('settings'); },
  /* Registrierung direkt per Passkey (ohne Passwort) */
  async passkeyRegNew() {
    const name = $('#rg-name').value.trim(), mail = $('#rg-mail').value.trim();
    if (!name || !mail) return toast('Bitte zuerst Name und E-Mail eingeben', true);
    const r = await api('POST', '/auth/register', { name, email: mail, startBal: regStartBal() });
    const ok = await withPasskeyDialog('reg', () => webauthnCreate());
    S.limited = r.status !== 'active';
    if (r.status === 'active') { S.regStep = 'done'; if (ok) return enter(); render(); } else { S.regStep = 'pending'; S.regPasskey = ok; render(); }
  },
  /* Passkey nach der Registrierung mit Passwort einrichten */
  async passkeyReg() { const ok = await withPasskeyDialog('reg', () => webauthnCreate()); if (ok) { toast('Passkey eingerichtet'); await enter(); } },
  pkCancel() { closeModal(); },
  forgot(el, e) { e.preventDefault(); S.forgotSent = false; go('forgot'); },
  async forgotSend() { const mail = $('#fg-mail').value.trim(); if (!mail) return toast('Bitte E-Mail-Adresse eingeben', true); const r = await api('POST', '/auth/forgot', { email: mail }); S.forgotSent = true; S.forgotMail = r.mail; render(); },
  async resetSave() {
    const token = new URLSearchParams(location.hash.split('?')[1] || '').get('token'), pw = $('#rs-pw').value;
    const miss = pwChecks(pw).filter(c => !c[1]).map(c => c[0]); if (miss.length) return toast('Dem Passwort fehlt: ' + miss.join(', '), true);
    await api('POST', '/auth/reset', { token, password: pw }); toast('Passwort geändert – bitte neu anmelden'); go('login');
  },
  async resendVerify() { const mail = ($('#lg-mail') || $('#rg-mail') || {}).value || S.verifyMail || ''; if (!mail) return toast('Bitte E-Mail-Adresse eingeben', true); await api('POST', '/auth/resend-verification', { email: mail }); toast('Mail ist unterwegs'); },
  pwOpen() { S.modal = { type: 'pwchange' }; renderModalOnly(); },
  async pwSave() {
    const cur = $('#pw-cur').value, pw = $('#rg-pw').value, miss = pwChecks(pw).filter(c => !c[1]).map(c => c[0]);
    if (!cur) return toast('Bitte das aktuelle Passwort eingeben', true); if (miss.length) return toast('Dem neuen Passwort fehlt: ' + miss.join(', '), true);
    await api('POST', '/me/password', { current: cur, password: pw }); closeModal(); toast('Passwort geändert');
  },
  /* E-Mail (SMTP) */
  async smtpTog() { const c = S.smtp.config; c.enabled = !c.enabled; await H.smtpSave(true); },
  async smtpNotify(el) { const k = el.dataset.k; S.smtp.config[k] = !S.smtp.config[k]; await H.smtpSave(true); },
  async smtpSave(quiet) {
    const c = S.smtp.config, v = id => { const e = $('#' + id); return e ? e.value : null; };
    const body = { ...c, host: v('sm-host') ?? c.host, port: +(v('sm-port') ?? c.port), security: v('sm-sec') ?? c.security, username: v('sm-user') ?? c.username, fromEmail: v('sm-from') ?? c.fromEmail, fromName: v('sm-name') ?? c.fromName, newPassword: v('sm-pw') || null, clearPassword: !!($('#sm-clear') && $('#sm-clear').checked) };
    try { await api('PUT', '/admin/smtp', body); if (quiet !== true) toast('Einstellungen gespeichert'); } finally { S.smtp = await api('GET', '/admin/smtp'); render(); }
  },
  async smtpTest() { const to = $('#sm-to').value.trim(); S.smtpTest = await api('POST', '/admin/smtp/test', { to: to || null }); render(); },
  /* Passwort vergessen: Sicherheitsfragen */
  async fqStart() {
    const mail = ($('#fg-mail') || {}).value.trim(); if (!mail) return toast('Bitte zuerst deine E-Mail-Adresse eingeben', true);
    const r = await api('POST', '/auth/security-questions', { email: mail });
    S.fq = { email: mail, questions: r.questions }; render();
  },
  fqBack(el, e) { e.preventDefault(); S.fq = null; render(); },
  async fqSave() {
    const answers = [1, 2, 3].map(i => $('#fq-a' + i).value.trim()), pw = $('#rs-pw').value;
    if (answers.some(a => !a)) return toast('Bitte alle drei Fragen beantworten', true);
    const miss = pwChecks(pw).filter(c => !c[1]).map(c => c[0]); if (miss.length) return toast('Dem Passwort fehlt: ' + miss.join(', '), true);
    await api('POST', '/auth/reset-questions', { email: S.fq.email, answers, password: pw });
    S.fq = null; toast('Passwort geändert – bitte neu anmelden'); go('login');
  },
  /* Eigene Sicherheitsfragen */
  secqOpen() { S.modal = { type: 'secq' }; renderModalOnly(); },
  async secqSave() {
    const questions = readSecQ('mq'), qp = secQProblem(questions), cur = $('#mq-cur').value;
    if (qp) return toast(qp, true); if (!cur) return toast('Bitte dein Passwort zur Bestätigung eingeben', true);
    await api('PUT', '/me/security-questions', { current: cur, questions }); closeModal(); toast('Sicherheitsfragen gespeichert'); await reload(); render();
  },
  /* Initialpasswort eines Administrators: erzwungene Änderung */
  async pwRequired() {
    const pw = $('#rs-pw').value, miss = pwChecks(pw).filter(c => !c[1]).map(c => c[0]);
    if (miss.length) return toast('Dem Passwort fehlt: ' + miss.join(', '), true);
    if (pw !== $('#rs-pw2').value) return toast('Die Passwörter stimmen nicht überein', true);
    const needQ = S.pwGate && S.pwGate.needsQuestions, questions = needQ ? readSecQ('pg') : [], qp = needQ ? secQProblem(questions) : '';
    if (qp) return toast(qp, true);
    await api('POST', '/me/password/required', { password: pw, questions });
    S.pwGate = null; S.ready = false; await enter();
  },
  /* Administration: Initialpasswort setzen */
  pwAdminOpen() { S.modal = { type: 'adminpw', u: S.modal.u }; renderModalOnly(); },
  async pwAdminSet() {
    const u = S.modal.u, r = await api('POST', `/admin/users/${u.id}/password`, { password: $('#ap-pw').value.trim() || null });
    S.modal = { type: 'adminpwDone', u, password: r.password, pwLogin: r.pwLogin }; renderModalOnly(); try { await reload(); } catch (e) { /* egal */ }
  },
  async pwCopy() { try { await navigator.clipboard.writeText(S.modal.password); toast('Passwort kopiert'); } catch (e) { toast('Bitte das Passwort markieren und kopieren', true); } },
  startBalOpen() { S.modal = { type: 'startbal' }; renderModalOnly(); },
  async startBalSave() {
    const hours = regStartBal();
    await api('POST', '/me/start-balance', { hours }); closeModal(); toast('Überstunden-Stand festgelegt'); await sync();
  },
  async setupSave() {
    const code = $('#su-code').value.trim(), name = $('#su-name').value.trim(), mail = $('#su-mail').value.trim(), pw = $('#rg-pw').value;
    if (!code) return toast('Bitte den Einrichtungscode eingeben', true);
    if (!name) return toast('Bitte Namen eingeben', true);
    if (!/^\S+@\S+\.\S+$/.test(mail)) return toast('Bitte gültige E-Mail eingeben', true);
    const miss = pwChecks(pw).filter(c => !c[1]).map(c => c[0]);
    if (miss.length) return toast('Dem Passwort fehlt: ' + miss.join(', '), true);
    if (pw !== $('#su-pw2').value) return toast('Die Passwörter stimmen nicht überein', true);
    const questions = S.reg.secQuestions ? readSecQ('sq') : [], qp = S.reg.secQuestions ? secQProblem(questions) : '';
    if (qp) return toast(qp, true);
    await api('POST', '/setup', { code, name, email: mail, password: pw, questions, startBal: regStartBal() });
    S.setup = false; history.replaceState(null, '', '#/dashboard'); await enter();
  },
  async register() {
    const name = $('#rg-name').value.trim(), mail = $('#rg-mail').value.trim(), pw = $('#rg-pw').value;
    if (!name) return toast('Bitte Namen eingeben', true);
    if (!/^\S+@\S+\.\S+$/.test(mail)) return toast('Bitte gültige E-Mail eingeben', true);
    const miss = pwChecks(pw).filter(c => !c[1]).map(c => c[0]);
    if (miss.length) return toast('Dem Passwort fehlt: ' + miss.join(', '), true);
    const questions = S.reg.secQuestions ? readSecQ('sq') : [], qp = S.reg.secQuestions ? secQProblem(questions) : '';
    if (qp) return toast(qp, true);
    const r = await api('POST', '/auth/register', { name, email: mail, password: pw, startBal: regStartBal(), questions });
    S.verifyMail = mail; S.regStep = r.status === 'verify' ? 'verify' : r.status === 'active' ? 'done' : 'pending'; S.regPasskey = false; render();
  },
  regReset() { S.regStep = 'form'; render(); },
  async regContinue() { await enter(); },
  async logout() {
    try { await api('POST', '/auth/logout', {}); } catch (e) { /* egal */ }
    S.loggedIn = false; S.ready = false; S.demoOpen = false; S.mfaGate = null; S.pwGate = null; S.mfa = null; go('login');
  },
  async demoLogin(el) {
    try { await api('POST', '/auth/logout', {}); } catch (e) { /* egal */ }
    S.ready = false; S.demoOpen = false;
    await api('POST', '/auth/login', { email: el.dataset.email, password: DEMO_PW }); await enter();
  },
  theme() { return setThemePref({ system: 'light', light: 'dark', dark: 'system' }[S.themePref]); },
  themePick(el) { return setThemePref(el.dataset.v); },
  demoToggle() { S.demoOpen = !S.demoOpen; $('#demo').innerHTML = demoPanel(); },
  speed(el) { S.speed = +el.dataset.v; render(); },
  os(el) { S.os = el.dataset.v; render(); },
  toast(el) { toast(el.dataset.m); },
  goToday() { S.dayDate = TODAY; },
  goDash() { go('dashboard'); },
  /* Tagesansicht */
  dayNav(el) {
    if (el.dataset.to) S.dayDate = el.dataset.to;
    else { const d = addDays(S.dayDate, +el.dataset.n); if (d > TODAY || d < '2026-01-01') return; S.dayDate = d; }
    render();
  },
  /* Statistik */
  statMode(el) { S.statMode = el.dataset.v; render(); },
  statNav(el) {
    if (el.dataset.to) { S.statRef = el.dataset.to; return render(); }
    const n = +el.dataset.n, d = parseD(S.statRef);
    if (S.statMode === 'week') S.statRef = addDays(S.statRef, 7 * n);
    else if (S.statMode === 'month') S.statRef = fmtD(new Date(d.getFullYear(), d.getMonth() + n, 1, 12));
    else S.statRef = fmtD(new Date(d.getFullYear() + n, 0, 1, 12));
    render();
  },
  exportCsv,
  /* Abwesenheiten */
  calNav(el) {
    if (el.dataset.to) S.calMonth = el.dataset.to;
    else { const [y, m] = S.calMonth.split('-').map(Number), d = new Date(y, m - 1 + +el.dataset.n, 1, 12); S.calMonth = fmtD(d).slice(0, 7); }
    render();
  },
  calDay(el) {
    const d = el.dataset.d, ab = S.absences.find(a => d >= a.from && d <= a.to);
    S.modal = { type: 'abs', f: ab ? { ...ab } : { type: 'vacation', from: d, to: d, note: '' } }; renderModalOnly();
  },
  absNew() { S.modal = { type: 'abs', f: { type: 'vacation', from: TODAY, to: TODAY, note: '' } }; renderModalOnly(); },
  absNewDate(el) { S.modal = { type: 'abs', f: { type: 'vacation', from: el.dataset.d, to: el.dataset.d, note: '' } }; renderModalOnly(); },
  absEdit(el) { S.modal = { type: 'abs', f: { ...S.absences.find(a => a.id === +el.dataset.id) } }; renderModalOnly(); },
  absType(el) { S.modal.f.type = el.dataset.v; renderModalOnly(); },
  async absSave() {
    const f = S.modal.f; if (!f.from || !f.to || f.to < f.from) return toast('Bitte einen gültigen Zeitraum wählen', true);
    const body = { type: f.type, from: f.from, to: f.to, note: f.note || '' };
    if (f.id) await api('PUT', '/absences/' + f.id, body); else await api('POST', '/absences', body);
    closeModal(); toast('Abwesenheit gespeichert'); await sync();
  },
  async absDel(el) { await api('DELETE', '/absences/' + el.dataset.id); S.modal = null; toast('Eintrag gelöscht'); await sync(); },
  /* Companion: meldet Ereignisse an den Server (im Prototyp mit simulierter Uhrzeit) */
  async cAct(el) {
    const k = el.dataset.k; if (k === 'newday') S.now = 7 * 60 + 58;
    await api('POST', '/companion', { kind: k, now: nowQ(), os: S.os });
    toast({ newday: 'Neuer Arbeitstag – Rechnerstart ' + hm(S.now) + ' gemerkt', boot: 'Rechner gestartet', lock: 'Bildschirm gesperrt', unlock: 'Entsperrt', pause: 'Tracking gestoppt – die Pause wird nicht gezählt', quit: 'Companion beendet – Tracking gestoppt', shutdown: 'Rechner heruntergefahren', start: 'Tracking läuft' }[k]); await sync();
  },
  /* Zeiterfassung im Web: Beenden jetzt oder mit eigenem Ende, mit Vorschau von Brutto und Netto */
  async stopOpen(el) { S.modal = { type: 'stop', mode: el.dataset.m, at: Math.floor(S.now), prev: null, err: '' }; renderModalOnly(); await H.stopPreview(); },
  async stopMode(el, e) { e.preventDefault(); S.modal.mode = S.modal.mode === 'custom' ? 'now' : 'custom'; S.modal.prev = null; S.modal.err = ''; renderModalOnly(); await H.stopPreview(); },
  async stopPreview() {
    const m = S.modal; if (!m || m.type !== 'stop') return;
    const mode = m.mode, at = m.at;
    try { const r = await api('POST', '/tracking/stop-preview', { at: mode === 'custom' ? at : null, now: nowQ() }); if (S.modal === m && m.mode === mode) { m.prev = r; m.err = ''; } }
    catch (e) { if (S.modal === m && m.mode === mode) { m.prev = null; m.err = e.message; } }
    renderModalOnly();
  },
  async stopDo() {
    const m = S.modal;
    if (m.mode === 'custom') await api('POST', '/companion', { kind: 'stopat', until: m.at, now: nowQ() }); else await api('POST', '/companion', { kind: 'stop', now: nowQ() });
    closeModal(); toast('Tracking beendet – Ende ' + hm(m.prev ? m.prev.at : S.now)); await sync();
  },
  async cAnswer(el) {
    await api('POST', '/companion', { kind: 'answer', answer: el.dataset.v, now: nowQ() });
    toast(el.dataset.v === 'no' ? 'Okay – das Tracking bleibt aus' : 'Tracking gestartet'); await sync();
  },
  pickDate(el) { const n = el.parentElement.querySelector('.dnative'); try { n.showPicker(); } catch (e) { n.focus(); n.click(); } },
  async xdAdd() {
    const name = $('#xd-name').value.trim(), m = String($('#xd-date').value).trim().match(/^(\d{1,2})[.\/-](\d{1,2})\.?$/);
    if (!name || !m) return toast('Bitte Name und Datum im Format TT.MM. eingeben', true);
    S.set.extraDays.custom.push({ day: +m[1], month: +m[2], name }); await saveSet();
  },
  async xdDel(el) { S.set.extraDays.custom.splice(+el.dataset.i, 1); await saveSet(); },
  async pairCode() { const r = await api('POST', '/devices/pair-code', {}); S.pairCode = { code: r.code, until: Date.now() + r.expiresIn * 1000 }; render(); },
  copyCode() { navigator.clipboard.writeText(S.pairCode.code).then(() => toast('Code kopiert'), () => toast('Kopieren nicht möglich – bitte markieren', true)); },
  devRevoke(el) { const d = S.devices.find(x => x.id === el.dataset.id); confirmBox('Kopplung aufheben?', `„${d.name}“ kann danach keine Zeiten mehr melden und muss neu gekoppelt werden.`, 'Aufheben', async () => { await api('DELETE', '/devices/' + d.id); toast('Kopplung aufgehoben'); await sync(); }); },
  /* Zeiten nachträglich bearbeiten */
  dayEdit() {
    const date = S.dayDate, today = date === TODAY, segs = today ? S.day.segs : (dayStat(date).segs || []);
    S.modal = { type: 'dayedit', date, rows: segs.filter(g => g.kind !== 'off').map(g => ({ from: g.from, to: g.to, kind: g.kind, app: g.kind === 'app' ? appName(g.app) : '', note: g.note || '', open: g.to == null })) };
    renderModalOnly();
  },
  dayEditAdd() {
    const rows = S.modal.rows, last = rows.filter(r => !r.open).reduce((m, r) => Math.max(m, r.to), 0), from = Math.min(last || 9 * 60, 1380);
    rows.push({ from, to: Math.min(from + 60, 1440), kind: 'app', app: 'Manuell', note: '' }); renderModalOnly();
  },
  dayEditDel(el) { S.modal.rows.splice(+el.dataset.i, 1); renderModalOnly(); },
  async dayEditSave() {
    const m = S.modal;
    // Namen der Apps zurück auf die gemeldeten Schlüssel abbilden (Anzeigename „VS Code“ → Schlüssel „code“)
    const key = n => { const k = Object.keys(APPS).find(k => APPS[k].n === n); return k || n || 'Manuell'; };
    const segs = m.rows.filter(r => !r.open).map(r => ({ from: r.from, to: r.to, kind: r.kind, app: r.kind === 'app' ? key(String(r.app || '').trim()) : null, note: r.kind === 'app' ? (r.note || null) : null }));
    const r = await api('PUT', '/days/' + m.date, { segs });
    closeModal(); toast(`Gespeichert · Arbeitszeit ${dur(r.before)} → ${dur(r.after)}`); await sync();
  },
  switchesTog() { S.showSwitches = !S.showSwitches; localStorage.setItem('tickly.switches', S.showSwitches ? '1' : '0'); render(); },
  async winAdd() { S.set.autoTrack.windows.push({ days: [true, true, true, true, true, false, false], from: 540, to: 1020 }); await saveSet(); },
  async winDel(el) { S.set.autoTrack.windows.splice(+el.dataset.i, 1); await saveSet(); },
  async cApp(el) { await api('POST', '/companion', { kind: 'app', app: el.dataset.k, now: nowQ() }); await sync(); },
  closeWin() { S.cwin = 'closed'; toast('Läuft weiter im Infobereich – Tracking aktiv'); render(); },
  openWin(el, e) { e.preventDefault(); S.cwin = 'open'; render(); },
  /* Einstellungen */
  async setTab(el) { S.setTab = el.dataset.v; if (S.setTab === 'sec') S.mySessions = await api('GET', '/sessions'); render(); },
  async sessEnd(el) { await api('DELETE', '/admin/sessions/' + el.dataset.id); toast('Sitzung von ' + el.dataset.who + ' beendet'); await loadAdminTab(); render(); },
  devEnd(el) { confirmBox('Kopplung aufheben?', `„${el.dataset.who}“ kann danach keine Zeiten mehr melden und muss neu gekoppelt werden.`, 'Aufheben', async () => { await api('DELETE', '/admin/devices/' + el.dataset.id); toast('Kopplung aufgehoben'); await loadAdminTab(); render(); }); },
  async endAllSess(el) { const r = await api('POST', `/admin/users/${el.dataset.id}/end-sessions`, {}); toast(`${r.ended} Sitzung${r.ended === 1 ? '' : 'en'} beendet`); },
  async mySessEnd(el) { await api('DELETE', '/sessions/' + el.dataset.id); S.mySessions = await api('GET', '/sessions'); toast('Abgemeldet'); render(); },
  async tog(el) { const p = el.dataset.k; setPath(p, !getPath(p)); await afterChange(p); },
  async pick(el) { const p = el.dataset.k; setPath(p, el.dataset.v); await afterChange(p); },
  async preset(el) { S.set.hours = el.dataset.v.split(',').map(Number); await saveSet(); },
  async ruleAdd() { S.set.rules.push({ after: 10, min: 60 }); await saveSet(); },
  async ruleDel(el) { S.set.rules.splice(+el.dataset.i, 1); await saveSet(); },
  async exclAdd() { const v = $('#excl-in').value.trim(); if (!v) return; S.set.excluded.push(v); await saveSet(); },
  async exclDel(el) { S.set.excluded.splice(+el.dataset.i, 1); await saveSet(); },
  wipeApps() { confirmBox('App-Verlauf löschen?', 'Alle gespeicherten App-Nutzungsdaten werden unwiderruflich gelöscht. Arbeitszeiten bleiben erhalten.', 'Löschen', () => toast('App-Verlauf gelöscht (im Prototyp ohne Wirkung)')); },
  pkDel(el) { const id = +el.dataset.id; confirmBox('Passkey entfernen?', 'Mit diesem Passkey ist danach keine Anmeldung mehr möglich.', 'Entfernen', async () => { await api('DELETE', '/passkeys/' + id); toast('Passkey entfernt'); await sync(); }); },
  pkRename(el) { const p = S.passkeys.find(x => x.id === +el.dataset.id); S.modal = { type: 'rename', id: p.id, name: p.name }; renderModalOnly(); },
  async renameOk() { const v = $('#rn-in').value.trim(), id = S.modal.id; if (v) await api('PUT', '/passkeys/' + id, { name: v }); closeModal(); await sync(); },
  async saveMe() { await api('POST', '/me', { name: S.me.name, email: S.me.email }); toast('Profil gespeichert'); await sync(); },
  /* Admin */
  async adminTab(el) { S.adminTab = el.dataset.v; await loadAdminTab(); render(); },
  async secTog() { S.sec.policy.enabled = !S.sec.policy.enabled; render(); await H.secSave(); },
  async secSave() {
    const p = { ...S.sec.policy, allowlist: ($('#sec-allow') ? $('#sec-allow').value : S.sec.policy.allowlist.join(',')).split(/[,;\s]+/).filter(Boolean) };
    await api('PUT', '/admin/security', p); toast('Einstellungen gespeichert'); await loadAdminTab(); render();
  },
  async ipUnblock(el) { await api('POST', '/admin/ip-blocks/unblock', { ip: el.dataset.ip }); toast(el.dataset.ip + ' entsperrt'); await loadAdminTab(); render(); },
  async ipBlock() {
    const ip = $('#blk-ip').value.trim(); if (!ip) return toast('Bitte eine IP-Adresse eingeben', true);
    await api('POST', '/admin/ip-blocks', { ip, minutes: +($('#blk-min').value || 0) }); toast(ip + ' gesperrt'); await loadAdminTab(); render();
  },
  async logKind(el) { S.logKind = el.dataset.v; await loadLog(); render(); },
  async logSearch() { S.logQ = $('#log-q').value.trim(); await loadLog(); render(); },
  async logReload() { await loadLog(); render(); },
  async logIp(el, e) { e.preventDefault(); S.logQ = el.dataset.ip; await loadLog(); render(); },
  async logMore() { const r = await api('GET', logUrl(S.log.next)); S.log = { entries: S.log.entries.concat(r.entries), next: r.next }; render(); },
  async bkTog() { S.bkPol.policy.enabled = !S.bkPol.policy.enabled; render(); await H.bkSave(); },
  async bkSave() {
    const t = $('#bk-time') ? $('#bk-time').value.trim() : S.bkPol.policy.time, k = $('#bk-keep') ? +$('#bk-keep').value : S.bkPol.policy.keep;
    const to = document.querySelector('input[name="bk-target"]:checked');
    await api('PUT', '/admin/backup-policy', { enabled: S.bkPol.policy.enabled, time: t, keep: k, toDir: to ? to.value === 'dir' : S.bkPol.policy.toDir });
    toast('Einstellungen gespeichert'); await loadAdminTab(); render();
  },
  async backupNow() { await api('POST', '/admin/backups', {}); toast('Sicherung angelegt'); await loadAdminTab(); render(); },
  restoreFile() {
    const f = $('#bk-file').files[0]; if (!f) return toast('Bitte zuerst eine Sicherungsdatei auswählen', true);
    confirmBox('Alle Daten ersetzen?', `Der gesamte Datenbestand wird durch „${f.name}“ ersetzt. Der jetzige Stand wird vorher automatisch gesichert.`, 'Wiederherstellen', async () => {
      const r = await fetch('/api/admin/restore', { method: 'POST', credentials: 'same-origin', body: f });
      const j = await r.json().catch(() => ({})); if (!r.ok) throw new Error(j.error || 'Wiederherstellung fehlgeschlagen');
      toast(`Wiederhergestellt: ${j.users} Benutzer, ${j.days} Tage`); S.backups = await api('GET', '/admin/backups'); await sync();
    });
  },
  restoreSaved(el) {
    confirmBox('Alle Daten ersetzen?', `Der gesamte Datenbestand wird durch die Sicherung vom ${el.dataset.at} ersetzt. Der jetzige Stand wird vorher automatisch gesichert.`, 'Wiederherstellen', async () => {
      const j = await api('POST', `/admin/backups/${el.dataset.id}/restore`, {});
      toast(`Wiederhergestellt: ${j.users} Benutzer, ${j.days} Tage`); S.backups = await api('GET', '/admin/backups'); await sync();
    });
  },
  adminFilter(el) { S.adminFilter = el.dataset.v; render(); },
  userEdit(el) { S.modal = { type: 'user', u: { ...S.users.find(u => u.id === el.dataset.id) } }; renderModalOnly(); },
  userInvite() { S.modal = { type: 'user', u: { isNew: true, name: '', email: '', role: 'user', week: S.reg.weekly } }; renderModalOnly(); },
  inviteDirect() { S.modal.u.direct = !S.modal.u.direct; renderModalOnly(); },
  async userSave() {
    const u = S.modal.u, body = { name: u.name, email: u.email, role: u.role, week: +u.week, startBal: u.isNew ? undefined : +u.startBal };
    if (u.isNew && u.direct) {
      const r = await api('POST', '/admin/users', { ...body, direct: true, password: (u.pw || '').trim() || null });
      S.modal = { type: 'adminpwDone', u: { name: u.name }, password: r.password, pwLogin: true, created: true }; renderModalOnly(); await sync(); return;
    }
    if (u.isNew) { const r = await api('POST', '/admin/users', body); toast(r.mailed ? 'Einladung per E-Mail gesendet an ' + u.email : 'Benutzer angelegt – ohne SMTP-Server wird keine Einladung verschickt'); }
    else { await api('PUT', '/admin/users/' + u.id, body); toast('Benutzer gespeichert'); }
    closeModal(); await sync();
  },
  async userStatus(el) {
    const u = S.users.find(x => x.id === el.dataset.id), st = el.dataset.v;
    await api('POST', `/admin/users/${u.id}/status`, { status: st });
    toast(st === 'active' ? u.name + ' ist freigeschaltet' : u.name + ' wurde gesperrt'); S.modal = null; await sync();
  },
  userDel(el) { const u = S.users.find(x => x.id === el.dataset.id); confirmBox(u.status === 'pending' ? 'Registrierung ablehnen?' : 'Benutzer löschen?', `${u.name} (${u.email}) und alle erfassten Daten werden entfernt.`, u.status === 'pending' ? 'Ablehnen' : 'Löschen', async () => { await api('DELETE', '/admin/users/' + u.id); toast('Entfernt'); await sync(); }); },
  async pkReset() { const id = S.modal.u.id; await api('POST', `/admin/users/${id}/reset-passkeys`, {}); toast('Passkeys zurückgesetzt'); await reload(); S.modal.u.passkeys = 0; renderModalOnly(); render(); },
  modalClose: closeModal,
  ovClose() { },
  async confirmOk() { const f = S.modal.fn; closeModal(); await f(); }
};
async function afterChange(path) {
  // Erst nach der Antwort neu zeichnen (nicht im Ereignis des Eingabefelds, das gerade den Fokus verliert)
  if (path === 'modal.at') { if (S.modal) { S.modal.prev = null; S.modal.err = ''; } return H.stopPreview(); }
  if (path.startsWith('set.')) return saveSet();
  if (path.startsWith('reg.')) return saveReg();
  render();
}
const fail = e => { if (e && e.status === 401 && S.loggedIn && !/falsch/.test(e.message)) return expire();
  // Initialpasswort: auf die Seite zum Ändern wechseln
  if (e && e.status === 403 && S.loggedIn && !S.pwGate && /neues Passwort fest/.test(e.message)) { api('GET', '/session').then(ses => { if (ses.limited && ses.reason === 'password') { S.pwGate = ses; S.ready = false; render(); } }); return; }
  // Karenzzeit gerade abgelaufen: auf die Einrichtungsseite wechseln
  if (e && e.status === 403 && S.loggedIn && !S.mfaGate && /Passkey oder eine Authenticator-App/.test(e.message)) { api('GET', '/session').then(ses => { if (ses.limited && ses.reason === 'mfa') { S.mfaGate = ses; S.ready = false; render(); } }).catch(() => {}); return; } toast(e && e.message || 'Fehler', true); };
document.addEventListener('click', e => {
  const el = e.target.closest('[data-a]'); if (!el) return;
  const a = el.dataset.a;
  if (a === 'ovClose') { if (e.target === el) closeModal(); return; }
  if (el.disabled || el.hasAttribute('disabled')) return;
  if (el.tagName === 'A' && a !== 'goToday') e.preventDefault();
  if (H[a]) Promise.resolve(H[a](el, e)).catch(fail);
});
document.addEventListener('keydown', e => {
  if (e.key === 'Escape' && S.modal) closeModal();
  if (e.key === 'Enter') {
    const t = e.target && e.target.id, go = { 'lg-pw': 'login', 'mfa-code': 'totpLogin', 'log-q': 'logSearch',  'blk-ip': 'ipBlock' }[t];
    if (go) H[go]().catch(fail);
    else if (t === 'totp-code') (document.querySelector('[data-a="totpEnable"],[data-a="totpDo"]') || {}).click?.();
  }
});

/* generische Eingabe-Bindung */
function onInput(e, live) {
  const el = e.target.closest ? e.target.closest('[data-in]') : null; if (!el) return;
  let v = el.value;
  if (el.dataset.fmt === 'date') {
    const iso = parseDE(v), max = el.dataset.max;
    if (!iso || (max && iso > max)) { el.classList.add('inv'); toast(max && iso ? 'Das Datum darf nicht nach heute liegen' : 'Bitte ein Datum im Format TT.MM.JJJJ eingeben', true); return; }
    el.classList.remove('inv'); v = iso; el.value = dMid(iso);
  } else if (el.dataset.fmt === 'time') {
    const m = parseTime(v);
    if (m == null) { el.classList.add('inv'); toast('Bitte eine Uhrzeit im Format HH:MM eingeben', true); return; }
    el.classList.remove('inv'); v = m; el.value = hm(m);
  } else if (el.dataset.type === 'num') { v = parseFloat(String(v).replace(',', '.')); if (isNaN(v)) return; }
  const path = el.dataset.in;
  if (path === 'dayDate') { if (v && v <= TODAY) { S.dayDate = v; render(); } return; }
  setPath(path, v);
  if (live) return;
  if (el.dataset.rm) return renderModalOnly();
  if (el.dataset.nr) return;
  afterChange(path).catch(fail);
}
document.addEventListener('change', e => onInput(e, false));
document.addEventListener('change', e => {
  const t = e.target; if (!t || !t.dataset || !t.dataset.sq) return;
  const c = $('#' + t.id.replace('-q', '-c')); if (c) { c.style.display = t.value === '__own' ? '' : 'none'; if (t.value === '__own') c.focus(); }
  syncSecQ(t.id.replace(/-q\d$/, ''));
});
document.addEventListener('input', e => {
  if (e.target.id === 'sess-q') { S.sessQ = e.target.value; const p = e.target.selectionStart; render(); const n = $('#sess-q'); if (n) { n.focus(); n.setSelectionRange(p, p); } return; }
  if (e.target.id === 'rg-carry') { const r = document.querySelector('input[name="rg-bal"][value="carry"]'); if (r) r.checked = true; return; }
  if (e.target.id === 'rg-pw' || e.target.id === 'rs-pw') { $('#pw-rules').innerHTML = pwRulesHtml(e.target.value); return; }
  if (e.target.id === 'admin-q') { S.adminQ = e.target.value; const p = e.target.selectionStart; render(); const n = $('#admin-q'); n.focus(); n.setSelectionRange(p, p); return; }
  if (e.target.dataset && e.target.dataset.nr) onInput(e, true);
});

/* ---------- Uhr (Prototyp: simulierte Uhrzeit, optional im Zeitraffer) ---------- */
let tickN = 0, ticking = false;
setInterval(async () => {
  if (!S.loggedIn || !S.ready) return;
  S.now = Math.min(1439.9, S.now + S.speed / 60); tickN++;
  const c = document.querySelector('.clockchip b'); if (c) c.textContent = hm(S.now);
  const rn = document.getElementById('ring-n'); if (rn) rn.textContent = durS(+rn.dataset.base + (rn.dataset.run === '1' ? S.now - +rn.dataset.at : 0));
  if (!(S.speed > 1 || tickN % 10 === 0)) return;
  // Der Companion prüft regelmäßig, ob ein Zeitfenster für den automatischen Start begonnen hat
  if (S.demo && S.day && ['ask', 'idle'].includes(S.day.stop) && S.set.autoTrack.enabled && !S.modal) {
    try { await api('POST', '/companion', { kind: 'tick', now: nowQ() }); await refreshToday(); } catch (e) { return fail(e); }
  }
  if (!['dashboard', 'day', 'companion'].includes(S.route) || S.modal || ticking) return;
  if (document.activeElement && /^(INPUT|SELECT|TEXTAREA)$/.test(document.activeElement.tagName)) return;
  if ($('#tip').style.display === 'block') return;
  ticking = true;
  try { await refreshToday(); if (S.dateChanged) { S.dateChanged = false; await reload(); } render(); } catch (e) { fail(e); } finally { ticking = false; }
}, 1000);

/* ---------- Start ---------- */
(async function boot() {
  const q = new URLSearchParams(location.search);
  const stored = localStorage.getItem('tickly.theme');
  S.themePref = ['light', 'dark'].includes(q.get('theme')) ? q.get('theme') : ['system', 'light', 'dark'].includes(stored) ? stored : 'system';
  S.themeLocked = ['light', 'dark'].includes(q.get('theme')); // Adresse mit ?theme= gilt für diesen Aufruf
  applyTheme();
  try {
    const cfg = await api('GET', '/config');
    TODAY = cfg.today; TRACK_START = cfg.trackStart; S.dayDate = TODAY; S.statRef = TODAY; S.calMonth = TODAY.slice(0, 7);
    S.demoHint = cfg.demoHint; S.setup = !!cfg.setup; S.demo = !!cfg.demo; if (!S.demo) S.now = cfg.now;
    S.questionList = cfg.questionList || []; S.reg = { ...S.reg, secQuestions: cfg.secQuestions, open: cfg.registrationOpen, approval: cfg.approval, domains: cfg.domains, minPw: cfg.minPw, passkeyPrompt: cfg.passkeyPrompt };
    // Entwickler-Abkürzung: ?autologin=andre@example.de meldet mit dem Demo-Passwort an
    if (q.get('autologin') && cfg.demo) await api('POST', '/auth/login', { email: q.get('autologin'), password: DEMO_PW });
    const ses = await api('GET', '/session');
    if (ses.loggedIn && !ses.limited) S.loggedIn = true;
    else if (ses.loggedIn && ses.reason === 'password') { S.loggedIn = true; S.pwGate = ses; }
    else if (ses.loggedIn && ses.reason === 'mfa') { S.loggedIn = true; S.mfaGate = ses; }
  } catch (e) { setTimeout(() => toast(e.message, true), 100); }
  /* Deep-Links für Demos: ?stat=year&set=rec&admin=reg&os=mac&day=2026-10-21&modal=abs */
  if (q.get('stat')) S.statMode = q.get('stat'); if (q.get('set')) S.setTab = q.get('set'); if (q.get('admin')) S.adminTab = q.get('admin');
  if (q.get('os')) S.os = q.get('os'); if (q.get('day')) S.dayDate = q.get('day');
  const mo = q.get('modal');
  if (mo === 'abs') S.modal = { type: 'abs', f: { type: 'vacation', from: '2026-11-02', to: '2026-11-06', note: '' } };
  if (mo === 'passkey') S.modal = { type: 'passkey', mode: 'login', step: 'wait' };
  window.__keepModal = !!S.modal;
  window.addEventListener('hashchange', route);
  route();
})();
