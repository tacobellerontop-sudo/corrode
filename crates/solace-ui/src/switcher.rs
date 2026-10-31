//! Ctrl+K quick switcher: jump to any conversation, channel or community by name.
use crate::{
	Solace,
	icons::{self, Icon},
	nav,
	paint::{self, AvatarShape},
	sidebar::{conversation_avatar, conversation_title},
	theme,
};
use client_core::{Command, State};
use egui::{Align2, Key, Rect, Sense, TextEdit, Ui, pos2, vec2};
use model::Id;

const ROWS: usize = 8;
const ROW: f32 = 46.0;

#[derive(Clone, Copy, PartialEq)]
enum Target {
	Channel(Id),
	Community(Id),
}

struct Entry {
	target: Target,
	title: String,
	place: String,
	score: u32,
}

/// Lower is better; `None` when the query's letters don't all appear in order.
fn score(query: &str, name: &str) -> Option<u32> {
	if query.is_empty() {
		return Some(0);
	}
	let name = name.to_lowercase();
	if name.starts_with(query) {
		return Some(0);
	}
	if let Some(at) = name.find(query) {
		return Some(10 + at as u32);
	}
	// Letters in order, with gaps (e.g. "gnl" finds "general").
	let mut chars = name.chars();
	let mut gaps = 0;
	for wanted in query.chars() {
		let mut skipped = 0;
		loop {
			match chars.next() {
				Some(c) if c == wanted => break,
				Some(_) => skipped += 1,
				None => return None,
			}
		}
		gaps += skipped;
	}
	Some(100 + gaps)
}

fn entries(state: &State, query: &str) -> Vec<Entry> {
	let query = query.trim().trim_start_matches('#').to_lowercase();
	let mut out = Vec::new();
	if query.is_empty() {
		for channel in nav::recent(state, ROWS) {
			out.push(Entry {
				target: Target::Channel(channel.id),
				title: title(state, channel),
				place: place(state, channel),
				score: 0,
			});
		}
		return out;
	}
	for channel in nav::direct(state) {
		let title = conversation_title(state, channel);
		if let Some(score) = score(&query, &title) {
			out.push(Entry {
				target: Target::Channel(channel.id),
				place: place(state, channel),
				title,
				score,
			});
		}
	}
	for channel in state
		.channels
		.iter()
		.filter(|c| c.guild.is_some() && nav::is_text(c.kind) && state.can_view(c.id))
	{
		if let Some(score) = score(&query, &channel.name) {
			out.push(Entry {
				target: Target::Channel(channel.id),
				title: title(state, channel),
				place: place(state, channel),
				score: score + 1,
			});
		}
	}
	for guild in nav::communities(state) {
		if let Some(score) = score(&query, &guild.name) {
			out.push(Entry {
				target: Target::Community(guild.id),
				title: guild.name.clone(),
				place: "Community".into(),
				score,
			});
		}
	}
	out.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.title.cmp(&b.title)));
	out.truncate(ROWS);
	out
}

fn title(state: &State, channel: &model::Channel) -> String {
	if channel.guild.is_some() {
		format!("#{}", channel.name)
	} else {
		conversation_title(state, channel)
	}
}

fn place(state: &State, channel: &model::Channel) -> String {
	match channel.guild.and_then(|g| state.guild(g)) {
		Some(guild) if nav::is_voice(channel.kind) => format!("Voice room · {}", guild.name),
		Some(guild) => guild.name.clone(),
		None if channel.kind == 3 => "Group".into(),
		None => "Direct message".into(),
	}
}

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let Some((mut query, mut index)) = solace.switcher.clone() else {
		return;
	};
	let ctx = ui.ctx().clone();
	let list = entries(state, &query);
	if ctx.input(|i| i.key_pressed(Key::ArrowDown)) && !list.is_empty() {
		index = (index + 1) % list.len();
	}
	if ctx.input(|i| i.key_pressed(Key::ArrowUp)) && !list.is_empty() {
		index = (index + list.len() - 1) % list.len();
	}
	index = index.min(list.len().saturating_sub(1));
	let height = 110.0 + list.len().max(1) as f32 * ROW;
	let mut chosen = None;
	let mut submit = false;
	let dismissed = crate::overlays::modal(&ctx, "switcher", vec2(560.0, height), |ui, rect, p| {
		let field = Rect::from_min_size(
			rect.left_top() + vec2(20.0, 20.0),
			vec2(rect.width() - 40.0, 48.0),
		);
		ui.painter().rect(
			field,
			14,
			p.canvas,
			egui::Stroke::new(1.0, p.accent.gamma_multiply(0.4)),
			egui::StrokeKind::Inside,
		);
		icons::paint_at(
			ui.painter(),
			Icon::Search,
			field.left_center() + vec2(22.0, 0.0),
			18.0,
			p.muted,
		);
		let edit = Rect::from_min_max(field.min + vec2(44.0, 0.0), field.max - vec2(14.0, 0.0));
		let id = egui::Id::unique("switcher-input");
		let before = query.clone();
		let response = ui.put(
			edit,
			TextEdit::singleline(&mut query)
				.id(id)
				.frame(egui::Frame::NONE)
				.font(theme::sans(16.0))
				.text_color(p.text)
				.align(Align2::LEFT_CENTER)
				.hint_text(
					egui::RichText::new("Where would you like to go?")
						.color(p.faint)
						.font(theme::sans(16.0)),
				)
				.desired_width(edit.width()),
		);
		if query != before {
			index = 0;
		}
		if !response.has_focus() {
			response.request_focus();
		}
		if ui.input(|i| i.key_pressed(Key::Enter)) {
			submit = true;
		}
		let mut y = field.bottom() + 14.0;
		paint::label(
			ui.painter(),
			pos2(rect.left() + 26.0, y + 6.0),
			Align2::LEFT_CENTER,
			if query.trim().is_empty() {
				"RECENT"
			} else {
				"RESULTS"
			},
			theme::medium(11.0),
			p.muted,
		);
		y += 18.0;
		if list.is_empty() {
			paint::label(
				ui.painter(),
				pos2(rect.left() + 26.0, y + ROW / 2.0),
				Align2::LEFT_CENTER,
				"Nothing matches",
				theme::sans(13.5),
				p.muted,
			);
		}
		for (i, entry) in list.iter().enumerate() {
			let row = Rect::from_min_size(
				pos2(rect.left() + 12.0, y),
				vec2(rect.width() - 24.0, ROW - 2.0),
			);
			let response = ui.interact(row, ui.scope_id().with(("switch", i)), Sense::click());
			if response.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
				index = i;
			}
			if i == index {
				ui.painter().rect_filled(row, 11, p.selected);
			}
			let icon = Rect::from_min_size(row.left_center() + vec2(10.0, -15.0), vec2(30.0, 30.0));
			match entry.target {
				Target::Channel(id) => match state.channel(id) {
					Some(channel) if channel.guild.is_none() => {
						conversation_avatar(solace, ui.painter(), icon, state, channel, p)
					}
					Some(channel) => icons::paint_at(
						ui.painter(),
						if nav::is_voice(channel.kind) {
							Icon::Speaker
						} else {
							Icon::Hash
						},
						icon.center(),
						16.0,
						p.text_soft,
					),
					None => {}
				},
				Target::Community(id) => {
					if let Some(guild) = state.guild(id) {
						let texture = solace.images.get_opt(guild.icon_key().as_deref());
						paint::avatar(
							ui.painter(),
							icon,
							AvatarShape::Rounded(9.0),
							texture,
							&guild.name,
							guild.id.0,
							p,
						);
					}
				}
			}
			let left = icon.right() + 12.0;
			let title = paint::text_left(
				ui.painter(),
				pos2(left, row.center().y),
				&entry.title,
				theme::medium(14.0),
				p.text,
				row.width() * 0.55,
			);
			paint::text_left(
				ui.painter(),
				pos2(title.right() + 10.0, row.center().y),
				&entry.place,
				theme::sans(12.0),
				p.muted,
				row.right() - title.right() - 60.0,
			);
			if i == index {
				paint::label(
					ui.painter(),
					row.right_center() - vec2(12.0, 0.0),
					Align2::RIGHT_CENTER,
					"Enter",
					theme::sans(11.0),
					p.faint,
				);
			}
			if response.clicked() {
				chosen = Some(entry.target);
			}
			y += ROW;
		}
	});
	if submit && chosen.is_none() {
		chosen = list.get(index).map(|e| e.target);
	}
	solace.switcher = Some((query, index));
	match chosen {
		Some(Target::Channel(channel)) => {
			commands.extend(state.select(channel));
			if let Some(c) = state.channel(channel) {
				solace.pane = if c.guild.is_some() {
					crate::Pane::Explore
				} else {
					crate::Pane::Direct
				};
			}
			solace.switcher = None;
		}
		Some(Target::Community(guild)) => {
			commands.extend(state.select_guild(guild));
			solace.pane = crate::Pane::Explore;
			solace.switcher = None;
		}
		None if dismissed => solace.switcher = None,
		None => {}
	}
}

#[cfg(test)]
mod tests {
	use super::score;

	#[test]
	fn prefix_beats_substring_beats_scattered_letters() {
		let prefix = score("gen", "general").unwrap();
		let inside = score("eral", "general").unwrap();
		let scattered = score("gnl", "general").unwrap();
		assert!(prefix < inside && inside < scattered);
		assert_eq!(score("xyz", "general"), None);
	}
}
