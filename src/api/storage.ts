import { getNative } from "../native";
import { createLogger } from "./logger";

/**
 * Persistent, synchronous JSON storage split into namespaces.
 *
 * Backends, in order of preference:
 *  1. `CorrodeNative.storage` provided by the injector (survives cache/storage clears).
 *  2. `localStorage`. Discord deletes `window.localStorage`, so we borrow it from a
 *     throwaway same-origin iframe, the same way Discord itself does.
 *  3. In-memory (nothing persists; a warning is logged).
 *
 * Each namespace is stored as one JSON blob under `corrode:<namespace>`.
 */

const log = createLogger("Storage");
const PREFIX = "corrode:";

export interface Backend {
  name: string;
  get(key: string): string | null | undefined;
  set(key: string, value: string): void;
  delete(key: string): void;
}

function findLocalStorage(): Storage | undefined {
  try {
    const ls = (globalThis as any).localStorage as Storage | undefined;
    if (ls) return ls;
  } catch {
    /* access can throw */
  }
  if (typeof document === "undefined") return undefined;
  try {
    const frame = document.createElement("iframe");
    frame.style.display = "none";
    (document.body ?? document.documentElement).appendChild(frame);
    const ls = frame.contentWindow?.localStorage;
    frame.remove();
    return ls;
  } catch {
    return undefined;
  }
}

function memoryBackend(): Backend {
  const map = new Map<string, string>();
  return {
    name: "memory",
    get: (k) => map.get(k),
    set: (k, v) => void map.set(k, v),
    delete: (k) => void map.delete(k),
  };
}

export function detectBackend(): Backend {
  const native = getNative().storage;
  if (native) return { name: "native", get: (k) => native.get(k), set: (k, v) => native.set(k, v), delete: (k) => native.delete(k) };
  const ls = findLocalStorage();
  if (ls) return { name: "localStorage", get: (k) => ls.getItem(k), set: (k, v) => ls.setItem(k, v), delete: (k) => ls.removeItem(k) };
  log.warn("No persistent storage available; settings will be lost on reload");
  return memoryBackend();
}

let backend: Backend | undefined;
export function getBackend(): Backend {
  return (backend ??= detectBackend());
}
/** Override the backend (tests, or an injector that wants to set it explicitly). */
export function setBackend(b: Backend) {
  backend = b;
  stores.clear();
}

type Listener = (value: any, key: string) => void;

export interface Store<T extends Record<string, any> = Record<string, any>> {
  readonly namespace: string;
  get<K extends keyof T & string>(key: K): T[K] | undefined;
  get<K extends keyof T & string>(key: K, fallback: T[K]): T[K];
  set<K extends keyof T & string>(key: K, value: T[K]): void;
  delete(key: keyof T & string): void;
  has(key: keyof T & string): boolean;
  keys(): string[];
  /** A snapshot of all values. */
  all(): Partial<T>;
  /** Listen to one key, or to every key with `"*"`. Returns an unsubscribe. */
  subscribe(key: (keyof T & string) | "*", cb: Listener): () => void;
  /** Delete the whole namespace. */
  clear(): void;
}

const stores = new Map<string, Store<any>>();

export function createStore<T extends Record<string, any> = Record<string, any>>(namespace: string): Store<T> {
  const existing = stores.get(namespace);
  if (existing) return existing as Store<T>;

  const storageKey = PREFIX + namespace;
  let data: Record<string, any> = {};
  try {
    const raw = getBackend().get(storageKey);
    if (raw) data = JSON.parse(raw) ?? {};
  } catch (err) {
    log.error(`Namespace "${namespace}" is corrupt, starting fresh`, err);
  }
  const listeners = new Map<string, Set<Listener>>();

  const persist = () => {
    try {
      if (Object.keys(data).length) getBackend().set(storageKey, JSON.stringify(data));
      else getBackend().delete(storageKey);
    } catch (err) {
      log.error(`Failed to save namespace "${namespace}"`, err);
    }
  };
  const emit = (key: string, value: any) => {
    for (const k of [key, "*"]) {
      for (const cb of listeners.get(k) ?? []) {
        try {
          cb(value, key);
        } catch (err) {
          log.error("Storage listener threw", err);
        }
      }
    }
  };
  const clone = <V>(v: V): V => (v === undefined ? v : structuredClone(v));

  const store: Store<T> = {
    namespace,
    get(key: string, fallback?: any) {
      return key in data ? clone(data[key]) : fallback;
    },
    set(key, value) {
      if (value === undefined) return store.delete(key);
      data[key] = clone(value);
      persist();
      emit(key, clone(value));
    },
    delete(key) {
      if (!(key in data)) return;
      delete data[key];
      persist();
      emit(key, undefined);
    },
    has: (key) => key in data,
    keys: () => Object.keys(data),
    all: () => clone(data) as Partial<T>,
    subscribe(key, cb) {
      let set = listeners.get(key);
      if (!set) listeners.set(key, (set = new Set()));
      set.add(cb);
      return () => set!.delete(cb);
    },
    clear() {
      const keys = Object.keys(data);
      data = {};
      persist();
      for (const k of keys) emit(k, undefined);
    },
  };
  stores.set(namespace, store);
  return store;
}
