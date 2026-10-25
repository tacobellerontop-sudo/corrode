//! GPU texture cache for avatars, community icons and message media.
//!
//! Views ask for a key each frame they want to paint it. Missing keys are queued once (with a
//! retry delay) for the desktop's credential-free image worker; decoded pixels come back
//! through [`Images::accept`]. The cache is bounded by entry count and estimated bytes and
//! evicts the least recently painted textures.
use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions, Vec2, vec2};
use std::{
	collections::{HashMap, HashSet},
	time::{Duration, Instant},
};

const MAX_TEXTURES: usize = 384;
const MAX_BYTES: usize = 96 * 1024 * 1024;
const RETRY: Duration = Duration::from_secs(20);
/// Longest edge requested for in-timeline media.
pub const MEDIA_EDGE: u32 = 512;

struct Entry {
	texture: TextureHandle,
	size: Vec2,
	bytes: usize,
	used: u64,
}

#[derive(Default)]
pub struct Images {
	entries: HashMap<String, Entry>,
	bytes: usize,
	frame: u64,
	pending: HashSet<String>,
	failed: HashMap<String, Instant>,
	queue: Vec<String>,
}

impl Images {
	/// Call once per frame before painting.
	pub fn begin_frame(&mut self) {
		self.frame = self.frame.wrapping_add(1);
	}

	/// Texture for `key` if loaded; otherwise schedule it and return `None`.
	pub fn get(&mut self, key: &str) -> Option<(TextureId, Vec2)> {
		if let Some(entry) = self.entries.get_mut(key) {
			entry.used = self.frame;
			return Some((entry.texture.id(), entry.size));
		}
		self.request(key);
		None
	}

	pub fn get_opt(&mut self, key: Option<&str>) -> Option<(TextureId, Vec2)> {
		key.and_then(|key| self.get(key))
	}

	fn request(&mut self, key: &str) {
		if self.pending.contains(key) || self.queue.len() >= 256 {
			return;
		}
		if let Some(at) = self.failed.get(key) {
			if at.elapsed() < RETRY {
				return;
			}
			self.failed.remove(key);
		}
		self.pending.insert(key.to_owned());
		self.queue.push(key.to_owned());
	}

	/// Keys newly wanted since the last call, for the image worker.
	pub fn take_requests(&mut self) -> Vec<String> {
		std::mem::take(&mut self.queue)
	}

	/// Deliver a decoded image (or a failure) for a previously requested key.
	pub fn accept(&mut self, ctx: &Context, key: String, image: Option<ColorImage>) {
		self.pending.remove(&key);
		let Some(image) = image else {
			if self.failed.len() > 4096 {
				self.failed.clear();
			}
			self.failed.insert(key, Instant::now());
			return;
		};
		self.insert(ctx, key, image);
	}

	/// Insert pixels directly (used for locally generated artwork and previews).
	pub fn insert(&mut self, ctx: &Context, key: String, image: ColorImage) {
		let size = vec2(image.size[0] as f32, image.size[1] as f32);
		let bytes = image.size[0] * image.size[1] * 4;
		let texture = ctx.load_texture(key.clone(), image, TextureOptions::LINEAR);
		if let Some(old) = self.entries.insert(
			key,
			Entry {
				texture,
				size,
				bytes,
				used: self.frame,
			},
		) {
			self.bytes -= old.bytes;
		}
		self.bytes += bytes;
		self.evict();
	}

	fn evict(&mut self) {
		while self.entries.len() > MAX_TEXTURES || self.bytes > MAX_BYTES {
			let Some(oldest) = self
				.entries
				.iter()
				.filter(|(_, entry)| entry.used != self.frame)
				.min_by_key(|(_, entry)| entry.used)
				.map(|(key, _)| key.clone())
			else {
				break;
			};
			if let Some(entry) = self.entries.remove(&oldest) {
				self.bytes -= entry.bytes;
			}
		}
	}

	/// Forget everything (account switch / logout).
	pub fn clear(&mut self) {
		*self = Self::default();
	}

	pub fn is_pending(&self, key: &str) -> bool {
		self.pending.contains(key)
	}
}

/// Image-worker key for a message attachment or embed image, sized for the timeline.
pub fn media_key(media: &model::EmbedMedia) -> Option<String> {
	let source = media
		.proxy_url
		.as_deref()
		.or(media.url.as_deref())
		.filter(|source| source.len() <= 2048)?;
	if media.width == 0 || media.height == 0 {
		return Some(format!("embed:{source}"));
	}
	let mut url = url::Url::parse(source).ok()?;
	let longest = media.width.max(media.height).max(MEDIA_EDGE) as u64;
	let width = ((media.width as u64 * MEDIA_EDGE as u64) / longest).max(1);
	let height = ((media.height as u64 * MEDIA_EDGE as u64) / longest).max(1);
	let query: Vec<(String, String)> = url
		.query_pairs()
		.filter(|(key, _)| key != "width" && key != "height")
		.map(|(key, value)| (key.into_owned(), value.into_owned()))
		.collect();
	url.set_query(None);
	url.query_pairs_mut()
		.extend_pairs(query)
		.append_pair("width", &width.to_string())
		.append_pair("height", &height.to_string());
	Some(format!("embed:{url}"))
}

/// Image-worker key for a group DM icon.
pub fn group_icon_key(channel: &model::Channel) -> Option<String> {
	let hash = channel.icon.as_deref()?;
	model::valid_avatar_hash(hash).then(|| format!("group-icon-{}-{hash}", channel.id))
}

/// Image-worker key for a custom emoji.
pub fn emoji_key(id: model::Id) -> String {
	format!("emoji-{id}")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn media_keys_resize_and_keep_signatures() {
		let media = model::EmbedMedia {
			proxy_url: Some(
				"https://media.discordapp.net/attachments/1/2/a.png?ex=1&hm=sig&width=9".into(),
			),
			width: 2048,
			height: 1024,
			..Default::default()
		};
		assert_eq!(
			media_key(&media).unwrap(),
			"embed:https://media.discordapp.net/attachments/1/2/a.png?ex=1&hm=sig&width=512&height=256"
		);
		let unknown = model::EmbedMedia {
			url: Some("https://cdn.discordapp.com/attachments/1/2/b.png".into()),
			..Default::default()
		};
		assert_eq!(
			media_key(&unknown).unwrap(),
			"embed:https://cdn.discordapp.com/attachments/1/2/b.png"
		);
	}

	#[test]
	fn missing_keys_are_requested_once() {
		let mut images = Images::default();
		assert!(images.get("1-abc").is_none());
		assert!(images.get("1-abc").is_none());
		assert_eq!(images.take_requests(), vec!["1-abc".to_owned()]);
		assert!(images.take_requests().is_empty());
		let ctx = Context::default();
		images.accept(&ctx, "1-abc".into(), None);
		assert!(images.get("1-abc").is_none());
		assert!(
			images.take_requests().is_empty(),
			"failures wait before retrying"
		);
		images.insert(
			&ctx,
			"1-abc".into(),
			ColorImage::filled([2, 2], egui::Color32::RED),
		);
		assert!(images.get("1-abc").is_some());
	}
}
