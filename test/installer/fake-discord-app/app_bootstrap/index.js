// A stand-in for Discord's desktop main process, packed into app.asar by test/installer-e2e.mjs.
// It opens a "Discord" window with its own preload (like Discord's mainScreenPreload), serves the
// fake Discord page from test/harness, checks what the page sees and writes that to $FAKE_DISCORD_RESULT.
"use strict";
const { app, BrowserWindow } = require("electron");
const http = require("http");
const fs = require("fs");
const path = require("path");

const harness = process.env.FAKE_DISCORD_HARNESS;
const resultFile = process.env.FAKE_DISCORD_RESULT;
const done = (result) => {
  fs.writeFileSync(resultFile, JSON.stringify(result, null, 2));
  app.quit();
};
setTimeout(() => done({ error: "timed out" }), 40000);

// The test page: test/harness/index.html without its fake CorrodeNative and its own copy of Corrode.
const page = fs
  .readFileSync(path.join(harness, "index.html"), "utf8")
  .replace(/<script>\s*window\.CorrodeNative[\s\S]*?<\/script>/, "")
  .replace(/if \(new URLSearchParams\(location\.search\)\.get\("mode"\) === "late"\) \{[\s\S]*?\n    \}/, "");

app.whenReady().then(() => {
  const server = http.createServer((req, res) => {
    if (req.url.startsWith("/fake-discord.js")) {
      res.writeHead(200, { "content-type": "text/javascript" });
      return res.end(fs.readFileSync(path.join(harness, "fake-discord.js")));
    }
    res.writeHead(200, { "content-type": "text/html" });
    res.end(page);
  });
  server.listen(0, "127.0.0.1", () => {
    const win = new BrowserWindow({
      title: "Discord",
      show: false,
      webPreferences: { preload: path.join(__dirname, "mainScreenPreload.js"), contextIsolation: true, sandbox: true },
    });
    win.webContents.on("console-message", (_e, _level, message) => console.log("[page]", message));
    win.webContents.once("did-finish-load", async () => {
      try {
        const result = await win.webContents.executeJavaScript(`(async () => {
          const r = { discordNative: window.DiscordNative?.isFake === true, corrode: !!window.corrode };
          if (!window.corrode) return r;
          await window.corrode.ready;
          r.loadMode = window.corrode.loadMode;
          const n = window.CorrodeNative;
          r.nativeName = n?.name;
          r.nativeVersion = n?.version;
          if (!n) return r;
          n.storage.set("corrode:test", "hello");
          r.storage = n.storage.get("corrode:test");
          await n.themes.write("Test.theme.css", "/** @name Test */ body { color: red; }");
          r.themes = (await n.themes.list()).files.map((f) => f.file);
          r.themeRead = await n.themes.read("Test.theme.css");
          r.badName = await n.themes.read("../settings.json").then(() => "allowed", (e) => e.message);
          const media = await n.themes.writeMedia("bg.png", new Blob([new Uint8Array([1, 2, 3])]));
          r.media = media;
          r.mediaBytes = [...new Uint8Array(await n.themes.readMedia(media))];
          r.hasUpdates = typeof n.updates?.check === "function";
          return r;
        })()`);
        result.appPath = app.getAppPath();
        result.appName = app.name;
        done(result);
      } catch (err) {
        done({ error: String(err) });
      }
    });
    win.loadURL(`http://127.0.0.1:${server.address().port}/`);
  });
});
