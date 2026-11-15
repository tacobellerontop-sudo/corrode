import { createScopedCss, type InjectedStyle } from "../api/css";
import { createScopedFlux } from "../api/flux";
import { createLogger } from "../api/logger";
import { createScopedPatcher } from "../api/patcher";
import { createStore, type Store } from "../api/storage";
import * as webpack from "../api/webpack";
import { getNative } from "../native";
import { showToast } from "../ui/toast";
import type { PluginContext, PluginDefinition, PluginSource, PluginStatus, SettingsSchema } from "./types";

const log = createLogger("Plugins");

interface PluginState {
  enabled?: boolean;
}
interface ExternalRecord {
  source: string;
  url?: string;
  installedAt: number;
}

interface Entry {
  def: PluginDefinition<any>;
  source: PluginSource;
  url?: string;
  running: boolean;
  error?: string;
  ctx?: PluginContext<any>;
  dispose?: () => void;
  /** Whether this plugin's source patches were registered with webpack at boot. */
  patchesRegistered: boolean;
  /** External plugin whose code failed to evaluate; it can only be uninstalled. */
  broken?: boolean;
}

type ChangeListener = () => void;

const ID_RE = /^[A-Za-z0-9_.-]{1,64}$/;

/** Expression that resolves a plugin definition at runtime (used for `$self` in patches). */
export const selfExpression = (id: string) => `globalThis.corrode.plugins.getDefinition(${JSON.stringify(id)})`;

export class PluginManager {
  private entries = new Map<string, Entry>();
  private state: Store<Record<string, PluginState>>;
  private external: Store<Record<string, ExternalRecord>>;
  private listeners = new Set<ChangeListener>();
  private ready = false;

  constructor() {
    this.state = createStore("plugins");
    this.external = createStore("externalPlugins");
  }

  /* ---------------------------- registration ---------------------------- */

  register(def: PluginDefinition<any>, source: PluginSource = "builtin", url?: string) {
    if (!def || typeof def !== "object") throw new TypeError("Plugin definition must be an object");
    if (!ID_RE.test(def.id ?? "")) throw new TypeError(`Invalid plugin id "${def.id}" (letters, digits, _ . - only)`);
    if (!def.name) throw new TypeError(`Plugin "${def.id}" has no name`);
    if (this.entries.has(def.id)) throw new Error(`A plugin with id "${def.id}" is already registered`);

    const entry: Entry = { def, source, url, running: false, patchesRegistered: false };
    this.entries.set(def.id, entry);

    if (this.isEnabled(def.id) && def.patches?.length) {
      for (const p of def.patches) webpack.addPatch(p, def.id, selfExpression(def.id));
      entry.patchesRegistered = true;
    }
    if (this.ready && this.isEnabled(def.id)) void this.start(def.id);
    this.emit();
  }

  /** Loads external plugins saved in storage. Errors are recorded per plugin, never thrown. */
  loadExternal() {
    for (const id of this.external.keys()) {
      const rec = this.external.get(id)!;
      try {
        const def = evaluatePlugin(rec.source, id);
        if (def.id !== id) throw new Error(`Plugin id changed from "${id}" to "${def.id}"`);
        this.register(def, "external", rec.url);
      } catch (err) {
        log.error(`Failed to load external plugin "${id}"`, err);
        this.entries.set(id, {
          def: { id, name: id, description: "Failed to load" },
          source: "external",
          url: rec.url,
          running: false,
          error: String((err as Error)?.message ?? err),
          patchesRegistered: false,
          broken: true,
        });
      }
    }
  }

  /* ------------------------------- queries ------------------------------- */

  getDefinition(id: string): PluginDefinition<any> | undefined {
    return this.entries.get(id)?.def;
  }

  isEnabled(id: string): boolean {
    const def = this.entries.get(id)?.def;
    if (def?.required) return true;
    return this.state.get(id)?.enabled ?? def?.enabledByDefault ?? false;
  }

  list(): PluginStatus[] {
    return [...this.entries.entries()]
      .map(([id, e]) => ({
        id,
        definition: e.def,
        source: e.source,
        url: e.url,
        enabled: this.isEnabled(id),
        running: e.running,
        error: e.error,
        restartPending: !!e.def.patches?.length && this.isEnabled(id) !== e.patchesRegistered,
      }))
      .sort((a, b) => a.definition.name.localeCompare(b.definition.name));
  }

  get(id: string): PluginStatus | undefined {
    return this.list().find((p) => p.id === id);
  }

  /** Persistent settings store of a plugin. */
  settingsStore(id: string): Store {
    return createStore(`plugin.${id}`);
  }

  subscribe(cb: ChangeListener): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }

  private emit() {
    for (const l of [...this.listeners]) {
      try {
        l();
      } catch (err) {
        log.error("Listener threw", err);
      }
    }
  }

  /* ------------------------------ lifecycle ------------------------------ */

  /** Called once Discord is ready: starts every enabled plugin. */
  async startAll() {
    this.ready = true;
    await Promise.all([...this.entries.keys()].filter((id) => this.isEnabled(id)).map((id) => this.start(id)));
    const unapplied = webpack.getPatches().filter((p) => !p.applied && !p.noWarn);
    for (const p of unapplied) log.warn(`Patch by "${p.owner}" has not applied (yet): find=${String(p.find)}`);
  }

  async stopAll() {
    for (const id of [...this.entries.keys()].reverse()) this.stop(id);
    this.ready = false;
  }

  async enable(id: string) {
    if (!this.entries.has(id)) throw new Error(`Unknown plugin "${id}"`);
    this.state.set(id, { ...this.state.get(id), enabled: true });
    if (this.ready) await this.start(id);
    this.emit();
  }

  disable(id: string) {
    const e = this.entries.get(id);
    if (!e) throw new Error(`Unknown plugin "${id}"`);
    if (e.def.required) throw new Error(`"${e.def.name}" is required and can't be disabled`);
    this.state.set(id, { ...this.state.get(id), enabled: false });
    this.stop(id);
    this.emit();
  }

  async toggle(id: string, enabled = !this.isEnabled(id)) {
    if (enabled) await this.enable(id);
    else this.disable(id);
  }

  async start(id: string) {
    const e = this.entries.get(id);
    if (!e || e.running || e.broken) return;
    const def = e.def;

    const pluginLog = log.child(def.name);
    const patcher = createScopedPatcher(def.id);
    const flux = createScopedFlux();
    const css = createScopedCss(def.id);
    const store = this.settingsStore(def.id);
    const cleanups: (() => void)[] = [];
    const abort = new AbortController();
    const schema: SettingsSchema = def.settings ?? {};

    const settings = new Proxy({} as Record<string, unknown>, {
      get: (_, key: string) => store.get(key, schema[key]?.default),
      set: (_, key: string, value) => {
        store.set(key, value);
        return true;
      },
      has: (_, key: string) => key in schema || store.has(key),
      ownKeys: () => [...new Set([...Object.keys(schema), ...store.keys()])],
      getOwnPropertyDescriptor: (_, key: string) => ({
        enumerable: true,
        configurable: true,
        writable: true,
        value: store.get(key, schema[key as string]?.default),
      }),
    });

    const ctx: PluginContext<any> = {
      id: def.id,
      logger: pluginLog,
      settings,
      storage: createStore(`pluginData.${def.id}`),
      patcher,
      flux,
      css,
      signal: abort.signal,
      toast: (msg, opts) => showToast(msg, { title: def.name, ...opts }),
      onStop: (fn) => void cleanups.push(fn),
    };

    let style: InjectedStyle | undefined;
    const renderCss = () => {
      if (!def.css) return;
      try {
        const text = typeof def.css === "function" ? def.css(settings) : def.css;
        if (style) style.update(text);
        else style = css.inject(text);
      } catch (err) {
        pluginLog.error("css() threw", err);
      }
    };

    const dispose = () => {
      abort.abort();
      try {
        def.stop?.(ctx);
      } catch (err) {
        pluginLog.error("stop() threw", err);
      }
      for (const fn of cleanups.reverse()) {
        try {
          fn();
        } catch (err) {
          pluginLog.error("cleanup threw", err);
        }
      }
      unsubSettings();
      flux.disposeAll();
      patcher.unpatchAll();
      css.removeAll();
    };

    const unsubSettings = store.subscribe("*", (value, key) => {
      renderCss();
      try {
        def.onSettingChange?.(key, value, ctx);
      } catch (err) {
        pluginLog.error("onSettingChange threw", err);
      }
      this.emit();
    });

    e.ctx = ctx;
    e.dispose = dispose;
    e.running = true;
    e.error = undefined;
    try {
      renderCss();
      for (const [type, handler] of Object.entries(def.flux ?? {})) {
        flux.subscribe(type, (action) => handler(action, ctx));
      }
      await def.start?.(ctx);
      if (!abort.signal.aborted) pluginLog.info("Started");
    } catch (err) {
      // Stopped while start() was still awaiting something: not an error.
      if (abort.signal.aborted) return;
      pluginLog.error("Failed to start", err);
      e.error = String((err as Error)?.message ?? err);
      e.running = false;
      e.dispose = undefined;
      dispose();
      showToast(`${def.name} failed to start: ${e.error}`, { kind: "error" });
    }
    this.emit();
  }

  stop(id: string) {
    const e = this.entries.get(id);
    if (!e?.running) return;
    e.running = false;
    e.dispose?.();
    e.dispose = undefined;
    e.ctx = undefined;
    log.child(e.def.name).info("Stopped");
    this.emit();
  }

  /* --------------------------- external plugins --------------------------- */

  /** Installs (or updates) an external plugin from source code. Returns its id. */
  async installFromSource(source: string, url?: string): Promise<string> {
    const def = evaluatePlugin(source, url ?? "pasted plugin");
    const existing = this.entries.get(def.id);
    if (existing && existing.source !== "external") throw new Error(`"${def.id}" conflicts with a built-in plugin`);
    if (existing) this.removeEntry(def.id);
    this.external.set(def.id, { source, url, installedAt: Date.now() });
    this.register(def, "external", url);
    if (def.patches?.length) showToast(`${def.name} patches Discord's code; restart to fully apply it.`, { kind: "warning" });
    return def.id;
  }

  async installFromUrl(url: string): Promise<string> {
    return this.installFromSource(await fetchText(url), url);
  }

  /** Re-downloads an external plugin from its URL. */
  async update(id: string): Promise<void> {
    const url = this.entries.get(id)?.url;
    if (!url) throw new Error("Plugin was not installed from a URL");
    const wasEnabled = this.isEnabled(id);
    await this.installFromUrl(url);
    if (wasEnabled && this.ready) await this.start(id);
  }

  uninstall(id: string) {
    const e = this.entries.get(id);
    if (!e || e.source !== "external") throw new Error(`"${id}" is not an external plugin`);
    this.removeEntry(id);
    this.external.delete(id);
    this.state.delete(id);
    this.settingsStore(id).clear();
    createStore(`pluginData.${id}`).clear();
    this.emit();
  }

  private removeEntry(id: string) {
    this.stop(id);
    webpack.removePatchesFor(id);
    this.entries.delete(id);
  }
}

/* -------------------------------------------------------------------------- */

/**
 * External plugins are CommonJS-style scripts:
 *
 *   module.exports = corrode.definePlugin({ id: "myPlugin", name: "My Plugin", start(ctx) { ... } });
 *
 * They run with full page privileges, exactly like built-in plugins.
 */
export function evaluatePlugin(source: string, label: string): PluginDefinition<any> {
  const module: { exports: any } = { exports: {} };
  const fn = new Function("corrode", "module", "exports", `${source}\n//# sourceURL=corrode://plugin/${encodeURIComponent(label)}`);
  fn((globalThis as any).corrode, module, module.exports);
  const def = module.exports?.default ?? module.exports;
  if (!def || typeof def !== "object" || !def.id) throw new Error("Plugin did not export a definition via module.exports");
  return def;
}

export async function fetchText(url: string): Promise<string> {
  const native = getNative();
  if (native.fetchText) return native.fetchText(url);
  const res = await fetch(url, { cache: "no-store" });
  if (!res.ok) throw new Error(`HTTP ${res.status} fetching ${url}`);
  return res.text();
}
