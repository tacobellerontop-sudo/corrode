// A miniature imitation of Discord's webpack runtime and a few of its modules, used by the
// unit tests and the browser smoke test. It mirrors the real JSONP chunk-loading behaviour,
// including webpack replacing `push` on the chunk array.

export function installRuntime(win = globalThis) {
  const modules = {};
  const cache = {};
  function wreq(id) {
    if (cache[id]) return cache[id].exports;
    const module = (cache[id] = { id, loaded: false, exports: {} });
    modules[id].call(module.exports, module, module.exports, wreq);
    module.loaded = true;
    return module.exports;
  }
  wreq.m = modules;
  wreq.c = cache;
  wreq.d = (exports, defs) => {
    for (const k in defs) Object.defineProperty(exports, k, { enumerable: true, get: defs[k] });
  };
  const installed = {};
  function jsonpCallback(parentPush, data) {
    const [ids, mods, runtime] = data;
    for (const id in mods) modules[id] = mods[id];
    if (runtime) runtime(wreq);
    if (parentPush) parentPush(data);
    for (const id of ids) installed[id] = 0;
  }
  const g = (win.webpackChunkdiscord_app = win.webpackChunkdiscord_app || []);
  g.forEach(jsonpCallback.bind(null, 0));
  g.push = jsonpCallback.bind(null, g.push.bind(g));
  return wreq;
}

// Factories are written as strings-in-functions on purpose so source patches can match them.
export const coreModules = {
  // Flux dispatcher
  100: function (module, exports, require) {
    class Dispatcher {
      constructor() {
        this._subs = {};
      }
      dispatch(action) {
        for (const cb of this._subs[action.type] ?? []) cb(action);
        return Promise.resolve();
      }
      subscribe(type, cb) {
        (this._subs[type] ??= new Set()).add(cb);
      }
      unsubscribe(type, cb) {
        this._subs[type]?.delete(cb);
      }
      register() {}
    }
    const instance = new Dispatcher();
    require.d(exports, { Z: () => instance });
  },
  // Typing actions
  200: function (module, exports, require) {
    const calls = [];
    const TypingActions = {
      calls,
      startTyping(channelId) {
        calls.push(channelId);
      },
      stopTyping() {},
    };
    require.d(exports, { Z: () => TypingActions });
  },
  // A store
  300: function (module, exports, require) {
    class UserStore {
      getName() {
        return "UserStore";
      }
      addChangeListener() {}
      getCurrentUser() {
        return { id: "1", username: "me" };
      }
    }
    UserStore.displayName = "UserStore";
    require.d(exports, { default: () => new UserStore() });
  },
  // A module that source patches target
  400: function (module, exports, require) {
    require.d(exports, {
      greet: () =>
        function greet(name) {
          return "Hello, " + name + "!";
        },
    });
  },
};

/** Pushes a chunk and executes its modules, like Discord does when a chunk loads. */
export function loadChunk(win, modules, run = true) {
  win.webpackChunkdiscord_app.push([[Symbol("chunk")], modules, run ? (r) => Object.keys(modules).forEach((id) => r(id)) : undefined]);
}
