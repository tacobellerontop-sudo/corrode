//! Message text: a bounded Discord-markdown subset laid out with egui's text engine, plus
//! inline emoji (Twemoji atlas or custom emoji textures), mentions, links and spoilers.
use crate::{
	clock, emoji,
	images::{self, Images},
	theme::{self, Palette},
};
use client_core::State;
use egui::{
	Align, Color32, FontId, Galley, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2,
	text::{CCursor, LayoutJob, TextFormat, TextWrapping},
	vec2,
};
use model::Id;
use std::{ops::Range, sync::Arc};

/// Parsing is bounded so a hostile message can never stall a frame.
const MAX_SOURCE_BYTES: usize = 8192;
const MAX_INLINE_OBJECTS: usize = 128;
const PLACEHOLDER: char = '\u{2003}';

#[derive(Clone, Debug, PartialEq)]
pub enum EmojiKind {
	Unicode(String),
	Custom(Id),
}

/// Names for mentions, resolved against the open conversation.
pub struct Names<'a> {
	pub state: &'a State,
	pub guild: Option<Id>,
	pub mentions: &'a [model::User],
}

impl Names<'_> {
	fn user(&self, id: Id) -> String {
		if let Some(user) = self.mentions.iter().find(|u| u.id == id) {
			return user.name.clone();
		}
		if let Some(user) = self.state.user.as_ref().filter(|u| u.id == id) {
			return user.name.clone();
		}
		if let Some(list) = &self.state.members {
			for slot in list.slots.iter().flatten() {
				if let model::MemberSlot::Person(member) = slot
					&& member.user.id == id
				{
					return member
						.nick
						.clone()
						.unwrap_or_else(|| member.user.name.clone());
				}
			}
		}
		"unknown".into()
	}
	fn channel(&self, id: Id) -> String {
		self.state
			.channel(id)
			.map_or_else(|| "unknown".into(), |c| c.name.clone())
	}
	fn role(&self, id: Id) -> String {
		self.guild
			.and_then(|guild| self.state.permissions.guilds.get(&guild))
			.and_then(|guild| guild.roles.as_ref())
			.and_then(|roles| roles.iter().find(|r| r.id == id))
			.map_or_else(|| "role".into(), |r| r.name.clone())
	}
}

pub struct Style {
	pub size: f32,
	pub color: Color32,
	pub line_height: f32,
}

impl Style {
	pub fn body(p: &Palette) -> Self {
		Self {
			size: 14.0,
			color: p.text_soft,
			line_height: 22.0,
		}
	}
}

enum Raw {
	Text(String),
	Code(String),
	Quote(String),
}

fn split_blocks(source: &str) -> Vec<Raw> {
	let mut blocks = Vec::new();
	let mut rest = source;
	while !rest.is_empty() {
		if let Some(start) = rest.find("```") {
			let after = &rest[start + 3..];
			if let Some(end) = after.find("```") {
				push_lines(&mut blocks, &rest[..start]);
				let mut code = &after[..end];
				// A language tag on the opening fence is not part of the code.
				if let Some(newline) = code.find('\n')
					&& !code[..newline].contains(' ')
					&& code[..newline].len() <= 16
				{
					code = &code[newline + 1..];
				}
				blocks.push(Raw::Code(code.trim_end_matches('\n').to_owned()));
				rest = after[end + 3..].trim_start_matches('\n');
				continue;
			}
		}
		push_lines(&mut blocks, rest);
		break;
	}
	blocks
}

fn push_lines(blocks: &mut Vec<Raw>, text: &str) {
	let text = text.trim_matches('\n');
	if text.is_empty() {
		return;
	}
	for line in text.split('\n') {
		let quoted = line
			.strip_prefix("> ")
			.or_else(|| (line == ">").then_some(""));
		match (quoted, blocks.last_mut()) {
			(Some(q), Some(Raw::Quote(existing))) => {
				existing.push('\n');
				existing.push_str(q);
			}
			(Some(q), _) => blocks.push(Raw::Quote(q.to_owned())),
			(None, Some(Raw::Text(existing))) => {
				existing.push('\n');
				existing.push_str(line);
			}
			(None, _) => blocks.push(Raw::Text(line.to_owned())),
		}
	}
}

#[derive(Default, Clone, Copy, PartialEq)]
struct Marks {
	bold: bool,
	italic: bool,
	underline: bool,
	strike: bool,
	spoiler: bool,
}

struct Builder<'a> {
	job: LayoutJob,
	chars: usize,
	emojis: Vec<(usize, EmojiKind, f32)>,
	links: Vec<(Range<usize>, String)>,
	spoilers: Vec<Range<usize>>,
	p: &'a Palette,
	style: &'a Style,
	emoji_px: f32,
	placeholder_font: FontId,
	revealed: bool,
}

impl Builder<'_> {
	fn format(&self, marks: Marks) -> TextFormat {
		let font = if marks.bold {
			theme::semibold(self.style.size)
		} else {
			theme::sans(self.style.size)
		};
		let mut color = if marks.bold {
			self.p.text
		} else {
			self.style.color
		};
		let mut background = Color32::TRANSPARENT;
		if marks.spoiler && !self.revealed {
			background = self.p.faint;
			color = self.p.faint;
		}
		TextFormat {
			font_id: font,
			color,
			background,
			italics: marks.italic,
			underline: if marks.underline {
				Stroke::new(1.0, color)
			} else {
				Stroke::NONE
			},
			strikethrough: if marks.strike {
				Stroke::new(1.0, color)
			} else {
				Stroke::NONE
			},
			line_height: Some(self.style.line_height),
			valign: Align::Center,
			..Default::default()
		}
	}
	fn push(&mut self, text: &str, format: TextFormat) {
		if text.is_empty() {
			return;
		}
		self.chars += text.chars().count();
		self.job.append(text, 0.0, format);
	}
	fn text(&mut self, text: &str, marks: Marks) {
		let start = self.chars;
		let format = self.format(marks);
		self.push(text, format);
		if marks.spoiler {
			self.spoilers.push(start..self.chars);
		}
	}
	fn styled(&mut self, text: &str, format: TextFormat) {
		self.push(text, format);
	}
	fn emoji(&mut self, kind: EmojiKind, marks: Marks) {
		let format = TextFormat {
			font_id: self.placeholder_font.clone(),
			color: Color32::TRANSPARENT,
			line_height: Some(self.style.line_height.max(self.emoji_px + 4.0)),
			valign: Align::Center,
			..Default::default()
		};
		let at = self.chars;
		self.push(&PLACEHOLDER.to_string(), format);
		if marks.spoiler {
			self.spoilers.push(at..self.chars);
		}
		self.emojis.push((at, kind, self.emoji_px));
	}
}

/// True when a closing `marker` appears later on the same paragraph.
fn closes(rest: &str, marker: &str) -> bool {
	rest.find(marker).is_some_and(|i| i > 0)
}

fn inline(builder: &mut Builder, source: &str, names: &Names, objects: &mut usize) {
	let mut marks = Marks::default();
	let mut plain = String::new();
	let mut i = 0;
	macro_rules! flush {
		() => {
			if !plain.is_empty() {
				let text = std::mem::take(&mut plain);
				builder.text(&text, marks);
			}
		};
	}
	while i < source.len() {
		let rest = &source[i..];
		let previous_alnum = i > 0
			&& source[..i]
				.chars()
				.last()
				.is_some_and(char::is_alphanumeric);
		if rest.starts_with('\\') && rest.len() > 1 {
			let next = rest[1..].chars().next().unwrap();
			if next.is_ascii_punctuation() {
				plain.push(next);
				i += 1 + next.len_utf8();
				continue;
			}
		}
		// Toggled emphasis markers only open when they also close.
		let toggle = [("**", 0u8), ("__", 1), ("~~", 2), ("||", 3)]
			.into_iter()
			.find(|(marker, _)| rest.starts_with(marker));
		if let Some((marker, kind)) = toggle {
			let open = match kind {
				0 => marks.bold,
				1 => marks.underline,
				2 => marks.strike,
				_ => marks.spoiler,
			};
			if open || closes(&rest[2..], marker) {
				flush!();
				match kind {
					0 => marks.bold = !marks.bold,
					1 => marks.underline = !marks.underline,
					2 => marks.strike = !marks.strike,
					_ => marks.spoiler = !marks.spoiler,
				}
				i += 2;
				continue;
			}
		}
		let italic_marker = rest.starts_with('*') || (rest.starts_with('_') && !previous_alnum);
		if italic_marker
			&& (marks.italic || (closes(&rest[1..], &rest[..1]) && !rest[1..].starts_with(' ')))
		{
			flush!();
			marks.italic = !marks.italic;
			i += 1;
			continue;
		}
		if rest.starts_with('`')
			&& let Some(end) = rest[1..].find('`')
			&& end > 0
		{
			flush!();
			let code = &rest[1..1 + end];
			builder.styled(
				code,
				TextFormat {
					font_id: theme::mono(builder.style.size - 1.0),
					color: builder.p.text,
					background: builder.p.surface_alt,
					line_height: Some(builder.style.line_height),
					valign: Align::Center,
					..Default::default()
				},
			);
			i += end + 2;
			continue;
		}
		if *objects < MAX_INLINE_OBJECTS && rest.starts_with('<') {
			let mention = if let Some((id, len)) = model::role_mention_prefix(rest) {
				Some((format!("@{}", names.role(id)), len))
			} else if let Some((id, len)) = model::user_mention_prefix(rest) {
				Some((format!("@{}", names.user(id)), len))
			} else if let Some((id, len)) = model::channel_mention_prefix(rest) {
				Some((format!("#{}", names.channel(id)), len))
			} else {
				None
			};
			if let Some((label, len)) = mention {
				flush!();
				*objects += 1;
				let mut format = builder.format(marks);
				format.font_id = theme::medium(builder.style.size);
				format.color = builder.p.accent;
				format.background = builder.p.accent_soft;
				builder.styled(&label, format);
				i += len;
				continue;
			}
			if let Some((id, _, len)) = emoji::custom_prefix(rest) {
				flush!();
				*objects += 1;
				builder.emoji(EmojiKind::Custom(id), marks);
				i += len;
				continue;
			}
			if let Some((seconds, style, len)) = model::timestamp_prefix(rest)
				&& let Some(label) = clock::discord_timestamp(seconds, style)
			{
				flush!();
				*objects += 1;
				let mut format = builder.format(marks);
				format.background = builder.p.surface_alt;
				builder.styled(&label, format);
				i += len;
				continue;
			}
		}
		if rest.starts_with('@')
			&& !previous_alnum
			&& let Some(len) = model::mass_mention_prefix(rest)
		{
			flush!();
			let mut format = builder.format(marks);
			format.font_id = theme::medium(builder.style.size);
			format.color = builder.p.accent;
			format.background = builder.p.accent_soft;
			builder.styled(&rest[..len], format);
			i += len;
			continue;
		}
		if *objects < MAX_INLINE_OBJECTS
			&& (rest.starts_with("https://") || rest.starts_with("http://"))
		{
			let mut end = rest
				.find(|c: char| c.is_whitespace() || c == '<' || c == '>')
				.unwrap_or(rest.len());
			while end > 0 && rest[..end].ends_with(['.', ',', ')', '!', '?', ':', ';', '"', '\'']) {
				end -= 1;
			}
			let url = &rest[..end];
			if url.len() > "https://".len() {
				flush!();
				*objects += 1;
				let start = builder.chars;
				let mut format = builder.format(marks);
				format.color = builder.p.accent;
				builder.styled(url, format);
				builder.links.push((start..builder.chars, url.to_owned()));
				i += end;
				continue;
			}
		}
		if *objects < MAX_INLINE_OBJECTS
			&& !rest.is_ascii()
			&& let Some(len) = emoji::prefix(rest)
		{
			flush!();
			*objects += 1;
			builder.emoji(EmojiKind::Unicode(rest[..len].to_owned()), marks);
			i += len;
			continue;
		}
		let c = rest.chars().next().unwrap();
		plain.push(if c == PLACEHOLDER { ' ' } else { c });
		i += c.len_utf8();
	}
	flush!();
}

/// Messages that are only emoji (up to 27) render large, like the reference client.
fn jumbo(source: &str) -> bool {
	let mut rest = source.trim();
	let mut count = 0;
	while !rest.is_empty() {
		rest = rest.trim_start();
		if let Some((_, _, len)) = emoji::custom_prefix(rest) {
			rest = &rest[len..];
		} else if let Some(len) = emoji::prefix(rest) {
			rest = &rest[len..];
		} else {
			return false;
		}
		count += 1;
		if count > 27 {
			return false;
		}
	}
	count > 0
}

pub struct Laid {
	pub blocks: Vec<LaidBlock>,
	pub size: Vec2,
}

pub enum LaidBlock {
	Text {
		offset: Vec2,
		galley: Arc<Galley>,
		emojis: Vec<(Rect, EmojiKind)>,
		links: Vec<(Vec<Rect>, String)>,
		spoilers: Vec<Rect>,
	},
	Code {
		rect: Rect,
		galley: Arc<Galley>,
	},
	QuoteBar {
		rect: Rect,
	},
}

fn char_rects(galley: &Galley, range: Range<usize>) -> Vec<Rect> {
	let mut rects: Vec<Rect> = Vec::new();
	for index in range {
		let a = galley.pos_from_cursor(CCursor::new(index));
		let b = galley.pos_from_cursor(CCursor::new(index + 1));
		let right = if (b.min.y - a.min.y).abs() < 1.0 {
			b.min.x
		} else {
			a.min.x + 4.0
		};
		let rect = Rect::from_min_max(a.min, egui::pos2(right, a.max.y));
		match rects.last_mut() {
			Some(last) if (last.min.y - rect.min.y).abs() < 1.0 => *last = last.union(rect),
			_ => rects.push(rect),
		}
	}
	rects
}

fn finish(painter: &Painter, builder: Builder, wrap: f32, offset: Vec2) -> LaidBlock {
	let mut job = builder.job;
	job.wrap = TextWrapping {
		max_width: wrap,
		..Default::default()
	};
	let galley = painter.layout_job(job);
	let emojis = builder
		.emojis
		.into_iter()
		.map(|(index, kind, px)| {
			let rect = char_rects(&galley, index..index + 1)
				.into_iter()
				.next()
				.unwrap_or(Rect::NOTHING);
			(Rect::from_center_size(rect.center(), Vec2::splat(px)), kind)
		})
		.collect();
	let links = builder
		.links
		.into_iter()
		.map(|(range, url)| (char_rects(&galley, range), url))
		.collect();
	let spoilers = builder
		.spoilers
		.into_iter()
		.flat_map(|range| char_rects(&galley, range))
		.collect();
	LaidBlock::Text {
		offset,
		galley,
		emojis,
		links,
		spoilers,
	}
}

/// Lay out `source` within `width`. Cheap to call every frame: galleys are cached by egui.
pub fn layout(
	painter: &Painter,
	source: &str,
	names: &Names,
	style: &Style,
	width: f32,
	revealed: bool,
) -> Laid {
	let p = theme::palette(painter.ctx());
	let source = if source.len() > MAX_SOURCE_BYTES {
		let mut end = MAX_SOURCE_BYTES;
		while !source.is_char_boundary(end) {
			end -= 1;
		}
		&source[..end]
	} else {
		source
	};
	let emoji_px = if jumbo(source) {
		40.0
	} else {
		(style.size * 1.35).round()
	};
	// Size an em space so its advance is the emoji's width plus a hair of breathing room.
	let probe = theme::mono(10.0);
	let unit = painter
		.fonts_mut(|f| f.glyph_width(&probe, PLACEHOLDER))
		.max(0.1);
	let placeholder_font = theme::mono(10.0 * (emoji_px + 2.0) / unit);
	let width = width.max(40.0);
	let mut objects = 0;
	let mut blocks = Vec::new();
	let mut y = 0.0;
	let mut widest: f32 = 0.0;
	let gap = 6.0;
	for raw in split_blocks(source) {
		if !blocks.is_empty() {
			y += gap;
		}
		match raw {
			Raw::Text(text) | Raw::Quote(text) if text.is_empty() => {}
			Raw::Text(text) => {
				let mut builder = new_builder(&p, style, emoji_px, &placeholder_font, revealed);
				inline(&mut builder, &text, names, &mut objects);
				let block = finish(painter, builder, width, vec2(0.0, y));
				if let LaidBlock::Text { galley, .. } = &block {
					y += galley.size().y;
					widest = widest.max(galley.size().x);
				}
				blocks.push(block);
			}
			Raw::Quote(text) => {
				let mut builder = new_builder(&p, style, emoji_px, &placeholder_font, revealed);
				inline(&mut builder, &text, names, &mut objects);
				let block = finish(painter, builder, width - 16.0, vec2(16.0, y));
				if let LaidBlock::Text { galley, .. } = &block {
					blocks.push(LaidBlock::QuoteBar {
						rect: Rect::from_min_size(egui::pos2(0.0, y), vec2(3.0, galley.size().y)),
					});
					y += galley.size().y;
					widest = widest.max(galley.size().x + 16.0);
				}
				blocks.push(block);
			}
			Raw::Code(code) => {
				let mut job = LayoutJob::simple(code, theme::mono(13.0), p.text, width - 28.0);
				job.wrap.max_rows = 40;
				let galley = painter.layout_job(job);
				let rect = Rect::from_min_size(
					egui::pos2(0.0, y),
					vec2(
						width.min(galley.size().x + 28.0).max(120.0),
						galley.size().y + 20.0,
					),
				);
				y += rect.height();
				widest = widest.max(rect.width());
				blocks.push(LaidBlock::Code { rect, galley });
			}
		}
	}
	Laid {
		blocks,
		size: vec2(widest, y),
	}
}

fn new_builder<'a>(
	p: &'a Palette,
	style: &'a Style,
	emoji_px: f32,
	placeholder_font: &FontId,
	revealed: bool,
) -> Builder<'a> {
	Builder {
		job: LayoutJob::default(),
		chars: 0,
		emojis: Vec::new(),
		links: Vec::new(),
		spoilers: Vec::new(),
		p,
		style,
		emoji_px,
		placeholder_font: placeholder_font.clone(),
		revealed,
	}
}

/// What the reader did with the text this frame.
#[derive(Default)]
pub struct Interaction {
	pub link: Option<String>,
	pub spoiler_clicked: bool,
}

/// Paint laid-out text at `origin`, handling link and spoiler clicks.
pub fn paint(
	ui: &mut Ui,
	origin: Pos2,
	laid: &Laid,
	images: &mut Images,
	id: egui::Id,
) -> Interaction {
	let p = theme::palette(ui.ctx());
	let mut interaction = Interaction::default();
	for (index, block) in laid.blocks.iter().enumerate() {
		match block {
			LaidBlock::Text {
				offset,
				galley,
				emojis,
				links,
				spoilers,
			} => {
				let at = origin + *offset;
				ui.painter().galley(at, galley.clone(), p.text_soft);
				for (rect, kind) in emojis {
					let rect = rect.translate(at.to_vec2());
					match kind {
						EmojiKind::Unicode(text) => {
							emoji::paint(ui.painter(), rect, text);
						}
						EmojiKind::Custom(emoji_id) => {
							if let Some((texture, _)) = images.get(&images::emoji_key(*emoji_id)) {
								ui.painter().image(
									texture,
									rect,
									Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
									Color32::WHITE,
								);
							} else {
								ui.painter().rect_filled(rect.shrink(2.0), 4, p.surface_alt);
							}
						}
					}
				}
				for (link_index, (rects, url)) in links.iter().enumerate() {
					for (part, rect) in rects.iter().enumerate() {
						let rect = rect.translate(at.to_vec2());
						let response =
							ui.interact(rect, id.with((index, link_index, part)), Sense::click());
						if response.hovered() {
							ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
							ui.painter().line_segment(
								[
									rect.left_bottom() - vec2(0.0, 3.0),
									rect.right_bottom() - vec2(0.0, 3.0),
								],
								Stroke::new(1.0, p.accent),
							);
						}
						if response.clicked() {
							interaction.link = Some(url.clone());
						}
						response.on_hover_text(url.as_str());
					}
				}
				for (part, rect) in spoilers.iter().enumerate() {
					let rect = rect.translate(at.to_vec2());
					let response =
						ui.interact(rect, id.with(("spoiler", index, part)), Sense::click());
					if response.clicked() {
						interaction.spoiler_clicked = true;
					}
				}
			}
			LaidBlock::Code { rect, galley } => {
				let rect = rect.translate(origin.to_vec2());
				ui.painter().rect(
					rect,
					10,
					p.surface_alt,
					Stroke::new(1.0, p.hairline),
					egui::StrokeKind::Inside,
				);
				ui.painter()
					.galley(rect.min + vec2(14.0, 10.0), galley.clone(), p.text);
			}
			LaidBlock::QuoteBar { rect } => {
				ui.painter()
					.rect_filled(rect.translate(origin.to_vec2()), 2, p.faint);
			}
		}
	}
	interaction
}

/// Plain-text preview of message content for list subtitles (no markup).
pub fn preview(source: &str, names: &Names, max_chars: usize) -> String {
	let mut out = String::new();
	let mut rest = source;
	while let Some(c) = rest.chars().next() {
		if out.chars().count() >= max_chars {
			out.push('…');
			break;
		}
		if rest.starts_with('<') {
			if let Some((id, len)) = model::user_mention_prefix(rest) {
				out.push('@');
				out.push_str(&names.user(id));
				rest = &rest[len..];
				continue;
			}
			if let Some((id, len)) = model::channel_mention_prefix(rest) {
				out.push('#');
				out.push_str(&names.channel(id));
				rest = &rest[len..];
				continue;
			}
			if let Some((_, name, len)) = emoji::custom_prefix(rest) {
				out.push(':');
				out.push_str(name);
				out.push(':');
				rest = &rest[len..];
				continue;
			}
		}
		if matches!(c, '*' | '_' | '~' | '|' | '`') {
			rest = &rest[1..];
			continue;
		}
		out.push(if c == '\n' { ' ' } else { c });
		rest = &rest[c.len_utf8()..];
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	fn names(state: &State) -> Names<'_> {
		Names {
			state,
			guild: None,
			mentions: &[],
		}
	}

	#[test]
	fn blocks_split_code_and_quotes() {
		let blocks = split_blocks("hello\n> quoted\n> more\n```rust\nfn x() {}\n```\nafter");
		assert!(matches!(&blocks[0], Raw::Text(t) if t == "hello"));
		assert!(matches!(&blocks[1], Raw::Quote(t) if t == "quoted\nmore"));
		assert!(matches!(&blocks[2], Raw::Code(t) if t == "fn x() {}"));
		assert!(matches!(&blocks[3], Raw::Text(t) if t == "after"));
	}

	#[test]
	fn jumbo_only_for_pure_emoji() {
		assert!(jumbo("🔥 🔥"));
		assert!(jumbo("<:wave:9001>"));
		assert!(!jumbo("nice 🔥"));
		assert!(!jumbo(""));
	}

	#[test]
	fn inline_markup_strips_markers_and_tracks_links_and_emoji() {
		let state = State::default();
		let ctx = egui::Context::default();
		let mut output = ctx.run_ui(Default::default(), |ui| {
			let p = theme::LIGHT;
			let style = Style::body(&p);
			let laid = layout(
				ui.painter(),
				"**Bold** and *soft* see https://example.com/a. 🔥 `code` **unclosed",
				&names(&state),
				&style,
				600.0,
				false,
			);
			let LaidBlock::Text {
				galley,
				emojis,
				links,
				..
			} = &laid.blocks[0]
			else {
				panic!("text block");
			};
			assert!(
				galley
					.job
					.text
					.starts_with("Bold and soft see https://example.com/a. ")
			);
			assert!(galley.job.text.ends_with("**unclosed"));
			assert_eq!(links.len(), 1);
			assert_eq!(links[0].1, "https://example.com/a");
			assert_eq!(emojis.len(), 1);
			assert!(emojis[0].0.width() > 10.0);
		});
		output.textures_delta.clear();
	}

	#[test]
	fn preview_flattens_markup() {
		let state = State::default();
		assert_eq!(
			preview("**hi**\nthere <:wave:9001>", &names(&state), 40),
			"hi there :wave:"
		);
	}
}
