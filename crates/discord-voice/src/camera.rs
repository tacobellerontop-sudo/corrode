//! User-started, ephemeral camera capture. No capture stream starts before `start`.

use openh264::{
	OpenH264API,
	encoder::{BitRate, Encoder, EncoderConfig, FrameRate, Profile},
	formats::{RgbSliceU8, YUVBuffer},
};
use std::sync::{
	Arc, Mutex,
	atomic::{AtomicBool, Ordering},
};
use std::{thread, time::Duration};

mod windows;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
pub const MAX_ENCODED_BYTES: usize = 128 * 1024;
pub const SUPPORTED: bool = true;
const FRAME_INTERVAL: Duration = Duration::from_millis(67);
// Includes asynchronous teardown: rapid toggles cannot accumulate camera workers.
static RUNNING: AtomicBool = AtomicBool::new(false);

pub struct Frame {
	pub rgb: Vec<u8>,
	pub h264: Vec<u8>,
}

#[derive(Default)]
struct Shared {
	stopped: AtomicBool,
	active: AtomicBool,
	finished: AtomicBool,
	error: Mutex<Option<&'static str>>,
}

pub struct Camera {
	shared: Arc<Shared>,
}

/// Device identities and friendly labels, limited to 32 entries and 136 KiB total.
pub type DeviceList = Vec<(String, String)>;

/// Enumeration never opens a capture stream.
pub fn devices() -> Result<DeviceList, &'static str> {
	windows::devices()
}

impl Camera {
	/// Call only after an explicit camera-on gesture in a call or settings preview.
	pub fn start(
		device: Option<String>,
		on_frame: Arc<dyn Fn(Frame) + Send + Sync>,
		wake: Arc<dyn Fn() + Send + Sync>,
	) -> Result<Self, &'static str> {
		if !SUPPORTED {
			return Err("Camera capture is unavailable on this platform");
		}
		if device
			.as_ref()
			.is_some_and(|id| id.len() > 4096 || id.contains('\0'))
		{
			return Err("Invalid camera device selection");
		}
		if RUNNING
			.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
			.is_err()
		{
			return Err("Previous camera session is still closing; try again shortly");
		}
		let shared = Arc::new(Shared::default());
		let worker = shared.clone();
		if thread::Builder::new()
			.name("corrode-camera".into())
			.spawn(move || {
				let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					run(&worker, device.as_deref(), &on_frame, &wake)
				}))
				.unwrap_or(Err("Camera worker failed"));
				worker.active.store(false, Ordering::Release);
				if !worker.stopped.load(Ordering::Acquire)
					&& let Err(error) = result
					&& let Ok(mut slot) = worker.error.lock()
				{
					*slot = Some(error);
				}
				worker.finished.store(true, Ordering::Release);
				RUNNING.store(false, Ordering::Release);
				wake();
			})
			.is_err()
		{
			RUNNING.store(false, Ordering::Release);
			return Err("Camera worker could not start");
		}
		Ok(Self { shared })
	}

	pub fn stop(&self) {
		self.shared.stopped.store(true, Ordering::Release);
		self.shared.active.store(false, Ordering::Release);
	}

	pub fn stopped(&self) -> bool {
		self.shared.finished.load(Ordering::Acquire)
	}

	pub fn error(&self) -> Option<&'static str> {
		*self.shared.error.lock().ok()?
	}

	pub fn active(&self) -> bool {
		!self.shared.stopped.load(Ordering::Acquire) && self.shared.active.load(Ordering::Acquire)
	}
}

/// Camera encoder preferring the platform hardware H.264 encoder (Media Foundation on
/// Windows) and falling back to openh264 when it is unavailable or fails mid-stream. Baseline
/// profile and one IDR per picture either way, so the wire format does not change.
struct CameraEncoder {
	diagnostics: crate::diagnostics::EncoderRegistration,
	software: Option<Encoder>,
	yuv: YUVBuffer,
	hardware: Option<crate::video_encode::hardware::Encoder>,
}

impl CameraEncoder {
	fn new() -> Result<Self, &'static str> {
		let config = crate::video_encode::Config {
			width: WIDTH as u32,
			height: HEIGHT as u32,
			fps: 15,
			bit_rate: 600_000,
			max_bytes: MAX_ENCODED_BYTES,
			profile: crate::video_encode::Profile::Baseline,
		};
		let hardware = crate::video_encode::hardware::Encoder::new(config).ok();
		let software = if hardware.is_some() {
			None
		} else {
			Some(encoder()?)
		};
		Ok(Self {
			diagnostics: crate::diagnostics::EncoderRegistration::new(false, software.is_none()),
			software,
			yuv: YUVBuffer::new(WIDTH, HEIGHT),
			hardware,
		})
	}

	/// Encodes one 640×480 packed RGB picture, keeping the frame's pixels for local preview.
	fn encode(&mut self, rgb: Vec<u8>) -> Result<Option<Frame>, &'static str> {
		if rgb.len() != WIDTH * HEIGHT * 3 {
			return Err("Camera did not provide a bounded 640×480 RGB frame");
		}
		if let Some(hardware) = self.hardware.as_mut() {
			// ponytail: every picture stays independently decodable, matching the software
			// encoder, because the sender drops to the latest frame; inter prediction would
			// freeze a receiver until the next refresh.
			let encoded = {
				use openh264::formats::YUVSource;
				self.yuv.read_rgb8(RgbSliceU8::new(&rgb, (WIDTH, HEIGHT)));
				hardware.encode(self.yuv.y(), self.yuv.u(), self.yuv.v(), true)
			};
			match encoded {
				Ok((h264, _)) => {
					if h264.len() > MAX_ENCODED_BYTES {
						return Err("Camera encoded frame exceeded its 128 KiB limit");
					}
					return Ok((!h264.is_empty()).then_some(Frame { rgb, h264 }));
				}
				Err(_) => {
					self.hardware = None;
					self.diagnostics.set(None);
					self.software = Some(encoder()?);
					self.diagnostics.set(Some(false));
				}
			}
		}
		let software = self
			.software
			.as_mut()
			.ok_or("Camera H264 encoder could not start")?;
		encode_rgb(software, &mut self.yuv, rgb)
	}
}

fn encoder() -> Result<Encoder, &'static str> {
	Encoder::with_api_config(
		OpenH264API::from_source(),
		EncoderConfig::new()
			.bitrate(BitRate::from_bps(600_000))
			.max_frame_rate(FrameRate::from_hz(15.0))
			.profile(Profile::Baseline)
			.num_threads(1)
			.debug(false),
	)
	.map_err(|_| "Camera H264 encoder could not start")
}

fn encode_rgb(
	encoder: &mut Encoder,
	yuv: &mut YUVBuffer,
	rgb: Vec<u8>,
) -> Result<Option<Frame>, &'static str> {
	if rgb.len() != WIDTH * HEIGHT * 3 {
		return Err("Camera did not provide a bounded 640×480 RGB frame");
	}
	yuv.read_rgb8(RgbSliceU8::new(&rgb, (WIDTH, HEIGHT)));
	// ponytail: independently decodable frames tolerate latest-slot drops;
	// add feedback-aware inter frames when bandwidth adaptation is implemented.
	encoder.force_intra_frame();
	let bits = encoder
		.encode(yuv)
		.map_err(|_| "Camera frame could not be encoded")?;
	if bits.raw_info().iFrameSizeInBytes < 0
		|| bits.raw_info().iFrameSizeInBytes as usize > MAX_ENCODED_BYTES
	{
		return Err("Camera encoded frame exceeded its 128 KiB limit");
	}
	let h264 = bits.to_vec();
	Ok((!h264.is_empty()).then_some(Frame { rgb, h264 }))
}

fn run(
	shared: &Shared,
	device: Option<&str>,
	on_frame: &Arc<dyn Fn(Frame) + Send + Sync>,
	wake: &Arc<dyn Fn() + Send + Sync>,
) -> Result<(), &'static str> {
	let mut encoder = CameraEncoder::new()?;
	let mut emit = |rgb| {
		if !shared.stopped.load(Ordering::Acquire)
			&& let Some(frame) = encoder.encode(rgb)?
			&& !shared.stopped.load(Ordering::Acquire)
		{
			on_frame(frame);
			shared.active.store(true, Ordering::Release);
			wake();
		}
		Ok(())
	};
	windows::run(shared, device, &mut emit)
}

impl Drop for Camera {
	fn drop(&mut self) {
		self.stop();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn camera_rejects_unbounded_or_nul_device_ids_before_starting_worker() {
		for id in ["x".repeat(4097), "dshow:bad\0id".into()] {
			let error = Camera::start(
				Some(id),
				Arc::new(|_| panic!("no capture")),
				Arc::new(|| {}),
			)
			.err();
			assert_eq!(error, Some("Invalid camera device selection"));
		}
	}
	use openh264::formats::YUVSource;

	#[test]
	fn hardware_camera_frames_decode_and_stay_independently_decodable() {
		let mut encoder = CameraEncoder::new().unwrap();
		// Exactly one encoder is live: hardware when the machine offers it, openh264 otherwise.
		assert_eq!(encoder.hardware.is_some(), encoder.software.is_none());
		for length in [0, WIDTH * HEIGHT * 3 - 1, WIDTH * HEIGHT * 3 + 1] {
			assert!(encoder.encode(vec![0; length]).is_err());
		}
		let mut decoder = openh264::decoder::Decoder::new().unwrap();
		for value in [0, 96, 255] {
			let mut rgb = vec![value; WIDTH * HEIGHT * 3];
			// Flat pictures compress to almost nothing; vary one row so the size check bites.
			for (index, pixel) in rgb
				.as_chunks_mut::<3>()
				.0
				.iter_mut()
				.take(WIDTH)
				.enumerate()
			{
				*pixel = [(index % 251) as u8, value, (index % 97) as u8];
			}
			let Some(frame) = encoder.encode(rgb).unwrap() else {
				continue;
			};
			assert_eq!(frame.rgb.len(), WIDTH * HEIGHT * 3);
			assert!(frame.h264.len() <= MAX_ENCODED_BYTES);
			// The sender drops to the latest frame, so each picture must stand alone.
			assert!(crate::video_receive::is_keyframe(&frame.h264));
			assert!(crate::video_receive::has_parameter_sets(&frame.h264));
			let decoded = decoder.decode(&frame.h264).unwrap().unwrap();
			assert_eq!(decoded.dimensions(), (WIDTH, HEIGHT));
		}
	}

	#[test]
	fn camera_frames_are_bounded_independently_decodable_and_stop_is_immediate() {
		let mut encoder = encoder().unwrap();
		let mut yuv = YUVBuffer::new(WIDTH, HEIGHT);
		for length in [0, WIDTH * HEIGHT * 3 - 1, WIDTH * HEIGHT * 3 + 1] {
			assert!(encode_rgb(&mut encoder, &mut yuv, vec![0; length]).is_err());
		}
		for value in [0, 127, 255] {
			let frame = encode_rgb(&mut encoder, &mut yuv, vec![value; WIDTH * HEIGHT * 3])
				.unwrap()
				.unwrap();
			assert!(frame.h264.len() <= MAX_ENCODED_BYTES);
			let mut decoder = openh264::decoder::Decoder::new().unwrap();
			let decoded = decoder.decode(&frame.h264).unwrap().unwrap();
			assert_eq!(decoded.dimensions(), (WIDTH, HEIGHT));
		}
		let camera = Camera {
			shared: Arc::new(Shared::default()),
		};
		camera.shared.active.store(true, Ordering::Release);
		assert!(camera.active());
		camera.stop();
		// A late native callback cannot turn a stopped camera back on.
		camera.shared.active.store(true, Ordering::Release);
		assert!(!camera.active());
		assert!(!camera.stopped());
		camera.shared.finished.store(true, Ordering::Release);
		assert!(camera.stopped());
	}
}
