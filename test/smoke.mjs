// Browser smoke test: loads dist/corrode.js into the fake-Discord harness in Chromium,
// early and late, and checks boot, plugins, the settings UI and unload.
// Usage: npm run build && npm run smoke   (set SCREENSHOTS=1 to save screenshots to test/artifacts)
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { mkdirSync, readFileSync } from "node:fs";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const root = fileURLToPath(new URL("..", import.meta.url));
const types = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json" };

const server = createServer((req, res) => {
  let path = decodeURIComponent(new URL(req.url, "http://x").pathname);
  if (path.endsWith("/")) path += "index.html";
  const file = normalize(join(root, path));
  if (!file.startsWith(root)) return res.writeHead(403).end();
  try {
    const body = readFileSync(file);
    res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream" }).end(body);
  } catch {
    res.writeHead(404).end();
  }
}).listen(0);
const base = `http://127.0.0.1:${server.address().port}`;

const assert = (cond, msg) => {
  if (!cond) throw new Error(`Assertion failed: ${msg}`);
};

const browser = await chromium.launch();
let failed = false;
try {
  for (const mode of ["early", "late"]) {
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
    const errors = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
    await page.goto(`${base}/test/harness/?mode=${mode}`);
    await page.waitForFunction(() => window.corrode);
    await page.evaluate(() => window.corrode.ready);

    const info = await page.evaluate(() => ({
      loadMode: window.corrode.loadMode,
      running: window.corrode.plugins.list().filter((p) => p.running).map((p) => p.id),
      user: window.corrode.common.getCurrentUser()?.username,
    }));
    assert(info.loadMode === mode, `load mode ${info.loadMode} !== ${mode}`);
    assert(info.user === "me", "common.UserStore resolved");

    // Enable Silent Typing and check the patch works through the real bundle.
    const typed = await page.evaluate(async () => {
      await window.corrode.plugins.enable("silentTyping");
      const T = window.corrode.webpack.findByProps("startTyping");
      T.startTyping("123");
      const whileOn = T.calls.length;
      window.corrode.plugins.disable("silentTyping");
      T.startTyping("123");
      return [whileOn, T.calls.length];
    });
    assert(typed[0] === 0 && typed[1] === 1, `silentTyping patch: ${typed}`);

    // Keyword Notify -> toast via flux.
    await page.evaluate(async () => {
      await window.corrode.plugins.enable("keywordNotify");
      window.corrode.plugins.settingsStore("keywordNotify").set("keywords", "deploy");
      window.corrode.flux.dispatch({ type: "MESSAGE_CREATE", message: { content: "deploy is done", author: { id: "2", username: "ana" }, channel_id: "9" } });
    });
    const toast = await page.evaluate(() => document.querySelector("corrode-toasts")?.shadowRoot?.textContent ?? "");
    assert(toast.includes("deploy is done"), `toast shown: ${toast}`);

    // A settings tab can be embedded in a container (used for the pages inside Discord's settings).
    const embedded = await page.evaluate(() => {
      const box = document.body.appendChild(document.createElement("div"));
      const remove = window.corrode.ui.mountSettings(box, "plugins");
      const root = box.querySelector("corrode-settings").shadowRoot;
      const out = { cards: root.querySelectorAll(".card").length, close: !!root.querySelector(".close"), nav: !!root.querySelector("nav") };
      remove();
      box.remove();
      return { ...out, removed: !document.querySelector("corrode-settings") };
    });
    assert(embedded.cards === 4 && !embedded.close && !embedded.nav && embedded.removed, `embedded settings: ${JSON.stringify(embedded)}`);

    // Sidebar entry injected into the fake settings sidebar, and it opens the window.
    await page.waitForSelector("[data-corrode-entry][role=tab]");
    await page.click("[data-corrode-entry][role=tab]");
    await page.waitForSelector("corrode-settings", { state: "attached" });
    const cards = await page.evaluate(() => document.querySelector("corrode-settings").shadowRoot.querySelectorAll(".card").length);
    assert(cards === 4, `plugin cards: ${cards}`);

    // Expand a plugin's settings.
    await page.locator("corrode-settings .card[data-plugin=hideClutter] button:has-text('Settings')").click();
    if (process.env.SCREENSHOTS) {
      mkdirSync(join(root, "test/artifacts"), { recursive: true });
      await page.screenshot({ path: join(root, `test/artifacts/settings-${mode}.png`) });
      await page.locator("corrode-settings .tab:has-text('About')").click();
      await page.screenshot({ path: join(root, `test/artifacts/about-${mode}.png`) });
    }
    await page.keyboard.press("Escape");
    assert(!(await page.$("corrode-settings")), "Escape closes settings");

    // Updates: the startup check announces the update, and the Updates page installs it.
    const notice = await page.evaluate(() => document.querySelector("corrode-toasts")?.shadowRoot?.textContent ?? "");
    assert(notice.includes("An update for Corrode is available"), `update toast: ${notice}`);
    await page.evaluate(() => window.corrode.ui.openSettings("updates"));
    const upd = page.locator("corrode-settings .card[data-update=corrode]");
    await upd.locator("text=Update available").waitFor();
    assert((await page.locator("corrode-settings .card[data-update=client]").textContent()).includes("Up to date"), "client is up to date");
    await upd.locator("button:has-text('Update Corrode')").click();
    await upd.locator("text=Restart to finish").waitFor();
    assert(await upd.locator("button:has-text('Restart now')").count(), "restart button shown");
    if (process.env.SCREENSHOTS) await page.screenshot({ path: join(root, `test/artifacts/updates-${mode}.png`) });
    await page.keyboard.press("Escape");

    // Shortcut opens it.
    await page.keyboard.press("Control+Shift+KeyO");
    assert(await page.$("corrode-settings"), "shortcut opens settings");
    await page.keyboard.press("Escape");

    // Unload removes everything.
    const leftovers = await page.evaluate(() => {
      window.corrode.unload();
      return {
        global: "corrode" in window,
        styles: document.querySelectorAll("style[data-corrode]").length,
        entry: document.querySelectorAll("[data-corrode-entry]").length,
        typingPatched: !!window.webpackChunkdiscord_app && Object.getOwnPropertyDescriptor(window.webpackChunkdiscord_app, "push")?.get !== undefined,
      };
    });
    assert(!leftovers.global && !leftovers.styles && !leftovers.entry && !leftovers.typingPatched, `unload leftovers: ${JSON.stringify(leftovers)}`);

    assert(!errors.length, `page errors: ${errors.join("\n")}`);
    console.log(`✓ ${mode} load (${info.running.length} plugins running at boot)`);
    await page.close();
  }
} catch (err) {
  failed = true;
  console.error("✗", err.message);
} finally {
  await browser.close();
  server.close();
}
process.exit(failed ? 1 : 0);
