# Writing plugins

A plugin is a plain object. Built-in plugins live in [`src/builtins`](../src/builtins) and are
registered in `src/builtins/index.ts`. External plugins are single JavaScript files that users
install from a URL in Settings > Plugins (see [`examples/hello-world.plugin.js`](../examples/hello-world.plugin.js)).

```ts
import { definePlugin } from "../plugins/types";

export default definePlugin({
  id: "myPlugin",              // stable; used as the storage key
  name: "My Plugin",
  description: "What it does, in one line.",
  authors: ["you"],
  enabledByDefault: false,

  settings: {                  // rendered automatically in the settings window
    loud: { type: "boolean", label: "Be loud", default: false },
    name: { type: "string", label: "Name", default: "", placeholder: "…" },
    size: { type: "number", label: "Size", default: 12, min: 8, max: 32 },
    mode: { type: "select", label: "Mode", default: "a", options: [{ label: "A", value: "a" }, { label: "B", value: "b" }] },
  },

  css: (s) => `.foo { font-size: ${s.size}px }`,   // or a plain string

  flux: {
    MESSAGE_CREATE(action, ctx) { /* ... */ },
  },

  async start(ctx) { /* set things up */ },
  stop(ctx) { /* optional: anything not covered by automatic cleanup */ },
  onSettingChange(key, value, ctx) { /* optional */ },
});
```

External plugins use the same shape, written as `module.exports = corrode.definePlugin({ ... })`.

## The context (`ctx`)

Everything registered through `ctx` is removed automatically when the plugin stops, so most
plugins don't need a `stop()`.

| Member | |
| --- | --- |
| `ctx.settings` | Live values. Read `ctx.settings.loud`; assign to save. |
| `ctx.storage` | Private persistent key/value store (`get`, `set`, `delete`, `subscribe`, …). |
| `ctx.patcher` | `before`, `instead`, `after` function patches (see below). |
| `ctx.flux` | `subscribe(type, cb)`, `intercept(cb)`, `dispatch(action)`. |
| `ctx.css` | `inject(css)` returns `{ update, remove }`. |
| `ctx.signal` | `AbortSignal` aborted on stop. Pass it to `waitFor`. |
| `ctx.toast(msg, opts?)` | Show a toast. |
| `ctx.logger` | Prefixed console logger. |
| `ctx.onStop(fn)` | Register extra cleanup. |

## Finding Discord's modules

Discord ships as a webpack bundle. Corrode finds modules by shape:

```js
const { webpack } = corrode;                        // or import from "../api/webpack" in built-ins
const { filters } = webpack;

webpack.findByProps("startTyping", "stopTyping");   // an object with these keys
webpack.findStore("UserStore");                     // a Flux store
webpack.findByCode("some string in the function");  // a function by its source
webpack.find(filters.byPrototype("render"));        // any filter, or your own (m) => boolean
webpack.find(filter, { all: true });                // every match

await webpack.waitFor(filters.byProps("foo"), { signal: ctx.signal, timeout: 30_000 });
const Lazy = webpack.lazy(filters.byProps("foo")); // resolves on first use
```

Common modules are pre-resolved in `corrode.common`: `React`, `ReactDOM`, `FluxDispatcher`,
`UserStore`, `ChannelStore`, `SelectedChannelStore`, `GuildStore`, `MessageStore`,
`getCurrentUser()`.

## Function patches

```js
ctx.patcher.before(obj, "method", (args, self) => { /* return a new args array to replace them */ });
ctx.patcher.instead(obj, "method", (args, original, self) => original(...args));
ctx.patcher.after(obj, "method", (args, result, self) => { /* return a value to replace result */ });
```

Each returns an `unpatch()` function. Patches stack, a throwing callback is logged and skipped
(Discord keeps working), and the original is restored when the last patch is removed.

## Flux

```js
ctx.flux.subscribe("CHANNEL_SELECT", (action) => { ... });
ctx.flux.intercept((action) => action.type === "TYPING_START"); // return true to block
```

Interceptors run before Discord's stores see the action and may mutate it.

## Source patches

For changes that need Discord's code itself rewritten, declare `patches`. They're applied to
module source before Discord runs it, so they need Corrode to be loaded early
([INJECTOR.md](INJECTOR.md#early-vs-late-loading)), and toggling the plugin needs a restart.

```js
patches: [
  {
    find: "some unique string in the module",          // or an array of strings, or a RegExp
    replacement: {
      match: /([\w$]+)\.isStaff\(\)/,                  // string or RegExp
      replace: "$self.isStaff($1)",                    // $self = this plugin definition
    },
  },
],
isStaff(user) { return true; },
```

Corrode warns in the console (and in Settings > About) when a patch never applies, which usually
means Discord changed the code it targets.

## Tips

- Keep `id`s stable: settings are stored under them.
- Prefer `waitFor` with `ctx.signal` over polling.
- Selectors based on `aria-label` text only work in one language; prefer `href`s, roles, or class
  name fragments like `[class*="sidebar"]` where you can.
