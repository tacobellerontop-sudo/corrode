//! One explicit profile picture selection. The chosen file is decoded, cropped to a square
//! and re-encoded as a small PNG off the UI thread; no file path is uploaded or retained.
use eframe::egui;
use image::GenericImageView;
use model::Id;
use std::{
	io::{Cursor, Read},
	path::Path,
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
		mpsc,
	},
};

/// (session generation, account, picker revision) the selection belongs to.
pub type Scope = (u64, Id, u64);
pub type Selected = Result<Option<(String, egui::ColorImage)>, &'static str>;
const MAX_INPUT: usize = 8 * 1024 * 1024;
/// Profile pictures are sent as PNGs of at most this edge (the protocol's icon limit).
const EDGE: u32 = 256;

struct Choosing {
	scope: Scope,
	result: mpsc::Receiver<Selected>,
	cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
pub struct Picture {
	choosing: Option<Choosing>,
}

impl Picture {
	pub fn start(
		&mut self,
		scope: Scope,
		runtime: &tokio::runtime::Handle,
		context: &egui::Context,
		parent: Arc<winit::window::Window>,
	) -> Result<(), &'static str> {
		if self.choosing.is_some() {
			return Err("Close the previous image picker first");
		}
		let dialog = platform::save::icon_source(parent, "Choose profile picture");
		let (send, result) = mpsc::sync_channel(1);
		let cancelled = Arc::new(AtomicBool::new(false));
		let flag = cancelled.clone();
		let context = context.clone();
		runtime.spawn(async move {
			let result = match dialog.await {
				Some(path) if !flag.load(Ordering::Acquire) => {
					let decode_flag = flag.clone();
					tokio::task::spawn_blocking(move || {
						if decode_flag.load(Ordering::Acquire) {
							Ok(None)
						} else {
							read(&path).map(Some)
						}
					})
					.await
					.unwrap_or(Err("Image preparation interrupted; choose it again"))
				}
				_ => Ok(None),
			};
			let _ = send.send(if flag.load(Ordering::Acquire) {
				Ok(None)
			} else {
				result
			});
			context.request_repaint();
		});
		self.choosing = Some(Choosing {
			scope,
			result,
			cancelled,
		});
		Ok(())
	}

	pub fn cancel(&self) {
		if let Some(choosing) = &self.choosing {
			choosing.cancelled.store(true, Ordering::Release);
		}
	}

	/// The finished selection, unless the session or account changed while choosing.
	pub fn poll(&mut self, state: &client_core::State) -> Option<(Scope, Selected)> {
		let choosing = self.choosing.as_ref()?;
		if choosing.scope.0 != state.generation
			|| state
				.user
				.as_ref()
				.is_none_or(|user| user.id != choosing.scope.1)
		{
			self.cancel();
		}
		let result = match choosing.result.try_recv() {
			Ok(result) => result,
			Err(mpsc::TryRecvError::Empty) => return None,
			Err(mpsc::TryRecvError::Disconnected) => {
				Err("Image preparation interrupted; choose it again")
			}
		};
		let choosing = self.choosing.take()?;
		(!choosing.cancelled.load(Ordering::Acquire)).then_some((choosing.scope, result))
	}
}

impl Drop for Picture {
	fn drop(&mut self) {
		self.cancel();
	}
}

fn read(path: &Path) -> Result<(String, egui::ColorImage), &'static str> {
	let metadata =
		std::fs::symlink_metadata(path).map_err(|_| "Could not open the chosen image")?;
	if !metadata.is_file() || metadata.file_type().is_symlink() {
		return Err("Choose a regular image file");
	}
	if metadata.len() == 0 || metadata.len() > MAX_INPUT as u64 {
		return Err("Choose an image up to 8 MB");
	}
	let file = std::fs::File::open(path).map_err(|_| "Could not open the chosen image")?;
	let mut bytes = Vec::with_capacity(metadata.len() as usize);
	file.take(MAX_INPUT as u64 + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| "Could not read the chosen image")?;
	decode(&bytes)
}

fn decode(bytes: &[u8]) -> Result<(String, egui::ColorImage), &'static str> {
	if bytes.len() > MAX_INPUT {
		return Err("Choose an image up to 8 MB");
	}
	let mut reader = image::ImageReader::new(Cursor::new(bytes))
		.with_guessed_format()
		.map_err(|_| "Choose a PNG, JPEG, GIF or WebP image")?;
	let mut limits = image::Limits::default();
	limits.max_image_width = Some(4096);
	limits.max_image_height = Some(4096);
	limits.max_alloc = Some(64 * 1024 * 1024);
	reader.limits(limits);
	let image = reader
		.decode()
		.map_err(|_| "Image is unsupported or too large; use at most 4096 × 4096 pixels")?;
	let edge = image.width().min(image.height());
	let crop = image.view(
		(image.width() - edge) / 2,
		(image.height() - edge) / 2,
		edge,
		edge,
	);
	let image = image::DynamicImage::ImageRgba8(image::imageops::thumbnail(
		&*crop,
		edge.min(EDGE),
		edge.min(EDGE),
	));
	let mut png = Cursor::new(Vec::new());
	image
		.write_to(&mut png, image::ImageFormat::Png)
		.map_err(|_| "Could not prepare the picture")?;
	let uri = discord_protocol::group_actions::icon_data_uri(png.get_ref())
		.filter(|uri| model::valid_avatar_uri(uri))
		.ok_or("Prepared picture exceeds 256 KB; choose a simpler image")?;
	let pixels = image.into_rgba8();
	let preview = egui::ColorImage::from_rgba_unmultiplied(
		[pixels.width() as usize, pixels.height() as usize],
		pixels.as_raw(),
	);
	Ok((uri, preview))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pictures_are_square_bounded_and_invalid_input_is_rejected() {
		let mut png = Cursor::new(Vec::new());
		image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
			512,
			320,
			image::Rgba([255, 0, 0, 255]),
		))
		.write_to(&mut png, image::ImageFormat::Png)
		.unwrap();
		let (uri, preview) = decode(png.get_ref()).unwrap();
		assert_eq!(preview.size, [256, 256]);
		assert!(model::valid_avatar_uri(&uri));
		assert!(decode(b"invalid").is_err());
		assert!(decode(&vec![0; MAX_INPUT + 1]).is_err());
		png = Cursor::new(Vec::new());
		image::DynamicImage::new_rgba8(4097, 1)
			.write_to(&mut png, image::ImageFormat::Png)
			.unwrap();
		assert!(decode(png.get_ref()).is_err());
	}

	#[test]
	fn a_selection_from_another_session_is_discarded() {
		let (send, result) = mpsc::sync_channel(1);
		let mut picker = Picture {
			choosing: Some(Choosing {
				scope: (99, Id(1), 1),
				result,
				cancelled: Arc::new(AtomicBool::new(false)),
			}),
		};
		let state = test_support::demo_state();
		assert!(picker.poll(&state).is_none());
		send.send(Ok(None)).unwrap();
		assert!(picker.poll(&state).is_none());
		assert!(picker.choosing.is_none());
	}
}
