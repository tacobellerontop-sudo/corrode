//! Home dashboard, shown when no conversation is open. Direct messages lead: a greeting,
//! message-first stats, your recent conversations and who's online, then live voice rooms and
//! a compact row of communities.
use crate::{
	Pane, Solace, clock,
	icons::{self, Icon},
	nav,
	paint::{self, AvatarShape, ButtonKind},
	scenery::Mood,
	sidebar::{conversation_avatar, conversation_title},
	theme::{self, Palette, mix},
};
use client_core::{Command, State};
use egui::{Align2, Color32, Rect, Sense, Ui, pos2, vec2};
use model::{Channel, Id};

const PAD: f32 = 32.0;
const GAP: f32 = 18.0;
const HERO: f32 = 196.0;
const TILE: f32 = 98.0;
const ROW: f32 = 58.0;
const CARD_HEAD: f32 = 56.0;
const COMMUNITY_TILE: f32 = 132.0;
const MESSAGES: usize = 8;

fn greeting() -> &'static str {
	match clock::now().hour() {
		5..=11 => "Good morning",
		12..=17 => "Good afternoon",
		_ => "Good evening",
	}
}

struct Snapshot {
	recent: Vec<Channel>,
	rooms: Vec<(Channel, Vec<Id>)>,
	online: Vec<(model::User, String)>,
	unread: usize,
	mentions: u32,
}

fn snapshot(state: &State) -> Snapshot {
	let unread = state
		.channels
		.iter()
		.filter(|c| nav::is_dm(c) && nav::channel_unread(state, c))
		.count();
	let mentions = state
		.channels
		.iter()
		.filter(|c| nav::is_dm(c))
		.map(|c| state.mention_count(c.id))
		.fold(0u32, u32::saturating_add);
	Snapshot {
		recent: nav::direct(state)
			.into_iter()
			.take(MESSAGES)
			.cloned()
			.collect(),
		rooms: nav::live_rooms(state)
			.into_iter()
			.take(4)
			.map(|(c, people)| (c.clone(), people))
			.collect(),
		online: nav::online_people(state).into_iter().take(8).collect(),
		unread,
		mentions,
	}
}

fn card(ui: &Ui, rect: Rect, title: &str, action: Option<&str>, p: &Palette) {
	paint::card(ui.painter(), rect, 18, p);
	paint::label(
		ui.painter(),
		rect.left_top() + vec2(20.0, 28.0),
		Align2::LEFT_CENTER,
		title,
		theme::serif(18.0),
		p.text,
	);
	if let Some(action) = action {
		paint::label(
			ui.painter(),
			rect.right_top() + vec2(-20.0, 28.0),
			Align2::RIGHT_CENTER,
			action,
			theme::sans(12.5),
			p.muted,
		);
	}
}

fn empty_line(ui: &Ui, rect: Rect, text: &str, p: &Palette) {
	paint::label(
		ui.painter(),
		rect.left_top() + vec2(20.0, CARD_HEAD + 18.0),
		Align2::LEFT_CENTER,
		text,
		theme::sans(13.0),
		p.faint,
	);
}

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let p = theme::palette(ui.ctx());
	let area = ui.max_rect();
	let data = snapshot(state);
	let width = (area.width() - 2.0 * PAD).max(320.0);
	let two_columns = width >= 820.0;
	let left_w = if two_columns {
		(width - GAP) * 0.6
	} else {
		width
	};
	let right_w = if two_columns {
		width - GAP - left_w
	} else {
		width
	};

	let guilds: Vec<model::Guild> = nav::communities(state).cloned().collect();
	let per_row = ((width - 40.0 + 12.0) / (150.0 + 12.0)).floor().max(1.0) as usize;
	let community_rows = guilds.len().div_ceil(per_row).max(1);
	let messages_h = CARD_HEAD + data.recent.len().max(1) as f32 * ROW + 14.0;
	let online_h = CARD_HEAD + data.online.len().max(1) as f32 * ROW + 14.0;
	let rooms_h = CARD_HEAD + data.rooms.len().max(1) as f32 * 76.0 + 10.0;
	let communities_h = CARD_HEAD + community_rows as f32 * (COMMUNITY_TILE + 12.0) + 8.0;
	let right_total = online_h + GAP + rooms_h;
	let columns_h = if two_columns {
		messages_h.max(right_total)
	} else {
		messages_h + GAP + right_total
	};
	let total = PAD + HERO + GAP + TILE + GAP + columns_h + GAP + communities_h + PAD;

	let mut child = ui.new_child(egui::UiBuilder::new().max_rect(area).id_salt("dashboard"));
	child.set_clip_rect(area);
	egui::ScrollArea::vertical()
		.id_salt("dashboard-scroll")
		.auto_shrink([false, false])
		.show(&mut child, |ui| {
			let (outer, _) = ui.allocate_exact_size(vec2(area.width(), total), Sense::hover());
			let left = outer.left() + PAD;
			let mut y = outer.top() + PAD;

			hero(
				solace,
				ui,
				state,
				&data,
				Rect::from_min_size(pos2(left, y), vec2(width, HERO)),
			);
			y += HERO + GAP;

			stats(
				solace,
				ui,
				&data,
				Rect::from_min_size(pos2(left, y), vec2(width, TILE)),
				&p,
			);
			y += TILE + GAP;

			// Messages first, people beside them, servers after.
			let messages = Rect::from_min_size(pos2(left, y), vec2(left_w, messages_h));
			messages_card(solace, ui, state, commands, &data.recent, messages, &p);

			let (right_x, right_y) = if two_columns {
				(left + left_w + GAP, y)
			} else {
				(left, messages.bottom() + GAP)
			};
			let online = Rect::from_min_size(pos2(right_x, right_y), vec2(right_w, online_h));
			online_card(solace, ui, state, commands, &data.online, online, &p);
			let rooms =
				Rect::from_min_size(pos2(right_x, online.bottom() + GAP), vec2(right_w, rooms_h));
			rooms_card(solace, ui, state, commands, &data.rooms, rooms, &p);

			let communities =
				Rect::from_min_size(pos2(left, y + columns_h + GAP), vec2(width, communities_h));
			communities_card(
				solace,
				ui,
				state,
				commands,
				&guilds,
				per_row,
				communities,
				&p,
			);
		});
}

fn hero(solace: &mut Solace, ui: &Ui, state: &State, data: &Snapshot, rect: Rect) {
	let texture = solace.scenery.texture(ui.ctx(), 7, Mood::Dawn).id();
	paint::cover_image(
		ui.painter(),
		rect,
		theme::size::RADIUS_CARD,
		texture,
		vec2(1280.0, 380.0),
	);
	paint::gradient(
		ui.painter(),
		rect,
		theme::size::RADIUS_CARD as f32,
		&[
			(0.0, Color32::from_black_alpha(20)),
			(1.0, Color32::from_black_alpha(130)),
		],
	);
	let name = state.user.as_ref().map_or("there".to_owned(), |u| {
		state.user_display_name(u).to_owned()
	});
	let now = clock::now();
	paint::label(
		ui.painter(),
		rect.left_top() + vec2(30.0, 40.0),
		Align2::LEFT_CENTER,
		&format!("{}, {} {}", now.weekday(), now.month(), now.day()),
		theme::medium(13.0),
		Color32::from_white_alpha(220),
	);
	paint::label(
		ui.painter(),
		rect.left_bottom() + vec2(30.0, -76.0),
		Align2::LEFT_CENTER,
		&format!("{}, {name}", greeting()),
		theme::serif(34.0),
		Color32::WHITE,
	);
	let in_voice: usize = data.rooms.iter().map(|(_, people)| people.len()).sum();
	let mut parts = Vec::new();
	parts.push(match data.unread {
		0 => "No unread messages".to_owned(),
		1 => "1 conversation is waiting for you".to_owned(),
		n => format!("{n} conversations are waiting for you"),
	});
	if !data.online.is_empty() {
		parts.push(format!("{} friends online", data.online.len()));
	}
	if in_voice > 0 {
		parts.push(format!(
			"{in_voice} {} hanging out in voice",
			if in_voice == 1 {
				"person is"
			} else {
				"people are"
			}
		));
	}
	paint::label(
		ui.painter(),
		rect.left_bottom() + vec2(30.0, -40.0),
		Align2::LEFT_CENTER,
		&parts.join("  ·  "),
		theme::sans(14.5),
		Color32::from_white_alpha(235),
	);
}

fn stats(solace: &mut Solace, ui: &mut Ui, data: &Snapshot, rect: Rect, p: &Palette) {
	let tiles = [
		(
			Icon::Bell,
			data.unread.to_string(),
			"Unread messages",
			Pane::Direct,
		),
		(
			Icon::At,
			data.mentions.to_string(),
			"Mentions",
			Pane::Activity,
		),
		(
			Icon::People,
			data.online.len().to_string(),
			"Friends online",
			Pane::Direct,
		),
		(
			Icon::Thread,
			data.recent.len().to_string(),
			"Recent conversations",
			Pane::Direct,
		),
	];
	let w = (rect.width() - 3.0 * 14.0) / 4.0;
	for (i, (icon, value, label, pane)) in tiles.into_iter().enumerate() {
		let tile = Rect::from_min_size(
			rect.min + vec2(i as f32 * (w + 14.0), 0.0),
			vec2(w, rect.height()),
		);
		let response = ui.interact(tile, ui.scope_id().with(("stat", i)), Sense::click());
		let t = paint::hover_t(ui, &response);
		paint::card(ui.painter(), tile, 16, p);
		if t > 0.0 {
			ui.painter()
				.rect_filled(tile.shrink(1.0), 15, p.hover.gamma_multiply(t * 0.5));
		}
		let chip = Rect::from_center_size(tile.left_center() + vec2(36.0, 0.0), vec2(42.0, 42.0));
		ui.painter()
			.circle_filled(chip.center(), 21.0, p.accent_soft);
		icons::paint_at(ui.painter(), icon, chip.center(), 19.0, p.accent);
		paint::label(
			ui.painter(),
			pos2(chip.right() + 14.0, tile.center().y - 10.0),
			Align2::LEFT_CENTER,
			&value,
			theme::serif_strong(27.0),
			p.text,
		);
		paint::text_left(
			ui.painter(),
			pos2(chip.right() + 14.0, tile.center().y + 16.0),
			label,
			theme::sans(12.5),
			p.muted,
			tile.right() - chip.right() - 24.0,
		);
		if response.clicked() {
			solace.pane = pane;
		}
	}
}

fn messages_card(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	recent: &[Channel],
	rect: Rect,
	p: &Palette,
) {
	card(ui, rect, "Messages", Some("Direct and group"), p);
	if recent.is_empty() {
		empty_line(
			ui,
			rect,
			"Message a friend and the conversation appears here",
			p,
		);
		return;
	}
	for (i, channel) in recent.iter().enumerate() {
		let row = Rect::from_min_size(
			rect.left_top() + vec2(10.0, CARD_HEAD + i as f32 * ROW),
			vec2(rect.width() - 20.0, ROW - 4.0),
		);
		let response = ui.interact(
			row,
			ui.scope_id().with(("message", channel.id)),
			Sense::click(),
		);
		let t = paint::hover_t(ui, &response);
		if t > 0.0 {
			ui.painter().rect_filled(row, 12, p.hover.gamma_multiply(t));
		}
		let icon = Rect::from_min_size(row.left_center() + vec2(10.0, -19.0), vec2(38.0, 38.0));
		conversation_avatar(solace, ui.painter(), icon, state, channel, p);
		let peer = channel.recipients.first().filter(|_| channel.kind == 1);
		let status = peer.and_then(|u| nav::status_of(state, u.id));
		if let Some(color) = paint::status_color(status, p) {
			paint::presence(
				ui.painter(),
				icon.right_bottom() - vec2(4.0, 4.0),
				4.5,
				color,
				mix(p.surface, p.hover, t),
			);
		}
		let place = match peer {
			_ if channel.kind == 3 => format!("Group · {} people", channel.recipients.len() + 1),
			Some(user) => nav::custom_status_of(state, user.id)
				.map_or_else(|| paint::status_label(status).to_owned(), str::to_owned),
			None => "Direct message".to_owned(),
		};
		let unread = nav::channel_unread(state, channel);
		let text_left = icon.right() + 14.0;
		let max = row.right() - text_left - 70.0;
		paint::text_left(
			ui.painter(),
			pos2(text_left, row.center().y - 9.0),
			&conversation_title(state, channel),
			if unread {
				theme::semibold(14.0)
			} else {
				theme::medium(14.0)
			},
			p.text,
			max,
		);
		paint::text_left(
			ui.painter(),
			pos2(text_left, row.center().y + 10.0),
			&place,
			theme::sans(12.0),
			p.muted,
			max,
		);
		if let Some(last) = channel.last_message {
			paint::label(
				ui.painter(),
				pos2(row.right() - 14.0, row.center().y - 8.0),
				Align2::RIGHT_CENTER,
				&clock::short_age(clock::snowflake(last)),
				theme::sans(11.0),
				p.faint,
			);
		}
		let mentions = state.mention_count(channel.id);
		if mentions > 0 {
			let badge = Rect::from_center_size(
				pos2(row.right() - 22.0, row.center().y + 10.0),
				vec2(if mentions > 9 { 26.0 } else { 18.0 }, 16.0),
			);
			ui.painter().rect_filled(badge, 8, p.dnd);
			paint::label(
				ui.painter(),
				badge.center(),
				Align2::CENTER_CENTER,
				&mentions.min(99).to_string(),
				theme::semibold(10.5),
				Color32::WHITE,
			);
		} else if unread {
			ui.painter().circle_filled(
				pos2(row.right() - 18.0, row.center().y + 10.0),
				4.0,
				p.accent,
			);
		}
		if response.clicked() {
			solace.pane = Pane::Direct;
			commands.extend(state.select(channel.id));
		}
	}
}

#[allow(clippy::too_many_arguments)]
fn communities_card(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	guilds: &[model::Guild],
	per_row: usize,
	rect: Rect,
	p: &Palette,
) {
	card(
		ui,
		rect,
		"Your communities",
		Some(&format!("{} joined", guilds.len())),
		p,
	);
	if guilds.is_empty() {
		empty_line(ui, rect, "Join a community with the + in the sidebar", p);
		return;
	}
	let inner = rect.width() - 40.0;
	let tile_w = (inner - (per_row as f32 - 1.0) * 12.0) / per_row as f32;
	for (i, guild) in guilds.iter().enumerate() {
		let (col, row) = (i % per_row, i / per_row);
		let tile = Rect::from_min_size(
			rect.left_top()
				+ vec2(
					20.0 + col as f32 * (tile_w + 12.0),
					CARD_HEAD + row as f32 * (COMMUNITY_TILE + 12.0),
				),
			vec2(tile_w, COMMUNITY_TILE),
		);
		let response = ui.interact(
			tile,
			ui.scope_id().with(("community", guild.id)),
			Sense::click(),
		);
		let t = paint::hover_t(ui, &response);
		ui.painter().rect(
			tile,
			14,
			mix(p.surface_alt, p.hover, t),
			egui::Stroke::new(1.0, p.hairline),
			egui::StrokeKind::Inside,
		);
		let banner = Rect::from_min_size(tile.min, vec2(tile.width(), 58.0));
		let texture = solace
			.scenery
			.texture(
				ui.ctx(),
				guild.id.0,
				if guild.id.0 % 2 == 0 {
					Mood::Dawn
				} else {
					Mood::Lake
				},
			)
			.id();
		paint::cover_image(
			ui.painter(),
			banner,
			egui::CornerRadius {
				nw: 14,
				ne: 14,
				sw: 0,
				se: 0,
			},
			texture,
			vec2(1280.0, 380.0),
		);
		let icon = Rect::from_min_size(banner.left_bottom() + vec2(12.0, -18.0), vec2(36.0, 36.0));
		ui.painter().rect_filled(icon.expand(2.5), 12, p.surface);
		let picture = solace.images.get_opt(guild.icon_key().as_deref());
		paint::avatar(
			ui.painter(),
			icon,
			AvatarShape::Rounded(10.0),
			picture,
			&guild.name,
			guild.id.0,
			p,
		);
		paint::text_left(
			ui.painter(),
			pos2(tile.left() + 12.0, tile.bottom() - 36.0),
			&guild.name,
			theme::serif(15.0),
			p.text,
			tile.width() - 24.0,
		);
		let sections = nav::sections(state, guild.id);
		let text = sections
			.iter()
			.flat_map(|s| &s.channels)
			.filter(|c| nav::is_text(c.kind))
			.count();
		let live = state
			.voice
			.roster
			.iter()
			.filter(|e| e.guild == guild.id)
			.count();
		let subtitle = if live > 0 {
			format!("{live} in voice")
		} else {
			format!("{text} {}", if text == 1 { "channel" } else { "channels" })
		};
		paint::text_left(
			ui.painter(),
			pos2(tile.left() + 12.0, tile.bottom() - 16.0),
			&subtitle,
			theme::sans(11.5),
			if live > 0 { p.online } else { p.muted },
			tile.width() - 30.0,
		);
		if nav::guild_unread(state, guild.id) {
			ui.painter().circle_filled(
				pos2(tile.right() - 14.0, tile.bottom() - 16.0),
				4.0,
				p.accent,
			);
		}
		if response.clicked() {
			solace.pane = Pane::Explore;
			if let Some(command) = state.select_guild(guild.id) {
				commands.push(command);
			} else if let Some(channel) = nav::landing(state, guild.id)
				&& let Some(command) = state.select(channel)
			{
				commands.push(command);
			}
		}
	}
}

fn rooms_card(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	rooms: &[(Channel, Vec<Id>)],
	rect: Rect,
	p: &Palette,
) {
	card(ui, rect, "Live in voice", None, p);
	if rooms.is_empty() {
		empty_line(ui, rect, "Nobody's in a voice room right now", p);
		return;
	}
	let active = state.voice.active.as_ref().map(|c| c.channel);
	for (i, (room, people)) in rooms.iter().enumerate() {
		let row = Rect::from_min_size(
			rect.left_top() + vec2(10.0, CARD_HEAD + i as f32 * 76.0),
			vec2(rect.width() - 20.0, 70.0),
		);
		ui.painter().rect_filled(row, 12, p.surface_alt);
		icons::paint_at(
			ui.painter(),
			Icon::Speaker,
			row.left_top() + vec2(20.0, 22.0),
			15.0,
			p.accent,
		);
		let place = room
			.guild
			.and_then(|g| state.guild(g))
			.map_or(String::new(), |g| g.name.clone());
		paint::text_left(
			ui.painter(),
			row.left_top() + vec2(34.0, 22.0),
			&room.name,
			theme::medium(13.5),
			p.text,
			row.width() - 130.0,
		);
		paint::text_left(
			ui.painter(),
			row.left_top() + vec2(34.0, 40.0),
			&place,
			theme::sans(11.5),
			p.muted,
			row.width() - 130.0,
		);
		// Overlapping faces of who's there.
		let mut x = row.left() + 34.0;
		for user in people.iter().take(5) {
			let face = Rect::from_center_size(pos2(x + 9.0, row.bottom() - 13.0), vec2(18.0, 18.0));
			ui.painter()
				.circle_filled(face.center(), 10.5, p.surface_alt);
			let known = nav::people(state)
				.into_iter()
				.map(|m| m.user.clone())
				.chain(
					state
						.channels
						.iter()
						.flat_map(|c| c.recipients.iter().cloned()),
				)
				.find(|u| u.id == *user);
			let texture = known
				.as_ref()
				.and_then(|u| solace.images.get(&u.avatar_key()));
			let name = known.map_or("?".to_owned(), |u| u.name);
			paint::avatar(
				ui.painter(),
				face,
				AvatarShape::Circle,
				texture,
				&name,
				user.0,
				p,
			);
			x += 14.0;
		}
		if people.len() > 5 {
			paint::label(
				ui.painter(),
				pos2(x + 12.0, row.bottom() - 13.0),
				Align2::LEFT_CENTER,
				&format!("+{}", people.len() - 5),
				theme::medium(11.0),
				p.muted,
			);
		}
		let button = Rect::from_min_size(row.right_center() - vec2(84.0, 15.0), vec2(74.0, 30.0));
		let here = active == Some(room.id);
		let enabled = !here && state.can_call(room.id) && active.is_none();
		let clicked = paint::pill_button(
			ui,
			ui.scope_id().with(("join-room", room.id)),
			button,
			None,
			if here { "Joined" } else { "Join" },
			ButtonKind::Primary,
			enabled,
		)
		.clicked();
		if clicked && let Some(command) = state.start_call(room.id, false) {
			commands.push(command);
		}
	}
}

fn online_card(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	online: &[(model::User, String)],
	rect: Rect,
	p: &Palette,
) {
	card(ui, rect, "Online now", None, p);
	if online.is_empty() {
		empty_line(ui, rect, "Friends who come online will show up here", p);
		return;
	}
	for (i, (user, status)) in online.iter().enumerate() {
		let row = Rect::from_min_size(
			rect.left_top() + vec2(10.0, CARD_HEAD + i as f32 * ROW),
			vec2(rect.width() - 20.0, ROW - 4.0),
		);
		let response = ui
			.interact(row, ui.scope_id().with(("online", user.id)), Sense::click())
			.on_hover_text(format!("Message {}", user.name));
		let t = paint::hover_t(ui, &response);
		if t > 0.0 {
			ui.painter().rect_filled(row, 12, p.hover.gamma_multiply(t));
		}
		let avatar = Rect::from_min_size(row.left_center() + vec2(10.0, -18.0), vec2(36.0, 36.0));
		let texture = solace.images.get(&user.avatar_key());
		paint::avatar(
			ui.painter(),
			avatar,
			AvatarShape::Circle,
			texture,
			&user.name,
			user.id.0,
			p,
		);
		let color = paint::status_color(Some(status), p).unwrap_or(p.faint);
		paint::presence(
			ui.painter(),
			avatar.right_bottom() - vec2(4.0, 4.0),
			4.5,
			color,
			mix(p.surface, p.hover, t),
		);
		let left = avatar.right() + 12.0;
		let custom = nav::custom_status_of(state, user.id).map(str::to_owned);
		paint::text_left(
			ui.painter(),
			pos2(left, row.center().y - 8.0),
			state.user_display_name(user),
			theme::medium(13.5),
			p.text,
			row.right() - left - 40.0,
		);
		paint::text_left(
			ui.painter(),
			pos2(left, row.center().y + 10.0),
			custom
				.as_deref()
				.unwrap_or(paint::status_label(Some(status))),
			theme::sans(11.5),
			p.muted,
			row.right() - left - 40.0,
		);
		icons::paint_at(
			ui.painter(),
			Icon::Thread,
			row.right_center() - vec2(20.0, 0.0),
			16.0,
			mix(p.faint, p.accent, t),
		);
		if response.clicked() {
			solace.pane = Pane::Direct;
			let command = match nav::dm_with(state, user.id) {
				Some(channel) => state.select(channel),
				None => state.open_friend_dm(user.id),
			};
			commands.extend(command);
		}
	}
}
