//! Navigation column: brand, search, communities (with the open community's channels) and
//! direct messages.
use crate::{
	Pane, Solace, clock,
	icons::{self, Icon},
	images, nav,
	paint::{self, AvatarShape},
	theme::{self, Palette, mix},
};
use client_core::{Command, State};
use egui::{Align2, Key, Rect, Response, Sense, TextEdit, Ui, pos2, vec2};
use model::{Channel, Guild, Id};

pub(crate) const SEARCH_ID: &str = "solace-search";

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let p = theme::palette(ui.ctx());
	let rect = ui.max_rect();
	ui.painter().rect_filled(rect, 0, p.sidebar);
	paint::hairline(ui.painter(), rect.right_top(), rect.right_bottom(), &p);

	let left = rect.left() + 20.0;
	paint::label(
		ui.painter(),
		pos2(left, rect.top() + 40.0),
		Align2::LEFT_CENTER,
		"Solace",
		theme::serif(25.0),
		p.text,
	);

	search_field(
		solace,
		ui,
		Rect::from_min_size(
			pos2(rect.left() + 16.0, rect.top() + 72.0),
			vec2(rect.width() - 32.0, 40.0),
		),
		&p,
	);

	let list = Rect::from_min_max(pos2(rect.left(), rect.top() + 128.0), rect.max);
	let mut child = ui.new_child(
		egui::UiBuilder::new()
			.max_rect(list)
			.id_salt("sidebar-list"),
	);
	child.set_clip_rect(list);
	egui::ScrollArea::vertical()
		.id_salt(("sidebar-scroll", solace.pane as u8))
		.auto_shrink([false, false])
		.show(&mut child, |ui| {
			ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
			let query = solace.search.trim().to_lowercase();
			match solace.pane {
				Pane::Home => {}
				Pane::Direct => direct(solace, ui, state, commands, &query, usize::MAX),
				Pane::Explore => communities(solace, ui, state, commands, &query, true),
				Pane::Activity => activity(solace, ui, state, commands, &query),
			}
			ui.add_space(24.0);
		});
}

fn search_field(solace: &mut Solace, ui: &mut Ui, rect: Rect, p: &Palette) {
	let id = egui::Id::unique(SEARCH_ID);
	let focused = ui.ctx().memory(|m| m.has_focus(id));
	ui.painter().rect(
		rect,
		12,
		p.surface,
		egui::Stroke::new(
			1.0,
			if focused {
				p.accent.gamma_multiply(0.5)
			} else {
				p.border
			},
		),
		egui::StrokeKind::Inside,
	);
	icons::paint_at(
		ui.painter(),
		Icon::Search,
		rect.left_center() + vec2(18.0, 0.0),
		16.0,
		p.muted,
	);
	let shortcut = if ui.ctx().os() == egui::os::OperatingSystem::Mac {
		"⌘ K"
	} else {
		"Ctrl K"
	};
	let hint = ui
		.painter()
		.layout_no_wrap(shortcut.into(), theme::sans(11.0), p.muted);
	let chip = Rect::from_min_size(
		rect.right_center() - vec2(hint.size().x + 22.0, 10.0),
		vec2(hint.size().x + 12.0, 20.0),
	);
	if solace.search.is_empty() {
		ui.painter().rect_filled(chip, 6, p.surface_alt);
		ui.painter()
			.galley(chip.center() - hint.size() / 2.0, hint, p.muted);
	}
	if ui.input(|i| i.modifiers.command && i.key_pressed(Key::K)) {
		ui.ctx().memory_mut(|m| m.request_focus(id));
	}
	let edit = Rect::from_min_max(
		rect.min + vec2(36.0, 0.0),
		pos2(chip.left() - 6.0, rect.bottom()),
	);
	let response = ui.put(
		edit,
		TextEdit::singleline(&mut solace.search)
			.id(id)
			.frame(egui::Frame::NONE)
			.font(theme::sans(13.5))
			.text_color(p.text)
			.hint_text(
				egui::RichText::new("Search anything...")
					.color(p.muted)
					.font(theme::sans(13.5)),
			)
			.align(egui::Align2::LEFT_CENTER)
			.desired_width(edit.width()),
	);
	if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
		solace.search.clear();
		response.surrender_focus();
	}
}

fn section_header(ui: &mut Ui, title: &str, action: Option<&str>) -> Option<Response> {
	let p = theme::palette(ui.ctx());
	let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
	paint::label(
		ui.painter(),
		rect.left_center() + vec2(20.0, 0.0),
		Align2::LEFT_CENTER,
		title,
		theme::medium(14.0),
		p.text,
	);
	let action = action?;
	let button = Rect::from_center_size(rect.right_center() - vec2(30.0, 0.0), vec2(26.0, 26.0));
	Some(paint::icon_button(
		ui,
		ui.scope_id().with(("section", title)),
		button,
		Icon::Plus,
		paint::ButtonKind::Ghost,
		false,
		action,
	))
}

fn row(ui: &mut Ui, height: f32, selected: bool) -> (Rect, Response) {
	let p = theme::palette(ui.ctx());
	let width = ui.available_width();
	let (outer, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
	let rect = Rect::from_min_max(outer.min + vec2(10.0, 0.0), outer.max - vec2(12.0, 0.0));
	let t = paint::hover_t(ui, &response);
	let fill = if selected {
		p.selected
	} else {
		p.hover.gamma_multiply(t)
	};
	if selected || t > 0.0 {
		ui.painter()
			.rect_filled(rect, theme::size::RADIUS_ITEM, fill);
	}
	(rect, response)
}

fn matches(query: &str, name: &str) -> bool {
	query.is_empty() || name.to_lowercase().contains(query)
}

fn communities(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	query: &str,
	expand_selected: bool,
) {
	let p = theme::palette(ui.ctx());
	if section_header(ui, "Communities", Some("Join a community")).is_some_and(|r| r.clicked()) {
		solace.join = Some(String::new());
	}
	let selected_guild = nav::selected_guild(state).map(|g| g.id);
	let guilds: Vec<Guild> = nav::communities(state)
		.filter(|g| {
			matches(query, &g.name)
				|| (!query.is_empty()
					&& nav::sections(state, g.id)
						.iter()
						.flat_map(|s| &s.channels)
						.any(|c| matches(query, &c.name)))
		})
		.cloned()
		.collect();
	if guilds.is_empty() {
		empty(
			ui,
			if query.is_empty() {
				"No communities yet"
			} else {
				"No matching communities"
			},
		);
	}
	for guild in guilds {
		let selected = selected_guild == Some(guild.id);
		let (rect, response) = row(ui, 57.0, selected);
		let icon = Rect::from_min_size(rect.left_center() + vec2(8.0, -19.0), vec2(38.0, 38.0));
		let texture = solace.images.get_opt(guild.icon_key().as_deref());
		paint::avatar(
			ui.painter(),
			icon,
			AvatarShape::Rounded(11.0),
			texture,
			&guild.name,
			guild.id.0,
			&p,
		);
		let text_left = icon.right() + 13.0;
		let max = rect.right() - text_left - 22.0;
		paint::text_left(
			ui.painter(),
			pos2(text_left, rect.center().y - 9.0),
			&guild.name,
			theme::serif(15.5),
			p.text,
			max,
		);
		let subtitle = match nav::member_counts(state, guild.id) {
			Some((total, online)) if total > 0 => format!(
				"{} members • {} online",
				nav::compact(total),
				nav::compact(online)
			),
			_ => {
				let sections = nav::sections(state, guild.id);
				let text = sections
					.iter()
					.flat_map(|s| &s.channels)
					.filter(|c| nav::is_text(c.kind))
					.count();
				let voice = sections
					.iter()
					.flat_map(|s| &s.channels)
					.filter(|c| nav::is_voice(c.kind))
					.count();
				let channels = if text == 1 { "channel" } else { "channels" };
				if voice > 0 {
					format!("{text} {channels} • {voice} voice")
				} else {
					format!("{text} {channels}")
				}
			}
		};
		paint::text_left(
			ui.painter(),
			pos2(text_left, rect.center().y + 10.0),
			&subtitle,
			theme::sans(11.5),
			p.muted,
			max,
		);
		if !selected && nav::guild_unread(state, guild.id) {
			ui.painter()
				.circle_filled(rect.right_center() - vec2(14.0, 0.0), 4.0, p.accent);
		}
		let mut selected = selected;
		if response.clicked() {
			if selected {
				// Clicking the open community again closes it: its channels fold away
				// and the conversation pane returns to the welcome view.
				state.open_home();
				selected = false;
			} else if let Some(command) = state.select_guild(guild.id) {
				commands.push(command);
			} else if let Some(channel) = nav::landing(state, guild.id)
				&& let Some(command) = state.select(channel)
			{
				commands.push(command);
			}
		}
		if (selected && expand_selected) || (!query.is_empty() && !matches(query, &guild.name)) {
			channels(solace, ui, state, commands, guild.id, query);
		}
	}
}

fn channels(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	guild: Id,
	query: &str,
) {
	let p = theme::palette(ui.ctx());
	ui.add_space(2.0);
	let sections: Vec<(Option<Channel>, Vec<Channel>)> = nav::sections(state, guild)
		.into_iter()
		.map(|s| {
			(
				s.category.cloned(),
				s.channels.into_iter().cloned().collect(),
			)
		})
		.collect();
	let in_call = state.voice.active.as_ref().map(|c| c.channel);
	for (category, channels) in sections {
		let visible: Vec<&Channel> = channels
			.iter()
			.filter(|c| matches(query, &c.name))
			.collect();
		if visible.is_empty() {
			continue;
		}
		if let Some(category) = &category {
			let collapsed = solace.collapsed.contains(&category.id) && query.is_empty();
			let (outer, response) =
				ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
			let color = if response.hovered() {
				p.text_soft
			} else {
				p.muted
			};
			icons::paint_at(
				ui.painter(),
				if collapsed {
					Icon::ChevronRight
				} else {
					Icon::ChevronDown
				},
				pos2(outer.left() + 31.0, outer.center().y),
				11.0,
				color,
			);
			paint::text_left(
				ui.painter(),
				pos2(outer.left() + 41.0, outer.center().y),
				&category.name.to_uppercase(),
				theme::medium(10.5),
				color,
				outer.width() - 60.0,
			);
			if response.clicked() && !solace.collapsed.remove(&category.id) {
				solace.collapsed.insert(category.id);
			}
			if collapsed {
				// Keep the open or unread channel reachable even when its category is folded.
				let selected = state.selected;
				for channel in visible.iter().filter(|c| selected == Some(c.id)) {
					channel_row(solace, ui, state, commands, channel, in_call, &p);
				}
				continue;
			}
		}
		for channel in visible {
			channel_row(solace, ui, state, commands, channel, in_call, &p);
		}
	}
	ui.add_space(6.0);
}

fn channel_row(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &Channel,
	in_call: Option<Id>,
	p: &Palette,
) {
	let selected = state.selected == Some(channel.id);
	let (rect, response) = row(ui, 32.0, selected);
	let voice = nav::is_voice(channel.kind);
	let unread = !selected && nav::channel_unread(state, channel);
	let color = if selected || unread { p.text } else { p.muted };
	let icon = match channel.kind {
		2 | 13 => Icon::Speaker,
		5 => Icon::Megaphone,
		15 | 16 => Icon::Thread,
		_ => Icon::Hash,
	};
	icons::paint_at(
		ui.painter(),
		icon,
		pos2(rect.left() + 26.0, rect.center().y),
		14.0,
		color,
	);
	let font = if unread {
		theme::semibold(13.5)
	} else {
		theme::sans(13.5)
	};
	paint::text_left(
		ui.painter(),
		pos2(rect.left() + 40.0, rect.center().y),
		&channel.name,
		font,
		color,
		rect.width() - 80.0,
	);
	if voice {
		let count = state
			.voice
			.roster
			.iter()
			.filter(|e| e.channel == channel.id)
			.count();
		if in_call == Some(channel.id) {
			icons::paint_at(
				ui.painter(),
				Icon::Waveform,
				rect.right_center() - vec2(16.0, 0.0),
				14.0,
				p.online,
			);
		} else if count > 0 {
			paint::label(
				ui.painter(),
				rect.right_center() - vec2(12.0, 0.0),
				Align2::RIGHT_CENTER,
				&count.to_string(),
				theme::medium(11.5),
				p.muted,
			);
		}
	} else if unread {
		ui.painter()
			.circle_filled(rect.right_center() - vec2(14.0, 0.0), 3.5, p.accent);
	}
	let response = if voice {
		response.on_hover_text("Join voice")
	} else {
		response
	};
	if response.clicked() {
		if voice {
			if in_call != Some(channel.id)
				&& let Some(command) = state.start_call(channel.id, false)
			{
				commands.push(command);
			}
		} else if let Some(command) = state.select(channel.id) {
			commands.push(command);
		}
	}
	let _ = solace;
}

pub(crate) fn conversation_title(state: &State, channel: &Channel) -> String {
	let name = state.conversation_name(channel);
	if !name.is_empty() {
		return name.to_owned();
	}
	let names: Vec<&str> = channel
		.recipients
		.iter()
		.map(|u| u.name.as_str())
		.take(4)
		.collect();
	if names.is_empty() {
		"Unnamed group".into()
	} else {
		names.join(", ")
	}
}

pub(crate) fn conversation_avatar(
	solace: &mut Solace,
	painter: &egui::Painter,
	rect: Rect,
	state: &State,
	channel: &Channel,
	p: &Palette,
) {
	if channel.kind == 1
		&& let Some(user) = channel.recipients.first()
	{
		let texture = solace.images.get(&user.avatar_key());
		paint::avatar(
			painter,
			rect,
			AvatarShape::Circle,
			texture,
			&user.name,
			user.id.0,
			p,
		);
	} else {
		let texture = solace
			.images
			.get_opt(images::group_icon_key(channel).as_deref());
		let title = conversation_title(state, channel);
		paint::avatar(
			painter,
			rect,
			AvatarShape::Circle,
			texture,
			&title,
			channel.id.0,
			p,
		);
	}
}

fn direct(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	query: &str,
	limit: usize,
) {
	let p = theme::palette(ui.ctx());
	if section_header(ui, "Direct Messages", Some("Find a conversation"))
		.is_some_and(|r| r.clicked())
	{
		ui.ctx()
			.memory_mut(|m| m.request_focus(egui::Id::unique(SEARCH_ID)));
	}
	let channels: Vec<Channel> = nav::direct(state)
		.into_iter()
		.filter(|c| matches(query, &conversation_title(state, c)))
		.take(limit)
		.cloned()
		.collect();
	if channels.is_empty() {
		empty(
			ui,
			if query.is_empty() {
				"No conversations yet"
			} else {
				"No matching conversations"
			},
		);
	}
	let now = std::time::Instant::now();
	let typing: Vec<Id> = state.typing_users(now).collect();
	for channel in channels {
		let selected = state.selected == Some(channel.id);
		let (rect, response) = row(ui, 54.0, selected);
		let avatar = Rect::from_min_size(rect.left_center() + vec2(8.0, -18.0), vec2(36.0, 36.0));
		conversation_avatar(solace, ui.painter(), avatar, state, &channel, &p);
		let peer = (channel.kind == 1)
			.then(|| channel.recipients.first())
			.flatten();
		let status = peer.and_then(|u| nav::status_of(state, u.id));
		if let Some(color) = paint::status_color(status, &p) {
			let ring = if selected {
				p.selected
			} else if response.hovered() {
				p.hover
			} else {
				p.sidebar
			};
			paint::presence(
				ui.painter(),
				avatar.right_bottom() - vec2(4.0, 4.0),
				4.5,
				color,
				ring,
			);
		}
		let text_left = avatar.right() + 12.0;
		let max = rect.right() - text_left - 12.0;
		let title = conversation_title(state, &channel);
		let unread = !selected && nav::channel_unread(state, &channel);
		paint::text_left(
			ui.painter(),
			pos2(text_left, rect.center().y - 9.0),
			&title,
			if unread {
				theme::semibold(14.0)
			} else {
				theme::sans(14.0)
			},
			p.text,
			max - 28.0,
		);
		let is_typing = selected && channel.recipients.iter().any(|r| typing.contains(&r.id));
		let subtitle_pos = pos2(text_left, rect.center().y + 10.0);
		if is_typing {
			ui.painter()
				.circle_filled(subtitle_pos + vec2(3.5, 0.0), 3.5, p.online);
			paint::text_left(
				ui.painter(),
				subtitle_pos + vec2(12.0, 0.0),
				"typing...",
				theme::sans(11.5),
				p.muted,
				max,
			);
		} else {
			let custom = peer.and_then(|u| nav::custom_status_of(state, u.id));
			let subtitle = if let Some(custom) = custom {
				custom.to_owned()
			} else if channel.kind == 3 {
				format!("{} members", channel.recipients.len() + 1)
			} else if let Some(status) = status.filter(|s| *s != "offline") {
				paint::status_label(Some(status)).to_owned()
			} else if let Some(last) = channel.last_message {
				format!("Active {}", clock::short_age(clock::snowflake(last)))
			} else {
				"Direct message".into()
			};
			paint::text_left(
				ui.painter(),
				subtitle_pos,
				&subtitle,
				theme::sans(11.5),
				p.muted,
				max,
			);
		}
		if unread {
			ui.painter()
				.circle_filled(rect.right_center() - vec2(14.0, 0.0), 4.0, p.accent);
		} else if let Some(last) = channel.last_message {
			paint::label(
				ui.painter(),
				pos2(rect.right() - 12.0, rect.center().y - 9.0),
				Align2::RIGHT_CENTER,
				&clock::short_age(clock::snowflake(last)),
				theme::sans(10.5),
				p.faint,
			);
		}
		if response.clicked()
			&& let Some(command) = state.select(channel.id)
		{
			commands.push(command);
		}
	}
}

fn activity(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	query: &str,
) {
	let p = theme::palette(ui.ctx());
	section_header(ui, "Activity", None);
	let unread: Vec<Channel> = state
		.channels
		.iter()
		.filter(|c| {
			(nav::is_text(c.kind) || nav::is_dm(c))
				&& nav::channel_unread(state, c)
				&& state.can_view(c.id)
		})
		.filter(|c| matches(query, &conversation_title(state, c)))
		.cloned()
		.collect();
	if unread.is_empty() {
		empty(ui, "You're all caught up");
		return;
	}
	for channel in unread {
		let selected = state.selected == Some(channel.id);
		let (rect, response) = row(ui, 50.0, selected);
		let avatar = Rect::from_min_size(rect.left_center() + vec2(8.0, -17.0), vec2(34.0, 34.0));
		let place = match channel.guild.and_then(|g| state.guild(g)).cloned() {
			Some(guild) => {
				let texture = solace.images.get_opt(guild.icon_key().as_deref());
				paint::avatar(
					ui.painter(),
					avatar,
					AvatarShape::Rounded(10.0),
					texture,
					&guild.name,
					guild.id.0,
					&p,
				);
				guild.name.clone()
			}
			None => {
				conversation_avatar(solace, ui.painter(), avatar, state, &channel, &p);
				"Direct message".into()
			}
		};
		let left = avatar.right() + 12.0;
		let max = rect.right() - left - 26.0;
		let title = if channel.guild.is_some() {
			format!("#{}", channel.name)
		} else {
			conversation_title(state, &channel)
		};
		paint::text_left(
			ui.painter(),
			pos2(left, rect.center().y - 8.0),
			&title,
			theme::semibold(13.5),
			p.text,
			max,
		);
		paint::text_left(
			ui.painter(),
			pos2(left, rect.center().y + 10.0),
			&place,
			theme::sans(11.5),
			p.muted,
			max,
		);
		ui.painter()
			.circle_filled(rect.right_center() - vec2(14.0, 0.0), 4.0, p.accent);
		if response.clicked()
			&& let Some(command) = state.select(channel.id)
		{
			commands.push(command);
		}
	}
	let _ = mix;
}

fn empty(ui: &mut Ui, text: &str) {
	let p = theme::palette(ui.ctx());
	let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
	paint::label(
		ui.painter(),
		rect.left_center() + vec2(22.0, 0.0),
		Align2::LEFT_CENTER,
		text,
		theme::sans(12.5),
		p.faint,
	);
}
