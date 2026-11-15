import { createLogger } from "./logger";

/**
 * Webpack integration for Discord's `webpackChunkdiscord_app` bundle.
 *
 * Two load modes are supported:
 *
 *  - **Early** (injected before Discord's scripts run): we trap the chunk global, so every
 *    module factory passes through us before Discord can execute it. Plugins' source
 *    patches (`find` / `replacement`) apply to every module.
 *  - **Late** (injected after Discord has booted): we hook the existing chunk array.
 *    Source patches still apply to lazily loaded chunks and to modules that were
 *    downloaded but not executed yet; modules that already ran cannot be patched.
 *
 * In both modes every factory is wrapped so that `waitFor` / `onModule` hear about
 * modules as soon as they execute.
 */

const log = createLogger("Webpack");

export const CHUNK_GLOBAL = "webpackChunkdiscord_app";

export type ModuleFactory = (module: any, exports: any, require: WebpackRequire) => void;
export interface WebpackRequire {
  (id: PropertyKey): any;
  m: Record<PropertyKey, ModuleFactory>;
  c: Record<PropertyKey, { id: PropertyKey; loaded: boolean; exports: any }>;
  [key: string]: any;
}
type Chunk = [ids: PropertyKey[], modules: Record<PropertyKey, ModuleFactory>, runtime?: (r: WebpackRequire) => void];

export type Filter = (candidate: any) => boolean;

export interface Replacement {
  /** String or RegExp to match in the module source. Use the `g` flag to replace every match. */
  match: string | RegExp;
  /**
   * Replacement text (supports `$1`-style groups) or replacer function. The token `$self`
   * is substituted with an expression that evaluates to the owning plugin's definition.
   */
  replace: string | ((substring: string, ...args: any[]) => string);
}

export interface ModulePatch {
  /** Identifies the module: a string (or every string in an array) that must appear in the factory source, or a RegExp. */
  find: string | string[] | RegExp;
  replacement: Replacement | Replacement[];
  /** Apply to every matching module instead of only the first one. */
  all?: boolean;
  /** Don't warn if this patch never applies. */
  noWarn?: boolean;
}

export interface RegisteredPatch extends ModulePatch {
  owner: string;
  /** True once the patch has been applied to at least one module. */
  applied: boolean;
  /** Ids of modules this patch was applied to. */
  appliedTo: Set<string>;
  /** Expression evaluating to the owner's plugin definition, used for `$self`. */
  selfExpr: string;
}

export type LoadMode = "early" | "late" | "uninitialised";

let mode: LoadMode = "uninitialised";
/**
 * Every webpack runtime seen on the page. Discord runs more than one (the main app, libdiscore's
 * wasm loader, ...), each with its own module cache, so lookups search all of them.
 */
const requires = new Set<WebpackRequire>();
const patches: RegisteredPatch[] = [];
const listeners = new Set<{ filter: Filter; cb: (exp: any, id: PropertyKey) => void }>();
const WRAPPED = Symbol.for("corrode.webpack.wrapped");
let restoreHooks: (() => void)[] = [];

export function getLoadMode(): LoadMode {
  return mode;
}

/* ------------------------------------------------------------------ */
/* Source patching                                                     */
/* ------------------------------------------------------------------ */

function matchesFind(code: string, find: ModulePatch["find"]): boolean {
  if (typeof find === "string") return code.includes(find);
  if (Array.isArray(find)) return find.every((f) => code.includes(f));
  find.lastIndex = 0;
  return find.test(code);
}

/** Registers a source patch. Must be called before the target module's chunk is loaded. */
export function addPatch(patch: ModulePatch, owner: string, selfExpr: string): RegisteredPatch {
  const reg: RegisteredPatch = { ...patch, owner, applied: false, appliedTo: new Set(), selfExpr };
  patches.push(reg);
  if (requires.size) patchPendingFactories();
  return reg;
}

export function removePatchesFor(owner: string) {
  for (let i = patches.length - 1; i >= 0; i--) if (patches[i].owner === owner) patches.splice(i, 1);
}

export function getPatches(): readonly RegisteredPatch[] {
  return patches;
}

function normaliseFactorySource(code: string): string {
  // Method shorthand (`123(e,t,n){...}`) isn't a valid expression; turn it into a function.
  const m = /^(async\s+)?([\w$]+)\s*\(/.exec(code);
  if (m && m[2] !== "function") return `${m[1] ?? ""}function${code.slice(m[0].length - 1)}`;
  return code;
}

function applySourcePatches(id: PropertyKey, factory: ModuleFactory): ModuleFactory {
  if (!patches.length) return factory;
  const originalCode = Function.prototype.toString.call(factory);
  let code = originalCode;
  const appliedNow: RegisteredPatch[] = [];

  for (const patch of patches) {
    if (patch.applied && !patch.all && !patch.appliedTo.has(String(id))) continue;
    if (!matchesFind(code, patch.find)) continue;

    let next = code;
    for (const r of Array.isArray(patch.replacement) ? patch.replacement : [patch.replacement]) {
      const before = next;
      const self = patch.selfExpr;
      if (typeof r.replace === "string") {
        next = next.replace(r.match as any, r.replace.replaceAll("$self", self));
      } else {
        const fn = r.replace;
        next = next.replace(r.match as any, (...args: any[]) => String(fn(...(args as [string]))).replaceAll("$self", self));
      }
      if (before === next) log.warn(`Patch by "${patch.owner}" found module ${String(id)} but replacement ${String(r.match)} did not match`);
    }
    if (next !== code) {
      code = next;
      appliedNow.push(patch);
    }
  }

  if (code === originalCode) return factory;
  try {
    const patched = (0, eval)(`0,${normaliseFactorySource(code)}\n//# sourceURL=corrode://webpack-module/${String(id)}`);
    for (const p of appliedNow) {
      p.applied = true;
      p.appliedTo.add(String(id));
    }
    log.debug(`Patched module ${String(id)} (${appliedNow.map((p) => p.owner).join(", ")})`);
    return patched;
  } catch (err) {
    log.error(`Module ${String(id)} failed to compile after patches by ${appliedNow.map((p) => p.owner).join(", ")}; using original`, err);
    return factory;
  }
}

/* ------------------------------------------------------------------ */
/* Factory wrapping                                                    */
/* ------------------------------------------------------------------ */

/** Wraps (or re-wraps, picking up newly added patches) a module factory. */
function wrapFactory(id: PropertyKey, factory: ModuleFactory): ModuleFactory {
  if (typeof factory !== "function") return factory;
  const original: ModuleFactory = (factory as any)[WRAPPED] ?? factory;
  const impl = applySourcePatches(id, original);
  const wrapped = function (this: any, module: any, exports: any, require: WebpackRequire) {
    if (typeof require === "function" && require.m && !requires.has(require)) requires.add(require);
    const res = impl.call(this, module, exports, require);
    notify(module?.exports, id);
    return res;
  } as ModuleFactory;
  Object.defineProperty(wrapped, WRAPPED, { value: original });
  Object.defineProperty(wrapped, "toString", {
    value: () => Function.prototype.toString.call(impl),
    configurable: true,
    writable: true,
  });
  return wrapped;
}

function wrapChunk(chunk: Chunk) {
  const modules = chunk?.[1];
  if (!modules || typeof modules !== "object") return;
  for (const id of Object.keys(modules)) {
    try {
      modules[id] = wrapFactory(id, modules[id]);
    } catch (err) {
      log.error(`Failed to wrap module ${id}`, err);
    }
  }
}

/** Late mode: wrap/patch factories that are registered but haven't executed yet. */
function patchPendingFactories() {
  for (const r of requires) {
    for (const id of Object.keys(r.m)) {
      if (r.c?.[id]) continue;
      r.m[id] = wrapFactory(id, r.m[id]);
    }
  }
}

/* ------------------------------------------------------------------ */
/* Hooking the chunk global                                            */
/* ------------------------------------------------------------------ */

function hookChunkArray(arr: Chunk[]) {
  if ((arr as any)[WRAPPED]) return;
  Object.defineProperty(arr, WRAPPED, { value: true, configurable: true });

  // Chunks pushed before we got here.
  for (const chunk of arr) wrapChunk(chunk);

  const ownPush = Object.getOwnPropertyDescriptor(arr, "push");
  let currentPush: (...items: any[]) => number = arr.push;
  // Discord loads more than one webpack runtime onto this array (the main app, libdiscore's wasm
  // loader, ...). Each runtime reads `push`, binds it as its parent and installs its own JSONP
  // callback, so pushes travel newest runtime -> ... -> oldest -> Array.prototype.push. Hand every
  // reader a wrapper around the *current* push so that chain stays intact, and only wrap a
  // chunk's factories once, at the outermost call.
  let depth = 0;
  const wrapPush = (fn: (...items: any[]) => number) =>
    function corrodePush(this: unknown, ...chunks: Chunk[]) {
      if (depth === 0) for (const c of chunks) wrapChunk(c);
      depth++;
      try {
        return fn.apply(arr, chunks);
      } finally {
        depth--;
      }
    };
  let exposed = wrapPush(currentPush);

  Object.defineProperty(arr, "push", {
    configurable: true,
    get: () => exposed,
    set(v) {
      currentPush = v;
      exposed = wrapPush(v);
    },
  });

  restoreHooks.push(() => {
    if (ownPush) Object.defineProperty(arr, "push", { ...ownPush, value: currentPush });
    else Object.defineProperty(arr, "push", { value: currentPush, writable: true, configurable: true });
    delete (arr as any)[WRAPPED];
  });
}

/** Looks like webpack's `__webpack_require__` (minified or not). */
function isWebpackRequire(fn: unknown): fn is WebpackRequire {
  if (typeof fn !== "function") return false;
  try {
    const src = Function.prototype.toString.call(fn);
    return /exports\s*:\s*\{\s*\}/.test(src) || src.includes("__webpack_modules__");
  } catch {
    return false;
  }
}

/**
 * Early mode: catch every webpack runtime as it starts. A runtime assigns its module table with
 * `__webpack_require__.m = __webpack_modules__`; trapping that assignment on Function.prototype
 * lets us wrap the modules bundled into the runtime's own script. Those never pass through the
 * chunk array, and they include core modules such as Flux.
 */
function hookRuntimeCreation(win: any) {
  const proto = win.Function?.prototype ?? Function.prototype;
  if (Object.getOwnPropertyDescriptor(proto, "m")) return;
  Object.defineProperty(proto, "m", {
    configurable: true,
    enumerable: false,
    set(this: any, modules: any) {
      Object.defineProperty(this, "m", { value: modules, configurable: true, writable: true, enumerable: true });
      try {
        if (modules && typeof modules === "object" && isWebpackRequire(this)) {
          requires.add(this);
          for (const id of Object.keys(modules)) modules[id] = wrapFactory(id, modules[id]);
          log.debug(`Hooked a webpack runtime with ${Object.keys(modules).length} modules`);
        }
      } catch (err) {
        log.error("Failed to hook a webpack runtime", err);
      }
    },
  });
  restoreHooks.push(() => {
    const d = Object.getOwnPropertyDescriptor(proto, "m");
    if (d?.set) delete (proto as any).m;
  });
}

/**
 * Hooks Discord's webpack. Call once, as early as possible. Returns the detected load mode.
 */
export function init(win: any = globalThis): LoadMode {
  if (mode !== "uninitialised") return mode;
  const existing = win[CHUNK_GLOBAL];

  if (Array.isArray(existing)) {
    mode = "late";
    hookChunkArray(existing);
    getRequire(win);
    patchPendingFactories();
    log.info("Loaded late: source patches only apply to modules that have not run yet");
  } else {
    mode = "early";
    let value: Chunk[] | undefined;
    Object.defineProperty(win, CHUNK_GLOBAL, {
      configurable: true,
      enumerable: true,
      get: () => value,
      set(v: Chunk[]) {
        value = v;
        if (Array.isArray(v)) hookChunkArray(v);
        Object.defineProperty(win, CHUNK_GLOBAL, { value: v, writable: true, configurable: true, enumerable: true });
      },
    });
    restoreHooks.push(() => {
      const d = Object.getOwnPropertyDescriptor(win, CHUNK_GLOBAL);
      if (d?.set) delete win[CHUNK_GLOBAL];
    });
    hookRuntimeCreation(win);
    log.info("Loaded early: waiting for Discord's webpack");
  }
  return mode;
}

/** Undo the chunk hooks (patched factories that already ran stay patched until reload). */
export function teardown() {
  for (const r of restoreHooks.reverse()) {
    try {
      r();
    } catch (err) {
      log.warn("Failed to restore webpack hook", err);
    }
  }
  restoreHooks = [];
  listeners.clear();
  patches.length = 0;
  mode = "uninitialised";
  requires.clear();
}

/** Every webpack runtime found so far (probing the chunk array for any not seen yet). */
function allRequires(win: any = globalThis): WebpackRequire[] {
  const chunks = win[CHUNK_GLOBAL];
  if (Array.isArray(chunks)) {
    try {
      // Each runtime on the array runs a chunk's runtime callback with its own require.
      chunks.push([[Symbol("corrode")], {}, (r: WebpackRequire) => void (r && r.m && requires.add(r))]);
      chunks.pop();
    } catch (err) {
      log.warn("Could not obtain webpack require", err);
    }
  }
  return [...requires].filter((r) => r && r.m);
}

/** Returns Discord's main `__webpack_require__` (the runtime with the most modules), if it is up. */
export function getRequire(win: any = globalThis): WebpackRequire | undefined {
  let best: WebpackRequire | undefined;
  let bestSize = -1;
  for (const r of allRequires(win)) {
    const size = Object.keys(r.m).length;
    if (size > bestSize) {
      best = r;
      bestSize = size;
    }
  }
  return best;
}

/* ------------------------------------------------------------------ */
/* Finding modules                                                     */
/* ------------------------------------------------------------------ */

const PROBE = `__corrode_probe_${Math.random().toString(36).slice(2)}`;

function isUsable(value: any): boolean {
  if (value == null) return false;
  const t = typeof value;
  if (t !== "object" && t !== "function") return false;
  if (value === globalThis || (typeof Window !== "undefined" && value instanceof Window)) return false;
  try {
    // Proxies that claim to have every property would match every filter.
    if (value[PROBE] !== undefined) return false;
  } catch {
    return false;
  }
  return true;
}

/** Yields the export object, `default`, and every named export of a module. */
function* candidates(exports: any): Generator<any> {
  if (!isUsable(exports)) return;
  yield exports;
  if (typeof exports !== "object") return;
  let keys: string[];
  try {
    keys = Object.keys(exports);
  } catch {
    return;
  }
  for (const k of keys) {
    let v: any;
    try {
      v = exports[k];
    } catch {
      continue;
    }
    if (v !== exports && isUsable(v)) yield v;
  }
}

function test(filter: Filter, value: any): boolean {
  try {
    return !!filter(value);
  } catch {
    return false;
  }
}

function notify(exports: any, id: PropertyKey) {
  if (!listeners.size) return;
  for (const l of [...listeners]) {
    for (const c of candidates(exports)) {
      if (test(l.filter, c)) {
        try {
          l.cb(c, id);
        } catch (err) {
          log.error("onModule callback threw", err);
        }
        break;
      }
    }
  }
}

export interface FindOptions {
  /** Return every match instead of the first. */
  all?: boolean;
}

/** Searches modules that have already executed. */
export function find(filter: Filter): any;
export function find(filter: Filter, opts: { all: true }): any[];
export function find(filter: Filter, opts: FindOptions = {}): any {
  const found: any[] = [];
  const caches = allRequires().map((r) => r.c).filter(Boolean);
  for (const mod of caches.flatMap((c) => Object.values(c))) {
    if (!mod) continue;
    for (const c of candidates(mod.exports)) {
      if (test(filter, c)) {
        if (!opts.all) return c;
        found.push(c);
        break;
      }
    }
  }
  return opts.all ? found : undefined;
}

/** Calls `cb` for every module matching `filter` that executes from now on. Returns an unsubscribe. */
export function onModule(filter: Filter, cb: (exports: any, id: PropertyKey) => void): () => void {
  const l = { filter, cb };
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Resolves with the first match, now or when the module eventually loads. */
export function waitFor(filter: Filter, opts: { signal?: AbortSignal; timeout?: number } = {}): Promise<any> {
  const existing = find(filter);
  if (existing !== undefined) return Promise.resolve(existing);
  return new Promise((resolve, reject) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const stop = onModule(filter, (exp) => {
      cleanup();
      resolve(exp);
    });
    const onAbort = () => {
      cleanup();
      reject(new Error("waitFor aborted"));
    };
    function cleanup() {
      stop();
      if (timer) clearTimeout(timer);
      opts.signal?.removeEventListener("abort", onAbort);
    }
    opts.signal?.addEventListener("abort", onAbort);
    if (opts.timeout) {
      timer = setTimeout(() => {
        cleanup();
        // A module may have appeared without running through a wrapped factory.
        const late = find(filter);
        if (late !== undefined) resolve(late);
        else reject(new Error(`waitFor timed out after ${opts.timeout}ms`));
      }, opts.timeout);
    }
  });
}

/**
 * Returns a proxy that resolves the module on first use. Handy for top-level
 * `const Foo = lazy(filters.byProps("foo"))` declarations in plugins.
 */
export function lazy<T = any>(filter: Filter): T {
  let cached: any;
  const get = () => (cached ??= find(filter));
  return new Proxy(function () {} as any, {
    get: (_, p) => {
      const t = get();
      if (t == null) {
        if (p === "then") return undefined;
        throw new Error(`Corrode lazy module not found (accessing "${String(p)}")`);
      }
      const v = Reflect.get(t, p);
      return typeof v === "function" ? v.bind(t) : v;
    },
    set: (_, p, v) => Reflect.set(get(), p, v),
    has: (_, p) => p in (get() ?? {}),
    apply: (_, self, args) => Reflect.apply(get(), self, args),
    construct: (_, args) => Reflect.construct(get(), args),
    ownKeys: () => Reflect.ownKeys(get() ?? {}),
    getOwnPropertyDescriptor: (_, p) => {
      const d = Reflect.getOwnPropertyDescriptor(get() ?? {}, p);
      if (d) d.configurable = true;
      return d;
    },
  }) as T;
}

/** Finds the source text of module factories (patched where applicable). */
export function findModuleId(...code: string[]): PropertyKey | undefined {
  const req = getRequire();
  if (!req) return undefined;
  for (const id of Object.keys(req.m)) {
    const src = Function.prototype.toString.call(req.m[id]);
    if (code.every((c) => src.includes(c))) return id;
  }
  return undefined;
}

/* ------------------------------------------------------------------ */
/* Filters                                                             */
/* ------------------------------------------------------------------ */

export const filters = {
  /** Object (or function) that has all of these properties. */
  byProps:
    (...props: string[]): Filter =>
    (m) =>
      props.every((p) => m[p] !== undefined),
  /** Class/constructor whose prototype has all of these properties. */
  byPrototype:
    (...props: string[]): Filter =>
    (m) =>
      typeof m === "function" && m.prototype != null && props.every((p) => m.prototype[p] !== undefined),
  /** Function whose source contains all of these strings. */
  byCode:
    (...code: string[]): Filter =>
    (m) => {
      if (typeof m !== "function") return false;
      const src = Function.prototype.toString.call(m);
      return code.every((c) => src.includes(c));
    },
  /** Flux store by name, e.g. `"UserStore"`. */
  byStoreName:
    (name: string): Filter =>
    (m) =>
      typeof m?.getName === "function" &&
      typeof m.addChangeListener === "function" &&
      (m.constructor?.displayName === name || m.getName() === name),
  /** React component (or anything) with this `displayName`. */
  byDisplayName:
    (name: string): Filter =>
    (m) =>
      m?.displayName === name,
};

export const findByProps = (...props: string[]) => find(filters.byProps(...props));
export const findByPrototype = (...props: string[]) => find(filters.byPrototype(...props));
export const findByCode = (...code: string[]) => find(filters.byCode(...code));
export const findStore = (name: string) => find(filters.byStoreName(name));
export const findByDisplayName = (name: string) => find(filters.byDisplayName(name));
