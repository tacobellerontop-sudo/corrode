# Platform support and packaging

Windows is the supported platform. The OS integrations in `crates/platform` (credential
store, WebView2 login window, toast notifications, file dialogs, Media Foundation) are
Windows-only, so `crates/platform`, `crates/discord-voice` and the `solace` desktop binary
build only for Windows targets. Every other crate, including the `solace-ui` interface,
builds and tests on Linux and macOS too.

| Platform | Build/runtime requirements | Status |
|---|---|---|
| Windows | Rust MSVC toolchain, Visual Studio C++ build tools, CMake (for bundled libopus), system graphics drivers, WebView2 Runtime 101+, Credential Manager | Supported |

## Rendering

Solace draws its whole interface itself: layout, icons, avatars, cards, gradients and the
procedural landscape artwork are epaint geometry and textures, rendered on the GPU through
egui's wgpu backend. There is no web view or native widget toolkit in the main window.

Windows defaults to DirectX 12 to avoid reported startup crashes in Intel's Vulkan driver.
`WGPU_BACKEND` overrides the backend (for example `dx12` or `vulkan`). Only adapters that can
present to the window are eligible; the saved GPU preference (`model::GpuPreference`) orders
them.

The window uses native decorations. Its size and position are remembered between launches;
egui's widget memory is not persisted.

## Local data

| Data | Location |
|---|---|
| Message cache, drafts, saved-account roster, appearance | `%LOCALAPPDATA%\Solace\client.sqlite3` |
| Image cache | `%LOCALAPPDATA%\Solace\avatars\<account>` |
| Discord token | OS credential store, service `app.solace.desktop` |

On first launch after the rename, an existing `%LOCALAPPDATA%\Corrode` folder is moved to
`Solace`, and saved logins are read from the previous `cz.viceverse.corrode` (and older
`cz.viceverse.serein`) credential services; new writes go to `app.solace.desktop`. Logging
out removes the entry from every service name. See [storage policy](storage-policy.md) for
bounds and cleanup ordering.

## Sign-in

The temporary login window loads `discord.com` in an ephemeral WebView2 child (vendored Wry,
Windows backend only) beneath Solace's header strip; passwords and 2FA never leave Discord's
page. Token sign-in is also available. A new sign-in requires confirming ownership of the
account. Popup-dependent authentication methods may not work.

## Voice

DM calls and server voice rooms are built in (WASAPI audio, Opus, DAVE end-to-end
encryption). Solace's interface offers join/leave, mute, deafen, device selection, a
microphone test and focused push-to-talk (hold V). Camera and screen-sharing support remains
in the media engine but has no controls in the Solace interface yet. See
[the voice adapter](../crates/discord-voice/README.md) and [voice notes](voice.md).

## Notifications

System notifications require a Start Menu shortcut carrying Solace's AppUserModelID
(`app.solace.desktop`); the per-user installer creates it. See
[Windows packaging](../packaging/windows/README.md).

## Packaging

`cargo xtask package` builds the locked release configuration and produces the unsigned
Windows installer through [packaging/windows/installer.nsi](../packaging/windows/installer.nsi).
Installer signing remains open work.

## Development on other systems

The interface can be previewed anywhere wgpu runs, without an account or network:

```sh
cargo run -p solace-ui --example preview            # light
cargo run -p solace-ui --example preview -- --dark  # night palette
cargo run -p solace-ui --example preview -- --signin
cargo run -p solace-ui --example preview -- --out shot.png   # save a screenshot and exit
```

On Windows, `cargo run -p solace --features demo -- --demo` runs the full desktop app with the
same synthetic showcase.

## Not included

The previous interface's extension system, in-app updater, tray icon, automatic startup,
window transparency/blur and custom title strip were removed with it.
