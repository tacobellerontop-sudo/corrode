# Platform support and packaging

Windows is the only supported platform. **Windows x64 has local build evidence.** The
release workflow also targets Windows arm64 on a native GitHub Actions runner; build and
runtime validation remain pending. Windows has offline tests and a process/window startup
smoke check only. Minimum OS versions, other architectures, real screen-reader support and
native login-method support are not certified.

Windows defaults to DirectX 12 to avoid reported startup access violations in Intel's
Vulkan driver (`igvk64.dll`). The existing `WGPU_BACKEND` environment override remains
available (for example, `dx12` or `vulkan`). Affected users confirmed that forcing
DX12 launches successfully; the new default still needs native Windows validation.

Windows turns off winit's undecorated drop-shadow hack after the window exists, including
after title-bar changes. egui-winit enables that hack for custom chrome. While restored it
adds one pixel to `WM_NCCALCSIZE` top and bottom. Maximizing skips the shift, which is why
only that state looked sharp. Native DPI and eframe's physical surface sizing remain unchanged.
For offline inspection, run `cargo run --locked -p corrode --features demo -- --demo --demo-rendering`.
The diagnostic shows the physical client size, logical viewport, native/egui scale and WGPU
surface dimensions sampled by a render callback, plus alternating one-pixel stripes.
The surface sample is from the previous paint: compare at rest after resizing, maximizing,
restoring, toggling the title bar and moving between monitors (including mixed DPI).
Surface/client dimensions should match and stripes/text/images should stay sharp. If dimensions
match but blur persists, the surface-resolution hypothesis is not established; investigate
driver/DWM presentation on that machine. Windows visual acceptance remains unverified.

The custom title strip requests a native window move on the initial primary-button press,
including over its nonselectable context title. It does not wait for egui's text/drag threshold.
Caption buttons and other clickable title-strip controls keep their own actions; Windows
double-click maximize/restore remains available. Synthetic egui input tests check command
dispatch, not native OS window movement, which still requires a desktop interaction check.
On Windows, Appearance settings can hide this 36 px strip and use the native title bar
and window buttons instead. The device preference defaults to showing the custom strip and
is saved with other app preferences; older saved settings keep that default.

| Platform | Build/runtime requirements | Status |
|---|---|---|
| Windows | Rust MSVC toolchain, Visual Studio C++ build tools, system graphics drivers, WebView2 Runtime 101+ (current supported runtime recommended), Credential Manager | Local x64 checks and unsigned release packaging on Windows 11 build 26200; synthetic process/window startup passed. Visual interaction, InPrivate behavior, IME and accessibility unverified |

SQLite is bundled through rusqlite; it is an embedded client cache, with no database service.

`cargo xtask package` builds the locked default release configuration and produces the
unsigned Windows installer through [packaging/windows/installer.nsi](../packaging/windows/installer.nsi).
See [Windows packaging](../packaging/windows/README.md) for build dependencies, installer
layout and installation commands. Staged artifacts under `dist` remain unsigned and are
not certified installers. Windows installer signing and release reproducibility remain
open work.

The webview lives only during login and uses the vendored Wry WebView2 backend on Windows.
Voice is built in. Audio devices open only for explicit playback, device testing, or a call
reaching required encrypted readiness. Popup-dependent authentication and third-party
embedded challenges may not work; do not claim all Discord login methods without live tests.

## Window transparency and blur

Enable Transparency & blur in Appearance and restart to create an alpha-capable
rendering surface. Transparency works where the OS supports alpha windows.
Blur is requested through the Windows 11 22H2+ system backdrop (Acrylic/Mica).
Unsupported Windows versions retain transparency without native blur; system
accessibility and appearance preferences can also suppress effects. The system
determines blur strength; zero disables it. Windows native appearance remains
unverified.

## Built-in voice

`cargo run --locked` includes native DM and guild audio. Source builds require CMake and a
C/C++ toolchain for statically bundled libopus. Audio uses WASAPI. See
[the voice adapter](../crates/discord-voice/README.md) for codec/protocol dependencies and
limitations.

`cargo xtask package` stages the standard release including voice under `dist`.
Windows x64 voice release packaging and synthetic protocol/audio tests pass; physical audio
and live calls remain unverified on Windows. CMake is a source-build dependency, not a
runtime voice service.

Device choices, voice keybinds and the owner's mute/deafen intent are device-local. Mute
and deafen can be changed while idle, survive restart and apply to the first voice-state
packet when the next call is joined. Voice bindings use native Windows global registration
when they include a modifier. Unmodified focused Push to Talk (V by default) observes key
state without consuming typed text and is never registered as an OS-global shortcut.
Unavailable/conflicting registrations fall back to focused input. Use headphones because
there is no acoustic echo cancellation. No signing, physical audio or live-compatibility
claim follows from compilation alone.

Native emoji use eframe system-font fallback and installed OS color fonts. Windows emoji
coverage is unverified and depends on installed fonts (e.g. Segoe UI Emoji); no OS font is
redistributed.

## Camera capture (September 12, 2026)

Outgoing in-call capture uses Windows Graphics Capture. The existing camera button becomes
available after the voice server negotiates H264; capture starts only after an explicit
click in a connected call. The adapter sends 640×480 video at most 15 encoded frames/s.
Windows needs desktop camera permission. Settings provide a device picker in call controls
and an explicit local camera preview outside calls. Physical selection/preview remains
unverified. See [camera limits and validation](voice.md#camera-in-calls-windows).
Windows compilation does not establish working physical capture or delivery to an official
Discord client; these remain unverified.

## Invite verification (September 13, 2026)

Invite verification uses a temporary WebView2 child. It loads a local verification page and
hCaptcha's official widget after the user chooses Verify. The local custom-protocol origin is
`https://corrode-captcha.verification.invalid/`; it is not a Discord page, public server or
account-login surface. No account token enters it. Domain restrictions, provider rejection
and a missing WebView2 Runtime fail visibly. Live CAPTCHA acceptance remains unverified.
Widget loading and synthetic checks do not establish live Discord challenge acceptance.

## Opt-out tray icon (September 13, 2026)

Windows General settings offer Show Corrode in System Tray, on by default; turning it off
falls back to ordinary window minimize/close. Minimizing
keeps the window in the taskbar, including taskbar clicks and automatic startup. The icon supports
keyboard/mouse restore and a Show Corrode / Quit menu. Quit uses the normal unsaved
work/download exit checks; while the icon is live, the window Close button hides the
window instead of exiting, and Corrode keeps running with its logic ticking so
notifications and calls continue. Show restores the window. Disabling the setting,
or a tray that reports itself unavailable, restores a hidden window immediately, so
Close can never strand the application without a way back.
The adapter uses existing user32/Shell APIs and dependencies, with no background
polling. A synthetic native Windows test verifies registration,
minimize/restore, own-window taskbar recovery, Quit event and cleanup.

## Opt-in automatic startup

General settings offer automatic launch at Windows sign-in and a dependent Start
Corrode minimized preference. Both default off. Registration uses the current user's
Run key; no administrator access, service, scheduled task or new dependency is needed.
Windows Startup Apps can override this registration. Disable startup before deleting
a portable installation, or re-enable it after moving the executable.
Minimized launches stay in the taskbar even when the saved tray preference is enabled;
the tray can attach safely after a minimized launch. Tray failures leave the window
recoverable. Without a tray icon the Close button still exits, and the tray Quit action retains unsaved
work checks.
Offline tests cover isolated registry writes/removal, launch flags and settings
interaction; an actual Windows sign-out/sign-in has not been exercised.

## In-app updates

Settings → Updates provides automatic checking/downloading, Production and Nightly
release channels, a manual check and an explicit restart action. The title strip
shows an available or downloaded update on Windows. Update controls are
also accessible from the signed-out screen. Automatic checking runs at startup
once saved preferences are available, then every hour while running; turning
it off disables automatic downloads while background checks and title-bar notices
remain active. Nightly is the default channel and automatic downloads are off by
default. Switching channels never installs an
older semantic version. Nightly checks inspect the latest 100 published releases.

Packages come from this repository's existing GitHub releases and must match the
platform/architecture asset name, published length and `SHA256SUMS.txt`. Downloads
and installation preparation run outside rendering; installation is handed off
only after the application's existing close/unsaved-work gates permit shutdown.
GitHub HTTPS and repository access are the update trust boundary; release checksums
alone are not an independent publisher signature.

The local `--features demo -- --demo --demo-check-updates` debug path exercises
synthetic update states, preference compatibility and settings rendering without
network access or replacing an installation. It is not evidence of a successful
live release upgrade or of Windows native installation behavior.

In-app installation requires an extracted Windows release or an installed, writable
per-user installation under `%LOCALAPPDATA%\Programs\Corrode`. Windows currently relies
on the repository's HTTPS/checksum trust boundary because its published packages are
unsigned. When installed via the per-user installer, write permissions are maintained
without administrator elevation, and the update helper automatically updates the
Windows uninstall `DisplayVersion` registry key upon successful upgrade. Native
helpers wait for the old process to exit, retain a rollback copy during replacement,
and relaunch Corrode. A failed recovery leaves its backup available with a visible
recovery path on the next update attempt.

## Screen sharing

Screen capture uses Windows Graphics Capture. The initial adapter accepts source
dimensions up to 3840×2160.

Optional Windows stream audio uses native process loopback with
`PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE`, excluding Corrode and its child
processes across outputs. This requires Windows build 20348+ (Windows 11 or Windows
Server 2022; ordinary Windows 10 22H2 is older). Unsupported systems or failed
isolation report an audio error; turn audio off to share video alone. There is no
whole-output fallback. Hardware exclusion and receiving sound in an official client
remain unverified.

## Attachment codecs

Inline attachment decoding and live H.264 use Windows Media Foundation.
