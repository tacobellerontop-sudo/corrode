//! Inline attachment decoding. The caller owns validated, bounded network I/O and hands over an
//! anonymous read-only stream: the decoder never sees a URL or an account token, and yields
//! bounded RGBA frames and stereo PCM samples.
//!
//! * Windows: Media Foundation (`media_foundation`).
use std::io::{Read, Seek};

mod media_foundation;
pub use media_foundation::Decoder;

/// Hardware-backed decoding of one live H.264 elementary stream (camera or Go Live).
/// Windows uses the Media Foundation decoder with a DXGI device manager when available.
#[path = "live_windows.rs"]
pub mod live;

/// Largest live access unit any backend accepts.
pub const MAX_ACCESS_UNIT: usize = 2 * 1024 * 1024 + 64 * 1024;

/// One decoded live picture, tightly packed RGBA.
pub struct LiveFrame {
	pub width: u32,
	pub height: u32,
	pub rgba: Vec<u8>,
}

/// Receives live pictures from the decoder's own thread, in decode order.
pub type LiveSink = Box<dyn Fn(LiveFrame) + Send + Sync>;

/// Longest attachment any backend plays inline.
pub const MAX_SECONDS: f64 = 2.0 * 60.0 * 60.0;
/// Largest single decoded frame or compressed sample any backend accepts.
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const UNSUPPORTED: &str = "This video format or codec is not supported on this system.";
pub const INVALID: &str = "The video could not be decoded safely.";
pub const TOO_LARGE: &str = "Inline playback supports videos up to 1080p.";
pub const TOO_LONG: &str = "Videos longer than two hours are not supported.";

pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

#[derive(Clone, Copy, Debug)]
pub struct Info {
	pub width: u32,
	pub height: u32,
	pub duration: f64,
	/// Zero when the attachment has no audio track.
	pub sample_rate: u32,
	pub channels: u16,
}

pub enum Sample {
	Video {
		pts: f64,
		width: u32,
		height: u32,
		rgba: Vec<u8>,
	},
	Audio {
		pts: f64,
		frames: Vec<[f32; 2]>,
	},
}

/// The backend reads tracks independently; the player polls through one interface.
impl Decoder {
	pub fn poll_video(&mut self) -> Result<std::task::Poll<Option<Sample>>, &'static str> {
		self.read_video().map(std::task::Poll::Ready)
	}

	pub fn poll_audio(&mut self) -> Result<std::task::Poll<Option<Sample>>, &'static str> {
		self.read_audio().map(std::task::Poll::Ready)
	}
}

/// The shared inline-player texture bound: 1080p worth of pixels within a 1920 px square.
pub fn check_dimensions(width: u32, height: u32) -> Result<(), &'static str> {
	if width == 0 || height == 0 {
		return Err(INVALID);
	}
	if width > 1920 || height > 1920 || u64::from(width) * u64::from(height) > 1920 * 1080 {
		return Err(TOO_LARGE);
	}
	Ok(())
}
