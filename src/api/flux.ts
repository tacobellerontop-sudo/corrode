import { createLogger } from "./logger";
import { instead, type Unpatch } from "./patcher";
import { filters, find, waitFor } from "./webpack";

/**
 * Hooks into Discord's Flux dispatcher: subscribe to actions, intercept (and optionally
 * block or rewrite) actions before stores see them, and dispatch your own.
 */

const log = createLogger("Flux");

export interface FluxAction {
  type: string;
  [key: string]: any;
}

export interface FluxDispatcher {
  dispatch(action: FluxAction): unknown;
  subscribe(type: string, cb: (action: any) => void): void;
  unsubscribe(type: string, cb: (action: any) => void): void;
  [key: string]: any;
}

/** Return `true` to block the action. The action object may be mutated in place. */
export type Interceptor = (action: FluxAction) => boolean | void;

export const dispatcherFilter = filters.byProps("dispatch", "subscribe", "unsubscribe", "register");

let dispatcher: FluxDispatcher | undefined;
const interceptors = new Set<Interceptor>();
let unpatchDispatch: Unpatch | undefined;

export function getDispatcher(): FluxDispatcher | undefined {
  return (dispatcher ??= find(dispatcherFilter));
}

export async function waitForDispatcher(opts?: { signal?: AbortSignal; timeout?: number }): Promise<FluxDispatcher> {
  return (dispatcher ??= await waitFor(dispatcherFilter, opts));
}

function requireDispatcher(): FluxDispatcher {
  const d = getDispatcher();
  if (!d) throw new Error("Flux dispatcher not available yet");
  return d;
}

function installInterceptorHook(d: FluxDispatcher) {
  if (unpatchDispatch) return;
  unpatchDispatch = instead(
    d,
    "dispatch",
    (args, original) => {
      const action = args[0] as FluxAction;
      if (action && typeof action === "object") {
        for (const i of [...interceptors]) {
          try {
            if (i(action) === true) return Promise.resolve();
          } catch (err) {
            log.error(`Interceptor threw on ${action.type}`, err);
          }
        }
      }
      return original(...args);
    },
    "corrode:flux",
  );
}

/** Subscribe to an action type. Returns an unsubscribe. */
export function subscribe<A extends FluxAction = FluxAction>(type: string, cb: (action: A) => void): () => void {
  const d = requireDispatcher();
  const safe = (action: A) => {
    try {
      cb(action);
    } catch (err) {
      log.error(`Subscriber for ${type} threw`, err);
    }
  };
  d.subscribe(type, safe);
  return () => d.unsubscribe(type, safe);
}

/** Intercept every action before it reaches stores. Return `true` to block it. */
export function intercept(cb: Interceptor): () => void {
  installInterceptorHook(requireDispatcher());
  interceptors.add(cb);
  return () => {
    interceptors.delete(cb);
    if (!interceptors.size) {
      unpatchDispatch?.();
      unpatchDispatch = undefined;
    }
  };
}

export function dispatch(action: FluxAction): unknown {
  return requireDispatcher().dispatch(action);
}

export function teardown() {
  interceptors.clear();
  unpatchDispatch?.();
  unpatchDispatch = undefined;
  dispatcher = undefined;
}

export interface ScopedFlux {
  subscribe<A extends FluxAction = FluxAction>(type: string, cb: (action: A) => void): () => void;
  intercept(cb: Interceptor): () => void;
  dispatch(action: FluxAction): unknown;
  disposeAll(): void;
}

export function createScopedFlux(): ScopedFlux {
  const disposers = new Set<() => void>();
  const track = (d: () => void) => {
    const w = () => {
      disposers.delete(w);
      d();
    };
    disposers.add(w);
    return w;
  };
  return {
    subscribe: (type, cb) => track(subscribe(type, cb)),
    intercept: (cb) => track(intercept(cb)),
    dispatch,
    disposeAll() {
      for (const d of [...disposers]) d();
    },
  };
}
