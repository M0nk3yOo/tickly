import { useEffect, useRef, useState } from "react";
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
  paused: { text: "Tracking gestoppt", tone: "warn" },
  asking: { text: "Rückfrage", tone: "warn" },
  idle: { text: "Tracking aus", tone: "bad" },
  stopped: { text: "Companion nicht aktiv", tone: "bad" },
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

function Pair({ s, set }: { s: UiState; set: (s: UiState) => void }) {
  const [server, setServer] = useState(s.server || "");
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
        <label>
          Server-Adresse
          <input value={server} onChange={(e) => setServer(e.target.value)} placeholder="zeit.example.com" autoCapitalize="off" spellCheck={false} />
        </label>
        <label>
          Kopplungscode
          <input className="code" value={code} onChange={(e) => setCode(e.target.value.toUpperCase())} placeholder="ABCD-EFGH" maxLength={9} spellCheck={false} onKeyDown={(e) => e.key === "Enter" && server && code && go()} />
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
  const act = async (k: "pause" | "resume" | "answer", val?: string) => set(await api.act(k, val));
  const pct = st && st.soll > 0 ? Math.min(100, (st.counted / st.soll) * 100) : 0;
  const fresh = !st?.start;
  // Sekunden laufen im Fenster weiter, zwischen zwei Meldungen des Servers (alle 30 s)
  const stamp = useRef(Date.now());
  const [, setTick] = useState(0);
  useEffect(() => { stamp.current = Date.now(); }, [s]);
  useEffect(() => { const t = setInterval(() => setTick((n) => n + 1), 1000); return () => clearInterval(t); }, []);
  const live = (st?.counted ?? 0) + (track === "running" ? (Date.now() - stamp.current) / 60000 : 0);
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
        {st?.mfa?.required && !st.mfa.compliant && (
          <div className="callout warn">
            <b>Bitte Konto absichern.</b>
            <span>
              Dein Administrator verlangt einen Passkey oder eine Authenticator-App. {st.mfa.expired ? "Die Frist ist abgelaufen." : `Noch ${st.mfa.daysLeft ?? 0} Tag${st.mfa.daysLeft === 1 ? "" : "e"}.`} Richte es in der Weboberfläche unter Einstellungen → Sicherheit ein.
            </span>
            <button className="btn sm" onClick={() => api.dashboard()}>Weboberfläche öffnen</button>
          </div>
        )}
        {track === "asking" && (
          <div className="ask">
            <h2>Arbeitest du gerade?</h2>
            <p>Der Rechner läuft außerhalb deiner automatischen Zeitfenster. Soll die Zeiterfassung laufen?</p>
            <div className="row">
              <button className="btn primary" onClick={() => act("answer", "work")}>Ja, starten</button>
              {fresh && <button className="btn" onClick={() => act("answer", "boot")}>Ja, ab Rechnerstart</button>}
              <button className="btn" onClick={() => act("answer", "no")}>Nein</button>
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
            <button className="btn grow" onClick={() => act("pause")}>Tracking stoppen</button>
          ) : (
            track !== "asking" && <button className="btn primary grow" onClick={() => act("resume")}>{track === "paused" ? "Fortsetzen" : "Tracking starten"}</button>
          )}
          <button className="btn" onClick={() => api.dashboard()}>Dashboard</button>
        </div>
        <div className="foot">
          <div className="muted xs">
            {s.userName} · {s.deviceName} · {s.os === "macos" ? "macOS" : "Windows"}<br />
            Version {s.version} · {st?.policy.autostart ? "startet mit dem Rechner" : "startet nicht automatisch"}
          </div>
          <div className="row">
            <button className="btn sm danger" onClick={() => api.quit()}>Beenden</button>
          </div>
        </div>
        <p className="muted xs">Fenster schließen (✕) lässt den Companion im Hintergrund weiterlaufen. <b>Beenden</b> stoppt die Zeiterfassung.</p>
      </div>
    </>
  );
}
