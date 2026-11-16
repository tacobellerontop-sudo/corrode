// Corrode for the official Discord desktop app: the preload for Discord's main window.
// patcher.js puts this in place of Discord's own preload, which is required at the end so
// Discord works as usual. Runs before any of Discord's page scripts.
"use strict";

const { contextBridge, ipcRenderer, webFrame } = require("electron");

let info = {};
try {
  info = ipcRenderer.sendSync("corrode:init") || {};
} catch (err) {
  console.error("[Corrode] Couldn't reach the main process:", err);
}

if (info.code) {
  try {
    const call = async (name, ...args) => {
      const r = await ipcRenderer.invoke(`corrode:${name}`, ...args);
      if (!r || !r.ok) throw new Error((r && r.error) || `corrode:${name} failed`);
      return r.value;
    };
    const storage = (op, key, value) => ipcRenderer.sendSync("corrode:storage", op, key, value);

    // The raw bridge. A tiny script in the page (below) wraps it as window.CorrodeNative, so
    // values that can't cross the context bridge (Blob) are turned into ones that can first.
    contextBridge.exposeInMainWorld("__CorrodeNativeRaw", {
      name: String(info.name || "Corrode Installer"),
      version: String(info.version || ""),
      storageGet: (key) => storage("get", String(key)),
      storageSet: (key, value) => void storage("set", String(key), String(value)),
      storageDelete: (key) => void storage("delete", String(key)),
      storageKeys: () => storage("keys"),
      fetchText: (url) => call("fetch-text", String(url)),
      relaunch: () => ipcRenderer.send("corrode:relaunch"),
      openDevTools: () => ipcRenderer.send("corrode:open-devtools"),
      eject: () => call("eject"),
      updatesCheck: () => call("updates-check"),
      updatesCorrode: () => call("updates-corrode"),
      updatesClient: () => call("updates-client"),
      themesList: () => call("themes-list"),
      themesRead: (file) => call("themes-read", String(file)),
      themesWrite: (file, css) => call("themes-write", String(file), String(css)),
      themesDelete: (file) => call("themes-delete", String(file)),
      themesOpenFolder: () => call("themes-open-folder"),
      themesWriteMedia: (name, bytes) => call("themes-media-write", String(name), bytes),
      themesReadMedia: (p) => call("themes-media-read", String(p)),
    });

    const wrapper = `(() => {
  const raw = window.__CorrodeNativeRaw;
  const toBytes = async (d) =>
    d instanceof Uint8Array ? d : d instanceof ArrayBuffer ? new Uint8Array(d)
    : d && typeof d.arrayBuffer === "function" ? new Uint8Array(await d.arrayBuffer())
    : ArrayBuffer.isView(d) ? new Uint8Array(d.buffer, d.byteOffset, d.byteLength) : new Uint8Array(0);
  const native = {
    name: raw.name,
    version: raw.version,
    storage: Object.freeze({ get: raw.storageGet, set: raw.storageSet, delete: raw.storageDelete, keys: raw.storageKeys }),
    fetchText: raw.fetchText,
    relaunch: raw.relaunch,
    openDevTools: raw.openDevTools,
    eject: raw.eject,
    updates: Object.freeze({ check: raw.updatesCheck, updateCorrode: raw.updatesCorrode, updateClient: raw.updatesClient }),
    themes: Object.freeze({
      list: raw.themesList,
      read: raw.themesRead,
      write: raw.themesWrite,
      delete: raw.themesDelete,
      openFolder: raw.themesOpenFolder,
      writeMedia: async (name, data) => raw.themesWriteMedia(name, await toBytes(data)),
      readMedia: async (p) => {
        const u = await raw.themesReadMedia(p);
        return u.buffer.slice(u.byteOffset, u.byteOffset + u.byteLength);
      },
    }),
  };
  Object.defineProperty(window, "CorrodeNative", { value: Object.freeze(native), configurable: false, enumerable: false });
})();`;

    // Run Corrode in the page before Discord's scripts, so its webpack hooks see every module.
    webFrame.executeJavaScript(`${wrapper}\n${info.code}\n//# sourceURL=corrode.js`).catch((err) => console.error("[Corrode] failed to start:", err));
  } catch (err) {
    console.error("[Corrode] Couldn't start:", err);
  }
} else if (info.error) {
  console.error("[Corrode]", info.error);
}

// Discord's own preload (DiscordNative and friends).
if (info.originalPreload) require(info.originalPreload);
