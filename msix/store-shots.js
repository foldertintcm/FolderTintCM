// Renders the Microsoft Store screenshots from store-shots.html, 1920 × 1080,
// through headless Edge's DevTools protocol (Node 22: fetch and WebSocket
// built in, nothing to install).
//
//   node msix/store-shots.js <msedge.exe> <base url of PRY.FolderTintCM> <out dir>
//
// The project's root must be served over HTTP (the page loads the website's
// images); out dir gets es/ and en/, with 01-menu … 04-folders.
const { spawn } = require("child_process");
const fs = require("fs");
const path = require("path");

const [edge, base, out] = process.argv.slice(2);
const SHOTS = ["menu", "picker", "light", "folders"];
const port = 9334;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const profile = fs.mkdtempSync(path.join(require("os").tmpdir(), "foldertintcm-shots-"));
const browser = spawn(edge, ["--headless=new", "--disable-gpu", "--hide-scrollbars",
  `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "about:blank"], { stdio: "ignore" });

(async () => {
  try {
    let target;
    for (let i = 0; i < 50 && !target; i++) {
      await sleep(200);
      try {
        target = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page");
      } catch { /* not up yet */ }
    }
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((r) => ws.addEventListener("open", r, { once: true }));
    let id = 0;
    const pending = new Map();
    ws.addEventListener("message", (e) => {
      const m = JSON.parse(e.data);
      if (m.id && pending.has(m.id)) { pending.get(m.id)(m.result); pending.delete(m.id); }
    });
    const send = (method, params = {}) => new Promise((r) => {
      const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params }));
    });
    await send("Emulation.setDeviceMetricsOverride", { width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false });
    await send("Page.enable");
    for (const lang of ["es", "en"]) {
      fs.mkdirSync(path.join(out, lang), { recursive: true });
      for (const [i, shot] of SHOTS.entries()) {
        await send("Page.navigate", { url: `${base}/app/msix/store-shots.html?shot=${shot}&lang=${lang}` });
        await sleep(1500);
        const png = await send("Page.captureScreenshot", { format: "png" });
        const file = path.join(out, lang, `0${i + 1}-${shot}.png`);
        fs.writeFileSync(file, Buffer.from(png.data, "base64"));
        console.log(file);
      }
    }
    ws.close();
  } finally {
    browser.kill();
  }
})();
