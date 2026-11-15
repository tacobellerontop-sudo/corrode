# Making plugins

A Corrode plugin is one JavaScript object that says what it is and what it does. This guide
walks through making your own, adding it to Corrode, and everything a plugin can use.

- [Your first plugin](#your-first-plugin)
- [Adding a plugin to Corrode](#adding-a-plugin-to-corrode)
- [What a plugin can contain](#what-a-plugin-can-contain)
- [Recipes](#recipes)
- [The context (`ctx`)](#the-context-ctx)
- [Finding Discord's modules](#finding-discords-modules)
- [Function patches](#function-patches), [Flux](#flux), [Source patches](#source-patches)
- [Making it a built-in plugin](#making-it-a-built-in-plugin)
- [When it doesn't work](#when-it-doesnt-work)

## Your first plugin

Save this as `my-plugin.js` (or just copy it):

```js
module.exports = corrode.definePlugin({
  id: "myPlugin",                 // unique, never change it: settings are stored under it
  name: "My Plugin",
  description: "Says hello and makes my own messages green.",
  authors: ["you"],

  // Shown as controls on the plugin's card. Read them with ctx.settings.<key>.
  settings: {
    greeting: { type: "string", label: "Greeting", default: "Hello!" },
  },

  // CSS that is on the page while the plugin is enabled.
  css: `[class*="mentioned"] { outline: 1px solid #23a55a; }`,

  // Runs when the plugin is enabled (and at startup if it was left enabled).
  start(ctx) {
    ctx.toast(ctx.settings.greeting);
  },
});
```

Two rules: the file must assign the plugin to `module.exports`, and it needs an `id`. `corrode`
(the whole public API, the same object as `window.corrode`) is available in the file.

## Adding a plugin to Corrode

Open Discord's **User Settings > Corrode > Plugins** (or press Ctrl/Cmd + Shift + O).

| How | Steps |
| --- | --- |
| **Paste the code** | Click **Paste code**, paste the file's contents, click **Install pasted plugin**. Quickest while you're writing one: change the code, paste it again, and it replaces the old version. |
| **From a link** | Put the file somewhere that serves it as plain text (a GitHub repo's *Raw* link, a gist's *Raw* link), paste the URL into **Install plugin from URL** and click **Install**. Plugins installed this way get an **Update** button that downloads the file again. |
| **From the console** | ``corrode.plugins.installFromSource(`...code...`)`` or `corrode.plugins.installFromUrl("https://...")`. |

The plugin then appears in the list with an **External** badge, switched on. Its settings are
under **Settings** on its card, and **Uninstall** removes it. Installed plugins are saved and load
again every time Discord starts.

Plugins run with full access to Discord and your account. Only install code you have read or
trust, and never paste something a stranger sends you.

## What a plugin can contain

Every part except `id` and `name` is optional. Use only what you need.

```js
module.exports = corrode.definePlugin({
  id: "myPlugin",
  name: "My Plugin",
  description: "What it does, in one line.",
  authors: ["you"],
  version: "1.0.0",

  settings: {                  // rendered automatically on the plugin's card
    loud: { type: "boolean", label: "Be loud", default: false },
    name: { type: "string", label: "Name", default: "", placeholder: "…" },   // multiline: true for a text box
    size: { type: "number", label: "Size", default: 12, min: 8, max: 32 },
    mode: { type: "select", label: "Mode", default: "a", options: [{ label: "A", value: "a" }, { label: "B", value: "b" }] },
  },

  css: (s) => `.foo { font-size: ${s.size}px }`,   // a string, or a function of the settings

  flux: {                      // react to things happening in Discord
    MESSAGE_CREATE(action, ctx) { /* a message arrived: action.message */ },
  },

  async start(ctx) { /* set things up */ },
  stop(ctx) { /* optional: undo anything Corrode can't undo for you */ },
  onSettingChange(key, value, ctx) { /* optional */ },
});
```

| Part | Use it for |
| --- | --- |
| `css` | Hiding, recolouring or resizing things. No JavaScript needed, and it updates live when a setting changes. |
| `flux` | Reacting to events: messages, typing, presence, channel switches. See [Flux](#flux). |
| `start(ctx)` | Everything else: patching Discord's functions, intercepting events, timers, DOM work. |
| `settings` | Options the user can change. Add `description` for help text and `restartRequired: true` if a change only applies after a restart. |

## Recipes

**Hide something.** Find the element in DevTools (Ctrl+Shift+I), then:

```js
css: `[class*="channelAppLauncher"] { display: none !important; }`,
```

Discord's class names look like `name_ab12cd` and the suffix changes between releases, so match
the stable part with `[class*="name"]`.

**React to a message.**

```js
flux: {
  MESSAGE_CREATE({ message }, ctx) {
    if (message.content.includes("pizza")) ctx.toast(`${message.author.username} mentioned pizza`);
  },
},
```

**Block or change an event before Discord sees it.**

```js
start(ctx) {
  ctx.flux.intercept((action) => {
    if (action.type === "TYPING_START") return true;   // true = Discord never sees it
  });
},
```

**Change what you send.**

```js
async start(ctx) {
  const { webpack } = corrode;
  const MessageActions = await webpack.waitFor(webpack.filters.byProps("sendMessage", "editMessage"), { signal: ctx.signal });
  ctx.patcher.before(MessageActions, "sendMessage", (args) => {
    const message = args[1];                            // args: channelId, message, ...
    message.content = message.content.replace(/\bteh\b/g, "the");
  });
},
```

**Remember something between restarts.**

```js
start(ctx) {
  const runs = (ctx.storage.get("runs") ?? 0) + 1;
  ctx.storage.set("runs", runs);
},
```

**Do something on a timer or to the page.** Clean up after yourself with `ctx.onStop`:

```js
start(ctx) {
  const timer = setInterval(() => ctx.logger.info("tick"), 60_000);
  ctx.onStop(() => clearInterval(timer));
},
```

The built-in plugins in [`src/builtins`](../src/builtins) are short, real examples of each of
these, and [`examples/hello-world.plugin.js`](../examples/hello-world.plugin.js) is a complete
installable plugin.

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

## Making it a built-in plugin

Built-in plugins ship inside Corrode and can be written in TypeScript with full type checking.

1. Create `src/builtins/myPlugin.ts`:

   ```ts
   import { definePlugin } from "../plugins/types";

   export default definePlugin({
     id: "myPlugin",
     name: "My Plugin",
     description: "What it does, in one line.",
     authors: ["you"],
     start(ctx) { /* ... */ },
   });
   ```

2. Add it to the list in `src/builtins/index.ts`, in the group it belongs to.
3. `npm run check` (types, tests, build and the browser smoke test, which turns every built-in
   plugin on at once), then `node scripts/list-plugins.mjs` to refresh
   [BUILTIN_PLUGINS.md](BUILTIN_PLUGINS.md).

`src/builtins/lib/helpers.ts` has ready-made pieces for the common jobs: `onSend` (change
outgoing messages), `watch` / `watchMessages` (handle elements as Discord adds them), `hotkey`,
`store("UserStore")`, `listen`, `every` and more. Each cleans up when the plugin stops.

Built-ins import helpers directly (`import { waitFor, filters } from "../api/webpack"`,
`import { common } from "../api/common"`) instead of going through the `corrode` global.

## When it doesn't work

- **"Plugin did not export a definition"**: the file has to end up assigning to `module.exports`.
- **An "Error" badge on the card**: `start()` threw. Hover the badge, or open DevTools
  (About > Open DevTools) and look for `[Corrode]` lines.
- **`waitFor` never finishes**: Discord doesn't have a module of that shape (any more). Give it
  a `timeout` so you get an error instead of silence, and check the names in DevTools with
  `corrode.webpack.findByProps("...")`.
- **CSS does nothing**: inspect the element; the class fragment or `aria-label` may differ, and
  `aria-label` text depends on Discord's language.
- **A source patch never applies**: Settings > About lists unapplied patches. Discord changed
  the code it targets.

## Tips

- Keep `id`s stable: settings are stored under them.
- Prefer `waitFor` with `ctx.signal` over polling.
- Selectors based on `aria-label` text only work in one language; prefer `href`s, roles, or class
  name fragments like `[class*="sidebar"]` where you can.
