import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Track = "running" | "locked" | "paused" | "asking" | "idle" | "stopped" | "";

export interface ServerState {
  track: Track;
  tracking: boolean;
  start: number | null;
  bootAt?: number | null;
  since: number | null;
  counted: number;
  soll: number;
  diff: number;
  balance: number;
  windowActive: boolean;
  dayOff: boolean;
  holiday: string | null;
  nightCheck?: { since: number; hours: number } | null;
  deviceClash?: { starter: string; other: string } | null;
  /** Offene Rückfrage nach einer längeren Sperre (Minuten seit Mitternacht) */
  away?: { from: number; to: number } | null;
  mfa?: { required: boolean; compliant: boolean; daysLeft: number | null; expired: boolean } | null;
  settings?: { theme?: "system" | "light" | "dark" } | null;
  policy: { trackApps: boolean; trackLock: boolean; autoTrack: { enabled: boolean; ask: boolean }; deviceAuto: boolean };
}

export interface View {
  connected: boolean;
  unauthorized: boolean;
  error: string | null;
  pending: number;
  estimated: boolean;
  locked: boolean;
  app: string | null;
  /** Zeitpunkt (Unix-ms), auf den sich state.counted bezieht */
  asOf: number;
  state: ServerState;
}

/** Neuere Version laut Server; `download` ist der Installer für dieses Betriebssystem */
export interface UpdateInfo {
  latest: string;
  notes: string;
  url: string;
  download: string;
  size?: number | null;
}

export interface UiState {
  paired: boolean;
  server: string;
  deviceName: string;
  userName: string;
  version: string;
  os: string;
  /** Startet der Companion mit dem Rechner? (Einstellung dieses Rechners, nicht des Servers) */
  autostart: boolean;
  /** Neuere Version verfügbar (nur gesetzt, wenn sie höher ist als diese) */
  update?: UpdateInfo | null;
  view: View | null;
}

const inTauri = "__TAURI_INTERNALS__" in window;

/** Vorschau im Browser (ohne Tauri): feste Beispieldaten, damit sich die Oberfläche entwickeln lässt. */
const mock: { s: UiState } = {
  s: {
    paired: new URLSearchParams(location.search).get("paired") !== "0",
    server: "https://zeit.example.com",
    deviceName: "DESKTOP-ANDRE",
    userName: "André Becker",
    version: "0.9.18",
    os: "windows",
    autostart: true,
    update: new URLSearchParams(location.search).get("update") === "1" ? { latest: "0.9.19", notes: "", url: "https://github.com/M0nk3yOo/tickly/releases/latest", download: "https://github.com/M0nk3yOo/tickly/releases/latest", size: 4_200_000 } : null,
    view: {
      connected: new URLSearchParams(location.search).get("offline") !== "1", unauthorized: false, error: null, pending: new URLSearchParams(location.search).get("offline") === "1" ? 14 : 0, estimated: new URLSearchParams(location.search).get("offline") === "1", locked: false, app: "VS Code", asOf: Date.now(),
      state: {
        track: (new URLSearchParams(location.search).get("track") as Track) || "running", tracking: true, start: new URLSearchParams(location.search).get("track") === "asking" ? null : 472, bootAt: 472, since: 842,
        counted: 357, soll: 480, diff: -123, balance: 1193, windowActive: true, dayOff: false, holiday: null,
        away: new URLSearchParams(location.search).get("away") === "1" ? { from: 725, to: 768 } : null,
        policy: { trackApps: true, trackLock: true, autoTrack: { enabled: true, ask: true }, deviceAuto: true },
      },
    },
  },
};

export const api = {
  get: (): Promise<UiState> => (inTauri ? invoke("get_view") : Promise.resolve(mock.s)),
  pair: (server: string, code: string): Promise<UiState> => (inTauri ? invoke("pair", { server, code }) : Promise.resolve(mock.s)),
  unpair: (): Promise<UiState> => (inTauri ? invoke("unpair") : Promise.resolve({ ...mock.s, paired: false })),
  act: (kind: "pause" | "resume" | "end" | "answer" | "away", value?: string): Promise<UiState> => (inTauri ? invoke("act", { kind, value }) : Promise.resolve(mock.s)),
  /** Weboberfläche im Browser öffnen, auf Wunsch gleich die Einstellungen */
  dashboard: (page?: "settings"): Promise<void> => (inTauri ? invoke("open_dashboard", { page }) : Promise.resolve()),
  /** Installer der neueren Version (oder mit notes die Versionshinweise) im Browser öffnen */
  openUpdate: (notes = false): Promise<void> => (inTauri ? invoke("open_update", { notes }) : Promise.resolve()),
  setAutostart: (on: boolean): Promise<UiState> => (inTauri ? invoke("set_autostart", { on }) : Promise.resolve({ ...mock.s, autostart: on })),
  setAutoTrack: (on: boolean): Promise<UiState> => (inTauri ? invoke("set_auto_track", { on }) : Promise.resolve(mock.s)),
  quit: (): Promise<void> => (inTauri ? invoke("quit_app") : Promise.resolve()),
  onState: (f: (s: UiState) => void): Promise<() => void> => (inTauri ? listen<UiState>("state", (e) => f(e.payload)) : Promise.resolve(() => {})),
};
