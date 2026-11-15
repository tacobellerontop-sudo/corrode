// A miniature imitation of Discord's webpack runtime and a few of its modules, used by the
// unit tests and the browser smoke test. It mirrors the real JSONP chunk-loading behaviour,
// including webpack replacing `push` on the chunk array.

/** `bundled` are modules shipped inside the runtime script itself; they never pass through the chunk array. */
export function installRuntime(win = globalThis, bundled = {}) {
  const modules = { ...bundled };
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
      getUser(id) {
        return { 1: { id: "1", username: "me" }, 2: { id: "2", username: "ana", globalName: "Ana" }, 3: { id: "3", username: "helper", bot: true } }[id];
      }
    }
    UserStore.displayName = "UserStore";
    require.d(exports, { default: () => new UserStore() });
  },
  // Message actions
  500: function (module, exports, require) {
    const sent = [];
    const MessageActions = {
      sent,
      sendMessage(channelId, message) {
        sent.push({ channelId, content: message.content });
        return Promise.resolve({ ok: true });
      },
      editMessage(channelId, messageId, message) {
        sent.push({ channelId, messageId, content: message.content });
        return Promise.resolve({ ok: true });
      },
      startEditMessage(channelId, messageId, content) {
        MessageActions.editing = { channelId, messageId, content };
      },
    };
    require.d(exports, { Z: () => MessageActions });
  },
  // More stores: users 1 (me), 2 (ana, a friend), 3 (a bot); DM channel 9 and #general (10) in guild 50.
  600: function (module, exports, require) {
    const users = {
      1: { id: "1", username: "me" },
      2: { id: "2", username: "ana", globalName: "Ana" },
      3: { id: "3", username: "helper", bot: true },
    };
    const channels = { 9: { id: "9", type: 1, recipients: ["2"] }, 10: { id: "10", type: 0, name: "general", guild_id: "50" } };
    const messages = {
      "9:100": { id: "100", channel_id: "9", author: users[2], content: "hello [x](https://example.org)", mentions: [] },
      "10:101": { id: "101", channel_id: "10", author: users[1], content: "mine", mentions: [] },
      "10:102": { id: "102", channel_id: "10", author: users[3], content: "beep", mentions: [] },
      "10:103": { id: "103", channel_id: "10", author: users[2], content: "hey <@1>", mentions: ["1"], mentioned: true },
    };
    const make = (name, methods) => {
      const Store = class {
        getName() {
          return name;
        }
        addChangeListener() {}
      };
      Store.displayName = name;
      return Object.assign(new Store(), methods);
    };
    require.d(exports, {
      a: () => make("ChannelStore", { getChannel: (id) => channels[id] }),
      b: () => make("MessageStore", { getMessage: (c, id) => messages[`${c}:${id}`] }),
      c: () => make("RelationshipStore", { isFriend: (id) => id === "2", isBlocked: () => false, isIgnored: () => false, getNickname: () => undefined }),
      d: () => make("PresenceStore", { getStatus: () => "offline" }),
      e: () => make("SelectedChannelStore", { getChannelId: () => "10", getVoiceChannelId: () => "77" }),
      f: () => make("GuildStore", { getGuild: (id) => (id === "50" ? { id: "50", name: "Test Server" } : undefined) }),
    });
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
