/**
 * Corrode entrypoint. This file is bundled into `dist/corrode.js`, a single self-contained
 * script that an injector evaluates in Discord's renderer (main world). See docs/INJECTOR.md.
 */
import { common } from "./api/common";
import { injectCss, removeAllCss } from "./api/css";
import * as flux from "./api/flux";
import { createLogger, logger } from "./api/logger";
import * as patcher from "./api/patcher";
import * as storage from "./api/storage";
import * as webpack from "./api/webpack";
import { builtinPlugins } from "./builtins";
import { getNative } from "./native";
import { PluginManager } from "./plugins/manager";
import { definePlugin } from "./plugins/types";
import { ThemeManager } from "./themes";
import { installEntryPoints } from "./ui/entry";
import { SettingsUI } from "./ui/settings";
import { removeToasts, showToast } from "./ui/toast";

export type { PluginDefinition, PluginContext, SettingsSchema, SettingDefinition } from "./plugins/types";
export type { CorrodeNative } from "./native";

const GLOBAL = "corrode";
const VERSION = typeof __CORRODE_VERSION__ === "string" ? __CORRODE_VERSION__ : "dev";
const COMMIT = typeof __CORRODE_COMMIT__ === "string" ? __CORRODE_COMMIT__ : "unknown";

function onDomReady(cb: () => void) {
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", cb, { once: true });
  else cb();
}

function boot() {
  const win = globalThis as any;
  if (win[GLOBAL]) {
    logger.warn(`Already loaded (v${win[GLOBAL].version}); ignoring second injection`);
    return;
  }

  const startedAt = performance.now();
  const native = getNative();
  const mode = webpack.init(win);
  const plugins = new PluginManager();
  const themes = new ThemeManager();
  let unloaded = false;
  const disposers: (() => void)[] = [];

  const ui = new SettingsUI({ plugins, themes, native, version: VERSION, commit: COMMIT, unload: () => api.unload() });

  /**
   * Public API, available as `window.corrode`. External plugins receive it as `corrode`.
   * Keep this surface stable: it's what third-party plugins build against.
   */
  const api = {
    version: VERSION,
    commit: COMMIT,
    /** "early" or "late"; see docs/INJECTOR.md. */
    loadMode: mode,
    /** Resolves once Discord's dispatcher is available and enabled plugins have started. */
    ready: undefined as unknown as Promise<void>,
    native,
    logger,
    createLogger,
    definePlugin,
    webpack: {
      find: webpack.find,
      waitFor: webpack.waitFor,
      onModule: webpack.onModule,
      lazy: webpack.lazy,
      filters: webpack.filters,
      findByProps: webpack.findByProps,
      findByPrototype: webpack.findByPrototype,
      findByCode: webpack.findByCode,
      findStore: webpack.findStore,
      findByDisplayName: webpack.findByDisplayName,
      findModuleId: webpack.findModuleId,
      getRequire: () => webpack.getRequire(win),
    },
    patcher: {
      before: patcher.before,
      after: patcher.after,
      instead: patcher.instead,
      getOriginal: patcher.getOriginal,
      isPatched: patcher.isPatched,
      createScoped: patcher.createScopedPatcher,
    },
    flux: {
      subscribe: flux.subscribe,
      intercept: flux.intercept,
      dispatch: flux.dispatch,
      getDispatcher: flux.getDispatcher,
    },
    storage: { createStore: storage.createStore },
    css: { inject: injectCss },
    common,
    plugins,
    themes,
    ui: {
      openSettings: (tab?: "plugins" | "themes" | "quickcss" | "about") => ui.open(tab),
      closeSettings: () => ui.close(),
      toast: showToast,
    },
    /**
     * Stops all plugins and removes everything Corrode added to the page (patches, styles,
     * UI, listeners) and the `window.corrode` global. Source patches that already ran stay
     * in effect until Discord reloads. Safe to call more than once.
     */
    unload() {
      if (unloaded) return;
      unloaded = true;
      logger.info("Unloading");
      ui.close();
      void plugins.stopAll();
      themes.stop();
      for (const d of disposers.reverse()) {
        try {
          d();
        } catch (err) {
          logger.error("Disposer threw", err);
        }
      }
      flux.teardown();
      webpack.teardown();
      removeAllCss();
      removeToasts();
      delete win[GLOBAL];
    },
  };

  // Set the global synchronously: `$self` in source patches resolves through it.
  Object.defineProperty(win, GLOBAL, { value: api, configurable: true, writable: false, enumerable: false });

  for (const def of builtinPlugins) {
    try {
      plugins.register(def, "builtin");
    } catch (err) {
      logger.error(`Couldn't register built-in plugin ${def.id}`, err);
    }
  }
  plugins.loadExternal();

  api.ready = (async () => {
    const dispatcher = await flux.waitForDispatcher();
    if (unloaded) return;
    logger.info("Discord is ready", dispatcher ? "" : "(no dispatcher)");
    onDomReady(() => {
      if (unloaded) return;
      themes.start();
      disposers.push(installEntryPoints(ui));
    });
    await plugins.startAll();
    logger.info(`Started in ${Math.round(performance.now() - startedAt)}ms (${mode} load)`);
  })().catch((err) => {
    logger.error("Startup failed", err);
    throw err;
  });
  // Avoid unhandled-rejection noise; consumers can still await api.ready.
  api.ready.catch(() => {});

  logger.info(`v${VERSION} (${COMMIT}) loaded ${mode}${native.name ? ` by ${native.name}` : ""}`);
}

try {
  boot();
} catch (err) {
  // Never let Corrode take Discord down with it.
  console.error("[Corrode] Fatal error during boot", err);
}
