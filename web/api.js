'use strict';
/* Tickly – API-Client und Passkey-Hilfen (WebAuthn) */

async function api(method, path, body) {
  let r;
  try {
    r = await fetch('/api' + path, { method, credentials: 'same-origin', headers: body !== undefined ? { 'Content-Type': 'application/json' } : {}, body: body !== undefined ? JSON.stringify(body) : undefined });
  } catch (e) { const err = new Error('Server nicht erreichbar'); err.status = 0; throw err; }
  let data = null; try { data = await r.json(); } catch (e) { /* leere Antwort */ }
  if (!r.ok) { const err = new Error((data && data.error) || 'Fehler ' + r.status); err.status = r.status; throw err; }
  return data;
}
const nowQ = () => Math.round(S.now * 100) / 100;
async function reload() { applySnapshot(await api('GET', '/snapshot?now=' + nowQ())); }
async function refreshToday() {
  const t = await api('GET', '/today?now=' + nowQ());
  if (t.date && t.date !== TODAY) { S.dateChanged = true; return; }
  if (!S.demo && t.now != null) S.now = t.now;
  S.day = t.day; S.night = t.nightCheck || null; DB.days[TODAY] = t.today; DB.forecastEnd = t.forecastEnd;
}

/* ---------- WebAuthn ---------- */
const b64uToBuf = s => { s = s.replace(/-/g, '+').replace(/_/g, '/'); const bin = atob(s + '='.repeat((4 - s.length % 4) % 4)); return Uint8Array.from(bin, c => c.charCodeAt(0)).buffer; };
const bufToB64u = b => { let s = ''; new Uint8Array(b).forEach(x => { s += String.fromCharCode(x); }); return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, ''); };

function passkeySupport() {
  if (!window.PublicKeyCredential) return 'Dieser Browser unterstützt keine Passkeys.';
  if (!window.isSecureContext) return 'Passkeys funktionieren nur über HTTPS oder auf localhost.';
  return null;
}
function deviceName() {
  const ua = navigator.userAgent, os = /Windows/.test(ua) ? 'Windows Hello' : /Mac/.test(ua) ? 'Touch ID · Mac' : /Android/.test(ua) ? 'Android' : /iPhone|iPad/.test(ua) ? 'Face ID · iOS' : 'Passkey';
  const br = /Edg\//.test(ua) ? 'Edge' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : /Firefox\//.test(ua) ? 'Firefox' : 'Browser';
  return `${os} · ${br}`;
}
function friendlyPasskeyError(e) {
  if (e && (e.name === 'NotAllowedError' || e.name === 'AbortError')) return 'Abgebrochen oder Zeitüberschreitung.';
  if (e && e.name === 'InvalidStateError') return 'Dieses Gerät ist bereits als Passkey registriert.';
  return e && e.message ? e.message : 'Passkey fehlgeschlagen.';
}

/** Passkey für das angemeldete (oder eingeschränkt angemeldete) Konto einrichten. */
async function webauthnCreate(name) {
  const bad = passkeySupport(); if (bad) throw new Error(bad);
  const start = await api('POST', '/passkey/register/start', {});
  const pk = start.options.publicKey;
  pk.challenge = b64uToBuf(pk.challenge); pk.user.id = b64uToBuf(pk.user.id);
  (pk.excludeCredentials || []).forEach(c => { c.id = b64uToBuf(c.id); });
  const cred = await navigator.credentials.create({ publicKey: pk });
  await api('POST', '/passkey/register/finish', {
    id: start.id, name: name || deviceName(),
    credential: { id: cred.id, rawId: bufToB64u(cred.rawId), type: cred.type, extensions: cred.getClientExtensionResults(), response: { attestationObject: bufToB64u(cred.response.attestationObject), clientDataJSON: bufToB64u(cred.response.clientDataJSON) } }
  });
}

/** Mit Passkey anmelden (E-Mail bestimmt, welche Passkeys erlaubt sind). */
async function webauthnGet(email) {
  const bad = passkeySupport(); if (bad) throw new Error(bad);
  const start = await api('POST', '/passkey/login/start', { email });
  const pk = start.options.publicKey;
  pk.challenge = b64uToBuf(pk.challenge);
  (pk.allowCredentials || []).forEach(c => { c.id = b64uToBuf(c.id); });
  const cred = await navigator.credentials.get({ publicKey: pk });
  const r = cred.response;
  await api('POST', '/passkey/login/finish', {
    id: start.id,
    credential: { id: cred.id, rawId: bufToB64u(cred.rawId), type: cred.type, extensions: cred.getClientExtensionResults(), response: { authenticatorData: bufToB64u(r.authenticatorData), clientDataJSON: bufToB64u(r.clientDataJSON), signature: bufToB64u(r.signature), userHandle: r.userHandle ? bufToB64u(r.userHandle) : null } }
  });
}
