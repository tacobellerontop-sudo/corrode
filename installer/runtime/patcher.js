// Corrode for the official Discord desktop app: the main-process side.
//
// The Corrode installer renames Discord's resources/app.asar to _app.asar and puts a small folder
// called app.asar in its place, whose index.js requires this file. This file then:
//   1. swaps Electron's BrowserWindow for one that gives Discord's main window our preload
//      (preload.js), which exposes window.CorrodeNative and runs Corrode before Discord's scripts;
//   2. answers that preload over IPC (settings storage, themes folder, updates, eject, ...);
//   3. on Windows, re-applies the patch to a newer app-x.y.z folder when Discord updates itself;
//   4. loads Discord from _app.asar exactly as Electron would have.
// If anything in 1-3 throws, Discord still starts, just without Corrode.
"use strict";

const electron = require("electron");
const path = require("path");
// original-fs ignores Electron's asar support, so app.asar folders and _app.asar are plain paths.
const fs = require("original-fs");
const crypto = require("crypto");

const { app } = electron;
const DATA = __dirname;
const RESOURCES = process.resourcesPath;
const ORIGINAL_ASAR = path.join(RESOURCES, "_app.asar");
const RELEASE = "https://github.com/tacobellerontop-sudo/corrode/releases/download/latest-main/";
const RELEASE_PAGE = "https://github.com/tacobellerontop-sudo/corrode/releases/tag/latest-main";
/** The injector API this runtime implements (docs/INJECTOR.md). */
const INJECTOR_API = 1;
const STUB_MARKER = "corrode-stub";

const log = (...a) => console.log("[Corrode]", ...a);
const logError = (...a) => console.error("[Corrode]", ...a);

function readJson(file, fallback) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {
    return fallback;
  }
}

/** Writes a temporary file and renames it, so a crash never leaves half a file. */
function writeAtomic(file, data) {
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, data);
  fs.renameSync(tmp, file);
}

const installer = readJson(path.join(DATA, "installer.json"), {});

// ---------------------------------------------------------------------------------------------
// The stub that replaces app.asar. Keep in sync with stubIndex() in installer/patch.go.
// ---------------------------------------------------------------------------------------------

function stubIndex(patcher) {
  return `// ${STUB_MARKER} v1: written by the Corrode installer. Run the installer again to remove Corrode.
"use strict";
const path = require("path");
const patcher = ${JSON.stringify(patcher)};
if (require("fs").existsSync(patcher)) {
  require(patcher);
} else {
  // Corrode's files are gone: start Discord without it.
  const { app } = require("electron");
  const asar = path.join(process.resourcesPath, "_app.asar");
  const pkg = require(path.join(asar, "package.json"));
  require.main.filename = path.join(asar, pkg.main);
  if (typeof app.setAppPath === "function") app.setAppPath(asar);
  require(require.main.filename);
}
`;
}

const STUB_PACKAGE = JSON.stringify({ name: "discord", main: "index.js", private: true }, null, 2) + "\n";

function isOurStub(dir) {
  try {
    return fs.readFileSync(path.join(dir, "index.js"), "utf8").includes(STUB_MARKER);
  } catch {
    return false;
  }
}

/** Puts the stub in place of resources/app.asar. Mirrors patch() in installer/patch.go. */
function patchResources(resources) {
  const appAsar = path.join(resources, "app.asar");
  const orig = path.join(resources, "_app.asar");
  let st;
  try {
    st = fs.statSync(appAsar);
  } catch {
    return false;
  }
  if (st.isDirectory()) return false; // already patched (by us or another mod)
  if (fs.existsSync(orig)) fs.renameSync(orig, path.join(resources, "_app.asar.old"));
  fs.renameSync(appAsar, orig);
  try {
    fs.mkdirSync(appAsar);
    fs.writeFileSync(path.join(appAsar, "package.json"), STUB_PACKAGE);
    fs.writeFileSync(path.join(appAsar, "index.js"), stubIndex(path.join(DATA, "patcher.js")));
  } catch (err) {
    fs.rmSync(appAsar, { recursive: true, force: true });
    fs.renameSync(orig, appAsar);
    throw err;
  }
  return true;
}

/** Puts Discord's own app.asar back. Mirrors unpatch() in installer/patch.go. */
function unpatchResources(resources) {
  const appAsar = path.join(resources, "app.asar");
  const orig = path.join(resources, "_app.asar");
  if (!isOurStub(appAsar) || !fs.existsSync(orig)) return false;
  const removing = path.join(resources, "app.asar.corrode-removing");
  fs.rmSync(removing, { recursive: true, force: true });
  fs.renameSync(appAsar, removing);
  try {
    fs.renameSync(orig, appAsar);
  } catch (err) {
    fs.renameSync(removing, appAsar);
    throw err;
  }
  fs.rmSync(removing, { recursive: true, force: true });
  return true;
}

// ---------------------------------------------------------------------------------------------
// Windows: Discord installs each update into a new app-x.y.z folder with a fresh app.asar, then
// restarts into it. Patch that folder on the way out so Corrode survives the update.
// ---------------------------------------------------------------------------------------------

function compareVersions(a, b) {
  const pa = a.replace(/^app-/, "").split(".").map((n) => parseInt(n, 10) || 0);
  const pb = b.replace(/^app-/, "").split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d;
  }
  return 0;
}

function patchLatestHost() {
  try {
    const current = path.basename(path.dirname(process.execPath));
    if (!current.startsWith("app-")) return;
    const root = path.dirname(path.dirname(process.execPath));
    const latest = fs
      .readdirSync(root)
      .filter((d) => d.startsWith("app-"))
      .reduce((best, d) => (compareVersions(d, best) > 0 ? d : best), current);
    if (latest === current) return;
    if (patchResources(path.join(root, latest, "resources"))) log(`Discord updated to ${latest}; Corrode was added to it.`);
  } catch (err) {
    logError("Couldn't add Corrode to the updated Discord:", err);
  }
}

// ---------------------------------------------------------------------------------------------
// Storage: a JSON file, read once and written on every change.
// ---------------------------------------------------------------------------------------------

const SETTINGS = path.join(DATA, "settings.json");
let store = null;
function getStore() {
  if (!store) {
    const s = readJson(SETTINGS, {});
    store = s && typeof s === "object" && !Array.isArray(s) ? s : {};
  }
  return store;
}
function saveStore() {
  try {
    writeAtomic(SETTINGS, JSON.stringify(store));
  } catch (err) {
    logError("Couldn't save settings:", err);
  }
}

// ---------------------------------------------------------------------------------------------
// Corrode builds: the one the installer shipped, or a newer one downloaded from the Updates page.
// ---------------------------------------------------------------------------------------------

const UPDATES = path.join(DATA, "updates");
const sha256 = (buf) => crypto.createHash("sha256").update(buf).digest("hex");

/** A build folder whose manifest checks out: { dir, manifest, code } or null. */
function readBuild(dir) {
  const manifest = readJson(path.join(dir, "manifest.json"), null);
  if (!manifest || manifest.injectorApiVersion !== INJECTOR_API || typeof manifest.main !== "string") return null;
  if (path.basename(manifest.main) !== manifest.main) return null;
  try {
    const buf = fs.readFileSync(path.join(dir, manifest.main));
    const want = manifest.files && manifest.files[manifest.main];
    if (want && want.sha256 && want.sha256 !== sha256(buf)) return null;
    return { dir, manifest, code: buf.toString("utf8") };
  } catch {
    return null;
  }
}

function currentBuild() {
  const bundled = readBuild(DATA);
  const updated = readBuild(UPDATES);
  if (updated && (!bundled || String(updated.manifest.builtAt) > String(bundled.manifest.builtAt))) return updated;
  return bundled;
}

let loaded = null; // the build this run of Discord loaded

// ---------------------------------------------------------------------------------------------
// Network
// ---------------------------------------------------------------------------------------------

async function fetchBuffer(url) {
  if (!/^https?:\/\//i.test(String(url))) throw new Error(`Only http(s) URLs can be fetched: ${url}`);
  const doFetch = electron.net && typeof electron.net.fetch === "function" ? electron.net.fetch.bind(electron.net) : fetch;
  const res = await doFetch(String(url), { cache: "no-store" });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}
const fetchJson = async (url) => JSON.parse((await fetchBuffer(url)).toString("utf8"));

// ---------------------------------------------------------------------------------------------
// Themes folder (<data>/themes). Same rules as AesDc's src-tauri/src/themes.rs.
// ---------------------------------------------------------------------------------------------

const THEMES = path.join(DATA, "themes");
const MAX_CSS = 32 * 1024 * 1024;
const MAX_MEDIA = 512 * 1024 * 1024;
const MEDIA_TYPES = ["png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "svg", "mp4", "webm", "mov", "m4v", "ogv"];

function plainName(name) {
  name = String(name);
  const ok =
    name.length > 0 &&
    name.length <= 200 &&
    !name.startsWith(".") &&
    !name.endsWith(".") &&
    !name.endsWith(" ") &&
    !/[\/\\:*?"<>|\x00-\x1f\x7f]/.test(name);
  if (!ok) throw new Error(`${JSON.stringify(name)} isn't a valid file name`);
  return name;
}
const ext = (name) => path.extname(name).slice(1).toLowerCase();
function themesDir() {
  fs.mkdirSync(THEMES, { recursive: true });
  return THEMES;
}
function cssPath(name) {
  name = plainName(name);
  if (ext(name) !== "css") throw new Error(`${name} isn't a .css file`);
  return path.join(themesDir(), name);
}
function mediaPath(rel) {
  const name = plainName(String(rel).replace(/^media\//, ""));
  if (!MEDIA_TYPES.includes(ext(name))) throw new Error(`${name} isn't a picture or video Corrode can use`);
  const dir = path.join(themesDir(), "media");
  fs.mkdirSync(dir, { recursive: true });
  return path.join(dir, name);
}

const themes = {
  list() {
    const dir = themesDir();
    const files = [];
    for (const name of fs.readdirSync(dir)) {
      if (ext(name) !== "css") continue;
      try {
        plainName(name);
        const st = fs.statSync(path.join(dir, name));
        if (st.isFile()) files.push({ file: name, modified: Math.floor(st.mtimeMs), size: st.size });
      } catch {}
    }
    files.sort((a, b) => a.file.toLowerCase().localeCompare(b.file.toLowerCase()));
    return { dir, files };
  },
  read(file) {
    const p = cssPath(file);
    if (fs.statSync(p).size > MAX_CSS) throw new Error(`${file} is too large`);
    return fs.readFileSync(p, "utf8");
  },
  write(file, css) {
    css = String(css);
    if (Buffer.byteLength(css) > MAX_CSS) throw new Error(`${file} is too large`);
    writeAtomic(cssPath(file), css);
  },
  delete(file) {
    fs.rmSync(cssPath(file), { force: true });
  },
  openFolder() {
    return electron.shell.openPath(themesDir()).then((err) => {
      if (err) throw new Error(err);
    });
  },
  writeMedia(name, data) {
    const bytes = Buffer.from(data);
    if (bytes.length > MAX_MEDIA) throw new Error(`${name} is too large (the limit is ${MAX_MEDIA / 1024 / 1024} MB)`);
    const first = mediaPath(name);
    const e = ext(first);
    const stem = path.basename(first, path.extname(first));
    let p = first;
    for (let n = 2; ; n++) {
      if (!fs.existsSync(p)) break;
      if (fs.statSync(p).size === bytes.length && fs.readFileSync(p).equals(bytes)) return `media/${path.basename(p)}`;
      p = path.join(path.dirname(first), `${stem}-${n}.${e}`);
    }
    writeAtomic(p, bytes);
    return `media/${path.basename(p)}`;
  },
  readMedia(rel) {
    const p = mediaPath(rel);
    if (fs.statSync(p).size > MAX_MEDIA) throw new Error(`${rel} is too large`);
    return fs.readFileSync(p);
  },
};

// ---------------------------------------------------------------------------------------------
// Updates
// ---------------------------------------------------------------------------------------------

const updates = {
  async check() {
    const result = {
      client: { version: installer.version, commit: installer.commit, available: false },
      corrode: { version: loaded && loaded.manifest.version, commit: loaded && loaded.manifest.commit, available: false },
    };
    await Promise.all([
      fetchJson(RELEASE + "manifest.json").then(
        (m) => {
          // Compare with what's on disk now, so a downloaded update doesn't show up again before the restart.
          const disk = currentBuild();
          result.corrode.latestVersion = m.version;
          result.corrode.latestCommit = m.commit;
          result.corrode.available = m.injectorApiVersion === INJECTOR_API && (!disk || disk.manifest.commit !== m.commit);
        },
        (err) => (result.corrode.error = String(err.message || err)),
      ),
      fetchJson(RELEASE + "installer.json").then(
        (m) => {
          result.client.latestVersion = m.version;
          result.client.latestCommit = m.commit;
          // Only a change to the files the installer puts into Discord (a hash of them) needs a new
          // installer; Corrode itself updates from the card above.
          result.client.available = !!m.runtimeVersion && m.runtimeVersion !== installer.runtimeVersion;
        },
        (err) => (result.client.error = String(err.message || err)),
      ),
    ]);
    return result;
  },
  async updateCorrode() {
    const manifest = await fetchJson(RELEASE + "manifest.json");
    if (manifest.injectorApiVersion !== INJECTOR_API) {
      throw new Error("This Corrode build needs a newer installer. Download it from the Updates page and run Repair.");
    }
    const main = String(manifest.main || "");
    if (!main || path.basename(main) !== main) throw new Error("The published manifest is invalid");
    const buf = await fetchBuffer(RELEASE + main);
    const want = manifest.files && manifest.files[main];
    if (!want || want.sha256 !== sha256(buf) || (want.size != null && want.size !== buf.length)) {
      throw new Error("The download didn't match the published checksum; try again in a minute");
    }
    fs.mkdirSync(UPDATES, { recursive: true });
    writeAtomic(path.join(UPDATES, main), buf);
    // The manifest goes last: a folder without a valid one is ignored.
    writeAtomic(path.join(UPDATES, "manifest.json"), JSON.stringify(manifest, null, 2));
  },
  async updateClient() {
    await electron.shell.openExternal(RELEASE_PAGE);
  },
};

// ---------------------------------------------------------------------------------------------
// IPC for preload.js
// ---------------------------------------------------------------------------------------------

const originalPreloads = new Map(); // webContents id -> Discord's own preload

function relaunch() {
  app.relaunch();
  app.exit(0);
}

function eject() {
  // Turn Corrode off first, so that even if Discord's files can't be put back while it runs,
  // the next start is a plain Discord. The installer's Uninstall finishes the job.
  fs.writeFileSync(path.join(DATA, "disabled"), "Corrode was uninstalled from inside Discord.\n");
  try {
    unpatchResources(RESOURCES);
  } catch (err) {
    logError("Couldn't restore Discord's app.asar while it runs; Corrode stays off until you run the installer:", err);
  }
  setTimeout(relaunch, 300);
}

function setupIpc() {
  const { ipcMain } = electron;
  const handle = (name, fn) =>
    ipcMain.handle(`corrode:${name}`, async (event, ...args) => {
      try {
        return { ok: true, value: await fn(event, ...args) };
      } catch (err) {
        return { ok: false, error: String((err && err.message) || err) };
      }
    });

  ipcMain.on("corrode:init", (event) => {
    let info = { originalPreload: originalPreloads.get(event.sender.id) };
    try {
      loaded = loaded || currentBuild();
      if (!loaded) throw new Error(`No Corrode build found in ${DATA}. Run the installer's Repair.`);
      info = {
        ...info,
        code: loaded.code,
        name: "Corrode Installer",
        version: installer.version || "dev",
        apiVersion: INJECTOR_API,
      };
    } catch (err) {
      logError(err);
      info.error = String(err.message || err);
    }
    event.returnValue = info;
  });

  ipcMain.on("corrode:storage", (event, op, key, value) => {
    try {
      const s = getStore();
      if (op === "get") event.returnValue = Object.prototype.hasOwnProperty.call(s, key) ? s[key] : null;
      else if (op === "keys") event.returnValue = Object.keys(s);
      else {
        if (op === "set") s[String(key)] = String(value);
        else if (op === "delete") delete s[key];
        saveStore();
        event.returnValue = null;
      }
    } catch (err) {
      logError("storage", err);
      event.returnValue = null;
    }
  });

  handle("fetch-text", (_e, url) => fetchBuffer(url).then((b) => b.toString("utf8")));
  ipcMain.on("corrode:relaunch", relaunch);
  ipcMain.on("corrode:open-devtools", (event) => event.sender.openDevTools());
  handle("eject", eject);
  handle("updates-check", () => updates.check());
  handle("updates-corrode", () => updates.updateCorrode());
  handle("updates-client", () => updates.updateClient());
  handle("themes-list", () => themes.list());
  handle("themes-read", (_e, f) => themes.read(f));
  handle("themes-write", (_e, f, css) => themes.write(f, css));
  handle("themes-delete", (_e, f) => themes.delete(f));
  handle("themes-open-folder", () => themes.openFolder());
  handle("themes-media-write", (_e, name, data) => themes.writeMedia(name, data));
  handle("themes-media-read", (_e, rel) => themes.readMedia(rel));
}

// ---------------------------------------------------------------------------------------------
// Give Discord's main window our preload.
// ---------------------------------------------------------------------------------------------

function patchBrowserWindow() {
  const PRELOAD = path.join(DATA, "preload.js");
  class BrowserWindow extends electron.BrowserWindow {
    constructor(options) {
      let original;
      // Discord's main window is the one with a title and a preload (the splash screen has no title).
      if (options && options.webPreferences && options.webPreferences.preload && options.title) {
        original = options.webPreferences.preload;
        options.webPreferences.preload = PRELOAD;
        // Our preload has to require() Discord's, which a sandboxed preload can't do.
        options.webPreferences.sandbox = false;
      }
      super(options);
      if (original) {
        const id = this.webContents.id;
        originalPreloads.set(id, original);
        this.webContents.once("destroyed", () => originalPreloads.delete(id));
      }
    }
  }
  Object.assign(BrowserWindow, electron.BrowserWindow);
  Object.defineProperty(BrowserWindow, "name", { value: "BrowserWindow", configurable: true });

  const electronPath = require.resolve("electron");
  const patched = { ...electron, BrowserWindow };
  const cached = require.cache[electronPath];
  if (cached) {
    delete cached.exports;
    cached.exports = patched;
  } else {
    throw new Error("electron isn't in require.cache; can't hook BrowserWindow");
  }
}

// ---------------------------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------------------------

const disabled = fs.existsSync(path.join(DATA, "disabled"));
if (disabled) {
  log("Corrode is turned off (uninstalled from inside Discord). Run the installer to remove it or turn it back on.");
} else {
  try {
    setupIpc();
    patchBrowserWindow();
    // CORRODE_TEST_HOST_UPDATE lets test/installer-e2e.mjs check this on Linux.
    if (process.platform === "win32" || process.env.CORRODE_TEST_HOST_UPDATE) app.on("before-quit", patchLatestHost);
  } catch (err) {
    logError("Couldn't set up; starting Discord without Corrode:", err);
  }
}

// Load Discord itself, the way Electron would have loaded app.asar.
const discordPkg = require(path.join(ORIGINAL_ASAR, "package.json"));
require.main.filename = path.join(ORIGINAL_ASAR, discordPkg.main);
if (typeof app.setAppPath === "function") app.setAppPath(ORIGINAL_ASAR);
require(require.main.filename);

module.exports = { patchResources, unpatchResources, stubIndex, compareVersions };
