# Corrode

A client mod for Discord, in the spirit of shelter, Vencord and BetterDiscord. This repository
builds **one injectable script** (`dist/corrode.min.js`) containing:

- a **plugin loader** for built-in plugins and external plugins installed from a URL,
- a **plugin API**: webpack module finding, function patching, source patching, Flux
  subscriptions and interceptors, persistent settings storage, CSS injection, toasts,
- **themes**: `.css` files from the client's themes folder or a URL, a **theme creator** that
  makes a theme from colours you pick (with an optional background picture or video, no code
  needed), and live **Quick CSS**,
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

Corrode ships with 200 plugins in five groups: appearance, sending, notices and filters, chat and
media, and tools and privacy. All are off until you turn them on in **User Settings > Corrode >
Plugins**. [docs/BUILTIN_PLUGINS.md](docs/BUILTIN_PLUGINS.md) lists every one.

To make your own, read [docs/PLUGINS.md](docs/PLUGINS.md): it walks through writing a plugin and
adding it to Corrode by pasting its code or installing it from a link.
[examples/hello-world.plugin.js](examples/hello-world.plugin.js) is a complete installable example.

## Layout

```
src/
  index.ts            entrypoint: boot, window.corrode, unload()
  native.ts           optional bridge the injector can expose (window.CorrodeNative)
  api/                webpack, patcher, flux, storage, css, common modules, logger
  plugins/            plugin types and manager (lifecycle, settings, external plugins)
  builtins/           built-in plugins (categories.ts: the groups on the Plugins page)
  themes.ts           themes (saved, themes folder, background layer) and Quick CSS
  themeCreator.ts     the theme creator's colours, presets and CSS output
  ui/                 settings window, sidebar entry and shortcut, toasts
scripts/build.mjs     esbuild build + manifest
test/                 unit tests (vitest + jsdom), fake Discord harness, browser smoke test
docs/                 injector contract, plugin guide
```

## Caveats

Client mods are against Discord's Terms of Service. Plugins run with full access to your
account inside the client: only install external plugins you trust.
