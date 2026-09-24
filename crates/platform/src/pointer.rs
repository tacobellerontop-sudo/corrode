/// Cursor position in physical pixels, in desktop coordinates.
#[allow(unsafe_code)]
pub fn cursor_position() -> Option<(f64, f64)> {
	use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
	let mut cursor = POINT::default();
	// SAFETY: stack POINT; GetCursorPos fails while the input desktop is locked.
	unsafe {
		GetCursorPos(&mut cursor).ok()?;
	}
	Some((f64::from(cursor.x), f64::from(cursor.y)))
}
