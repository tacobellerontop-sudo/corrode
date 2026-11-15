import { createLogger } from "./logger";

/**
 * Runtime function patcher (monkey patching) with clean, independent unpatching.
 *
 * Every patched method is replaced once by a wrapper that runs the registered
 * `before`, `instead` and `after` callbacks. Removing the last callback restores the
 * original function, provided nobody else replaced the method on top of us.
 * Callbacks are isolated: a throwing callback is logged and skipped, it never breaks
 * the original call.
 */

const log = createLogger("Patcher");
const STATE = Symbol.for("corrode.patcher.state");

export type Unpatch = () => void;

/** Runs before the original. Return a new array to replace the arguments. */
export type BeforeCallback = (args: any[], self: any) => any[] | void;
/** Runs instead of the original. Call `original(...args)` to run the previous implementation. */
export type InsteadCallback = (args: any[], original: (...args: any[]) => any, self: any) => any;
/** Runs after the original. Return anything other than `undefined` to replace the result. */
export type AfterCallback = (args: any[], result: any, self: any) => any;

interface Hook<T> {
  cb: T;
  owner: string;
}

interface PatchState {
  original: (...args: any[]) => any;
  wrapper: (...args: any[]) => any;
  before: Hook<BeforeCallback>[];
  instead: Hook<InsteadCallback>[];
  after: Hook<AfterCallback>[];
}

function getState(fn: unknown): PatchState | undefined {
  return typeof fn === "function" ? (fn as any)[STATE] : undefined;
}

function install(obj: any, key: PropertyKey): PatchState {
  const current = obj[key];
  const existing = getState(current);
  if (existing) return existing;
  if (typeof current !== "function") {
    throw new TypeError(`Cannot patch ${String(key)}: not a function (got ${typeof current})`);
  }

  const state: PatchState = {
    original: current,
    wrapper: undefined as any,
    before: [],
    instead: [],
    after: [],
  };

  const wrapper = function (this: any, ...args: any[]) {
    const self = this;
    const isConstruct = new.target !== undefined;

    for (const hook of state.before.slice()) {
      try {
        const res = hook.cb(args, self);
        if (Array.isArray(res)) args = res;
      } catch (err) {
        log.error(`before-patch from "${hook.owner}" on ${String(key)} threw`, err);
      }
    }

    const callOriginal = (...a: any[]) =>
      isConstruct ? Reflect.construct(state.original, a, new.target) : state.original.apply(self, a);

    let call: (...a: any[]) => any = callOriginal;
    for (const hook of state.instead.slice()) {
      const prev = call;
      call = (...a: any[]) => {
        try {
          return hook.cb(a, prev, self);
        } catch (err) {
          log.error(`instead-patch from "${hook.owner}" on ${String(key)} threw, falling back`, err);
          return prev(...a);
        }
      };
    }

    let result = call(...args);

    for (const hook of state.after.slice()) {
      try {
        const res = hook.cb(args, result, self);
        if (res !== undefined) result = res;
      } catch (err) {
        log.error(`after-patch from "${hook.owner}" on ${String(key)} threw`, err);
      }
    }
    return result;
  };

  // Keep the wrapper looking like the original (statics, name, length, toString).
  for (const prop of Reflect.ownKeys(current)) {
    if (prop === "prototype" || prop === "length" || prop === "name" || prop === "arguments" || prop === "caller") continue;
    try {
      Object.defineProperty(wrapper, prop, Object.getOwnPropertyDescriptor(current, prop)!);
    } catch {
      /* non-configurable, ignore */
    }
  }
  try {
    Object.defineProperty(wrapper, "name", { value: current.name, configurable: true });
    Object.defineProperty(wrapper, "length", { value: current.length, configurable: true });
  } catch {
    /* ignore */
  }
  if (current.prototype) wrapper.prototype = current.prototype;
  Object.defineProperty(wrapper, "toString", {
    value: () => Function.prototype.toString.call(state.original),
    configurable: true,
    writable: true,
  });
  Object.defineProperty(wrapper, STATE, { value: state });
  state.wrapper = wrapper;

  assign(obj, key, wrapper);
  return state;
}

function assign(obj: any, key: PropertyKey, value: unknown) {
  const desc = Object.getOwnPropertyDescriptor(obj, key);
  if (!desc || desc.writable || desc.set) {
    try {
      obj[key] = value;
      if (obj[key] === value) return;
    } catch {
      /* fall through to defineProperty */
    }
  }
  Object.defineProperty(obj, key, {
    value,
    writable: true,
    configurable: true,
    enumerable: desc?.enumerable ?? true,
  });
}

function addHook<K extends "before" | "instead" | "after">(
  kind: K,
  obj: any,
  key: PropertyKey,
  cb: PatchState[K][number]["cb"],
  owner: string,
): Unpatch {
  const state = install(obj, key);
  const hook = { cb, owner } as PatchState[K][number];
  (state[kind] as any[]).push(hook);

  let done = false;
  return () => {
    if (done) return;
    done = true;
    const list = state[kind] as any[];
    const idx = list.indexOf(hook);
    if (idx !== -1) list.splice(idx, 1);
    if (!state.before.length && !state.instead.length && !state.after.length && obj[key] === state.wrapper) {
      assign(obj, key, state.original);
    }
  };
}

export function before(obj: any, key: PropertyKey, cb: BeforeCallback, owner = "anonymous"): Unpatch {
  return addHook("before", obj, key, cb, owner);
}

export function instead(obj: any, key: PropertyKey, cb: InsteadCallback, owner = "anonymous"): Unpatch {
  return addHook("instead", obj, key, cb, owner);
}

export function after(obj: any, key: PropertyKey, cb: AfterCallback, owner = "anonymous"): Unpatch {
  return addHook("after", obj, key, cb, owner);
}

/** Returns the unpatched function if `fn` is a Corrode wrapper, otherwise `fn` itself. */
export function getOriginal<T>(fn: T): T {
  return (getState(fn)?.original as T) ?? fn;
}

export function isPatched(fn: unknown): boolean {
  return !!getState(fn);
}

export interface ScopedPatcher {
  before(obj: any, key: PropertyKey, cb: BeforeCallback): Unpatch;
  instead(obj: any, key: PropertyKey, cb: InsteadCallback): Unpatch;
  after(obj: any, key: PropertyKey, cb: AfterCallback): Unpatch;
  /** Removes every patch made through this scoped patcher. */
  unpatchAll(): void;
}

/** A patcher whose patches are all tracked and can be removed together (used for plugins). */
export function createScopedPatcher(owner: string): ScopedPatcher {
  const unpatches = new Set<Unpatch>();
  const track = (u: Unpatch): Unpatch => {
    const wrapped = () => {
      unpatches.delete(wrapped);
      u();
    };
    unpatches.add(wrapped);
    return wrapped;
  };
  return {
    before: (obj, key, cb) => track(before(obj, key, cb, owner)),
    instead: (obj, key, cb) => track(instead(obj, key, cb, owner)),
    after: (obj, key, cb) => track(after(obj, key, cb, owner)),
    unpatchAll() {
      for (const u of [...unpatches].reverse()) u();
    },
  };
}
