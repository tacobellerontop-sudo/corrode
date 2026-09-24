/// Permission types that can be requested by the webview.
///
/// See [`crate::WebViewBuilder::with_permission_handler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PermissionKind {
	/// Microphone access permission.
	Microphone,
	/// Camera access permission.
	Camera,
	/// Geolocation access permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION`.
	Geolocation,
	/// Notifications permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS`.
	Notifications,
	/// Clipboard read permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ`.
	ClipboardRead,
	/// Display capture permission (for getDisplayMedia).
	DisplayCapture,
	/// Midi access permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES`.
	Midi,
	/// Sensors (accelerometer, gyroscope, etc.) access permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS`.
	Sensors,
	/// Media key system access permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Not yet supported by the platform backend.
	MediaKeySystemAccess,
	/// Local fonts access permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS`.
	LocalFonts,
	/// Window management permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_WINDOW_MANAGEMENT`.
	WindowManagement,
	/// Pointer lock permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Not yet supported by the platform backend.
	PointerLock,
	/// Automatic downloads permission (multiple downloads without user interaction).
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS`.
	AutomaticDownloads,
	/// File system access permission (read/write via File System Access API).
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE`.
	FileSystemAccess,
	/// Media autoplay permission.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: Supported via `COREWEBVIEW2_PERMISSION_KIND_AUTOPLAY`.
	Autoplay,
	/// Other unrecognized permission type.
	Other,
}

impl std::fmt::Display for PermissionKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Microphone => write!(f, "microphone"),
			Self::Camera => write!(f, "camera"),
			Self::Geolocation => write!(f, "geolocation"),
			Self::Notifications => write!(f, "notifications"),
			Self::ClipboardRead => write!(f, "clipboard-read"),
			Self::DisplayCapture => write!(f, "display-capture"),
			Self::Midi => write!(f, "midi"),
			Self::Sensors => write!(f, "sensors"),
			Self::MediaKeySystemAccess => write!(f, "media-key-system-access"),
			Self::LocalFonts => write!(f, "local-fonts"),
			Self::WindowManagement => write!(f, "window-management"),
			Self::PointerLock => write!(f, "pointer-lock"),
			Self::AutomaticDownloads => write!(f, "automatic-downloads"),
			Self::FileSystemAccess => write!(f, "file-system-access"),
			Self::Autoplay => write!(f, "autoplay"),
			Self::Other => write!(f, "other"),
		}
	}
}

/// Response for permission requests.
///
/// See [`crate::WebViewBuilder::with_permission_handler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PermissionResponse {
	/// Grant the permission.
	Allow,
	/// Deny the permission.
	Deny,
	/// Use the platform or browser default behavior.
	///
	/// ## Platform-specific
	///
	/// - **Windows**: The default behavior is to continue the browser permission flow.
	#[default]
	Default,
}

impl std::fmt::Display for PermissionResponse {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Allow => write!(f, "allow"),
			Self::Deny => write!(f, "deny"),
			Self::Default => write!(f, "default"),
		}
	}
}
