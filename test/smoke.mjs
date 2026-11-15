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
    assert(embedded.cards === 200 && !embedded.close && !embedded.nav && embedded.removed, `embedded settings: ${JSON.stringify(embedded)}`);

    // Sidebar entry injected into the fake settings sidebar, and it opens the window.
    await page.waitForSelector("[data-corrode-entry][role=tab]");
    await page.click("[data-corrode-entry][role=tab]");
    await page.waitForSelector("corrode-settings", { state: "attached" });
    const cards = await page.evaluate(() => document.querySelector("corrode-settings").shadowRoot.querySelectorAll(".card").length);
    assert(cards === 200, `plugin cards: ${cards}`);

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

  // Every built-in plugin at once: all 200 must start, do their job together and clean up.
  {
    const page = await browser.newPage({ viewport: { width: 1200, height: 800 } });
    const errors = [];
    page.on("pageerror", (e) => errors.push(String(e)));
    page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
    await page.goto(`${base}/test/harness/?mode=early`);
    await page.waitForFunction(() => window.corrode);
    await page.evaluate(() => window.corrode.ready);

    const baseStyles = await page.evaluate(() => document.querySelectorAll("style[data-corrode]").length);
    const started = await page.evaluate(async () => {
      const { plugins } = window.corrode;
      for (const p of plugins.list()) await plugins.enable(p.id);
      // Keep the test quick: no send delay.
      plugins.settingsStore("undoSend").set("delay", 1);
      const list = plugins.list();
      return { total: list.length, running: list.filter((p) => p.running).length, failed: list.filter((p) => p.error).map((p) => `${p.id}: ${p.error}`) };
    });
    assert(started.total >= 200, `built-in plugins: ${started.total}`);
    assert(started.running === started.total && !started.failed.length, `all plugins start: ${started.running}/${started.total} ${started.failed.join("; ")}`);

    // Page plugins marked the chat rows.
    await page.waitForSelector("#chat-messages-10-101[data-corrode-own]");
    const marks = await page.evaluate(() => ({
      own: document.querySelectorAll("[data-corrode-own]").length,
      friend: document.querySelector("#chat-messages-10-103").hasAttribute("data-corrode-friend"),
      bot: document.querySelector("#chat-messages-10-102").hasAttribute("data-corrode-bot"),
      time: document.querySelector("#chat-messages-10-101 time").getAttribute("data-corrode-time"),
      ago: !!document.querySelector("#chat-messages-10-101 time").getAttribute("data-corrode-ago"),
      masked: document.querySelector("#message-content-102 a").getAttribute("data-corrode-masked"),
      scam: document.querySelector("#message-content-102 a").hasAttribute("data-corrode-scam"),
      age: document.querySelector("#message-username-103").title,
      loop: document.querySelector("video").loop,
      spellcheck: document.querySelector("[role=textbox]").getAttribute("spellcheck"),
      styles: document.querySelectorAll("style[data-corrode]").length,
    }));
    assert(marks.own === 1 && marks.friend && marks.bot, `row marks: ${JSON.stringify(marks)}`);
    assert(/^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d$/.test(marks.time) && marks.ago, `timestamps: ${JSON.stringify(marks)}`);
    assert(marks.masked === "goes to dlscord-gift.com" && marks.scam, `link warnings: ${JSON.stringify(marks)}`);
    assert(marks.age.startsWith("Account created") && marks.loop && marks.spellcheck === "false", `page tweaks: ${JSON.stringify(marks)}`);
    assert(marks.styles > 100, `stylesheets: ${marks.styles}`);

    // Sending goes through every transform together.
    const sent = await page.evaluate(async () => {
      const M = window.corrode.webpack.findByProps("sendMessage", "editMessage");
      const reminder = await M.sendMessage("10", { content: "!remind 1h stretch" });
      await M.sendMessage("10", { content: "teh  link https://a.com/p?utm_source=x&id=1 ;shrug {{2+2}} gh:user/repo [[now]]" });
      return { reminder, sent: M.sent };
    });
    assert(sent.sent.length === 1, `one message sent, the reminder was kept back: ${JSON.stringify(sent.sent)}`);
    const text = sent.sent[0].content;
    for (const part of ["@silent ", "https://a.com/p?id=1", "¯", " 4 ", "https://github.com/user/repo", "<t:"]) assert(text.includes(part), `sent text has ${JSON.stringify(part)}: ${text}`);
    assert(!text.includes("utm_source") && !text.includes("  "), `sent text cleaned: ${text}`);

    // Events: a DM, a ghost ping, a kept deletion and hidden bot messages.
    const events = await page.evaluate(async () => {
      const { flux } = window.corrode;
      const seen = [];
      const stop = flux.subscribe("MESSAGE_CREATE", (a) => seen.push(a.message.id));
      const stopDelete = flux.subscribe("MESSAGE_DELETE", (a) => seen.push(`deleted ${a.id}`));
      flux.dispatch({ type: "MESSAGE_CREATE", message: { id: "200", channel_id: "9", author: { id: "2", username: "ana" }, content: "psst", mentions: [], mention_everyone: true } });
      flux.dispatch({ type: "MESSAGE_CREATE", message: { id: "201", channel_id: "10", guild_id: "50", author: { id: "3", username: "helper", bot: true }, content: "beep", mentions: [] } });
      flux.dispatch({ type: "MESSAGE_DELETE", channelId: "10", id: "103" });
      flux.dispatch({ type: "MESSAGE_DELETE", channelId: "10", id: "101" });
      flux.dispatch({ type: "TYPING_START", channelId: "9", userId: "2" });
      flux.dispatch({ type: "VOICE_STATE_UPDATES", voiceStates: [{ userId: "2", channelId: "77" }] });
      flux.dispatch({ type: "PRESENCE_UPDATES", updates: [{ user: { id: "2" }, status: "online", activities: [] }] });
      stop();
      stopDelete();
      await new Promise((r) => setTimeout(r, 100));
      return { seen, toasts: document.querySelector("corrode-toasts")?.shadowRoot?.textContent ?? "" };
    });
    assert(events.seen.join() === "200,deleted 101", `filters: bot message hidden, others' deletions kept, own deletion passes: ${events.seen}`);
    for (const part of ["psst", "Ghost ping from Ana", "Ana joined", "Ana is online"]) assert(events.toasts.includes(part), `notice ${JSON.stringify(part)} in: ${events.toasts}`);

    // Turning everything off leaves nothing behind.
    const after = await page.evaluate(() => {
      const { plugins } = window.corrode;
      for (const p of plugins.list()) plugins.disable(p.id);
      const leftovers = [...document.querySelectorAll("*")].flatMap((el) => [...el.attributes].filter((a) => a.name.startsWith("data-corrode") && a.name !== "data-corrode-entry" && el.tagName !== "STYLE" && !el.tagName.startsWith("CORRODE")).map((a) => `${el.tagName}[${a.name}]`));
      return { running: plugins.list().filter((p) => p.running).length, styles: document.querySelectorAll("style[data-corrode]").length, leftovers, floats: document.querySelectorAll("[data-corrode-float]").length, title: document.querySelector("#message-username-103").title, fetchPatched: window.corrode.patcher.isPatched(window.fetch) };
    });
    assert(after.running === 0 && after.styles === baseStyles && !after.leftovers.length && !after.floats && !after.title && !after.fetchPatched, `clean stop: ${JSON.stringify(after)}`);

    assert(!errors.length, `page errors: ${errors.join("\n")}`);
    console.log(`✓ all ${started.total} built-in plugins start, work together and stop cleanly`);
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
