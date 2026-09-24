<p align="center"><img height="100" src="https://raw.githubusercontent.com/tauri-apps/wry/refs/heads/dev/.github/splash.png" alt="WRY Webview Rendering library" /></p>

[![](https://img.shields.io/crates/v/wry?style=flat-square)](https://crates.io/crates/wry) [![](https://img.shields.io/docsrs/wry?style=flat-square)](https://docs.rs/wry/)
[![License](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)](https://opencollective.com/tauri)
[![Chat Server](https://img.shields.io/badge/chat-discord-7289da.svg)](https://discord.gg/SpmNs4S)
[![website](https://img.shields.io/badge/website-tauri.app-purple.svg)](https://tauri.app)
[![https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg](https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg)](https://good-labs.github.io/greater-good-affirmation)
[![support](https://img.shields.io/badge/sponsor-Open%20Collective-blue.svg)](https://opencollective.com/tauri)

Wry is a WebView rendering library. This is Corrode's scoped fork and targets Windows only,
rendering through WebView2.

The webview requires a running event loop and a window type that implements [`HasWindowHandle`].
You can use a windowing library like [`tao`] or [`winit`].

### Examples

This example leverages the [`HasWindowHandle`] and targets Windows.
See the following example using [`winit`]:

```rust
#[derive(Default)]
struct App {
  window: Option<Window>,
  webview: Option<wry::WebView>,
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    let window = event_loop.create_window(Window::default_attributes()).unwrap();
    let webview = WebViewBuilder::new()
      .with_url("https://tauri.app")
      .build(&window)
      .unwrap();

    self.window = Some(window);
    self.webview = Some(webview);
  }

  fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
}

let event_loop = EventLoop::new().unwrap();
let mut app = App::default();
event_loop.run_app(&mut app).unwrap();
```

### Child webviews

You can use [`WebViewBuilder::build_as_child`] to create the webview as a child inside another window. This is supported on Windows.

```rust
#[derive(Default)]
struct App {
  window: Option<Window>,
  webview: Option<wry::WebView>,
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    let window = event_loop.create_window(Window::default_attributes()).unwrap();
    let webview = WebViewBuilder::new()
      .with_url("https://tauri.app")
      .with_bounds(Rect {
        position: LogicalPosition::new(100, 100).into(),
        size: LogicalSize::new(200, 200).into(),
      })
      .build_as_child(&window)
      .unwrap();

    self.window = Some(window);
    self.webview = Some(webview);
  }

  fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
}

let event_loop = EventLoop::new().unwrap();
let mut app = App::default();
event_loop.run_app(&mut app).unwrap();
```

### Platform Considerations

WebView2, provided by Microsoft Edge Chromium, is used to render webviews on Windows.
Wry supports Windows 7, 8, 10 and 11.

### Feature flags

Wry uses a set of feature flags to toggle several advanced features.

- `os-webview` (default): Enables the default WebView framework on the platform. This must be enabled
  for the crate to work. This feature was added in preparation of other ports like cef and servo.
- `serde`: Enables `dpi`'s `serde` feature.
- `devtools`: Enables devtools on release builds. Devtools are always enabled in debug builds.
- `tracing`: enables [`tracing`] for `evaluate_script`, `ipc_handler`, and `custom_protocols`.

### Partners

<table>
  <tbody>
    <tr>
      <td align="center" valign="middle">
        <a href="https://crabnebula.dev" target="_blank">
          <img src=".github/sponsors/crabnebula.svg" alt="CrabNebula" width="283">
        </a>
      </td>
    </tr>
  </tbody>
</table>

For the complete list of sponsors please visit our [website](https://tauri.app#sponsors) and [Open Collective](https://opencollective.com/tauri).

### License

Apache-2.0/MIT

[`tao`]: https://docs.rs/tao
[`winit`]: https://docs.rs/winit
[`tracing`]: https://docs.rs/tracing
