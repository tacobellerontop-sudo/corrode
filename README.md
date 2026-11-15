# Corrode

A client mod for Discord, in the spirit of shelter, Vencord and BetterDiscord. This repository
builds **one injectable script** (`dist/corrode.min.js`) containing:

- a **plugin loader** for built-in plugins and external plugins installed from a URL,
- a **plugin API**: webpack module finding, function patching, source patching, Flux
  subscriptions and interceptors, persistent settings storage, CSS injection, toasts,
- **themes** (CSS from a URL) and live **Quick CSS**,
- a **settings window**, opened from a "Corrode" entry in Discord's User Settings or with
  <kbd>Ctrl</kbd>/<kbd>⌘</kbd> + <kbd>Shift</kbd> + <kbd>O</kbd>,
- a few example plugins.

Getting the script into Discord (the client/injector and its ejector) lives in a separate
repository. The contract between the two is in **[docs/INJECTOR.md](docs/INJECTOR.md)**.

## Build

Requires Node 20+.

```sh
npm install
npm run build        # dist/corrode.js, dist/corrode.min.js, dist/manifest.json
npm run watch        # rebuild on change
npm run check        # typecheck + unit tests + build + browser smoke test
```

To try it without Discord, build and serve the repo root with any static server, then open
`/test/harness/?mode=early` (or `?mode=late`): a fake Discord with a webpack runtime, a few
modules and a settings sidebar. To try it in Discord without an injector, paste
`dist/corrode.js` into the DevTools console (that's a *late* load; see the injector doc).

## Built-in plugins

| Plugin | Shows off |
| --- | --- |
| **Silent Typing** | Finding a webpack module and patching a function |
| **Keyword Notify** | Flux subscriptions, settings and toasts |
| **Hide Typing Indicators** | Flux interception (blocking actions) |
| **Hide Clutter** | Settings-driven CSS injection |

All are off by default. [docs/PLUGINS.md](docs/PLUGINS.md) covers writing your own, and
[examples/hello-world.plugin.js](examples/hello-world.plugin.js) is an installable external plugin.

## Layout

```
src/
  index.ts            entrypoint: boot, window.corrode, unload()
  native.ts           optional bridge the injector can expose (window.CorrodeNative)
  api/                webpack, patcher, flux, storage, css, common modules, logger
  plugins/            plugin types and manager (lifecycle, settings, external plugins)
  builtins/           built-in plugins
  themes.ts           themes and Quick CSS
  ui/                 settings window, sidebar entry and shortcut, toasts
scripts/build.mjs     esbuild build + manifest
test/                 unit tests (vitest + jsdom), fake Discord harness, browser smoke test
docs/                 injector contract, plugin guide
```

## Caveats

Client mods are against Discord's Terms of Service. Plugins run with full access to your
account inside the client: only install external plugins you trust.
