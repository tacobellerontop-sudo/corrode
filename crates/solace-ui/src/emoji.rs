//! Bundled Twemoji atlas: one texture, no runtime requests.
use egui::{Color32, Context, Painter, Rect, TextureHandle, TextureId, pos2};
use image::ImageDecoder;
use std::sync::OnceLock;

const ATLAS: &[u8] = include_bytes!("../../../assets/twemoji/atlas.png");
const INDEX: &str = include_str!("../../../assets/twemoji/index.tsv");
const CELL: f32 = 32.0;
const COLUMNS: usize = 64;
static ENTRIES: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();

fn entries() -> &'static [(&'static str, usize)] {
	ENTRIES.get_or_init(|| {
		INDEX
			.lines()
			.map(|line| {
				let (text, index) = line.split_once('\t').expect("bundled emoji index");
				(text, index.parse().expect("bundled atlas cell"))
			})
			.collect()
	})
}

fn decode() -> Result<egui::ColorImage, image::ImageError> {
	let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(ATLAS))?;
	let (width, height) = decoder.dimensions();
	let mut image =
		egui::ColorImage::filled([width as usize, height as usize], Color32::TRANSPARENT);
	decoder.read_image(image.as_raw_mut())?;
	for pixel in &mut image.pixels {
		let [r, g, b, a] = pixel.to_array();
		*pixel = Color32::from_rgba_unmultiplied(r, g, b, a);
	}
	Ok(image)
}

#[derive(Clone)]
struct Atlas(TextureHandle);

/// Decode once at startup and keep the texture in egui's data store.
pub fn install(ctx: &Context) -> Result<(), image::ImageError> {
	let texture = ctx.load_texture("twemoji", decode()?, egui::TextureOptions::LINEAR);
	ctx.data_mut(|data| data.insert_temp(egui::Id::unique("solace-twemoji"), Atlas(texture)));
	entries();
	Ok(())
}

fn atlas(ctx: &Context) -> Option<(TextureId, egui::Vec2)> {
	ctx.data(|data| data.get_temp::<Atlas>(egui::Id::unique("solace-twemoji")))
		.map(|atlas| (atlas.0.id(), atlas.0.size_vec2()))
}

/// Atlas cell for a complete emoji sequence. Text presentation (VS15) stays text.
pub fn lookup(text: &str) -> Option<usize> {
	if text.is_ascii() || text.contains('\u{fe0e}') || text.len() > 128 {
		return None;
	}
	let normalized: String;
	let key = if text.contains('\u{fe0f}') {
		normalized = text.chars().filter(|c| *c != '\u{fe0f}').collect();
		normalized.as_str()
	} else {
		text
	};
	entries()
		.binary_search_by_key(&key, |(name, _)| *name)
		.ok()
		.map(|index| entries()[index].1)
}

/// Longest emoji sequence at the start of `text`, in bytes.
pub fn prefix(text: &str) -> Option<usize> {
	let first = text.chars().next()?;
	if (first as u32) < 0xa9 {
		return None;
	}
	let mut ends: Vec<usize> = text
		.char_indices()
		.skip(1)
		.map(|(i, _)| i)
		.take(11)
		.collect();
	ends.push(text.char_indices().nth(12).map_or(text.len(), |(i, _)| i));
	ends.sort_unstable();
	ends.dedup();
	ends.into_iter()
		.rev()
		.find(|end| lookup(&text[..*end]).is_some())
}

/// Paint the emoji `text` into `rect`. Returns false if it isn't a known emoji.
pub fn paint(painter: &Painter, rect: Rect, text: &str) -> bool {
	let Some(cell) = lookup(text) else {
		return false;
	};
	let Some((texture, size)) = atlas(painter.ctx()) else {
		return false;
	};
	let x = (cell % COLUMNS) as f32 * CELL;
	let y = (cell / COLUMNS) as f32 * CELL;
	let uv = Rect::from_min_max(
		pos2(x / size.x, y / size.y),
		pos2((x + CELL) / size.x, (y + CELL) / size.y),
	);
	painter.image(texture, rect, uv, Color32::WHITE);
	true
}

/// Parse `<:name:id>` / `<a:name:id>` custom emoji markup; returns (id, name, byte length).
pub fn custom_prefix(text: &str) -> Option<(model::Id, &str, usize)> {
	let body = text
		.strip_prefix("<:")
		.or_else(|| text.strip_prefix("<a:"))?;
	let end = body.as_bytes().iter().take(54).position(|b| *b == b'>')?;
	let (name, id) = body[..end].split_once(':')?;
	if !(2..=32).contains(&name.len())
		|| !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
	{
		return None;
	}
	let id: model::Id = id.parse().ok()?;
	(id.0 != 0).then_some((id, name, text.len() - body.len() + end + 1))
}

/// A short palette of reactions offered by the quick-react bar.
pub const QUICK: [&str; 8] = ["❤️", "👍", "😂", "🔥", "✨", "🎉", "👀", "🙏"];

/// Emoji picker groups (a compact, curated set).
pub const PICKER: [(&str, &[&str]); 4] = [
	(
		"Smileys",
		&[
			"😀", "😃", "😄", "😁", "😆", "😅", "😂", "🙂", "😉", "😊", "😇", "🥰", "😍", "🤩",
			"😘", "😋", "😛", "😜", "🤪", "🤗", "🤔", "🤨", "😐", "😶", "🙄", "😏", "😌", "😴",
			"😎", "🥳", "😮", "😲", "🥺", "😢", "😭", "😤", "😡", "🤯", "😳", "🫠",
		],
	),
	(
		"Gestures",
		&[
			"👍", "👎", "👏", "🙌", "🙏", "🤝", "👋", "✌️", "🤞", "👌", "🤌", "💪", "🫶", "👀",
			"🧠", "🫡",
		],
	),
	(
		"Hearts",
		&[
			"❤️", "🧡", "💛", "💚", "💙", "💜", "🤎", "🖤", "🤍", "💖", "💕", "💔", "❣️", "💯",
			"✨", "⭐",
		],
	),
	(
		"Nature",
		&[
			"🌿", "🌱", "🌸", "🌻", "🌙", "☀️", "🌊", "🔥", "🌈", "⛰️", "🏔️", "🌅", "🍃", "🍂",
			"☕", "🎉",
		],
	),
];

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sequences_resolve_completely() {
		for text in ["👍", "👍🏽", "❤️", "👩🏽‍💻", "🇨🇿", "🏳️‍🌈"]
		{
			assert!(lookup(text).is_some(), "missing {text}");
		}
		assert!(lookup("hello").is_none());
		assert_eq!(prefix("👍🏽 nice"), Some("👍🏽".len()));
		assert_eq!(prefix("❤️!"), Some("❤️".len()));
		assert_eq!(prefix("abc"), None);
		for group in PICKER {
			for emoji in group.1 {
				assert!(lookup(emoji).is_some(), "picker {emoji}");
			}
		}
		for emoji in QUICK {
			assert!(lookup(emoji).is_some(), "quick {emoji}");
		}
	}

	#[test]
	fn custom_markup_is_bounded() {
		assert_eq!(
			custom_prefix("<:wave:9001> hi"),
			Some((model::Id(9001), "wave", "<:wave:9001>".len()))
		);
		for bad in ["<:x:1>", "<:hello:0>", "<:../x:1>", "<:hello:https://e.x>"] {
			assert!(custom_prefix(bad).is_none(), "{bad}");
		}
	}
}
