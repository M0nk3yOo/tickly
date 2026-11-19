'use strict';
/* Tickly – Web-Oberfläche · Persönliche API-Schlüssel (Einstellungen → „API-Schlüssel“, Serververwaltung → „API-Schlüssel“).
   Eigenständig: Zustand in S.ak, Ansichten vApiKeys / vAdminApiKeys / vApiKeyModal, Handler in AK_H (main.js hängt sie in H ein). */

S.ak = { list: null, policy: null, loading: false, admin: null, adminLoading: false, q: '', status: 'all', form: { name: '', days: '90', write: '0' }, pol: null };

const AK_DAYS = [[30, '30 Tage'], [90, '90 Tage'], [180, '180 Tage'], [365, '1 Jahr'], [0, 'Unbegrenzt']];
const AK_STATUS = { active: ['Aktiv', 'good'], expired: ['Abgelaufen', 'warn'], revoked: ['Widerrufen', 'bad'] };
const akWhen = iso => iso ? fmtStamp(iso) : '–';
const akExp = k => k.expiresAt ? akWhen(k.expiresAt) : 'unbegrenzt';

async function akLoad() {
  S.ak.loading = true;
  try { const r = await api('GET', '/apikeys'); S.ak.list = r.keys; S.ak.policy = r.policy; }
  catch (e) { S.ak.list = S.ak.list || []; toast(e.message, true); }
  S.ak.loading = false; render();
}
async function akAdminLoad() {
  S.ak.adminLoading = true;
  try {
    const r = await api('GET', '/admin/apikeys?status=' + S.ak.status + (S.ak.q ? '&q=' + encodeURIComponent(S.ak.q) : ''));
    S.ak.admin = r; S.ak.policy = r.policy; if (!S.ak.pol) S.ak.pol = { ...r.policy };
  } catch (e) { S.ak.admin = S.ak.admin || { keys: [], total: 0, active: 0, policy: { enabled: true, allowUnlimited: true, maxDays: 365 } }; toast(e.message, true); }
  S.ak.adminLoading = false; render();
}

/* ---------- Einstellungen ---------- */
const akOwner = () => { if (S.ak.uid !== S.me.id) Object.assign(S.ak, { uid: S.me.id, list: null, admin: null, pol: null, policy: null, loading: false, adminLoading: false }); };
function vApiKeys() {
  akOwner();
  if (S.ak.list === null && !S.ak.loading) setTimeout(akLoad, 0);
  const A = S.ak, P = A.policy, list = A.list || [];
  const active = list.filter(k => k.status === 'active').length, max = P ? P.maxActive : 20;
  const off = P && !P.enabled;
  return `<div class="stack"><div class="card"><div class="ch"><h3>Persönliche API-Schlüssel</h3><button class="btn primary sm right" data-a="akNew" ${off || active >= max ? 'disabled' : ''}>${ic('plus', 15)} Neuer Schlüssel</button></div>
    <p class="hint" style="margin-bottom:12px">Mit einem Schlüssel greifen eigene Skripte und Programme auf deine Zeiten zu (Schnittstelle <span class="mono">/api/v1</span>), ohne dass du dein Passwort weitergibst. Ein Schlüssel kann nichts an Passwörtern, Passkeys, Sicherheit oder Geräten ändern. Er wird nur einmal angezeigt; hier bleibt nur ein Kürzel zur Wiedererkennung.</p>
    ${off ? `<div class="callout warn" style="margin-bottom:12px">${ic('info')}<span>Der Administrator hat API-Schlüssel ausgeschaltet. Bestehende Schlüssel werden nicht akzeptiert.</span></div>` : ''}
    ${A.list === null ? '<div class="hint">Wird geladen …</div>' : list.length ? `<div class="tbwrap"><table class="tb"><thead><tr><th>Bezeichnung</th><th>Schlüssel</th><th>Berechtigung</th><th>Angelegt</th><th>Läuft ab</th><th>Zuletzt benutzt</th><th>Status</th><th></th></tr></thead><tbody>
      ${list.map(k => `<tr style="${k.status === 'active' ? '' : 'opacity:.6'}"><td><b>${esc(k.name)}</b></td><td class="mono sm">${esc(k.prefix)}…</td><td class="sm">${k.write ? 'Lesen und Schreiben' : 'Nur lesen'}</td><td class="sm">${akWhen(k.createdAt)}</td><td class="sm">${akExp(k)}</td><td class="sm">${k.lastUsedAt ? fmtAgo(k.lastUsedAt) : 'nie'}</td>
        <td><span class="badge ${AK_STATUS[k.status][1]}">${AK_STATUS[k.status][0]}</span></td><td>${k.status === 'revoked' ? '' : `<button class="btn sm" data-a="akRevoke" data-id="${k.id}" data-name="${esc(k.name)}">Widerrufen</button>`}</td></tr>`).join('')}
      </tbody></table></div>` : '<div class="hint">Noch kein Schlüssel angelegt.</div>'}
    <p class="hint" style="margin-top:10px">${active} von ${max} aktiven Schlüsseln${P && !P.allowUnlimited ? ` · Laufzeit höchstens ${P.maxDays} Tage` : ''}. Beschreibung der Schnittstelle: <span class="mono">docs/API.md</span>.</p></div></div>`;
}

/* ---------- Serververwaltung ---------- */
function vAdminApiKeys() {
  akOwner();
  if (S.ak.admin === null && !S.ak.adminLoading) setTimeout(akAdminLoad, 0);
  const A = S.ak, R = A.admin, pol = A.pol || { enabled: true, allowUnlimited: true, maxDays: 365 };
  const rows = R ? R.keys : [];
  return `<div class="stack"><div class="card"><div class="ch"><h3>Richtlinie</h3></div>
      ${setRow('API-Schlüssel erlauben', 'Aus: Niemand kann neue Schlüssel anlegen, und vorhandene werden nicht mehr akzeptiert (sie bleiben gespeichert).', `<button class="switch ${pol.enabled ? 'on' : ''}" data-a="akPolTog" data-k="enabled" role="switch" aria-checked="${pol.enabled}"></button>`)}
      ${setRow('Unbegrenzte Laufzeit erlauben', 'Aus: Jeder Schlüssel braucht ein Ablaufdatum.', `<button class="switch ${pol.allowUnlimited ? 'on' : ''}" data-a="akPolTog" data-k="allowUnlimited" role="switch" aria-checked="${pol.allowUnlimited}"></button>`)}
      <div class="setrow"><div class="tx"><b>Längste Laufzeit</b><span>Gilt für befristete Schlüssel.</span></div><select id="ak-maxdays" style="width:130px">${[[30, '30 Tage'], [90, '90 Tage'], [180, '180 Tage'], [365, '1 Jahr'], [730, '2 Jahre'], [1095, '3 Jahre']].map(([v, l]) => `<option value="${v}" ${pol.maxDays === v ? 'selected' : ''}>${l}</option>`).join('')}</select></div>
      <div class="row" style="margin-top:12px"><button class="btn primary" data-a="akPolSave">Speichern</button></div></div>
    <div class="card" style="padding:0"><div class="row wrap" style="padding:16px 18px"><h3>Ausgestellte Schlüssel</h3><span class="muted sm">${R ? `${R.active} aktiv von ${R.total}` : ''}</span>
      <span class="right row"><select id="ak-status" style="width:150px">${[['all', 'Alle'], ['active', 'Aktiv'], ['expired', 'Abgelaufen'], ['revoked', 'Widerrufen']].map(([v, l]) => `<option value="${v}" ${A.status === v ? 'selected' : ''}>${l}</option>`).join('')}</select>
      <span style="position:relative"><span style="position:absolute;left:10px;top:9px;color:var(--muted)">${ic('search', 16)}</span><input type="search" id="ak-q" value="${esc(A.q)}" placeholder="Benutzer, Bezeichnung, Kürzel …" style="padding-left:34px;width:230px"></span></span></div>
    <div class="tbwrap"><table class="tb"><thead><tr><th>Benutzer</th><th>Bezeichnung</th><th>Schlüssel</th><th>Berechtigung</th><th>Angelegt</th><th>Läuft ab</th><th>Zuletzt benutzt</th><th>Status</th><th></th></tr></thead><tbody>
    ${R ? rows.map(k => `<tr><td><b>${esc(k.userName)}</b><br><span class="xs muted">${esc(k.email)}</span></td><td>${esc(k.name)}</td><td class="mono sm">${esc(k.prefix)}…</td><td class="sm">${k.write ? 'Lesen und Schreiben' : 'Nur lesen'}</td><td class="sm">${akWhen(k.createdAt)}</td><td class="sm">${akExp(k)}</td><td class="sm">${k.lastUsedAt ? fmtAgo(k.lastUsedAt) : 'nie'}</td>
      <td><span class="badge ${AK_STATUS[k.status][1]}">${AK_STATUS[k.status][0]}</span></td><td>${k.status === 'revoked' ? '' : `<button class="btn sm danger" data-a="akAdminRevoke" data-id="${k.id}" data-name="${esc(k.name)}" data-who="${esc(k.email)}">Ungültig machen</button>`}</td></tr>`).join('') || '<tr><td colspan="9" class="muted" style="text-align:center;padding:20px">Keine Schlüssel.</td></tr>' : '<tr><td colspan="9" class="muted" style="text-align:center;padding:20px">Wird geladen …</td></tr>'}
    </tbody></table></div></div></div>`;
}

/* ---------- Fenster ---------- */
function vApiKeyModal(m) {
  const P = S.ak.policy || { allowUnlimited: true, maxDays: 365 }, F = S.ak.form;
  if (m.type === 'apikeyNew') {
    const opts = AK_DAYS.filter(([d]) => d === 0 ? P.allowUnlimited : d <= P.maxDays);
    return `<div class="ov" data-a="ovClose"><div class="modal" style="width:min(460px,100%)"><div class="mh"><h3>Neuer API-Schlüssel</h3><button class="ibtn right" data-a="modalClose">${ic('x')}</button></div>
      <div class="mb"><label class="f">Bezeichnung<input type="text" id="ak-name" maxlength="60" value="${esc(F.name)}" placeholder="z. B. Auswertung im Büro-Skript" autocomplete="off"></label>
        <label class="f">Laufzeit<select id="ak-days">${opts.map(([d, l]) => `<option value="${d}" ${String(F.days) === String(d) ? 'selected' : ''}>${l}</option>`).join('')}</select></label>
        <label class="f">Berechtigung<select id="ak-perm"><option value="0" ${F.write === '0' ? 'selected' : ''}>Nur lesen</option><option value="1" ${F.write === '1' ? 'selected' : ''}>Lesen und Schreiben (Tracking, Abwesenheiten, Zeiten eintragen)</option></select></label>
        <p class="hint">Der Schlüssel gilt nur für die Schnittstelle <span class="mono">/api/v1</span>. Nach dem Anlegen wird er genau einmal angezeigt.</p></div>
      <div class="mf"><button class="btn" data-a="modalClose">Abbrechen</button><button class="btn primary" data-a="akCreate">Schlüssel anlegen</button></div></div></div>`;
  }
  return `<div class="ov"><div class="modal" style="width:min(560px,100%)"><div class="mh"><h3>Dein neuer Schlüssel</h3></div>
    <div class="mb"><div class="callout warn">${ic('shield')}<span><b>Jetzt kopieren.</b> Dieser Schlüssel wird nur jetzt angezeigt und lässt sich später nicht mehr abrufen. Behandle ihn wie ein Passwort.</span></div>
      <div class="row" style="gap:8px"><input type="text" readonly value="${esc(m.key)}" class="mono grow" style="user-select:all" onclick="this.select()"><button class="btn" data-a="akCopy">Kopieren</button></div>
      <p class="hint">„${esc(m.item.name)}“ · ${m.item.write ? 'Lesen und Schreiben' : 'Nur lesen'} · läuft ab: ${akExp(m.item)}</p>
      <p class="hint">Beispiel: <span class="mono" style="word-break:break-all">curl -H "Authorization: Bearer ${esc(m.item.prefix)}…" ${esc(location.origin)}/api/v1/status</span></p></div>
    <div class="mf"><button class="btn primary" data-a="akDone">Ich habe den Schlüssel gesichert</button></div></div></div>`;
}

/* ---------- Handler ---------- */
const AK_H = {
  akNew() { S.ak.form = { name: '', days: S.ak.policy && !S.ak.policy.allowUnlimited ? String(Math.min(90, S.ak.policy.maxDays)) : '90', write: '0' }; S.modal = { type: 'apikeyNew' }; renderModalOnly(); setTimeout(() => $('#ak-name') && $('#ak-name').focus(), 50); },
  async akCreate() {
    const name = $('#ak-name').value.trim(), days = +$('#ak-days').value, write = $('#ak-perm').value === '1';
    S.ak.form = { name, days: String(days), write: write ? '1' : '0' };
    if (!name) return toast('Bitte eine Bezeichnung eingeben', true);
    const r = await api('POST', '/apikeys', { name, days: days || null, write });
    S.modal = { type: 'apikey', key: r.key, item: r.item }; renderModalOnly();
    await akLoad();
  },
  async akCopy() { try { await navigator.clipboard.writeText(S.modal.key); toast('Schlüssel kopiert'); } catch (e) { toast('Bitte den Schlüssel markieren und kopieren', true); } },
  akDone() { closeModal(); },
  akRevoke(el) {
    confirmBox('Schlüssel widerrufen?', `„${el.dataset.name}“ wird sofort ungültig. Programme, die ihn nutzen, bekommen danach Fehler 401.`, 'Widerrufen', async () => { await api('DELETE', '/apikeys/' + el.dataset.id); toast('Schlüssel widerrufen'); await akLoad(); });
  },
  akAdminRevoke(el) {
    confirmBox('Schlüssel ungültig machen?', `„${el.dataset.name}“ von ${el.dataset.who} wird sofort ungültig.`, 'Ungültig machen', async () => { await api('DELETE', '/admin/apikeys/' + el.dataset.id); toast('Schlüssel ungültig gemacht'); await akAdminLoad(); });
  },
  akPolTog(el) { const p = S.ak.pol || (S.ak.pol = { ...S.ak.policy }); p[el.dataset.k] = !p[el.dataset.k]; render(); },
  async akPolSave() {
    const p = S.ak.pol; p.maxDays = +$('#ak-maxdays').value;
    await api('PUT', '/admin/apikey-policy', { enabled: !!p.enabled, allowUnlimited: !!p.allowUnlimited, maxDays: p.maxDays });
    toast('Richtlinie gespeichert'); S.ak.list = null; await akAdminLoad();
  },
};
document.addEventListener('input', e => {
  if (e.target.id === 'ak-q') { S.ak.q = e.target.value; clearTimeout(AK_H._t); AK_H._t = setTimeout(async () => { await akAdminLoad(); const n = $('#ak-q'); if (n) { n.focus(); n.setSelectionRange(n.value.length, n.value.length); } }, 250); }
});
document.addEventListener('change', e => {
  if (e.target.id === 'ak-status') { S.ak.status = e.target.value; akAdminLoad(); }
});
