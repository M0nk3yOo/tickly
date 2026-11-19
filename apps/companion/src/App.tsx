import { useEffect, useState } from "react";
import { api, Track, UiState } from "./api";

const hm = (m: number) => {
  const t = Math.round(Math.abs(m));
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
};
/** Stunden:Minuten:Sekunden */
const hms = (m: number) => {
  const t = Math.round(Math.max(0, m) * 60);
  return `${Math.floor(t / 3600)}:${String(Math.floor((t % 3600) / 60)).padStart(2, "0")}:${String(t % 60).padStart(2, "0")}`;
};
const clock = (m: number | null) => (m == null ? "–" : `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(Math.round(m % 60)).padStart(2, "0")}`);
const signed = (m: number) => (Math.round(m) === 0 ? "±0:00" : `${m > 0 ? "+" : "−"}${hm(m)}`);

const LABEL: Record<string, { text: string; tone: "good" | "warn" | "bad" }> = {
  running: { text: "Tracking läuft", tone: "good" },
  locked: { text: "Rechner gesperrt", tone: "warn" },
  paused: { text: "Tracking pausiert", tone: "warn" },
  asking: { text: "Rückfrage", tone: "warn" },
  idle: { text: "Tracking aus", tone: "bad" },
  stopped: { text: "Tracking beendet", tone: "bad" },
};

export function App() {
  const [s, setS] = useState<UiState | null>(null);
  useEffect(() => {
    api.get().then(setS);
    let off = () => {};
    api.onState(setS).then((f) => (off = f));
    return () => off();
  }, []);
  // Darstellung wie in der Weboberfläche eingestellt: System, Hell oder Dunkel
  const theme = s?.view?.state.settings?.theme ?? "system";
  useEffect(() => {
    if (theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.dataset.theme = theme;
  }, [theme]);
  if (!s) return <div className="shell" />;
  return <div className="shell">{s.paired ? <Main s={s} set={setS} /> : <Pair s={s} set={setS} />}</div>;
}

function Header({ s }: { s: UiState }) {
  const track = (s.view?.state.track || "") as Track;
  const l = LABEL[track] ?? { text: "Verbinde …", tone: "warn" as const };
  return (
    <div className="head">
      <img className="logo" src="/icon.svg" alt="" width={30} height={30} />
      <b>Tickly</b>
      {s.paired && (
        <span className={`badge ${l.tone}`}>
          <i className={`dot ${track}`} />
          {l.text}
        </span>
      )}
    </div>
  );
}

/** Der Tickly-Dienst, mit dem sich der Companion ohne weitere Angaben verbindet. */
const HOSTED = "tickly.m-hause.de";
const sameServer = (a: string, b: string) => a.trim().replace(/^https?:\/\//i, "").replace(/\/+$/, "").toLowerCase() === b;

function Pair({ s, set }: { s: UiState; set: (s: UiState) => void }) {
  // Ein früher eingetragener fremder Server bleibt als „Self-hosted“ vorgewählt
  const [own, setOwn] = useState(!!s.server && !sameServer(s.server, HOSTED));
  const [custom, setCustom] = useState(own ? s.server : "");
  const server = own ? custom : `https://${HOSTED}`;
  const [code, setCode] = useState("");
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const go = async () => {
    setErr("");
    setBusy(true);
    try {
      set(await api.pair(server, code));
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Header s={s} />
      <div className="body">
        <div>
          <h1>Gerät koppeln</h1>
          <p className="muted">Öffne Tickly im Browser, gehe zu <b>Companion-App</b> und erzeuge einen Kopplungscode.</p>
        </div>
        <div className="seg" role="radiogroup" aria-label="Server">
          <button type="button" role="radio" aria-checked={!own} className={own ? "" : "on"} onClick={() => setOwn(false)}>Tickly-Dienst</button>
          <button type="button" role="radio" aria-checked={own} className={own ? "on" : ""} onClick={() => setOwn(true)}>Self-hosted</button>
        </div>
        {own ? (
          <label>
            Server-Adresse
            <input value={custom} onChange={(e) => setCustom(e.target.value)} placeholder="zeit.example.com" autoCapitalize="off" spellCheck={false} autoFocus />
            <span className="muted xs">Die Adresse deines eigenen Tickly-Servers.</span>
          </label>
        ) : (
          <p className="muted xs">Der Companion verbindet sich mit {HOSTED}.</p>
        )}
        <label>
          Kopplungscode
          <input className="code" value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder="ABCD-EFGH" maxLength={9} spellCheck={false} onKeyDown={(e) => e.key === "Enter" && server.trim() && code.trim().length >= 8 && go()} />
        </label>
        {err && <div className="callout bad">{err}</div>}
        <button className="btn primary" disabled={busy || !server.trim() || code.trim().length < 8} onClick={go}>
          {busy ? "Koppeln …" : "Koppeln"}
        </button>
      </div>
    </>
  );
}

function Main({ s, set }: { s: UiState; set: (s: UiState) => void }) {
  const v = s.view;
  const st = v?.state;
  const track = (st?.track || "") as Track;
  const running = track === "running" || track === "locked";
  const act = async (k: "pause" | "resume" | "end" | "answer" | "away", val?: string) => set(await api.act(k, val));
  const [err, setErr] = useState("");
  const change = async (f: () => Promise<UiState>) => {
    setErr("");
    try {
      set(await f());
    } catch (e) {
      setErr(String(e));
    }
  };
  const pct = st && st.soll > 0 ? Math.min(100, (st.counted / st.soll) * 100) : 0;
  const fresh = !st?.start;
  // Die Sekunden laufen im Fenster weiter, ausgehend vom Zeitpunkt der Berechnung (nicht vom Eintreffen der Meldung)
  const [, setTick] = useState(0);
  useEffect(() => { const t = setInterval(() => setTick((n) => n + 1), 1000); return () => clearInterval(t); }, []);
  const live = (st?.counted ?? 0) + (track === "running" && v ? Math.max(0, Date.now() - v.asOf) / 60000 : 0);
  return (
    <>
      <Header s={s} />
      <div className="body">
        {v?.unauthorized && (
          <div className="callout bad">
            <b>Kopplung aufgehoben.</b> Dieses Gerät darf keine Zeiten mehr melden.
            <button className="btn sm" onClick={async () => set(await api.unpair())}>Neu koppeln</button>
          </div>
        )}
        {s.update && (
          <div className="callout warn">
            <b>Update verfügbar ({s.update.latest})</b>
            <span>
              Du nutzt Version {s.version}.{s.update.size ? ` Der Installer ist ${(s.update.size / 1048576).toFixed(1).replace(".", ",")} MB groß.` : ""} Lade ihn herunter und installiere ihn über die vorhandene Version; die Kopplung bleibt erhalten.
            </span>
            <div className="row">
              <button className="btn sm primary" onClick={() => api.openUpdate()}>Herunterladen</button>
              <button className="btn sm" onClick={() => api.openUpdate(true)}>Versionshinweise</button>
            </div>
          </div>
        )}
        {st?.mfa?.required && !st.mfa.compliant && (
          <div className="callout warn">
            <b>Bitte Konto absichern.</b>
            <span>
              Dein Administrator verlangt einen Passkey oder eine Authenticator-App. {st.mfa.expired ? "Die Frist ist abgelaufen." : `Noch ${st.mfa.daysLeft ?? 0} Tag${st.mfa.daysLeft === 1 ? "" : "e"}.`} Richte es in der Weboberfläche unter Einstellungen → Sicherheit ein.
            </span>
            <button className="btn sm" onClick={() => api.dashboard()}>Weboberfläche öffnen</button>
          </div>
        )}
        {st?.nightCheck && running && (
          <div className="callout warn">
            <b>Läuft dein Tracking noch?</b>
            <span>
              Es läuft seit gestern {clock(st.nightCheck.since)} Uhr ohne Unterbrechung ({hm(st.nightCheck.hours * 60)} Stunden) und passt nicht zu deinen bisherigen Zeiten. Beende es in der Weboberfläche mit dem Zeitpunkt, an dem du wirklich aufgehört hast, oder bestätige dort, dass du arbeitest.
            </span>
            <button className="btn sm" onClick={() => api.dashboard()}>Weboberfläche öffnen</button>
          </div>
        )}
        {st?.deviceClash && running && (
          <div className="callout warn">
            <b>Tracking läuft auf zwei Geräten</b>
            <span>
              Auf „{st.deviceClash.starter}“ wurde das Tracking gestartet, während „{st.deviceClash.other}“ noch lief. Beide melden Zeiten und Apps für denselben Tag und verfälschen sich gegenseitig. Beende den Companion auf einem der Geräte.
            </span>
            <button className="btn sm" onClick={() => api.dashboard()}>Weboberfläche öffnen</button>
          </div>
        )}
        {track === "asking" && (
          <div className="ask">
            <h2>Arbeitest du gerade?</h2>
            <p>
              Der Rechner läuft außerhalb deiner automatischen Zeitfenster. Soll die Zeiterfassung laufen?
              {fresh && st?.bootAt != null && ` „Ab ${clock(st.bootAt)} Uhr“ zählt rückwirkend ab dem Rechnerstart.`}
            </p>
            <div className="row nowrap">
              <button className="btn primary grow" onClick={() => act("answer", "work")}>Ja, jetzt starten</button>
              {fresh && <button className="btn grow" onClick={() => act("answer", "boot")}>Ja, ab {st?.bootAt != null ? `${clock(st.bootAt)} Uhr` : "Rechnerstart"}</button>}
              <button className="btn" onClick={() => act("answer", "no")}>Nein</button>
            </div>
          </div>
        )}
        {st?.away && (
          <div className="ask">
            <h2>Pause oder Arbeitszeit?</h2>
            <p>
              Der Rechner war von {clock(st.away.from)} bis {clock(st.away.to)} Uhr gesperrt ({hm(st.away.to - st.away.from)} h). Warst du in der Pause oder in einem Termin?
            </p>
            <div className="row nowrap">
              <button className="btn grow" onClick={() => act("away", "pause")}>Pause</button>
              <button className="btn primary grow" onClick={() => act("away", "work")}>Arbeitszeit</button>
            </div>
          </div>
        )}
        <div>
          <div className="muted xs">Heute{st?.holiday ? ` · ${st.holiday}` : ""}</div>
          <div className="big">
            {hms(live)} <span className="muted">/ {hm(st?.soll ?? 0)} h</span>
          </div>
          <div className="pbar"><i style={{ width: `${st && st.soll > 0 ? Math.min(100, (live / st.soll) * 100) : pct}%` }} /></div>
        </div>
        <div className="kvs">
          <div><span>Arbeitsbeginn</span><b>{clock(st?.start ?? null)}</b></div>
          <div><span>Aktive App</span><b>{track === "locked" ? "Gesperrt" : running ? v?.app ?? "–" : "–"}</b></div>
          <div><span>{(st?.diff ?? 0) < 0 ? "Noch bis zum Soll" : "Mehrzeit heute"}</span><b>{hm(st?.diff ?? 0)} h</b></div>
          <div><span>Überstunden gesamt{v && !v.connected ? " (geschätzt)" : ""}</span><b className={(st?.balance ?? 0) >= 0 ? "pos" : "neg"}>{signed(st?.balance ?? 0)}</b></div>
        </div>
        {v && !v.connected && !v.unauthorized && (
          <div className="callout warn">
            <b>Offline – die Zeiterfassung läuft weiter.</b>
            <span>
              {v.pending > 0 ? `${v.pending} Ereignis${v.pending === 1 ? "" : "se"} warten und werden automatisch übertragen, sobald der Server wieder erreichbar ist.` : "Neue Ereignisse werden übertragen, sobald der Server wieder erreichbar ist."}
              {v.estimated ? " Das Überstundenkonto ist bis dahin geschätzt." : ""}
            </span>
          </div>
        )}
        {v?.error && v.connected && !v.unauthorized && <div className="callout warn">{v.error}</div>}
        <div className="row">
          {running ? (
            <button className="btn grow" onClick={() => act("pause")}>Tracking pausieren</button>
          ) : (
            track !== "asking" && <button className="btn primary grow" onClick={() => act("resume")}>{track === "paused" ? "Tracking fortsetzen" : "Tracking starten"}</button>
          )}
          {(running || track === "paused") && <button className="btn" onClick={() => act("end")} title="Arbeitsende setzen – die App läuft weiter">Tracking beenden</button>}
        </div>
        <div className="row">
          <button className="btn grow" onClick={() => api.dashboard()}>Dashboard</button>
          <button className="btn grow" onClick={() => api.dashboard("settings")}>Einstellungen</button>
        </div>
        <div className="opts">
          <label className="opt">
            <span>
              Mit {s.os === "macos" ? "macOS" : "Windows"} starten
              <small>Der Arbeitsbeginn wird beim Hochfahren automatisch erkannt.</small>
            </span>
            <button type="button" role="switch" aria-checked={s.autostart} className={`switch ${s.autostart ? "on" : ""}`} onClick={() => change(() => api.setAutostart(!s.autostart))} />
          </label>
          <label className="opt">
            <span>
              Tracking automatisch starten
              <small>Beim Start in einem Zeitfenster (Einstellungen) startet das Tracking auf diesem Gerät von selbst. Aus: Du startest es selbst.</small>
            </span>
            <button type="button" role="switch" aria-checked={st?.policy.deviceAuto ?? true} className={`switch ${st?.policy.deviceAuto ?? true ? "on" : ""}`} disabled={!v?.connected} title={v?.connected ? undefined : "Braucht eine Verbindung zum Server"} onClick={() => change(() => api.setAutoTrack(!(st?.policy.deviceAuto ?? true)))} />
          </label>
        </div>
        {err && <div className="callout bad">{err}</div>}
        <div className="foot">
          <div className="muted xs">
            {s.userName} · {s.deviceName} · {s.os === "macos" ? "macOS" : "Windows"}<br />
            Version {s.version}
          </div>
          <div className="row">
            <button className="btn sm danger" onClick={() => api.quit()}>Beenden</button>
          </div>
        </div>
        <p className="muted xs">Fenster schließen (✕) lässt den Companion im Hintergrund weiterlaufen. <b>Beenden</b> beendet das Tracking.</p>
      </div>
    </>
  );
}
