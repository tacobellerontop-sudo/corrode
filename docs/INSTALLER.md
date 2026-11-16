# Corrode Installer (official Discord app)

The Corrode Installer puts Corrode into the **official Discord desktop app**, the same way
Vencord's installer does. It's separate from [AesDc](https://github.com/tacobellerontop-sudo/AesDc),
which is its own Discord client and loads Corrode by itself.

## Using it

Download the installer for your system from the
[latest-main release](https://github.com/tacobellerontop-sudo/corrode/releases/tag/latest-main):

| System | File |
| --- | --- |
| Windows (most PCs) | `CorrodeInstaller.exe` |
| Windows on ARM | `CorrodeInstaller-arm64.exe` |
| macOS, Apple Silicon | `CorrodeInstaller-macos-arm64` |
| macOS, Intel | `CorrodeInstaller-macos-x64` |
| Linux | `CorrodeInstaller-linux-x64` / `CorrodeInstaller-linux-arm64` |

Run it. It lists every Discord it finds (Stable, PTB, Canary, Development) and whether Corrode is
in it, then offers:

- **Install**: puts Corrode into the Discord you pick (or all of them). If Discord is open, it
  asks to close it, and starts it again afterwards (Windows and macOS).
- **Repair**: the same as Install. Use it if Corrode stops loading, or after Discord updates on
  macOS or Linux.
- **Uninstall**: puts Discord back exactly as it was. It asks whether to delete Corrode's
  settings and themes too.

Then open Discord: Corrode is under **User Settings > Corrode**, or press
<kbd>Ctrl</kbd>/<kbd>⌘</kbd> + <kbd>Shift</kbd> + <kbd>O</kbd>.

Notes per system:

- **Windows**: SmartScreen may warn because the installer isn't signed: *More info > Run anyway*.
  When Discord updates itself, Corrode adds itself to the new version as Discord restarts, so
  there's nothing to do.
- **macOS**: the file isn't signed either. In Terminal run
  `xattr -d com.apple.quarantine ~/Downloads/CorrodeInstaller-macos-*` and
  `chmod +x ~/Downloads/CorrodeInstaller-macos-*`, then run it. If it says it can't change
  Discord, allow Terminal under *System Settings > Privacy & Security > App Management*.
  A Discord update replaces the app, so run **Repair** after one.
- **Linux**: `chmod +x` it and run it in a terminal. Discord from a `.deb`/`.rpm` lives in a
  system folder, so run it with `sudo` (Corrode's files still go to your own home folder). Flatpak
  and Snap Discord are read-only and not supported. Run **Repair** after a Discord update.

### Command line

```
CorrodeInstaller -install   [-branch stable|ptb|canary|development|all] [-location <path>] [-no-start]
CorrodeInstaller -repair    [same options]
CorrodeInstaller -uninstall [same options] [-purge]
CorrodeInstaller -list
```

`-location` takes Discord's folder (on Windows `%LOCALAPPDATA%\Discord`), its `resources`
folder, or a `.app`. With an action flag nothing is asked and a running Discord is closed.

## What works in official Discord

Everything in Corrode works. The installer also provides the native bridge
(`window.CorrodeNative`, see [INJECTOR.md](INJECTOR.md)) that AesDc provides, so these work as in AesDc:

| Feature | In official Discord |
| --- | --- |
| Settings storage | Saved to `settings.json` in Corrode's folder, so logging out of Discord doesn't wipe them. |
| Themes folder | `themes/` in Corrode's folder: drop `.css` files in, the theme creator saves there, **Open themes folder** works, background pictures and videos go into `themes/media`. |
| Installing plugins and themes from a link | Yes (fetched by Discord's main process, so Discord's CSP doesn't block it). |
| Restart Discord, Open DevTools | Yes. |
| Updates page: **Update Corrode** | Downloads the newest Corrode from latest-main (checked against its SHA-256); it loads after **Restart now**. |
| Updates page: the installer card | Says when a newer installer is needed (only when the part of Corrode that goes into Discord changed), and **Update** opens the download page. It can't update the installer by itself. |
| **Uninstall Corrode** (Settings > About) | Turns Corrode off and restarts Discord. It also tries to put Discord's files back; if Windows won't allow that while Discord runs, Corrode stays off until you run the installer's Uninstall (or Repair, to turn it back on). |
| AesDc's custom title bar, low-memory mode | No: those are AesDc features, not Corrode ones. |

Corrode's folder is `%APPDATA%\Corrode` on Windows, `~/Library/Application Support/Corrode` on
macOS and `~/.config/Corrode` on Linux.

## How it works

Electron loads `resources/app.asar` as the app. The installer:

1. writes Corrode's files to Corrode's folder: `patcher.js` and `preload.js` (from
   [`installer/runtime`](../installer/runtime)), `corrode.min.js`, `manifest.json` and `installer.json`;
2. renames Discord's `resources/app.asar` to `_app.asar` and makes a folder called `app.asar`
   in its place, with a `package.json` and an `index.js` that requires `patcher.js`. If Corrode's
   folder is ever deleted, that `index.js` starts Discord normally instead.

`patcher.js` runs in Discord's main process. It replaces Electron's `BrowserWindow` so Discord's
main window gets `preload.js`, answers the bridge's calls over IPC, and then loads Discord from
`_app.asar`. On Windows it also watches for Discord's own updates (a new `app-x.y.z` folder) and
patches the new version as Discord restarts. `preload.js` exposes `window.CorrodeNative`, runs
Corrode in the page before Discord's scripts (so Corrode loads *early*), then runs Discord's own
preload.

If `app.asar` is already a folder that isn't Corrode's (Vencord and others use the same trick),
the installer stops and says so instead of stacking mods.

## Building and testing

```sh
npm run build                                       # Corrode itself
node scripts/build-installer.mjs                    # dist/installer/* for every system (needs Go 1.22+)
node scripts/build-installer.mjs --targets linux/amd64
(cd installer && go test ./...)                     # unit tests
npm install --no-save electron@33 @electron/asar@3
node test/installer-e2e.mjs                         # end-to-end, see below
```

`test/installer-e2e.mjs` builds a fake Discord: real Electron, with a stand-in Discord app
([`test/installer/fake-discord-app`](../test/installer/fake-discord-app)) as its `app.asar`,
laid out like Discord on Windows (`Discord/app-x.y.z/resources`). It runs the Linux installer
against it and checks that Corrode loads early, Discord's own preload still runs, storage,
themes and media work, a downloaded Discord update gets patched on quit, Repair, the off switch,
a deleted Corrode folder, and Uninstall. CI runs it on every push.

It's only a fake: nothing here has run against the real Discord app yet, and the Windows and
macOS installers are cross-compiled but have only been run on Linux.
