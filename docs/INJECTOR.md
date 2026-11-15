# Injector contract

Corrode itself never touches the Discord installation. A separate client/injector (with its
ejector) is responsible for getting `corrode.js` into Discord's renderer. This document is
everything that injector needs to know.

**Injector API version: 1** (also in the bundle banner and `manifest.json`). It only changes when
something below changes incompatibly.

## Build artifacts

`npm run build` writes to `dist/`:

| File | Purpose |
| --- | --- |
| `corrode.min.js` | Minified bundle. Ship this. |
| `corrode.js` | Readable bundle with an inline source map, for development. |
| `manifest.json` | `version`, `commit`, `injectorApiVersion`, `main` (file to load) and the size and `sha256` of each file. Use it to check for updates and verify downloads. |

CI attaches all three to every GitHub release tagged `v*`, so an injector can fetch
`https://github.com/<owner>/corrode/releases/latest/download/manifest.json` and then the file it names.

The bundle is a single self-contained IIFE: no imports, no network access at load time, no
Node APIs. It must run in the **main world** of the Discord renderer (the page's own JS
context, where `window.webpackChunkdiscord_app` lives), not in an isolated preload context.

## Loading it

Evaluate the file's contents in the main world. In Electron, from the injector's preload script:

```js
// preload.js (runs before the page; contextIsolation is on in Discord)
const { webFrame, contextBridge, ipcRenderer } = require("electron");
const fs = require("node:fs");
const path = require("node:path");

// 1. Optional: expose the native bridge (see below). Must happen before Corrode runs.
contextBridge.exposeInMainWorld("CorrodeNative", {
  name: "My Corrode Client",
  version: "1.0.0",
  storage: {
    get: (key) => ipcRenderer.sendSync("corrode:storage-get", key),
    set: (key, value) => ipcRenderer.sendSync("corrode:storage-set", key, value),
    delete: (key) => ipcRenderer.sendSync("corrode:storage-delete", key),
  },
  fetchText: (url) => ipcRenderer.invoke("corrode:fetch-text", url),
  relaunch: () => ipcRenderer.send("corrode:relaunch"),
  openDevTools: () => ipcRenderer.send("corrode:open-devtools"),
  eject: () => ipcRenderer.invoke("corrode:eject"),
});

// 2. Run Corrode in the main world as early as possible.
const code = fs.readFileSync(path.join(__dirname, "corrode.min.js"), "utf8");
webFrame.executeJavaScript(`${code}\n//# sourceURL=corrode.js`);
```

Other environments work too: a browser extension can inject a `<script>` into the page from a
content script with `"run_at": "document_start"`, and a userscript can use `@run-at document-start`.

### Early vs late loading

Corrode detects which case it is in and reports it as `corrode.loadMode`:

- **Early** (`"early"`, recommended): Corrode runs before Discord's own scripts. It traps the
  `webpackChunkdiscord_app` global and sees every module before Discord executes it, so plugin
  *source patches* (string/regex replacements in Discord's code) apply everywhere. Loading from
  a preload script with `webFrame.executeJavaScript` gives you this.
- **Late** (`"late"`): Corrode runs after Discord has booted (e.g. pasted into DevTools, or an
  injector that waits for `did-finish-load`). Runtime patching, Flux, CSS and the UI all work, but
  source patches only reach modules that haven't executed yet.

Either way, plugins start once Discord's Flux dispatcher is available; `await corrode.ready` to
wait for that.

### Content Security Policy

Discord's CSP blocks fetching arbitrary URLs. Built-in features don't need the network, but
installing plugins or themes from a URL does. Either provide `CorrodeNative.fetchText`
(recommended: the main process fetches, no CSP involved) or strip the
`content-security-policy` response headers in the main process
(`session.defaultSession.webRequest.onHeadersReceived`). External plugins are evaluated with
`new Function`, which needs `'unsafe-eval'`; Discord's desktop renderer currently allows it, but
strip the header if that changes.

## The native bridge (`window.CorrodeNative`)

Everything is optional. Without a bridge Corrode still works (as in a plain browser tab).
Type definition: [`src/native.ts`](../src/native.ts).

| Member | Used for |
| --- | --- |
| `name`, `version` | Shown in Settings > About. |
| `storage.get/set/delete(key)` | **Synchronous** string storage. Corrode keeps all of its settings here (keys start with `corrode:`). Without it, Corrode uses `localStorage`, which Discord can clear on logout. A JSON file in the injector's data directory is a good backend. |
| `fetchText(url)` | Fetch text for remote plugins and themes, bypassing CORS and CSP. |
| `relaunch()` | "Restart Discord" buttons (needed after toggling plugins that patch Discord's code). |
| `openDevTools()` | "Open DevTools" button. |
| `eject()` | "Uninstall Corrode" button. Corrode calls `corrode.unload()` first, then this. The injector should remove itself from the client and usually relaunch. |
| `updates.check()` | Optional. Resolves to `{ client, corrode }`, each `{ version?, commit?, latestVersion?, latestCommit?, available, error? }`. When present, Corrode shows an "Updates" page and checks once at startup. |
| `updates.updateCorrode()` | Optional. Download the latest Corrode build; it takes effect after `relaunch()`. Builds are published at `https://github.com/tacobellerontop-sudo/corrode/releases/download/latest-main/` (`manifest.json` lists the files and their SHA-256). |
| `updates.updateClient()` | Optional. Download and start the client's own update, or open its download page. |

## What Corrode exposes back

`window.corrode` (non-enumerable, read-only binding). The parts an injector cares about:

| Member | Meaning |
| --- | --- |
| `version`, `commit` | Build identity. |
| `loadMode` | `"early"` or `"late"`. |
| `ready` | Promise that resolves once plugins have started. |
| `unload()` | Stops every plugin and removes everything Corrode added: function patches, Flux hooks, styles, UI, listeners, the webpack hooks and the `window.corrode` global. Source patches that already ran stay until the page reloads, so an ejector should reload or relaunch after uninstalling. |
| `ui.openSettings(tab?)` | Open the settings window (`"plugins" | "themes" | "quickcss" | "about"`). |
| `ui.mountSettings(container, tab)` | Render one settings tab inside `container`; returns a function that removes it. |

Loading the bundle twice is harmless: the second copy sees `window.corrode`, logs a warning and exits.

## Ejecting

A clean eject is:

1. In the renderer, call `window.corrode?.unload()` (or let the "Uninstall Corrode" button do it through `CorrodeNative.eject`).
2. Remove the injector's hooks from the Discord installation.
3. Relaunch Discord.

User data lives in the bridge's storage (or `localStorage` keys starting with `corrode:`); delete it
too if the user asked for a full removal.
