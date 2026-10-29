// Erzeugt die Screenshots für die Dokumentation aus der laufenden Demo (scripts/dev.sh).
//
//   mkdir /tmp/shots && cd /tmp/shots && npm init -y && npm i puppeteer-core
//   scripts/dev.sh                         # in einem anderen Terminal, mit frischer Demo-Datenbank
//   (cd apps/companion && npm run dev)     # für die Bilder der Companion-App
//   node /pfad/zu/scripts/screenshots.mjs
//
// Benötigt Chrome oder Edge (CHROME=/Pfad/zum/Browser).
import { createRequire } from "module";
import path from "path";
const puppeteer = createRequire(process.cwd() + "/")("puppeteer-core");
const BASE = process.env.BASE || "http://localhost:3000";
const COMP = process.env.COMPANION || "http://localhost:1420";
const OUT = process.env.OUT || path.resolve(import.meta.dirname, "../docs/screenshots");
const CHROME = process.env.CHROME || "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const MAC = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";
const WIN = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 Edg/130.0";
const FF = "Mozilla/5.0 (X11; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0";

async function call(method, p, body, cookie, h = {}) {
  const r = await fetch(BASE + "/api" + p, { method, headers: { "content-type": "application/json", ...(cookie ? { cookie } : {}), ...h }, body: body === undefined ? undefined : JSON.stringify(body) });
  const j = await r.json().catch(() => ({}));
  return { status: r.status, json: j, cookie: (r.headers.get("set-cookie") || "").split(";")[0] };
}
const login = (email, ip, country, ua) => call("POST", "/auth/login", { email, password: "demo-passwort" }, null, { "cf-connecting-ip": ip, "cf-ipcountry": country, "user-agent": ua });

/* ---------- Demo-Daten anreichern ---------- */
let ADMIN_COOKIE = '';
async function seed() {
  const admin = await login("andre@example.de", "203.0.113.10", "DE", MAC);
  await login("lena.hoffmann@example.de", "198.51.100.23", "DE", WIN);
  await login("lena.hoffmann@example.de", "198.51.100.24", "DE", MAC);
  await login("sophie.wagner@example.de", "192.0.2.77", "AT", MAC);
  await login("murat.yilmaz@example.de", "198.51.100.88", "TR", FF);
  // Fehlversuche → Sperre
  for (let i = 0; i < 5; i++) await call("POST", "/auth/login", { email: "andre@example.de", password: "falsch" }, null, { "cf-connecting-ip": "185.220.101.4", "cf-ipcountry": "NL", "user-agent": FF });
  await call("POST", "/auth/login", { email: "unbekannt@example.com", password: "x" }, null, { "cf-connecting-ip": "203.0.113.41", "cf-ipcountry": "FR", "user-agent": FF });
  // Companion-Geräte (eines veraltet)
  const pair = async (email, name, os, version) => {
    const s = await login(email, "198.51.100.23", "DE", WIN);
    const code = (await call("POST", "/devices/pair-code", {}, s.cookie)).json.code;
    await call("POST", "/device/pair", { code, name, os, version });
    await call("POST", "/auth/logout", {}, s.cookie); // die Sitzung zum Koppeln soll in der Übersicht nicht auftauchen
  };
  await pair("lena.hoffmann@example.de", "NB-LENA-01", "windows", "0.8.2");
  await pair("sophie.wagner@example.de", "MacBook Pro von Sophie", "macos", "0.9.6");
  await pair("andre@example.de", "DESKTOP-ANDRE", "windows", "0.9.6");
  // E-Mail, Sicherung, manuelle Zeit
  await call("PUT", "/admin/smtp", { enabled: true, host: "smtp.example.com", port: 587, security: "starttls", username: "zeit@example.com", newPassword: "demo", fromEmail: "zeit@example.com", fromName: "Tickly", notifyAdminRegistration: true, notifyAdminBlock: true, notifyUserApproved: true }, admin.cookie);
  await call("POST", "/admin/backups", {}, admin.cookie);
  const snap = (await call("GET", "/snapshot?now=872", undefined, admin.cookie)).json;
  const d = snap.days.find((x) => x.date === "2026-10-21");
  const segs = d.segs.filter((g) => g.kind !== "off").map((g) => ({ from: g.from, to: g.to, kind: g.kind, app: g.app, note: g.note }));
  segs.push({ from: 1050, to: 1110, kind: "app", app: "Manuell", note: "Kundentermin vor Ort" });
  await call("PUT", "/days/2026-10-21", { segs }, admin.cookie);
  ADMIN_COOKIE = admin.cookie;
}

/* ---------- Bilder ---------- */
async function main() {
  await seed();
  const browser = await puppeteer.launch({ executablePath: CHROME, headless: "new", args: ["--no-sandbox", "--hide-scrollbars"] });
  const shot = async (name, url, { w = 1440, h = 900, full = true, act, theme, base = BASE, switches = true } = {}) => {
    const page = await browser.newPage();
    await page.setViewport({ width: w, height: h, deviceScaleFactor: 1 });
    // Eine Sitzung für alle Bilder, sonst füllt sich die Sitzungsliste mit Testanmeldungen
    if (base === BASE && !url.includes("index.html?theme=light#") && !/^\/index\.html\?theme=light$/.test(url)) {
      const [k, v] = ADMIN_COOKIE.split("="); await page.setCookie({ name: k, value: v, url: BASE });
    }
    await page.evaluateOnNewDocument((sw) => { localStorage.setItem("tickly.switches", sw ? "1" : "0"); }, switches);
    await page.goto(base + url, { waitUntil: "networkidle0" });
    await page.addStyleTag({ content: "#demo{display:none!important}" });
    await sleep(900);
    if (act) { await page.evaluate(act); await sleep(900); }
    if (full) {
      // Seitenhöhe messen und Fenster entsprechend vergrößern, damit auch die Seitenleiste bis unten reicht
      const ph = await page.evaluate(() => Math.max(document.body.scrollHeight, document.documentElement.scrollHeight));
      if (ph > h) { await page.setViewport({ width: w, height: Math.min(ph, 2600), deviceScaleFactor: 1 }); await sleep(300); }
    }
    await page.screenshot({ path: path.join(OUT, name + ".png"), fullPage: false });
    await page.close();
    console.log("✓", name);
  };
  const auto = "x=1";
  const th = (t = "light") => `theme=${t}`;
  await shot("01-anmeldung", `/index.html?${th()}`, { full: false });
  await shot("02-uebersicht", `/index.html?${auto}&${th()}#/dashboard`, { full: false });
  await shot("03-tagesansicht", `/index.html?${auto}&${th()}#/day`);
  await shot("04-tagesansicht-bearbeiten", `/index.html?${auto}&${th()}&day=2026-10-21#/day`, { full: false, h: 1000, act: "document.querySelector('[data-a=dayEdit]').click()" });
  await shot("05-statistik-woche", `/index.html?${auto}&${th()}&stat=week#/stats`);
  await shot("06-statistik-monat", `/index.html?${auto}&${th()}&stat=month#/stats`);
  await shot("07-abwesenheiten", `/index.html?${auto}&${th()}#/absences`, { full: false });
  await shot("08-einstellungen-arbeitszeit", `/index.html?${auto}&${th()}&set=work#/settings`, { full: false });
  await shot("09-einstellungen-tracking", `/index.html?${auto}&${th()}&set=trk#/settings`);
  await shot("10-einstellungen-sicherheit", `/index.html?${auto}&${th()}&set=sec#/settings`);
  await shot("11-admin-benutzer", `/index.html?${auto}&${th()}&admin=users#/admin`);
  await shot("12-admin-richtlinien", `/index.html?${auto}&${th()}&admin=reg#/admin`);
  await shot("13-admin-sperren", `/index.html?${auto}&${th()}&admin=sec#/admin`, { full: false });
  await shot("14-admin-sitzungen", `/index.html?${auto}&${th()}&admin=sess#/admin`);
  await shot("15-admin-protokoll", `/index.html?${auto}&${th()}&admin=log#/admin`);
  await shot("16-admin-email", `/index.html?${auto}&${th()}&admin=mail#/admin`, { full: false });
  await shot("17-admin-sicherung", `/index.html?${auto}&${th()}&admin=backup#/admin`);
  await shot("18-companion-seite", `/index.html?${auto}&${th()}#/companion`, { full: false });
  await shot("19-dunkel", `/index.html?${auto}&${th("dark")}#/dashboard`, { full: false });
  await shot("20-dunkel-tagesansicht", `/index.html?${auto}&${th("dark")}#/day`);
  // Companion-App (Vorschau der Oberfläche mit Beispieldaten)
  for (const [n, q] of [["21-app-laeuft", "track=running"], ["22-app-rueckfrage", "track=asking"], ["23-app-offline", "track=running&offline=1"], ["24-app-koppeln", "paired=0"]]) {
    await shot(n, `/?${q}`, { w: 420, h: 640, full: false, base: COMP });
  }
  await browser.close();
}
main().catch((e) => { console.error(e); process.exit(1); });
