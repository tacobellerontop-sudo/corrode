import { getDispatcher } from "./flux";
import { filters, find, findStore } from "./webpack";

/**
 * Frequently used Discord modules, resolved lazily on first access and cached.
 * Each getter returns `undefined` if the module cannot be found (e.g. Discord changed).
 */

const cache = new Map<string, any>();
function cached(key: string, resolve: () => any) {
  if (cache.has(key)) return cache.get(key);
  const v = resolve();
  if (v !== undefined) cache.set(key, v);
  return v;
}

export const common = {
  get React(): any {
    return cached("React", () => find(filters.byProps("useState", "useEffect", "createElement", "Fragment")));
  },
  get ReactDOM(): any {
    return cached("ReactDOM", () => find(filters.byProps("createPortal", "flushSync")));
  },
  get FluxDispatcher(): any {
    return getDispatcher();
  },
  get UserStore(): any {
    return cached("UserStore", () => findStore("UserStore"));
  },
  get ChannelStore(): any {
    return cached("ChannelStore", () => findStore("ChannelStore"));
  },
  get SelectedChannelStore(): any {
    return cached("SelectedChannelStore", () => findStore("SelectedChannelStore"));
  },
  get GuildStore(): any {
    return cached("GuildStore", () => findStore("GuildStore"));
  },
  get MessageStore(): any {
    return cached("MessageStore", () => findStore("MessageStore"));
  },
  /** Returns the logged-in user, if available. */
  getCurrentUser(): any {
    return common.UserStore?.getCurrentUser?.();
  },
};

export function clearCommonCache() {
  cache.clear();
}
