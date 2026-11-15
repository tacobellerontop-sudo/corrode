import type { ScopedCss } from "../api/css";
import type { ScopedFlux, FluxAction } from "../api/flux";
import type { Logger } from "../api/logger";
import type { ScopedPatcher } from "../api/patcher";
import type { Store } from "../api/storage";
import type { ModulePatch } from "../api/webpack";
import type { ToastOptions } from "../ui/toast";

interface BaseSetting<T> {
  label: string;
  description?: string;
  default: T;
  /** Changing this setting only takes effect after a Discord restart. */
  restartRequired?: boolean;
}
export interface BooleanSetting extends BaseSetting<boolean> {
  type: "boolean";
}
export interface StringSetting extends BaseSetting<string> {
  type: "string";
  placeholder?: string;
  multiline?: boolean;
}
export interface NumberSetting extends BaseSetting<number> {
  type: "number";
  min?: number;
  max?: number;
  step?: number;
}
export interface SelectSetting<T extends string = string> extends BaseSetting<T> {
  type: "select";
  options: { label: string; value: T }[];
}
export type SettingDefinition = BooleanSetting | StringSetting | NumberSetting | SelectSetting;
export type SettingsSchema = Record<string, SettingDefinition>;

/** Maps a settings schema to the type of its values. */
export type SettingsValues<S extends SettingsSchema> = {
  [K in keyof S]: S[K]["default"];
};

export interface PluginContext<S extends SettingsSchema = SettingsSchema> {
  readonly id: string;
  readonly logger: Logger;
  /** Live settings values. Reading returns the stored value or the default; assigning saves. */
  readonly settings: SettingsValues<S>;
  /** Private persistent storage for the plugin. */
  readonly storage: Store;
  /** Patches made here are removed automatically when the plugin stops. */
  readonly patcher: ScopedPatcher;
  /** Subscriptions/interceptors made here are removed automatically when the plugin stops. */
  readonly flux: ScopedFlux;
  /** Stylesheets injected here are removed automatically when the plugin stops. */
  readonly css: ScopedCss;
  /** Aborted when the plugin stops. Pass it to `webpack.waitFor` and anything else async. */
  readonly signal: AbortSignal;
  toast(message: string, opts?: ToastOptions): () => void;
  /** Register any cleanup to run when the plugin stops. */
  onStop(fn: () => void): void;
}

export interface PluginDefinition<S extends SettingsSchema = SettingsSchema> {
  /** Stable unique id, e.g. "silentTyping". Used as the storage key. */
  id: string;
  name: string;
  description?: string;
  authors?: string[];
  version?: string;
  /** Enabled on first run. Default false. */
  enabledByDefault?: boolean;
  /** Can't be disabled from the UI. */
  required?: boolean;
  settings?: S;
  /**
   * Source patches applied to Discord's webpack modules as they load. Toggling a plugin
   * with patches requires a restart. Inside replacements, `$self` refers to this definition.
   */
  patches?: ModulePatch[];
  /** Flux action handlers, subscribed while the plugin runs. */
  flux?: Record<string, (action: FluxAction, ctx: PluginContext<S>) => void>;
  /** CSS injected while the plugin runs. A function is re-evaluated whenever a setting changes. */
  css?: string | ((settings: SettingsValues<S>) => string);
  start?(ctx: PluginContext<S>): void | Promise<void>;
  stop?(ctx: PluginContext<S>): void;
  onSettingChange?(key: keyof S & string, value: unknown, ctx: PluginContext<S>): void;
  /** Free for plugins to stash things on (reachable from patches through `$self`). */
  [extra: string]: unknown;
}

export type PluginSource = "builtin" | "external";

export interface PluginStatus {
  id: string;
  definition: PluginDefinition<any>;
  source: PluginSource;
  enabled: boolean;
  running: boolean;
  /** Set when start() threw or the plugin failed to load. */
  error?: string;
  /** Plugin has source patches whose enabled state differs from what was applied at startup. */
  restartPending: boolean;
  /** For external plugins, where it was installed from. */
  url?: string;
}

/** Identity helper that gives plugin authors type inference for settings. */
export function definePlugin<S extends SettingsSchema = {}>(def: PluginDefinition<S>): PluginDefinition<S> {
  return def;
}
