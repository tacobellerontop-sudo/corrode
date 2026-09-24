// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! <p align="center"><img height="100" src="https://raw.githubusercontent.com/tauri-apps/wry/refs/heads/dev/.github/splash.png" alt="WRY Webview Rendering library" /></p>
//!
//! [![](https://img.shields.io/crates/v/wry?style=flat-square)](https://crates.io/crates/wry) [![](https://img.shields.io/docsrs/wry?style=flat-square)](https://docs.rs/wry/)
//! [![License](https://img.shields.io/badge/License-MIT%20or%20Apache%202-green.svg)](https://opencollective.com/tauri)
//! [![Chat Server](https://img.shields.io/badge/chat-discord-7289da.svg)](https://discord.gg/SpmNs4S)
//! [![website](https://img.shields.io/badge/website-tauri.app-purple.svg)](https://tauri.app)
//! [![https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg](https://good-labs.github.io/greater-good-affirmation/assets/images/badge.svg)](https://good-labs.github.io/greater-good-affirmation)
//! [![support](https://img.shields.io/badge/sponsor-Open%20Collective-blue.svg)](https://opencollective.com/tauri)
//!
//! Wry is a WebView rendering library. This is Corrode's scoped fork and targets Windows only,
//! rendering through WebView2.
//!
//! The webview requires a running event loop and a window type that implements [`HasWindowHandle`].
//! You can use a windowing library like [`tao`] or [`winit`].
//!
//! ## Examples
//!
//! This example leverages the [`HasWindowHandle`] and targets Windows.
//! See the following example using [`winit`]:
//!
//! ```no_run
//! # use wry::{WebViewBuilder, raw_window_handle};
//! # use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}};
//! #[derive(Default)]
//! struct App {
//!   window: Option<Window>,
//!   webview: Option<wry::WebView>,
//! }
//!
//! impl ApplicationHandler for App {
//!   fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//!     let window = event_loop.create_window(Window::default_attributes()).unwrap();
//!     let webview = WebViewBuilder::new()
//!       .with_url("https://tauri.app")
//!       .build(&window)
//!       .unwrap();
//!
//!     self.window = Some(window);
//!     self.webview = Some(webview);
//!   }
//!
//!   fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
//! }
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut app = App::default();
//! event_loop.run_app(&mut app).unwrap();
//! ```
//!
//! ## Child webviews
//!
//! You can use [`WebViewBuilder::build_as_child`] to create the webview as a child inside another window. This is supported on Windows.
//!
//! ```no_run
//! # use wry::{WebViewBuilder, raw_window_handle, Rect, dpi::*};
//! # use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}};
//! #[derive(Default)]
//! struct App {
//!   window: Option<Window>,
//!   webview: Option<wry::WebView>,
//! }
//!
//! impl ApplicationHandler for App {
//!   fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//!     let window = event_loop.create_window(Window::default_attributes()).unwrap();
//!     let webview = WebViewBuilder::new()
//!       .with_url("https://tauri.app")
//!       .with_bounds(Rect {
//!         position: LogicalPosition::new(100, 100).into(),
//!         size: LogicalSize::new(200, 200).into(),
//!       })
//!       .build_as_child(&window)
//!       .unwrap();
//!
//!     self.window = Some(window);
//!     self.webview = Some(webview);
//!   }
//!
//!   fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
//! }
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut app = App::default();
//! event_loop.run_app(&mut app).unwrap();
//! ```
//!
//! ## Platform Considerations
//!
//! WebView2, provided by Microsoft Edge Chromium, is used to render webviews on Windows.
//! Wry supports Windows 7, 8, 10 and 11.
//!
//! ## Feature flags
//!
//! Wry uses a set of feature flags to toggle several advanced features.
//!
//! - `os-webview` (default): Enables the default WebView framework on the platform. This must be enabled
//!   for the crate to work. This feature was added in preparation of other ports like cef and servo.
//! - `serde`: Enables `dpi`'s `serde` feature.
//! - `devtools`: Enables devtools on release builds. Devtools are always enabled in debug builds.
//! - `tracing`: enables [`tracing`] for `evaluate_script`, `ipc_handler`, and `custom_protocols`.
//!
//! ## Partners
//!
//! <table>
//!   <tbody>
//!     <tr>
//!       <td align="center" valign="middle">
//!         <a href="https://crabnebula.dev" target="_blank">
//!           <img src=".github/sponsors/crabnebula.svg" alt="CrabNebula" width="283">
//!         </a>
//!       </td>
//!     </tr>
//!   </tbody>
//! </table>
//!
//! For the complete list of sponsors please visit our [website](https://tauri.app#sponsors) and [Open Collective](https://opencollective.com/tauri).
//!
//! ## License
//!
//! Apache-2.0/MIT
//!
//! [`tao`]: https://docs.rs/tao
//! [`winit`]: https://docs.rs/winit
//! [`tracing`]: https://docs.rs/tracing

#![allow(clippy::new_without_default)]
#![allow(clippy::default_constructed_unit_structs)]
#![allow(clippy::type_complexity)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod custom_protocol_workaround;
mod error;
mod permissions;
mod proxy;
mod web_context;

/// Re-exported [raw-window-handle](https://docs.rs/raw-window-handle/latest/raw_window_handle/) crate.
pub use raw_window_handle;
use raw_window_handle::HasWindowHandle;

pub(crate) mod webview2;
pub use self::webview2::ScrollBarStyle;
use self::webview2::*;
use webview2_com::Microsoft::Web::WebView2::Win32::{
	ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Environment,
};

use std::{borrow::Cow, collections::HashMap, path::PathBuf, rc::Rc};

use http::{Request, Response};

pub use cookie;
pub use dpi;
pub use error::*;
pub use http;
pub use permissions::{PermissionKind, PermissionResponse};
pub use proxy::{ProxyConfig, ProxyEndpoint};
pub use web_context::WebContext;

/// A rectangular region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
	/// Rect position.
	pub position: dpi::Position,
	/// Rect size.
	pub size: dpi::Size,
}

impl Default for Rect {
	fn default() -> Self {
		Self {
			position: dpi::LogicalPosition::new(0, 0).into(),
			size: dpi::LogicalSize::new(0, 0).into(),
		}
	}
}

/// Resolves a custom protocol [`Request`] asynchronously.
///
/// See [`WebViewBuilder::with_asynchronous_custom_protocol`] for more information.
pub struct RequestAsyncResponder {
	pub(crate) responder: Box<dyn FnOnce(Response<Cow<'static, [u8]>>)>,
}

// SAFETY: even though the webview bindings do not indicate the responder is Send,
// it actually is and we need it in order to let the user do the protocol computation
// on a separate thread or async task.
unsafe impl Send for RequestAsyncResponder {}

impl RequestAsyncResponder {
	/// Resolves the request with the given response.
	pub fn respond<T: Into<Cow<'static, [u8]>>>(self, response: Response<T>) {
		let (parts, body) = response.into_parts();
		(self.responder)(Response::from_parts(parts, body.into()))
	}
}

/// Response for the new window request handler.
///
/// See [`WebViewBuilder::with_new_window_req_handler`].
pub enum NewWindowResponse {
	/// Allow the window to be opened with the default implementation.
	Allow,
	/// Allow the window to be opened, with the given platform webview instance.
	///
	/// ## Platform-specific:
	///
	/// **Windows**: The webview must use the same environment as the caller webview. See [`WebViewBuilderExtWindows::with_environment`].
	Create {
		/// The WebView2 instance to use for the new window.
		webview: ICoreWebView2,
	},
	/// Deny the window from being opened.
	Deny,
}

/// Information about the webview that initiated a new window request.
#[derive(Debug)]
pub struct NewWindowOpener {
	/// The instance of the webview that initiated the new window request.
	pub webview: ICoreWebView2,
	/// The environment of the webview that initiated the new window request.
	///
	/// The target webview environment **MUST** match the environment of the opener webview. See [`WebViewBuilderExtWindows::with_environment`].
	pub environment: ICoreWebView2Environment,
}

/// Window features of a window requested to open.
#[non_exhaustive]
#[derive(Debug)]
pub struct NewWindowFeatures {
	/// Specifies the size of the content area
	/// as defined by the user's operating system where the new window will be generated.
	pub size: Option<dpi::LogicalSize<f64>>,
	/// Specifies the position of the window relative to the work area
	/// as defined by the user's operating system where the new window will be generated.
	pub position: Option<dpi::LogicalPosition<f64>>,
	/// Information about the webview opener containing data that must be used when creating the new webview.
	pub opener: NewWindowOpener,
}

/// An id for a webview
pub type WebViewId<'a> = &'a str;

// WebViewAttributes is not stable enough to be pub.
struct WebViewAttributes<'a> {
	/// An id that will be passed when this webview makes requests in certain callbacks.
	pub id: Option<WebViewId<'a>>,

	/// Web context to be shared with this webview.
	#[allow(unused)]
	pub context: Option<&'a mut WebContext>,

	/// Whether the WebView should have a custom user-agent.
	pub user_agent: Option<String>,

	/// Whether the WebView window should be visible.
	pub visible: bool,

	/// Whether the WebView should be transparent.
	///
	/// ## Platform-specific:
	///
	/// **Windows 7**: Not supported.
	pub transparent: bool,

	/// Specify the webview background color. This will be ignored if `transparent` is set to `true`.
	///
	/// The color uses the RGBA format.
	///
	/// ## Platform-specific:
	///
	/// - **Windows**:
	///   - On Windows 7, transparency is not supported and the alpha value will be ignored.
	///   - On Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
	pub background_color: Option<RGBA>,

	/// Whether load the provided URL to [`WebView`].
	///
	/// ## Note
	///
	/// Data URLs are not supported, use [`html`](Self::html) option instead.
	pub url: Option<String>,

	/// Headers used when loading the requested [`url`](Self::url).
	pub headers: Option<http::HeaderMap>,

	/// Whether page zooming by hotkeys or gestures is enabled
	///
	/// ## Platform-specific
	///
	/// - Windows: Setting to `false` can't disable pinch zoom on WebView2 Runtime version before 91.0.865.0,
	///   see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10865-prerelease>
	pub zoom_hotkeys_enabled: bool,

	/// Whether load the provided html string to [`WebView`].
	/// This will be ignored if the `url` is provided.
	///
	/// # Warning
	///
	/// The Page loaded from html string will have `null` origin.
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** the string can not be larger than 2 MB (2 * 1024 * 1024 bytes) in total size
	pub html: Option<String>,

	/// A list of initialization javascript scripts to run when loading new pages.
	/// When webview load a new page, this initialization code will be executed.
	/// It is guaranteed that code is executed before `window.onload`.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: scripts are always injected into sub frames.
	pub initialization_scripts: Vec<InitializationScript>,

	/// A list of custom loading protocols with pairs of scheme uri string and a handling
	/// closure.
	///
	/// The closure takes an Id ([WebViewId]), [Request] and [RequestAsyncResponder] as arguments and returns a [Response].
	///
	/// # Note
	///
	///
	/// # Warning
	///
	/// Pages loaded from custom protocol will have different Origin on different platforms. And
	/// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
	/// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
	/// different Origin headers across platforms:
	///
	/// - Windows: `http://<scheme_name>.<path>` by default (so it will be `http://wry.path/to/page`). To use `https` instead of `http`, use [`WebViewBuilderExtWindows::with_https_scheme`].
	pub custom_protocols:
		HashMap<String, Box<dyn Fn(WebViewId, Request<Vec<u8>>, RequestAsyncResponder) + Send + Sync>>,

	/// The IPC handler to receive the message from Javascript on webview
	/// using `window.ipc.postMessage("insert_message_here")` to host Rust code.
	pub ipc_handler: Option<Box<dyn Fn(Request<String>)>>,

	/// A handler closure to process incoming [`DragDropEvent`] of the webview.
	///
	/// ## Blocking OS Default Behavior
	///
	/// Return `true` in the callback to block the OS' default behavior.
	///
	/// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
	/// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** This will disable the HTML Drag and Drop APIs like `draggable="true"`,
	///   since we replace the drag drop handler of WebView 2 on Windows.
	///   `handler`'s return value is ignored on Windows.
	pub drag_drop_handler: Option<Box<dyn Fn(DragDropEvent) -> bool>>,

	/// A navigation handler to decide if incoming url is allowed to navigate.
	///
	/// The closure take a `String` parameter as url and returns a `bool` to determine whether the navigation should happen.
	/// `true` allows to navigate and `false` does not.
	pub navigation_handler: Option<Box<dyn Fn(String) -> bool>>,

	/// A download started handler to manage incoming downloads.
	///
	/// The closure takes two parameters, the first is a `String` representing the url being downloaded from and the
	/// second is a mutable `PathBuf` reference that (possibly) represents where the file will be downloaded to. The latter
	/// parameter can be used to set the download location by assigning a new path to it, the assigned path _must_ be
	/// absolute. The closure returns a `bool` to allow or deny the download.
	///
	/// [`Self::default()`] sets a handler allowing all downloads to match browser behavior.
	pub download_started_handler: Option<Box<dyn FnMut(String, &mut PathBuf) -> bool + 'static>>,

	/// A download completion handler to manage downloads that have finished.
	///
	/// The closure is fired when the download completes, whether it was successful or not.
	/// The closure takes a `String` representing the URL of the original download request, an `Option<PathBuf>`
	/// potentially representing the filesystem path the file was downloaded to, and a `bool` indicating if the download
	/// succeeded. A value of `None` being passed instead of a `PathBuf` does not necessarily indicate that the download
	/// did not succeed, and may instead indicate some other failure, always check the third parameter if you need to
	/// know if the download succeeded.
	pub download_completed_handler: Option<Rc<dyn Fn(String, Option<PathBuf>, bool) + 'static>>,

	/// A new window request handler to decide if incoming url is allowed to be opened.
	///
	/// A new window is requested to be opened by the [window.open] API.
	///
	/// The closure take the URL to open and the window features object and returns [`NewWindowResponse`] to determine whether the window should open.
	///
	/// [window.open]: https://developer.mozilla.org/en-US/docs/Web/API/Window/open
	pub new_window_req_handler: Option<Box<dyn Fn(String, NewWindowFeatures) -> NewWindowResponse>>,

	/// Enables clipboard access for the page rendered on **Windows**.
	pub clipboard: bool,

	/// Enable web inspector which is usually called browser devtools.
	///
	/// Note this only enables devtools to the webview. To open it, you can call
	/// [`WebView::open_devtools`], or right click the page and open it from the context menu.
	pub devtools: bool,

	/// Whether clicking an inactive window also clicks through to the webview. Default is `false`.
	pub accept_first_mouse: bool,

	/// Indicates whether horizontal swipe gestures trigger backward and forward page navigation.
	///
	/// ## Platform-specific:
	///
	/// - Windows: Setting to `false` does nothing on WebView2 Runtime version before 92.0.902.0,
	///   see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10902-prerelease>
	pub back_forward_navigation_gestures: bool,

	/// Set a handler closure to process the change of the webview's document title.
	pub document_title_changed_handler: Option<Box<dyn Fn(String)>>,

	/// Run the WebView with incognito mode. Note that WebContext will be ignored if incognito is
	/// enabled.
	///
	/// ## Platform-specific:
	///
	/// - **Windows**: Requires WebView2 Runtime version 101.0.1210.39 or higher, does nothing on older versions,
	///   see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039>
	pub incognito: bool,

	/// Whether all media can be played without user interaction.
	pub autoplay: bool,

	/// Set a handler closure to process page load events.
	pub on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,

	/// Set a proxy configuration for the webview. Supports HTTP CONNECT and SOCKSv5 proxies
	pub proxy_config: Option<ProxyConfig>,

	/// Whether the webview should be focused when created.
	pub focused: bool,

	/// The webview bounds. Defaults to `x: 0, y: 0, width: 200, height: 200`.
	/// This is only effective if the webview was created by [`WebViewBuilder::new_as_child`].
	pub bounds: Option<Rect>,

	/// Whether background throttling should be disabled.
	///
	/// By default, browsers throttle timers and even unload the whole tab (view) to free resources after roughly 5 minutes when
	/// a view became minimized or hidden. This will permanently suspend all tasks until the documents visibility state
	/// changes back from hidden to visible by bringing the view back to the foreground.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Unsupported. Workarounds like a pending WebLock transaction might suffice.
	///
	/// see <https://github.com/tauri-apps/tauri/issues/5250#issuecomment-2569380578>
	pub background_throttling: Option<BackgroundThrottlingPolicy>,

	/// Whether JavaScript should be disabled.
	pub javascript_disabled: bool,

	/// A handler to intercept permission requests from the webview.
	///
	/// The handler receives the [`PermissionKind`] and should return
	/// the desired [`PermissionResponse`].
	///
	/// > [!NOTE]
	/// > This handler only triggers for new permission requests. If the user has already
	/// > allowed or denied a permission persistently within the webview, the browser
	/// > will use the saved preference instead of calling this handler.
	///
	/// ## Platform-specific:
	///
	/// - **Windows**: Fully supported via WebView2's PermissionRequested event.
	///
	/// ## Example
	///
	/// ```no_run
	/// # use wry::{WebViewBuilder, PermissionKind, PermissionResponse};
	/// let webview = WebViewBuilder::new()
	///     .with_permission_handler(|kind| {
	///         match kind {
	///             PermissionKind::Microphone => PermissionResponse::Allow,
	///             PermissionKind::Camera => PermissionResponse::Allow,
	///             _ => PermissionResponse::Default,
	///         }
	///     });
	/// ```
	pub permission_handler: Option<Box<dyn Fn(PermissionKind) -> PermissionResponse + Send + Sync>>,
	/// Controls the WebView's browser-level general autofill behavior.
	///
	/// **This option does not disable password or credit card autofill.**
	///
	/// When enabled, the WebView may automatically populate form fields using
	/// previously stored data such as addresses or contact information.
	///
	/// If not specified, this is `true` by default.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported. On Windows, WebView2's autofill feature (called
	///   "Suggestions") may not honor `autocomplete="off"` attributes on input
	///   elements in some cases. When this option is `false`, that autofill
	///   behavior will be disabled.
	pub general_autofill_enabled: bool,
}

impl Default for WebViewAttributes<'_> {
	fn default() -> Self {
		Self {
			id: Default::default(),
			context: None,
			user_agent: None,
			visible: true,
			transparent: false,
			background_color: None,
			url: None,
			headers: None,
			html: None,
			initialization_scripts: Default::default(),
			custom_protocols: Default::default(),
			ipc_handler: None,
			drag_drop_handler: None,
			navigation_handler: None,
			download_started_handler: Some(Box::new(|_, _| true)),
			download_completed_handler: None,
			new_window_req_handler: None,
			clipboard: false,
			#[cfg(debug_assertions)]
			devtools: true,
			#[cfg(not(debug_assertions))]
			devtools: false,
			zoom_hotkeys_enabled: false,
			accept_first_mouse: false,
			back_forward_navigation_gestures: false,
			document_title_changed_handler: None,
			incognito: false,
			autoplay: true,
			on_page_load_handler: None,
			proxy_config: None,
			focused: true,
			bounds: Some(Rect {
				position: dpi::LogicalPosition::new(0, 0).into(),
				size: dpi::LogicalSize::new(200, 200).into(),
			}),
			background_throttling: None,
			javascript_disabled: false,
			permission_handler: None,
			general_autofill_enabled: true,
		}
	}
}

/// Builder type of [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to construct WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebViewBuilder`] provides ability to setup initialization before web engine starts.
pub struct WebViewBuilder<'a> {
	attrs: WebViewAttributes<'a>,
	platform_specific: PlatformSpecificWebViewAttributes,
	/// Records errors before the [`WebViewBuilder::build`] is called
	error: crate::Result<()>,
}

impl<'a> WebViewBuilder<'a> {
	/// Create a new [`WebViewBuilder`].
	pub fn new() -> Self {
		Self {
			attrs: WebViewAttributes::default(),
			#[allow(clippy::default_constructed_unit_structs)]
			platform_specific: PlatformSpecificWebViewAttributes::default(),
			error: Ok(()),
		}
	}

	/// Create a new [`WebViewBuilder`] with a web context that can be shared with multiple [`WebView`]s.
	pub fn new_with_web_context(web_context: &'a mut WebContext) -> Self {
		let attrs = WebViewAttributes {
			context: Some(web_context),
			..Default::default()
		};

		Self {
			attrs,
			#[allow(clippy::default_constructed_unit_structs)]
			platform_specific: PlatformSpecificWebViewAttributes::default(),
			error: Ok(()),
		}
	}

	/// Set an id that will be passed when this webview makes requests in certain callbacks.
	pub fn with_id(mut self, id: WebViewId<'a>) -> Self {
		self.attrs.id = Some(id);
		self
	}

	/// Indicates whether horizontal swipe gestures trigger backward and forward page navigation.
	pub fn with_back_forward_navigation_gestures(mut self, gesture: bool) -> Self {
		self.attrs.back_forward_navigation_gestures = gesture;
		self
	}

	/// Sets whether the WebView should be transparent.
	///
	/// ## Platform-specific:
	///
	/// **Windows 7**: Not supported.
	pub fn with_transparent(mut self, transparent: bool) -> Self {
		self.attrs.transparent = transparent;
		self
	}

	/// Specify the webview background color. This will be ignored if `transparent` is set to `true`.
	///
	/// The color uses the RGBA format.
	///
	/// ## Platform-specific:
	///
	/// - **Windows**:
	///   - on Windows 7, transparency is not supported and the alpha value will be ignored.
	///   - on Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
	pub fn with_background_color(mut self, background_color: RGBA) -> Self {
		self.attrs.background_color = Some(background_color);
		self
	}

	/// Sets whether the WebView should be visible or not.
	pub fn with_visible(mut self, visible: bool) -> Self {
		self.attrs.visible = visible;
		self
	}

	/// Sets whether all media can be played without user interaction.
	pub fn with_autoplay(mut self, autoplay: bool) -> Self {
		self.attrs.autoplay = autoplay;
		self
	}

	/// Initialize javascript code when loading new pages. When webview load a new page, this
	/// initialization code will be executed. It is guaranteed that code is executed before
	/// `window.onload`.
	///
	/// ## Example
	/// ```ignore
	/// let webview = WebViewBuilder::new()
	///   .with_initialization_script("console.log('Running inside main frame only')")
	///   .with_url("https://tauri.app")
	///   .build(&window)
	///   .unwrap();
	/// ```
	///
	/// ## Platform-specific
	///
	///- **Windows:** scripts are always added to subframes.
	pub fn with_initialization_script<S: Into<String>>(self, js: S) -> Self {
		self.with_initialization_script_for_main_only(js, true)
	}

	/// Same as [`with_initialization_script`](Self::with_initialization_script) but with option to inject into main frame only or sub frames.
	///
	/// ## Example
	/// ```ignore
	/// let webview = WebViewBuilder::new()
	///   .with_initialization_script_for_main_only("console.log('Running inside main frame only')", true)
	///   .with_initialization_script_for_main_only("console.log('Running  main frame and sub frames')", false)
	///   .with_url("https://tauri.app")
	///   .build(&window)
	///   .unwrap();
	/// ```
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** scripts are always added to subframes regardless of the `for_main_frame_only` option.
	pub fn with_initialization_script_for_main_only<S: Into<String>>(
		mut self,
		js: S,
		for_main_frame_only: bool,
	) -> Self {
		let script = js.into();
		if !script.is_empty() {
			self
				.attrs
				.initialization_scripts
				.push(InitializationScript {
					script,
					for_main_frame_only,
				});
		}
		self
	}

	/// Register custom loading protocols with pairs of scheme uri string and a handling
	/// closure.
	///
	/// The closure takes a [Request] and returns a [Response]
	///
	/// When registering a custom protocol with the same name, only the last registered one will be used.
	///
	/// # Warning
	///
	/// Pages loaded from custom protocol will have different Origin on different platforms. And
	/// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
	/// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
	/// different Origin headers across platforms:
	///
	/// - Windows: `http://<scheme_name>.<path>` by default (so it will be `http://wry.path/to/page`). To use `https` instead of `http`, use [`WebViewBuilderExtWindows::with_https_scheme`].
	pub fn with_custom_protocol<F>(mut self, name: String, handler: F) -> Self
	where
		F: Fn(WebViewId, Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> + Send + Sync + 'static,
	{
		if self.attrs.custom_protocols.contains_key(&name) {
			let err = Err(crate::Error::DuplicateCustomProtocol(name));
			self.error = self.error.and(err);
			return self;
		}

		self.attrs.custom_protocols.insert(
			name,
			Box::new(move |id, request, responder| {
				let http_response = handler(id, request);
				responder.respond(http_response);
			}),
		);
		self
	}

	/// Same as [`Self::with_custom_protocol`] but with an asynchronous responder.
	///
	/// When registering a custom protocol with the same name, only the last registered one will be used.
	///
	/// # Warning
	///
	/// Pages loaded from custom protocol will have different Origin on different platforms. And
	/// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
	/// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
	/// different Origin headers across platforms:
	///
	/// - Windows: `http://<scheme_name>.<path>` by default (so it will be `http://wry.path/to/page`). To use `https` instead of `http`, use [`WebViewBuilderExtWindows::with_https_scheme`].
	///
	/// # Examples
	///
	/// ```no_run
	/// use wry::{WebViewBuilder, raw_window_handle};
	/// WebViewBuilder::new()
	///   .with_asynchronous_custom_protocol("wry".into(), |_webview_id, request, responder| {
	///     // here you can use a tokio task, thread pool or anything
	///     // to do heavy computation to resolve your request
	///     // e.g. downloading files, opening the camera...
	///     std::thread::spawn(move || {
	///       std::thread::sleep(std::time::Duration::from_secs(2));
	///       responder.respond(http::Response::builder().body(Vec::new()).unwrap());
	///     });
	///   });
	/// ```
	pub fn with_asynchronous_custom_protocol<F>(mut self, name: String, handler: F) -> Self
	where
		F: Fn(WebViewId, Request<Vec<u8>>, RequestAsyncResponder) + Send + Sync + 'static,
	{
		if self.attrs.custom_protocols.contains_key(&name) {
			let err = Err(crate::Error::DuplicateCustomProtocol(name));
			self.error = self.error.and(err);
			return self;
		}

		self.attrs.custom_protocols.insert(name, Box::new(handler));
		self
	}

	/// Set the IPC handler to receive the message from Javascript on webview
	/// using `window.ipc.postMessage("insert_message_here")` to host Rust code.
	pub fn with_ipc_handler<F>(mut self, handler: F) -> Self
	where
		F: Fn(Request<String>) + 'static,
	{
		self.attrs.ipc_handler = Some(Box::new(handler));
		self
	}

	/// A handler closure to process incoming [`DragDropEvent`] of the webview.
	///
	/// ## Blocking OS Default Behavior
	///
	/// Return `true` in the callback to block the OS' default behavior.
	///
	/// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
	/// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** This will disable the HTML Drag and Drop APIs like `draggable="true"`,
	///   since we replace the drag drop handler of WebView 2 on Windows.
	///   `handler`'s return value is ignored on Windows.
	pub fn with_drag_drop_handler<F>(mut self, handler: F) -> Self
	where
		F: Fn(DragDropEvent) -> bool + 'static,
	{
		self.attrs.drag_drop_handler = Some(Box::new(handler));
		self
	}

	/// Load the provided URL with given headers when the builder calling [`WebViewBuilder::build`] to create the [`WebView`].
	/// The provided URL must be valid.
	///
	/// ## Note
	///
	/// Data URLs are not supported, use [`html`](Self::with_html) option instead.
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** if the URL's scheme is a registered custom protocol,
	///   a work around is used that changes the URL this navigates to
	///   from `{protocol}://localhost/abc` to `{http_or_https}://{protocol}.localhost/abc`
	pub fn with_url_and_headers(mut self, url: impl Into<String>, headers: http::HeaderMap) -> Self {
		self.attrs.url = Some(url.into());
		self.attrs.headers = Some(headers);
		self
	}

	/// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the [`WebView`].
	/// The provided URL must be valid.
	///
	/// ## Note
	///
	/// Data URLs are not supported, use [`html`](Self::with_html) option instead.
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** if the URL's scheme is a registered custom protocol,
	///   a work around is used that changes the URL this navigates to
	///   from `{protocol}://localhost/abc` to `{http_or_https}://{protocol}.localhost/abc`
	pub fn with_url(mut self, url: impl Into<String>) -> Self {
		self.attrs.url = Some(url.into());
		self.attrs.headers = None;
		self
	}

	/// Set headers used when loading the requested [`url`](Self::with_url).
	pub fn with_headers(mut self, headers: http::HeaderMap) -> Self {
		self.attrs.headers = Some(headers);
		self
	}

	/// Load the provided HTML string when the builder calling [`WebViewBuilder::build`] to create the [`WebView`].
	/// This will be ignored if `url` is provided.
	///
	/// # Warning
	///
	/// The Page loaded from html string will have `null` origin.
	///
	/// ## Platform-specific:
	///
	/// - **Windows:** the string can not be larger than 2 MB (2 * 1024 * 1024 bytes) in total size
	pub fn with_html(mut self, html: impl Into<String>) -> Self {
		self.attrs.html = Some(html.into());
		self
	}

	/// Set a custom [user-agent](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/User-Agent) for the WebView.
	///
	/// ## Platform-specific
	///
	/// - Windows: Requires WebView2 Runtime version 86.0.616.0 or higher, does nothing on older versions,
	///   see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10790-prerelease>
	pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
		self.attrs.user_agent = Some(user_agent.into());
		self
	}

	/// Enable or disable web inspector which is usually called devtools.
	///
	/// Note this only enables devtools to the webview. To open it, you can call
	/// [`WebView::open_devtools`], or right click the page and open it from the context menu.
	pub fn with_devtools(mut self, devtools: bool) -> Self {
		self.attrs.devtools = devtools;
		self
	}

	/// Whether page zooming by hotkeys or gestures is enabled
	///
	/// ## Platform-specific
	///
	/// - Windows: Setting to `false` can't disable pinch zoom on WebView2 Runtime version before 91.0.865.0,
	///   see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10865-prerelease>
	pub fn with_hotkeys_zoom(mut self, zoom: bool) -> Self {
		self.attrs.zoom_hotkeys_enabled = zoom;
		self
	}

	/// Set a navigation handler to decide if incoming url is allowed to navigate.
	///
	/// The closure take a `String` parameter as url and returns a `bool` to determine whether the navigation should happen.
	/// `true` allows to navigate and `false` does not.
	pub fn with_navigation_handler(mut self, callback: impl Fn(String) -> bool + 'static) -> Self {
		self.attrs.navigation_handler = Some(Box::new(callback));
		self
	}

	/// Set a handler to intercept permission requests from the webview.
	///
	/// The handler receives the [`PermissionKind`] and should return
	/// the desired [`PermissionResponse`].
	///
	/// > [!NOTE]
	/// > This handler only triggers for new permission requests. If the user has already
	/// > allowed or denied a permission persistently within the webview, the browser
	/// > will use the saved preference instead of calling this handler.
	///
	/// ## Platform-specific:
	///
	/// - **Windows**: Fully supported via WebView2's PermissionRequested event.
	///
	/// ## Example
	///
	/// ```no_run
	/// # use wry::{WebViewBuilder, PermissionKind, PermissionResponse};
	/// let webview = WebViewBuilder::new()
	///     .with_permission_handler(|kind| {
	///         match kind {
	///             PermissionKind::Microphone => PermissionResponse::Allow,
	///             PermissionKind::Camera => PermissionResponse::Allow,
	///             _ => PermissionResponse::Default,
	///         }
	///     });
	/// ```
	pub fn with_permission_handler<F>(mut self, handler: F) -> Self
	where
		F: Fn(PermissionKind) -> PermissionResponse + Send + Sync + 'static,
	{
		self.attrs.permission_handler = Some(Box::new(handler));
		self
	}

	/// Set a download started handler to manage incoming downloads.
	///
	/// The closure takes two parameters, the first is a `String` representing the url being downloaded from and the
	/// second is a mutable `PathBuf` reference that (possibly) represents where the file will be downloaded to. The latter
	/// parameter can be used to set the download location by assigning a new path to it, the assigned path _must_ be
	/// absolute. The closure returns a `bool` to allow or deny the download.
	///
	/// By default a handler that allows all downloads is set to match browser behavior.
	pub fn with_download_started_handler(
		mut self,
		download_started_handler: impl FnMut(String, &mut PathBuf) -> bool + 'static,
	) -> Self {
		self.attrs.download_started_handler = Some(Box::new(download_started_handler));
		self
	}

	/// Sets a download completion handler to manage downloads that have finished.
	///
	/// The closure is fired when the download completes, whether it was successful or not.
	/// The closure takes a `String` representing the URL of the original download request, an `Option<PathBuf>`
	/// potentially representing the filesystem path the file was downloaded to, and a `bool` indicating if the download
	/// succeeded. A value of `None` being passed instead of a `PathBuf` does not necessarily indicate that the download
	/// did not succeed, and may instead indicate some other failure, always check the third parameter if you need to
	/// know if the download succeeded.
	pub fn with_download_completed_handler(
		mut self,
		download_completed_handler: impl Fn(String, Option<PathBuf>, bool) + 'static,
	) -> Self {
		self.attrs.download_completed_handler = Some(Rc::new(download_completed_handler));
		self
	}

	/// Enables clipboard access for the page rendered on **Windows**.
	pub fn with_clipboard(mut self, clipboard: bool) -> Self {
		self.attrs.clipboard = clipboard;
		self
	}

	/// Set a new window request handler to decide if incoming url is allowed to be opened.
	///
	/// A new window is requested to be opened by the [window.open] API.
	///
	/// The closure take the URL to open and the window features object and returns [`NewWindowResponse`] to determine whether the window should open.
	///
	/// [window.open]: https://developer.mozilla.org/en-US/docs/Web/API/Window/open
	pub fn with_new_window_req_handler(
		mut self,
		callback: impl Fn(String, NewWindowFeatures) -> NewWindowResponse + 'static,
	) -> Self {
		self.attrs.new_window_req_handler = Some(Box::new(callback));
		self
	}

	/// Sets whether clicking an inactive window also clicks through to the webview. Default is `false`.
	pub fn with_accept_first_mouse(mut self, accept_first_mouse: bool) -> Self {
		self.attrs.accept_first_mouse = accept_first_mouse;
		self
	}

	/// Set a handler closure to process the change of the webview's document title.
	pub fn with_document_title_changed_handler(
		mut self,
		callback: impl Fn(String) + 'static,
	) -> Self {
		self.attrs.document_title_changed_handler = Some(Box::new(callback));
		self
	}

	/// Run the WebView with incognito mode. Note that WebContext will be ignored if incognito is
	/// enabled.
	///
	/// ## Platform-specific:
	///
	/// - Windows: Requires WebView2 Runtime version 101.0.1210.39 or higher, does nothing on older versions,
	///   see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039>
	pub fn with_incognito(mut self, incognito: bool) -> Self {
		self.attrs.incognito = incognito;
		self
	}

	/// Set a handler to process page loading events.
	pub fn with_on_page_load_handler(
		mut self,
		handler: impl Fn(PageLoadEvent, String) + 'static,
	) -> Self {
		self.attrs.on_page_load_handler = Some(Box::new(handler));
		self
	}

	/// Set a proxy configuration for the webview. Supports HTTP CONNECT and SOCKSv5 proxies
	pub fn with_proxy_config(mut self, configuration: ProxyConfig) -> Self {
		self.attrs.proxy_config = Some(configuration);
		self
	}

	/// Set whether the webview should be focused when created.
	pub fn with_focused(mut self, focused: bool) -> Self {
		self.attrs.focused = focused;
		self
	}

	/// Specify the webview position relative to its parent if it will be created as a child.
	///
	/// Defaults to `x: 0, y: 0, width: 200, height: 200`.
	pub fn with_bounds(mut self, bounds: Rect) -> Self {
		self.attrs.bounds = Some(bounds);
		self
	}

	/// Set whether background throttling should be disabled.
	///
	/// By default, browsers throttle timers and even unload the whole tab (view) to free resources after roughly 5 minutes when
	/// a view became minimized or hidden. This will permanently suspend all tasks until the documents visibility state
	/// changes back from hidden to visible by bringing the view back to the foreground.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Unsupported. Workarounds like a pending WebLock transaction might suffice.
	///
	/// see <https://github.com/tauri-apps/tauri/issues/5250#issuecomment-2569380578>
	pub fn with_background_throttling(mut self, policy: BackgroundThrottlingPolicy) -> Self {
		self.attrs.background_throttling = Some(policy);
		self
	}

	/// Whether JavaScript should be disabled.
	pub fn with_javascript_disabled(mut self) -> Self {
		self.attrs.javascript_disabled = true;
		self
	}

	/// Controls the WebView's browser-level general autofill behavior.
	///
	/// **This option does not disable password or credit card autofill.**
	///
	/// When enabled, the WebView may automatically populate form fields using
	/// previously stored data such as addresses or contact information.
	///
	/// If not specified, this is `true` by default.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported. On Windows, WebView2's autofill feature (called
	///   "Suggestions") may not honor `autocomplete="off"` attributes on input
	///   elements in some cases. When this option is `false`, that autofill
	///   behavior will be disabled.
	pub fn with_general_autofill_enabled(mut self, enabled: bool) -> Self {
		self.attrs.general_autofill_enabled = enabled;
		self
	}

	/// Consume the builder and create the [`WebView`] from a type that implements [`HasWindowHandle`].
	///
	/// # Platform-specific:
	///
	/// - **Windows**: The webview will auto-resize when the passed handle is resized.
	///
	/// # Panics:
	///
	/// - Panics if the provided handle was not supported or invalid.
	pub fn build<W: HasWindowHandle>(self, window: &'a W) -> Result<WebView> {
		self.error?;

		InnerWebView::new(window, self.attrs, self.platform_specific).map(|webview| WebView { webview })
	}

	/// Consume the builder and create the [`WebView`] as a child window inside the provided [`HasWindowHandle`].
	///
	/// ## Platform-specific
	///
	/// - **Windows**: This will create the webview as a child window of the `parent` window.
	///
	/// # Panics:
	///
	/// - Panics if the provided handle was not support or invalid.
	pub fn build_as_child<W: HasWindowHandle>(self, window: &'a W) -> Result<WebView> {
		self.error?;

		InnerWebView::new_as_child(window, self.attrs, self.platform_specific)
			.map(|webview| WebView { webview })
	}
}

#[derive(Clone)]
pub(crate) struct PlatformSpecificWebViewAttributes {
	additional_browser_args: Option<String>,
	browser_accelerator_keys: bool,
	theme: Option<Theme>,
	use_https: bool,
	scroll_bar_style: ScrollBarStyle,
	browser_extensions_enabled: bool,
	extension_path: Option<PathBuf>,
	default_context_menus: bool,
	environment: Option<ICoreWebView2Environment>,
	profile_name: Option<String>,
}

impl Default for PlatformSpecificWebViewAttributes {
	fn default() -> Self {
		Self {
			additional_browser_args: None,
			browser_accelerator_keys: true, // This is WebView2's default behavior
			default_context_menus: true,    // This is WebView2's default behavior
			theme: None,
			use_https: false, // Default to the http scheme for custom protocols.
			scroll_bar_style: ScrollBarStyle::default(),
			browser_extensions_enabled: false,
			extension_path: None,
			environment: None,
			profile_name: None,
		}
	}
}

pub trait WebViewBuilderExtWindows {
	/// Pass additional args to WebView2 upon creating the webview.
	///
	/// ## Warning
	///
	/// - Webview instances with different browser arguments must also have different [data directories](WebContext::new).
	/// - By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
	///   `--autoplay-policy=no-user-gesture-required` if autoplay is enabled
	///   and `--proxy-server=<scheme>://<host>:<port>` if a proxy is set.
	///   so if you use this method, you have to add these arguments yourself if you want to keep the same behavior.
	fn with_additional_browser_args<S: Into<String>>(self, additional_args: S) -> Self;

	/// Determines whether browser-specific accelerator keys are enabled. When this setting is set to
	/// `false`, it disables all accelerator keys that access features specific to a web browser.
	/// The default value is `true`. See the following link to know more details.
	///
	/// Setting to `false` does nothing on WebView2 Runtime version before 92.0.902.0,
	/// see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10824-prerelease>
	///
	/// <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2settings#arebrowseracceleratorkeysenabled>
	fn with_browser_accelerator_keys(self, enabled: bool) -> Self;

	/// Determines whether the webview's default context menus are enabled. When this setting is set to `false`,
	/// it disables all context menus on the webview - menus on the window's native decorations for example are not affected.
	///
	/// The default value is `true` (context menus are enabled).
	///
	/// <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2settings#aredefaultcontextmenusenabled>
	fn with_default_context_menus(self, enabled: bool) -> Self;

	/// Specifies the theme of webview2. This affects things like `prefers-color-scheme`.
	///
	/// Defaults to [`Theme::Auto`] which will follow the OS defaults.
	///
	/// Requires WebView2 Runtime version 101.0.1210.39 or higher, does nothing on older versions,
	/// see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039>
	fn with_theme(self, theme: Theme) -> Self;

	/// Determines whether the custom protocols should use `https://<scheme>.path/to/page` instead of the default `http://<scheme>.path/to/page`.
	///
	/// Using a `http` scheme will allow mixed content when trying to fetch `http` endpoints
	/// and is therefore less secure.
	///
	/// The default value is `false`.
	fn with_https_scheme(self, enabled: bool) -> Self;

	/// Specifies the native scrollbar style to use with webview2.
	/// CSS styles that modify the scrollbar are applied on top of the native appearance configured here.
	///
	/// Defaults to [`ScrollBarStyle::Default`] which is the browser default used by Microsoft Edge.
	///
	/// Requires WebView2 Runtime version 125.0.2535.41 or higher, does nothing on older versions,
	/// see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/?tabs=dotnetcsharp#10253541>
	///
	/// ## Warning
	///
	/// Webview instances with different scroll bar styles must also have different [data directories](WebContext::new).
	fn with_scroll_bar_style(self, style: ScrollBarStyle) -> Self;

	/// Determines whether the ability to install and enable extensions is enabled.
	///
	/// By default, extensions are disabled.
	///
	/// Requires WebView2 Runtime version 120.0.2210.55 or higher, does nothing on older versions,
	/// see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10221055>
	///
	/// ## Warning
	///
	/// Webview instances with different browser extensions enabled settings must also have different [data directories](WebContext::new).
	fn with_browser_extensions_enabled(self, enabled: bool) -> Self;

	/// Set the path from which to load extensions from. Extensions stored in this path should be unpacked.
	///
	/// Does nothing if browser extensions are disabled. See [`with_browser_extensions_enabled`](Self::with_browser_extensions_enabled)
	fn with_extensions_path(self, path: impl Into<PathBuf>) -> Self;

	/// Set the environment for the webview.
	/// Useful if you need to share the same environment, for instance when using the [`WebViewBuilder::with_new_window_req_handler`].
	fn with_environment(self, environment: ICoreWebView2Environment) -> Self;

	/// Set the WebView2 profile name for this webview. Webviews with different
	/// profile names within the same environment have isolated cookies, storage,
	/// IndexedDB, cache, and other site data, while sharing the runtime.
	///
	/// When `None` (the default), the webview uses the unnamed default profile.
	///
	/// See <https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/multi-profile-support>
	/// for the underlying WebView2 multi-profile feature.
	///
	/// Profile names must follow the WebView2 naming rules (alphanumeric, `.`,
	/// `_`, `-`, ` `, up to 64 chars, not starting/ending with `.` or ` `).
	fn with_profile_name<S: Into<String>>(self, name: S) -> Self;
}

impl WebViewBuilderExtWindows for WebViewBuilder<'_> {
	fn with_additional_browser_args<S: Into<String>>(mut self, additional_args: S) -> Self {
		self.platform_specific.additional_browser_args = Some(additional_args.into());
		self
	}

	fn with_browser_accelerator_keys(mut self, enabled: bool) -> Self {
		self.platform_specific.browser_accelerator_keys = enabled;
		self
	}

	fn with_default_context_menus(mut self, enabled: bool) -> Self {
		self.platform_specific.default_context_menus = enabled;
		self
	}

	fn with_theme(mut self, theme: Theme) -> Self {
		self.platform_specific.theme = Some(theme);
		self
	}

	fn with_https_scheme(mut self, enabled: bool) -> Self {
		self.platform_specific.use_https = enabled;
		self
	}

	fn with_scroll_bar_style(mut self, style: ScrollBarStyle) -> Self {
		self.platform_specific.scroll_bar_style = style;
		self
	}

	fn with_browser_extensions_enabled(mut self, enabled: bool) -> Self {
		self.platform_specific.browser_extensions_enabled = enabled;
		self
	}

	fn with_extensions_path(mut self, path: impl Into<PathBuf>) -> Self {
		self.platform_specific.extension_path = Some(path.into());
		self
	}

	fn with_environment(mut self, environment: ICoreWebView2Environment) -> Self {
		self.platform_specific.environment.replace(environment);
		self
	}

	fn with_profile_name<S: Into<String>>(mut self, name: S) -> Self {
		self.platform_specific.profile_name = Some(name.into());
		self
	}
}

/// The fundamental type to present a [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to construct WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebView`] presents the actual WebView window and let you still able to perform actions on it.
pub struct WebView {
	webview: InnerWebView,
}

impl WebView {
	/// Returns the id of this webview.
	pub fn id(&self) -> WebViewId<'_> {
		self.webview.id()
	}

	/// Get the current url of the webview
	pub fn url(&self) -> Result<String> {
		self.webview.url()
	}

	/// Evaluate and run javascript code.
	pub fn evaluate_script(&self, js: &str) -> Result<()> {
		self
			.webview
			.eval(js, None::<Box<dyn Fn(String) + Send + 'static>>)
	}

	/// Evaluate and run javascript code with callback function. The evaluation result will be
	/// serialized into a JSON string and passed to the callback function.
	///
	/// Exception is ignored because of the limitation on windows. You can catch it yourself and return as string as a workaround.
	pub fn evaluate_script_with_callback(
		&self,
		js: &str,
		callback: impl Fn(String) + Send + 'static,
	) -> Result<()> {
		self.webview.eval(js, Some(callback))
	}

	/// Launch print modal for the webview content.
	pub fn print(&self) -> Result<()> {
		self.webview.print()
	}

	/// Get a list of cookies for specific url.
	pub fn cookies_for_url(&self, url: &str) -> Result<Vec<cookie::Cookie<'static>>> {
		self.webview.cookies_for_url(url)
	}

	/// Get the list of cookies.
	pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>> {
		self.webview.cookies()
	}

	/// Set a cookie for the webview.
	pub fn set_cookie(&self, cookie: &cookie::Cookie<'_>) -> Result<()> {
		self.webview.set_cookie(cookie)
	}

	/// Delete a cookie for the webview.
	pub fn delete_cookie(&self, cookie: &cookie::Cookie<'_>) -> Result<()> {
		self.webview.delete_cookie(cookie)
	}

	/// Open the web inspector which is usually called dev tool.
	#[cfg(any(debug_assertions, feature = "devtools"))]
	pub fn open_devtools(&self) {
		self.webview.open_devtools()
	}

	/// Close the web inspector which is usually called dev tool.
	///
	/// ## Platform-specific
	///
	/// - **Windows:** Not supported.
	#[cfg(any(debug_assertions, feature = "devtools"))]
	pub fn close_devtools(&self) {
		self.webview.close_devtools()
	}

	/// Gets the devtool window's current visibility state.
	///
	/// ## Platform-specific
	///
	/// - **Windows:** Not supported.
	#[cfg(any(debug_assertions, feature = "devtools"))]
	pub fn is_devtools_open(&self) -> bool {
		self.webview.is_devtools_open()
	}

	/// Set the webview zoom level
	pub fn zoom(&self, scale_factor: f64) -> Result<()> {
		self.webview.zoom(scale_factor)
	}

	/// Specify the webview background color.
	///
	/// The color uses the RGBA format.
	///
	/// ## Platform-specific:
	///
	/// - **Windows**:
	///   - On Windows 7, transparency is not supported and the alpha value will be ignored.
	///   - On Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
	pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
		self.webview.set_background_color(background_color)
	}

	/// Navigate to the specified url
	pub fn load_url(&self, url: &str) -> Result<()> {
		self.webview.load_url(url)
	}

	/// Reloads the current page.
	pub fn reload(&self) -> crate::Result<()> {
		self.webview.reload()
	}

	/// Go to the next page.
	pub fn go_forward(&self) -> Result<()> {
		self.webview.go_forward()
	}

	/// Go to the previous page.
	pub fn go_back(&self) -> Result<()> {
		self.webview.go_back()
	}

	pub fn can_go_forward(&self) -> Result<bool> {
		self.webview.can_go_forward()
	}

	pub fn can_go_back(&self) -> Result<bool> {
		self.webview.can_go_back()
	}

	/// Navigate to the specified url using the specified headers
	pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
		self.webview.load_url_with_headers(url, headers)
	}

	/// Load html content into the webview
	pub fn load_html(&self, html: &str) -> Result<()> {
		self.webview.load_html(html)
	}

	/// Clear all browsing data
	pub fn clear_all_browsing_data(&self) -> Result<()> {
		self.webview.clear_all_browsing_data()
	}

	pub fn bounds(&self) -> Result<Rect> {
		self.webview.bounds()
	}

	/// Set the webview bounds.
	///
	/// This is only effective if the webview was created as a child.
	pub fn set_bounds(&self, bounds: Rect) -> Result<()> {
		self.webview.set_bounds(bounds)
	}

	/// Shows or hides the webview.
	pub fn set_visible(&self, visible: bool) -> Result<()> {
		self.webview.set_visible(visible)
	}

	/// Try moving focus to the webview.
	pub fn focus(&self) -> Result<()> {
		self.webview.focus()
	}

	/// Try moving focus away from the webview back to the parent window.
	pub fn focus_parent(&self) -> Result<()> {
		self.webview.focus_parent()
	}
}

/// An event describing drag and drop operations on the webview.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DragDropEvent {
	/// A drag operation has entered the webview.
	Enter {
		/// List of paths that are being dragged onto the webview.
		paths: Vec<PathBuf>,
		/// Position of the drag operation, relative to the webview top-left corner.
		position: (i32, i32),
	},
	/// A drag operation is moving over the window.
	Over {
		/// Position of the drag operation, relative to the webview top-left corner.
		position: (i32, i32),
	},
	/// The file(s) have been dropped onto the window.
	Drop {
		/// List of paths that are being dropped onto the window.
		paths: Vec<PathBuf>,
		/// Position of the drag operation, relative to the webview top-left corner.
		position: (i32, i32),
	},
	/// The drag operation has been cancelled or left the window.
	Leave,
}

/// Get the WebView2 version on Windows.
#[cfg(feature = "os-webview")]
#[cfg_attr(docsrs, doc(cfg(feature = "os-webview")))]
pub fn webview_version() -> Result<String> {
	platform_webview_version()
}

/// The [memory usage target level][1]. There are two levels 'Low' and 'Normal' and the default
/// level is 'Normal'. When the application is going inactive, setting the level to 'Low' can
/// significantly reduce the application's memory consumption.
///
/// [1]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel
#[non_exhaustive]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryUsageLevel {
	/// The 'Normal' memory usage. Applications should set this level when they are becoming active.
	#[default]
	Normal,
	/// The 'Low' memory usage. Applications can reduce memory comsumption by setting this level when
	/// they are becoming inactive.
	Low,
}

/// Additional methods on `WebView` that are specific to Windows.
pub trait WebViewExtWindows {
	/// Returns the WebView2 controller.
	fn controller(&self) -> ICoreWebView2Controller;

	/// Webview environment.
	fn environment(&self) -> ICoreWebView2Environment;

	/// Webview instance.
	fn webview(&self) -> ICoreWebView2;

	/// Changes the webview2 theme.
	///
	/// Requires WebView2 Runtime version 101.0.1210.39 or higher, returns error on older versions,
	/// see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039>
	fn set_theme(&self, theme: Theme) -> Result<()>;

	/// Sets the [memory usage target level][1].
	///
	/// When to best use this mode depends on the app in question. Most commonly it's called when
	/// the app's visiblity state changes.
	///
	/// Please read the [guide for WebView2][2] for more details.
	///
	/// This method uses a WebView2 API added in Runtime version 114.0.1823.32. When it is used in
	/// an older Runtime version, it does nothing.
	///
	/// [1]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel
	/// [2]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.memoryusagetargetlevel?view=webview2-dotnet-1.0.2088.41#remarks
	fn set_memory_usage_level(&self, level: MemoryUsageLevel) -> Result<()>;

	/// Attaches this webview to the given HWND and removes it from the current one.
	fn reparent(&self, hwnd: isize) -> Result<()>;

	/// Returns the child HWND hosting this webview.
	fn hwnd(&self) -> windows::Win32::Foundation::HWND;
}

impl WebViewExtWindows for WebView {
	fn controller(&self) -> ICoreWebView2Controller {
		self.webview.controller.clone()
	}

	fn environment(&self) -> ICoreWebView2Environment {
		self.webview.env.clone()
	}

	fn webview(&self) -> ICoreWebView2 {
		self.webview.webview.clone()
	}

	fn set_theme(&self, theme: Theme) -> Result<()> {
		self.webview.set_theme(theme)
	}

	fn set_memory_usage_level(&self, level: MemoryUsageLevel) -> Result<()> {
		self.webview.set_memory_usage_level(level)
	}

	fn reparent(&self, hwnd: isize) -> Result<()> {
		self.webview.reparent(hwnd)
	}

	/// Returns the child HWND hosting this webview.
	fn hwnd(&self) -> windows::Win32::Foundation::HWND {
		self.webview.hwnd()
	}
}

/// WebView theme.
#[derive(Debug, Clone, Copy)]
pub enum Theme {
	/// Dark
	Dark,
	/// Light
	Light,
	/// System preference
	Auto,
}

/// Type alias for a color in the RGBA format.
///
/// Each value can be 0..255 inclusive.
pub type RGBA = (u8, u8, u8, u8);

/// Type of of page loading event
pub enum PageLoadEvent {
	/// Indicates that the content of the page has started loading
	Started,
	/// Indicates that the page content has finished loading
	Finished,
}

/// Background throttling policy
#[derive(Debug, Clone)]
pub enum BackgroundThrottlingPolicy {
	/// A policy where background throttling is disabled
	Disabled,
	/// A policy where a web view that's not in a window fully suspends tasks.
	Suspend,
	/// A policy where a web view that's not in a window limits processing, but does not fully suspend tasks.
	Throttle,
}

/// An initialization script
#[derive(Debug, Clone)]
pub struct InitializationScript {
	/// The script to run
	pub script: String,
	/// Whether the script should be injected to main frame only.
	///
	/// When set to false, the script is also injected to subframes.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: scripts are always injected into subframes regardless of this option.
	///   This will be the case until Webview2 implements a proper API to inject a script only on the main frame.
	pub for_main_frame_only: bool,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	#[cfg_attr(miri, ignore)]
	fn should_get_webview_version() {
		if let Err(error) = webview_version() {
			panic!("{}", error);
		}
	}
}
