//! Right-hand context column: voice room, members, community info and the profile card.
use crate::{
	SettingsPage, Solace, display_name,
	icons::{self, Icon},
	nav,
	paint::{self, AvatarShape, ButtonKind},
	scenery::Mood,
	sidebar::conversation_title,
	theme::{self, Palette, mix},
};
use client_core::{Command, State, voice::Phase};
use egui::{Align2, Color32, Rect, Sense, Ui, pos2, vec2};
use model::{Id, User};

const PAD: f32 = 20.0;

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let p = theme::palette(ui.ctx());
	let rect = ui.max_rect();
	ui.painter().rect_filled(rect, 0, p.aside);
	paint::hairline(ui.painter(), rect.left_top(), rect.left_bottom(), &p);

	// Keep the member list subscribed while the column is visible.
	if state.selected.is_some()
		&& state
			.members
			.as_ref()
			.is_none_or(|list| Some(list.channel) != state.selected)
		&& let Some(command) = state.request_members()
	{
		commands.push(command);
	}

	egui::ScrollArea::vertical()
		.id_salt("aside-scroll")
		.auto_shrink([false, false])
		.show(ui, |ui| {
			let width = rect.width() - 2.0 * PAD;
			ui.add_space(20.0);
			let y_cursor = |ui: &mut Ui, h: f32| -> Rect {
				let (outer, _) = ui.allocate_exact_size(vec2(rect.width(), h), Sense::hover());
				Rect::from_min_size(outer.min + vec2(PAD, 0.0), vec2(width, h))
			};
			if let Some(channel) = nav::selected_channel(state).cloned() {
				let card = y_cursor(ui, 208.0);
				voice_card(solace, ui, state, commands, &channel, card, &p);
				ui.add_space(22.0);
				members(solace, ui, state, commands, &channel, width, &p);
				if let Some(guild) = channel.guild.and_then(|g| state.guild(g)).cloned() {
					ui.add_space(18.0);
					community_info(solace, ui, state, &guild, width, &p);
				}
			}
			ui.add_space(14.0);
			let card = y_cursor(ui, 92.0);
			profile_card(solace, ui, state, card, &p);
			ui.add_space(24.0);
		});
}

fn voice_card(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &model::Channel,
	rect: Rect,
	p: &Palette,
) {
	// The room: the active call, else the busiest voice room of this community, else this DM.
	let active = state.voice.active.as_ref().map(|c| {
		(
			c.channel,
			c.phase,
			c.muted,
			c.deafened,
			c.participants.clone(),
		)
	});
	let room = match (&active, channel.guild) {
		(Some((id, ..)), _) => state.channel(*id).cloned(),
		(None, Some(guild)) => {
			let rooms = nav::voice_rooms(state, guild);
			rooms
				.iter()
				.max_by_key(|c| {
					state
						.voice
						.roster
						.iter()
						.filter(|e| e.channel == c.id)
						.count()
				})
				.map(|c| (*c).clone())
		}
		(None, None) => matches!(channel.kind, 1 | 3).then(|| channel.clone()),
	};

	let texture = solace
		.scenery
		.texture(ui.ctx(), room.as_ref().map_or(3, |r| r.id.0), Mood::Lake)
		.id();
	paint::cover_image(
		ui.painter(),
		rect,
		theme::size::RADIUS_CARD,
		texture,
		vec2(720.0, 480.0),
	);
	paint::gradient(
		ui.painter(),
		rect,
		theme::size::RADIUS_CARD as f32,
		&[
			(0.0, Color32::from_white_alpha(150)),
			(0.55, Color32::from_white_alpha(40)),
			(1.0, Color32::TRANSPARENT),
		],
	);
	ui.painter().rect_stroke(
		rect,
		theme::size::RADIUS_CARD,
		egui::Stroke::new(1.0, Color32::from_black_alpha(14)),
		egui::StrokeKind::Inside,
	);
	let ink = Color32::from_rgb(0x24, 0x30, 0x2c);
	let soft = Color32::from_rgb(0x56, 0x66, 0x60);
	paint::label(
		ui.painter(),
		rect.left_top() + vec2(18.0, 28.0),
		Align2::LEFT_CENTER,
		if channel.guild.is_some() {
			"Voice Room"
		} else {
			"Call"
		},
		theme::medium(12.5),
		ink,
	);
	let wave = Rect::from_center_size(rect.right_top() + vec2(-28.0, 28.0), vec2(28.0, 28.0));
	ui.painter()
		.circle_filled(wave.center(), 14.0, Color32::from_white_alpha(140));
	icons::paint_at(ui.painter(), Icon::Waveform, wave.center(), 14.0, ink);

	let Some(room) = room else {
		paint::label(
			ui.painter(),
			rect.left_top() + vec2(18.0, 60.0),
			Align2::LEFT_CENTER,
			"No voice rooms here",
			theme::serif(17.0),
			ink,
		);
		paint::label(
			ui.painter(),
			rect.left_top() + vec2(18.0, 84.0),
			Align2::LEFT_CENTER,
			"Calls appear here when available",
			theme::sans(12.0),
			soft,
		);
		return;
	};
	let title = if room.guild.is_some() {
		room.name.clone()
	} else {
		conversation_title(state, &room)
	};
	paint::text_left(
		ui.painter(),
		rect.left_top() + vec2(18.0, 60.0),
		&title,
		theme::serif(17.5),
		ink,
		rect.width() - 36.0,
	);

	let in_room = active.as_ref().is_some_and(|(id, ..)| *id == room.id);
	let participants: Vec<Id> = match &active {
		Some((id, _, _, _, list)) if *id == room.id => list.iter().map(|p| p.user).collect(),
		_ => state
			.voice
			.roster
			.iter()
			.filter(|e| e.channel == room.id)
			.map(|e| e.participant.user)
			.collect(),
	};
	let speaking = participants
		.iter()
		.filter(|u| solace.media.voice_speaking.contains(u))
		.count();
	let subtitle = match &active {
		Some((id, phase, ..)) if *id == room.id && *phase != Phase::Connected => {
			phase.label().trim_end_matches("...").to_owned()
		}
		_ if participants.is_empty() => "Quiet for now".to_owned(),
		_ => format!("{} listening • {} speaking", participants.len(), speaking),
	};
	paint::text_left(
		ui.painter(),
		rect.left_top() + vec2(18.0, 84.0),
		&subtitle,
		theme::sans(12.0),
		soft,
		rect.width() - 36.0,
	);

	// Overlapping avatars.
	let mut x = rect.left() + 18.0;
	let y = rect.top() + 124.0;
	for user in participants.iter().take(5) {
		let avatar = Rect::from_center_size(pos2(x + 16.0, y), vec2(32.0, 32.0));
		ui.painter()
			.circle_filled(avatar.center(), 18.0, Color32::from_white_alpha(230));
		let known = find_user(state, *user);
		let texture = known
			.as_ref()
			.and_then(|u| solace.images.get(&u.avatar_key()));
		let name = known.as_ref().map_or("?".to_owned(), |u| u.name.clone());
		paint::avatar(
			ui.painter(),
			avatar,
			AvatarShape::Circle,
			texture,
			&name,
			user.0,
			p,
		);
		if solace.media.voice_speaking.contains(user) {
			ui.painter()
				.circle_stroke(avatar.center(), 18.0, egui::Stroke::new(2.0, p.online));
		}
		x += 26.0;
	}
	if participants.len() > 5 {
		paint::label(
			ui.painter(),
			pos2(x + 20.0, y),
			Align2::LEFT_CENTER,
			&format!("+{}", participants.len() - 5),
			theme::medium(12.0),
			ink,
		);
	}

	let button_y = rect.bottom() - 34.0;
	if in_room {
		let (_, _, muted, deafened, _) = active.clone().unwrap();
		let leave =
			Rect::from_min_size(pos2(rect.left() + 18.0, button_y - 18.0), vec2(124.0, 36.0));
		if paint::pill_button(
			ui,
			ui.scope_id().with("voice-leave"),
			leave,
			Some(Icon::PhoneOff),
			"Leave",
			ButtonKind::Danger,
			true,
		)
		.clicked()
		{
			commands.extend(state.leave_call());
		}
		let mic = Rect::from_center_size(pos2(leave.right() + 28.0, button_y), vec2(36.0, 36.0));
		if paint::icon_button(
			ui,
			ui.scope_id().with("voice-mute"),
			mic,
			if muted { Icon::MicOff } else { Icon::Mic },
			ButtonKind::Soft,
			muted,
			if muted { "Unmute" } else { "Mute" },
		)
		.clicked()
		{
			solace.media.voice_muted = !muted;
			commands.extend(state.set_call_mute(!muted, deafened));
		}
		let ears = Rect::from_center_size(pos2(mic.right() + 26.0, button_y), vec2(36.0, 36.0));
		if paint::icon_button(
			ui,
			ui.scope_id().with("voice-deafen"),
			ears,
			if deafened {
				Icon::HeadphonesOff
			} else {
				Icon::Headphones
			},
			ButtonKind::Soft,
			deafened,
			if deafened { "Undeafen" } else { "Deafen" },
		)
		.clicked()
		{
			solace.media.voice_deafened = !deafened;
			commands.extend(state.set_call_mute(muted, !deafened));
		}
	} else {
		let join =
			Rect::from_min_size(pos2(rect.left() + 18.0, button_y - 18.0), vec2(132.0, 36.0));
		let enabled =
			state.can_call(room.id) && active.is_none() && solace.media.voice_switch_ready;
		let label = if room.guild.is_some() {
			"Join voice"
		} else {
			"Start call"
		};
		let response = paint::pill_button(
			ui,
			ui.scope_id().with("voice-join"),
			join,
			Some(Icon::Waveform),
			label,
			ButtonKind::Primary,
			enabled,
		);
		let response = if active.is_some() {
			response.on_hover_text("Leave your current call first")
		} else {
			response
		};
		if response.clicked()
			&& let Some(command) = state.start_call(room.id, room.guild.is_none())
		{
			commands.push(command);
		}
	}
	if let Some((id, Phase::Failed, ..)) = &active
		&& *id == room.id
		&& let Some(error) = state.voice.active.as_ref().and_then(|c| c.error)
	{
		solace.toasts.push(crate::Level::Error, error);
	}
}

pub(crate) fn find_user(state: &State, id: Id) -> Option<User> {
	if let Some(user) = state.user.as_ref().filter(|u| u.id == id) {
		return Some(user.clone());
	}
	if let Some(entry) = state.voice.roster.iter().find(|e| e.participant.user == id)
		&& let Some(member) = &entry.member
	{
		return Some(member.user.clone());
	}
	if let Some(member) = nav::people(state).into_iter().find(|m| m.user.id == id) {
		return Some(member.user.clone());
	}
	state
		.channels
		.iter()
		.flat_map(|c| c.recipients.iter())
		.find(|u| u.id == id)
		.cloned()
}

fn members(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &model::Channel,
	width: f32,
	p: &Palette,
) {
	let (header, _) = ui.allocate_exact_size(vec2(width + 2.0 * PAD, 30.0), Sense::hover());
	paint::label(
		ui.painter(),
		header.left_center() + vec2(PAD, 0.0),
		Align2::LEFT_CENTER,
		"Members",
		theme::medium(16.0),
		p.text,
	);
	let see_all = Rect::from_min_size(
		header.right_center() - vec2(PAD + 56.0, 10.0),
		vec2(56.0, 20.0),
	);
	let response = ui.interact(see_all, ui.scope_id().with("see-all"), Sense::click());
	paint::label(
		ui.painter(),
		see_all.right_center(),
		Align2::RIGHT_CENTER,
		if solace.members_expanded {
			"Show less"
		} else {
			"See all"
		},
		theme::sans(12.5),
		if response.hovered() { p.text } else { p.muted },
	);
	if response.clicked() {
		solace.members_expanded = !solace.members_expanded;
	}
	ui.add_space(6.0);

	// DMs know their participants; guild lists come from the member subscription.
	let mut people: Vec<(User, Option<String>, Option<String>)> = if channel.guild.is_none() {
		let mut list: Vec<User> = channel.recipients.clone();
		if let Some(own) = &state.user {
			list.push(own.clone());
		}
		list.into_iter()
			.map(|u| {
				let status = if state.user.as_ref().is_some_and(|own| own.id == u.id) {
					Some(if state.gateway_connected {
						"online".to_owned()
					} else {
						"offline".into()
					})
				} else {
					nav::status_of(state, u.id).map(str::to_owned)
				};
				(u, None, status)
			})
			.collect()
	} else {
		nav::people(state)
			.into_iter()
			.map(|m| (m.user.clone(), m.nick.clone(), m.status.clone()))
			.collect()
	};
	// Online first, as the reference shows active people at the top.
	people.sort_by_key(|(_, _, status)| match status.as_deref() {
		Some("online") => 0,
		Some("dnd") => 1,
		Some("idle") => 2,
		_ => 3,
	});
	let limit = if solace.members_expanded { 100 } else { 6 };
	if people.is_empty() {
		let (rect, _) = ui.allocate_exact_size(vec2(width + 2.0 * PAD, 40.0), Sense::hover());
		let text = if state.members.is_some() {
			"Nobody to show yet"
		} else {
			"Loading members..."
		};
		paint::label(
			ui.painter(),
			rect.left_center() + vec2(PAD, 0.0),
			Align2::LEFT_CENTER,
			text,
			theme::sans(12.5),
			p.faint,
		);
	}
	for (user, nick, status) in people.into_iter().take(limit) {
		let (outer, response) =
			ui.allocate_exact_size(vec2(width + 2.0 * PAD, 50.0), Sense::click());
		let rect = Rect::from_min_max(
			outer.min + vec2(PAD - 8.0, 0.0),
			outer.max - vec2(PAD - 8.0, 0.0),
		);
		let t = paint::hover_t(ui, &response);
		if t > 0.0 {
			ui.painter()
				.rect_filled(rect, 12, p.hover.gamma_multiply(t));
		}
		let avatar = Rect::from_min_size(rect.left_center() + vec2(8.0, -18.0), vec2(36.0, 36.0));
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
		let in_voice = state
			.voice
			.roster
			.iter()
			.any(|e| e.participant.user == user.id && e.channel.0 != 0);
		let (label, color) = if in_voice {
			("Listening", p.listening)
		} else {
			match status.as_deref() {
				Some("online") => ("Online", p.online),
				Some("idle") => ("Away", p.idle),
				Some("dnd") => ("Do not disturb", p.dnd),
				_ => ("Offline", p.faint),
			}
		};
		if status.as_deref().is_some_and(|s| s != "offline") || in_voice {
			paint::presence(
				ui.painter(),
				avatar.right_bottom() - vec2(4.0, 4.0),
				4.5,
				color,
				mix(p.aside, p.hover, t),
			);
		}
		let name = nick
			.filter(|n| !n.is_empty())
			.unwrap_or_else(|| display_name(state, &user));
		let left = avatar.right() + 12.0;
		paint::text_left(
			ui.painter(),
			pos2(left, rect.center().y - 8.0),
			&name,
			theme::sans(14.0),
			p.text,
			rect.right() - left - 8.0,
		);
		ui.painter()
			.circle_filled(pos2(left + 3.0, rect.center().y + 10.0), 3.0, color);
		paint::label(
			ui.painter(),
			pos2(left + 10.0, rect.center().y + 10.0),
			Align2::LEFT_CENTER,
			label,
			theme::sans(11.5),
			color,
		);
		if response.clicked()
			&& state.user.as_ref().is_none_or(|own| own.id != user.id)
			&& let Some(command) = state.open_friend_dm(user.id)
		{
			commands.push(command);
		}
		response.on_hover_text(format!("Message {}", user.name));
	}
}

fn community_info(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &State,
	guild: &model::Guild,
	width: f32,
	p: &Palette,
) {
	let sections = nav::sections(state, guild.id);
	let tags: Vec<String> = sections
		.iter()
		.filter_map(|s| s.category.map(|c| c.name.clone()))
		.take(6)
		.map(|name| {
			let lower = name.to_lowercase();
			let mut chars = lower.chars();
			chars
				.next()
				.map(|c| c.to_uppercase().chain(chars).collect())
				.unwrap_or_default()
		})
		.collect();
	let text_channels = sections
		.iter()
		.flat_map(|s| &s.channels)
		.filter(|c| nav::is_text(c.kind))
		.count();
	let voice = sections
		.iter()
		.flat_map(|s| &s.channels)
		.filter(|c| nav::is_voice(c.kind))
		.count();
	let description = format!(
		"{} gathers {text_channels} text {} and {voice} voice {}. Pick a room and say hello.",
		guild.name,
		if text_channels == 1 {
			"channel"
		} else {
			"channels"
		},
		if voice == 1 { "room" } else { "rooms" },
	);
	let galley = ui
		.painter()
		.layout(description, theme::sans(13.0), p.muted, width - 32.0);
	// Lay tags out first to know the card height.
	let mut tag_rows = 0.0;
	let mut x = 0.0;
	for tag in &tags {
		let w = ui
			.painter()
			.layout_no_wrap(tag.clone(), theme::sans(12.0), p.text)
			.size()
			.x + 20.0;
		if x + w > width - 32.0 || tag_rows == 0.0 {
			tag_rows += 1.0;
			x = 0.0;
		}
		x += w + 8.0;
	}
	let height = 132.0 + galley.size().y + tag_rows * 38.0;
	let (outer, _) = ui.allocate_exact_size(vec2(width + 2.0 * PAD, height), Sense::hover());
	let card = Rect::from_min_size(outer.min + vec2(PAD, 0.0), vec2(width, height));
	paint::card(ui.painter(), card, 16, p);
	paint::label(
		ui.painter(),
		card.left_top() + vec2(16.0, 26.0),
		Align2::LEFT_CENTER,
		"Community info",
		theme::medium(15.0),
		p.text,
	);
	let icon = Rect::from_min_size(card.left_top() + vec2(16.0, 50.0), vec2(48.0, 48.0));
	let texture = solace.images.get_opt(guild.icon_key().as_deref());
	paint::avatar(
		ui.painter(),
		icon,
		AvatarShape::Rounded(12.0),
		texture,
		&guild.name,
		guild.id.0,
		p,
	);
	paint::text_left(
		ui.painter(),
		pos2(icon.right() + 12.0, icon.center().y - 9.0),
		&guild.name,
		theme::serif(16.0),
		p.text,
		card.right() - icon.right() - 28.0,
	);
	let counts = match nav::member_counts(state, guild.id) {
		Some((total, online)) if total > 0 => format!(
			"{} members • {} online",
			nav::compact(total),
			nav::compact(online)
		),
		_ => format!("{} channels", text_channels + voice),
	};
	paint::text_left(
		ui.painter(),
		pos2(icon.right() + 12.0, icon.center().y + 11.0),
		&counts,
		theme::sans(12.0),
		p.muted,
		card.right() - icon.right() - 28.0,
	);
	let text_top = icon.bottom() + 14.0;
	ui.painter()
		.galley(pos2(card.left() + 16.0, text_top), galley.clone(), p.muted);
	let mut x = card.left() + 16.0;
	let mut y = text_top + galley.size().y + 14.0;
	for tag in &tags {
		let w = ui
			.painter()
			.layout_no_wrap(tag.clone(), theme::sans(12.0), p.text)
			.size()
			.x + 20.0;
		if x + w > card.right() - 16.0 {
			x = card.left() + 16.0;
			y += 38.0;
		}
		let chip = paint::chip(ui.painter(), pos2(x, y), tag, p);
		x = chip.right() + 8.0;
	}
}

fn profile_card(solace: &mut Solace, ui: &mut Ui, state: &State, rect: Rect, p: &Palette) {
	let Some(user) = state.user.clone() else {
		return;
	};
	let response = ui.interact(rect, ui.scope_id().with("profile-card"), Sense::click());
	let t = paint::hover_t(ui, &response);
	paint::card(ui.painter(), rect, 16, p);
	if t > 0.0 {
		ui.painter()
			.rect_filled(rect.shrink(1.0), 15, p.hover.gamma_multiply(t * 0.6));
	}
	paint::label(
		ui.painter(),
		rect.left_top() + vec2(16.0, 22.0),
		Align2::LEFT_CENTER,
		"Custom profile",
		theme::medium(15.0),
		p.text,
	);
	let avatar = Rect::from_min_size(rect.left_top() + vec2(16.0, 40.0), vec2(36.0, 36.0));
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
	paint::text_left(
		ui.painter(),
		pos2(avatar.right() + 12.0, avatar.center().y - 8.0),
		state.user_display_name(&user),
		theme::sans(13.5),
		p.text,
		rect.width() - 110.0,
	);
	paint::label(
		ui.painter(),
		pos2(avatar.right() + 12.0, avatar.center().y + 10.0),
		Align2::LEFT_CENTER,
		"Make it yours",
		theme::sans(11.5),
		p.muted,
	);
	icons::paint_at(
		ui.painter(),
		Icon::ChevronRight,
		pos2(rect.right() - 24.0, avatar.center().y),
		15.0,
		p.muted,
	);
	if response.clicked() {
		solace.settings = Some(SettingsPage::Account);
	}
}
