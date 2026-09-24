//! Native background blur and window chrome; transparency itself remains owned by winit/eframe.
use std::sync::Arc;
use winit::window::Window;

pub struct Blur {
	native_enabled: bool,
	window: Arc<Window>,
}

impl Blur {
	/// Initialize once on the window thread, before rendering starts.
	pub fn new(window: Arc<Window>) -> Self {
		Self {
			native_enabled: false,
			window,
		}
	}

	pub fn set_enabled(&mut self, enabled: bool) {
		if enabled == self.native_enabled {
			return;
		}
		self.native_enabled = enabled;
		use winit::platform::windows::{BackdropType, WindowExtWindows};
		self.window.set_system_backdrop(if enabled {
			BackdropType::TransientWindow
		} else {
			BackdropType::None
		});
	}
}

/// Round the top-level window corners (Windows 11). Borderless windows stay
/// square unless DWM opts in; harmless where rounding is unsupported.
pub fn round_corners(window: &Window) {
	#[cfg(target_os = "windows")]
	native::round_corners(window);
	#[cfg(not(target_os = "windows"))]
	let _ = window;
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod native {
	use super::Window;
	use windows::Win32::{
		Foundation::HWND,
		Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
	};
	use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

	pub fn round_corners(window: &Window) {
		let Ok(handle) = window.window_handle() else {
			return;
		};
		let RawWindowHandle::Win32(handle) = handle.as_raw() else {
			return;
		};
		let hwnd = HWND(handle.hwnd.get() as *mut _);
		let preference = DWMWCP_ROUND;
		// SAFETY: hwnd borrows the live winit window for the call; the
		// attribute buffer is an exactly sized stack value. A failed call
		// (older Windows, no DWM) just leaves square corners.
		unsafe {
			let _ = DwmSetWindowAttribute(
				hwnd,
				DWMWA_WINDOW_CORNER_PREFERENCE,
				&preference as *const _ as *const std::ffi::c_void,
				std::mem::size_of_val(&preference) as u32,
			);
		}
	}
}
