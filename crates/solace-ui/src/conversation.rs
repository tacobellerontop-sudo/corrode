//! The open conversation: header, community banner, virtualized timeline and composer.
use crate::{
	PickerTarget, Solace, clock, composer, display_name,
	icons::{self, Icon},
	images, nav,
	paint::{self, AvatarShape, ButtonKind},
	scenery::Mood,
	sidebar::{conversation_avatar, conversation_title},
	text::{self, Names, Style},
	theme::{self, Palette, mix},
};
use client_core::{Command, State};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use model::{Delivery, Id, Message};
use std::collections::HashMap;

const AVATAR: f32 = 38.0;
const GUTTER: f32 = 52.0;
const GROUP_WINDOW_MINUTES: i64 = 7;
const HERO_HEIGHT: f32 = 228.0;
const MAX_TEXT_WIDTH: f32 = 760.0;

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let p = theme::palette(ui.ctx());
	let rect = ui.max_rect();
	let Some(channel) = nav::selected_channel(state).cloned() else {
		welcome(solace, ui, state, rect, &p);
		return;
	};
	let header = Rect::from_min_size(rect.min, vec2(rect.width(), theme::size::HEADER));
	header_bar(solace, ui, state, commands, &channel, header, &p);

	let pad = theme::size::GUTTER;
	let composer_height = composer::height(solace, ui, state, rect.width() - 2.0 * pad);
	let composer_rect = Rect::from_min_max(
		pos2(rect.left() + pad, rect.bottom() - pad - composer_height),
		pos2(rect.right() - pad, rect.bottom() - pad),
	);
	// On roomy windows the community banner stays pinned above the conversation.
	let pinned_hero = channel.guild.is_some() && rect.height() >= 760.0;
	let mut top = header.bottom();
	if pinned_hero {
		let hero_rect = Rect::from_min_size(
			pos2(rect.left() + pad, top),
			vec2(rect.width() - 2.0 * pad, HERO_HEIGHT),
		);
		hero(solace, ui, state, hero_rect, &p);
		top = hero_rect.bottom() + 10.0;
	}
	let timeline = Rect::from_min_max(
		pos2(rect.left() + pad, top),
		pos2(rect.right() - 8.0, composer_rect.top() - 6.0),
	);
	timeline_view(
		solace,
		ui,
		state,
		commands,
		&channel,
		timeline,
		!pinned_hero,
		&p,
	);
	composer::show(solace, ui, state, commands, &channel, composer_rect);
}

fn welcome(solace: &mut Solace, ui: &mut Ui, state: &State, rect: Rect, p: &Palette) {
	let hero = Rect::from_min_size(
		rect.min + vec2(24.0, 24.0),
		vec2(rect.width() - 48.0, 300.0),
	);
	let texture = solace.scenery.texture(ui.ctx(), 7, Mood::Dawn).id();
	paint::cover_image(
		ui.painter(),
		hero,
		theme::size::RADIUS_CARD,
		texture,
		vec2(1280.0, 380.0),
	);
	paint::gradient(
		ui.painter(),
		hero,
		theme::size::RADIUS_CARD as f32,
		&[
			(0.35, Color32::TRANSPARENT),
			(1.0, Color32::from_black_alpha(130)),
		],
	);
	let name = state.user.as_ref().map_or("there".to_owned(), |u| {
		state.user_display_name(u).to_owned()
	});
	paint::label(
		ui.painter(),
		hero.left_bottom() + vec2(30.0, -78.0),
		Align2::LEFT_CENTER,
		&format!("Welcome back, {name}"),
		theme::serif(34.0),
		Color32::WHITE,
	);
	paint::label(
		ui.painter(),
		hero.left_bottom() + vec2(30.0, -40.0),
		Align2::LEFT_CENTER,
		"Pick a community or a conversation to begin.",
		theme::sans(15.0),
		Color32::from_white_alpha(230),
	);
	let status = if !state.gateway_connected && !state.demo {
		state.status
	} else {
		""
	};
	if !status.is_empty() {
		paint::label(
			ui.painter(),
			pos2(rect.center().x, hero.bottom() + 40.0),
			Align2::CENTER_CENTER,
			status,
			theme::sans(13.0),
			p.muted,
		);
	}
}

fn header_bar(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &model::Channel,
	rect: Rect,
	p: &Palette,
) {
	let icon = Rect::from_min_size(rect.min + vec2(24.0, 20.0), vec2(42.0, 42.0));
	let guild = channel.guild.and_then(|g| state.guild(g)).cloned();
	let (title, crumbs) = match &guild {
		Some(guild) => {
			let texture = solace.images.get_opt(guild.icon_key().as_deref());
			paint::avatar(
				ui.painter(),
				icon,
				AvatarShape::Rounded(11.0),
				texture,
				&guild.name,
				guild.id.0,
				p,
			);
			let mut crumbs = Vec::new();
			if let Some(category) = nav::category_of(state, channel) {
				crumbs.push(title_case(&category.name));
			}
			if let Some(parent) = channel
				.parent_id
				.and_then(|id| state.channel(id))
				.filter(|c| c.kind != 4)
			{
				crumbs.push(format!("#{}", parent.name));
			}
			crumbs.push(format!("#{}", channel.name));
			(guild.name.clone(), crumbs)
		}
		None => {
			conversation_avatar(solace, ui.painter(), icon, state, channel, p);
			let mut crumbs = vec![if channel.kind == 3 {
				"Group".to_owned()
			} else {
				"Direct message".to_owned()
			}];
			if let Some(peer) = channel.recipients.first().filter(|_| channel.kind == 1) {
				crumbs.push(paint::status_label(nav::status_of(state, peer.id)).to_owned());
			} else {
				crumbs.push(format!("{} members", channel.recipients.len() + 1));
			}
			(conversation_title(state, channel), crumbs)
		}
	};
	let text_left = icon.right() + 14.0;
	let buttons_left = rect.right() - 24.0 - 3.0 * 44.0 - 2.0 * 10.0;
	let max = buttons_left - text_left - 16.0;
	paint::text_left(
		ui.painter(),
		pos2(text_left, rect.top() + 32.0),
		&title,
		theme::serif(19.5),
		p.text,
		max,
	);
	paint::text_left(
		ui.painter(),
		pos2(text_left, rect.top() + 53.0),
		&crumbs.join("  /  "),
		theme::sans(12.5),
		p.muted,
		max,
	);

	let mut x = rect.right() - 24.0 - 22.0;
	let y = rect.top() + 41.0;
	let more = paint::icon_button(
		ui,
		ui.scope_id().with("hdr-more"),
		Rect::from_center_size(pos2(x, y), vec2(44.0, 44.0)),
		Icon::More,
		ButtonKind::Soft,
		solace.more_menu,
		"More",
	);
	if more.clicked() {
		solace.more_menu = !solace.more_menu;
	}
	x -= 54.0;
	let call_active = state
		.voice
		.active
		.as_ref()
		.is_some_and(|c| c.channel == channel.id);
	let callable = state.can_call(channel.id);
	let voice_target = match channel.guild {
		Some(guild) => nav::voice_rooms(state, guild).first().map(|c| c.id),
		None => callable.then_some(channel.id),
	};
	let label = if call_active {
		"Leave call"
	} else if channel.guild.is_some() {
		"Join voice room"
	} else {
		"Start a call"
	};
	let call = paint::icon_button(
		ui,
		ui.scope_id().with("hdr-call"),
		Rect::from_center_size(pos2(x, y), vec2(44.0, 44.0)),
		if call_active {
			Icon::PhoneOff
		} else {
			Icon::Video
		},
		if call_active {
			ButtonKind::Danger
		} else {
			ButtonKind::Soft
		},
		false,
		label,
	);
	if call.clicked() {
		if call_active {
			commands.extend(state.leave_call());
		} else if let Some(target) = voice_target
			&& let Some(command) = state.start_call(target, channel.guild.is_none())
		{
			commands.push(command);
		} else {
			solace
				.toasts
				.push(crate::Level::Info, "No voice room is available here");
		}
	}
	x -= 54.0;
	let people = paint::icon_button(
		ui,
		ui.scope_id().with("hdr-people"),
		Rect::from_center_size(pos2(x, y), vec2(44.0, 44.0)),
		Icon::People,
		ButtonKind::Soft,
		solace.aside_open,
		if solace.aside_open {
			"Hide details"
		} else {
			"Show details"
		},
	);
	if people.clicked() {
		solace.aside_open = !solace.aside_open;
	}
	if solace.more_menu {
		more_menu(solace, ui, state, commands, channel, more.rect);
	}
}

fn title_case(name: &str) -> String {
	let lower = name.to_lowercase();
	let mut chars = lower.chars();
	chars
		.next()
		.map(|c| c.to_uppercase().chain(chars).collect())
		.unwrap_or_default()
}

fn more_menu(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &model::Channel,
	anchor: Rect,
) {
	let p = theme::palette(ui.ctx());
	let mut items: Vec<(Icon, &str, u8)> = Vec::new();
	if state.can_mark_channel_read(channel.id) {
		items.push((Icon::Check, "Mark as read", 0));
	}
	if let Some(guild) = channel.guild
		&& state.can_mark_guild_read(guild)
	{
		items.push((Icon::Check, "Mark community as read", 1));
	}
	items.push((Icon::Thread, "Jump to latest", 2));
	items.push((Icon::Settings, "Settings", 3));
	let width = 230.0;
	let height = items.len() as f32 * 38.0 + 12.0;
	let rect = Rect::from_min_size(
		pos2(anchor.right() - width, anchor.bottom() + 8.0),
		vec2(width, height),
	);
	let mut chosen = None;
	let area = egui::Area::new(egui::Id::unique("more-menu"))
		.order(egui::Order::Foreground)
		.fixed_pos(rect.min)
		.show(ui.ctx(), |ui| {
			let (_, _) = ui.allocate_exact_size(rect.size(), Sense::hover());
			paint::floating(ui.painter(), rect, 14, &p);
			for (i, (icon, label, key)) in items.iter().enumerate() {
				let row = Rect::from_min_size(
					rect.min + vec2(6.0, 6.0 + i as f32 * 38.0),
					vec2(width - 12.0, 36.0),
				);
				let response = ui.interact(row, ui.scope_id().with(("more", i)), Sense::click());
				if response.hovered() {
					ui.painter().rect_filled(row, 9, p.hover);
				}
				icons::paint_at(
					ui.painter(),
					*icon,
					row.left_center() + vec2(18.0, 0.0),
					15.0,
					p.text_soft,
				);
				paint::label(
					ui.painter(),
					row.left_center() + vec2(36.0, 0.0),
					Align2::LEFT_CENTER,
					label,
					theme::sans(13.5),
					p.text,
				);
				if response.clicked() {
					chosen = Some(*key);
				}
			}
		});
	let clicked_outside = ui.input(|i| i.pointer.any_click())
		&& !area
			.response
			.rect
			.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()))
		&& !anchor.contains(ui.input(|i| i.pointer.interact_pos().unwrap_or_default()));
	match chosen {
		Some(0) => commands.extend(state.prepare_mark_channel_read(channel.id)),
		Some(1) => commands.extend(channel.guild.and_then(|g| state.prepare_mark_guild_read(g))),
		Some(2) => solace.scroll.jump_to_bottom = true,
		Some(3) => solace.settings = Some(crate::SettingsPage::Appearance),
		_ => {}
	}
	if chosen.is_some() || clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
		solace.more_menu = false;
	}
}

#[derive(Clone, Debug)]
enum Row {
	Hero,
	Intro,
	Loading,
	Day(String),
	Message { id: Id, continuation: bool },
	Pending(usize),
}

fn same_group(previous: &Message, message: &Message) -> bool {
	previous.author.id == message.author.id
		&& message.reply_to.is_none()
		&& !message.is_system()
		&& !previous.is_system()
		&& (clock::snowflake(message.id) - clock::snowflake(previous.id)).whole_minutes()
			< GROUP_WINDOW_MINUTES
}

fn build_rows(state: &State, channel: &model::Channel, with_hero: bool) -> Vec<Row> {
	let mut rows = Vec::new();
	if with_hero && channel.guild.is_some() {
		rows.push(Row::Hero);
	}
	if state.older_exhausted {
		rows.push(Row::Intro);
	} else if state.history_pending || state.can_load_older() {
		rows.push(Row::Loading);
	}
	let mut previous: Option<&Message> = None;
	for message in state.timeline.display_iter() {
		let day = clock::day(clock::snowflake(message.id));
		let new_day = previous.is_none_or(|prev| clock::day(clock::snowflake(prev.id)) != day);
		if new_day {
			rows.push(Row::Day(day));
		}
		let continuation = !new_day && previous.is_some_and(|prev| same_group(prev, message));
		rows.push(Row::Message {
			id: message.id,
			continuation,
		});
		previous = Some(message);
	}
	for (index, pending) in state.pending.iter().enumerate() {
		if pending.channel == channel.id
			&& pending.confirmed.is_none()
			&& pending.delivery != Delivery::Confirmed
		{
			rows.push(Row::Pending(index));
		}
	}
	rows
}

/// Count of replies to each message in the loaded window, for the thread affordance.
fn reply_counts(state: &State) -> HashMap<Id, usize> {
	let mut counts = HashMap::new();
	for message in state.timeline.display_iter() {
		if let Some(target) = message.reply_to {
			*counts.entry(target).or_insert(0) += 1;
		}
	}
	counts
}

#[allow(clippy::too_many_arguments)]
fn timeline_view(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &model::Channel,
	rect: Rect,
	with_hero: bool,
	p: &Palette,
) {
	let rows = build_rows(state, channel, with_hero);
	let replies = reply_counts(state);
	let width = (rect.width() - 16.0).max(200.0);
	let mut child = ui.new_child(
		egui::UiBuilder::new()
			.max_rect(rect)
			.id_salt(("timeline", channel.id)),
	);
	child.set_clip_rect(rect);

	// Measure every row (cached by message revision), then paint only the visible ones.
	let mut heights = Vec::with_capacity(rows.len());
	for row in &rows {
		heights.push(row_height(solace, &child, state, row, width, &replies));
	}
	let total: f32 = heights.iter().sum::<f32>() + 20.0;

	let mut area = egui::ScrollArea::vertical()
		.id_salt(("timeline-scroll", channel.id))
		.auto_shrink([false, false])
		.stick_to_bottom(true);
	if std::mem::take(&mut solace.scroll.jump_to_bottom) {
		area = area.vertical_scroll_offset(total.max(0.0));
	}
	// Keep the reading position when an older page is prepended above it.
	if let Some((anchor, old_total)) = solace.scroll.anchor
		&& state.timeline.row_ids().next() != Some(anchor)
		&& !state.history_pending
	{
		let grown = total - old_total;
		if grown > 0.0 {
			area = area.vertical_scroll_offset(solace.scroll.offset + grown);
		}
		solace.scroll.anchor = None;
	}
	let mut hovered_message = None;
	let output = area.show_viewport(&mut child, |ui, viewport| {
		ui.set_height(total);
		ui.set_width(width);
		let origin = ui.min_rect().min;
		let mut y = 0.0;
		for (row, height) in rows.iter().zip(&heights) {
			let row_rect = Rect::from_min_size(origin + vec2(0.0, y), vec2(width, *height));
			if y + height >= viewport.min.y - 200.0
				&& y <= viewport.max.y + 200.0
				&& let Some(id) = paint_row(solace, ui, state, commands, row, row_rect, &replies, p)
			{
				hovered_message = Some(id);
			}
			y += height;
		}
	});
	// Rows fade out beneath the top edge instead of being cut by it.
	if output.state.offset.y > 1.0 {
		let fade = Rect::from_min_size(rect.min, vec2(rect.width(), 22.0));
		paint::gradient(
			ui.painter(),
			fade,
			0.0,
			&[(0.0, p.canvas), (1.0, p.canvas.gamma_multiply(0.0))],
		);
	}
	solace.scroll.offset = output.state.offset.y;
	solace.scroll.content_height = total;
	let viewport_h = output.inner_rect.height();
	solace.scroll.at_bottom = output.state.offset.y + viewport_h >= total - 24.0;

	// Older history when the reader nears the top.
	let first = state.timeline.row_ids().next();
	let latest = state.timeline.row_ids().next_back();
	if output.state.offset.y < 400.0
		&& !state.history_pending
		&& state.can_load_older()
		&& solace.scroll.anchor.is_none()
		&& let Some(first) = first
		&& let Some(command) = state.older_history()
	{
		solace.scroll.anchor = Some((first, total));
		commands.push(command);
	}
	// Newer history when a window opened mid-conversation reaches its end.
	if solace.scroll.at_bottom && state.can_load_newer() && !state.history_pending {
		commands.extend(state.newer_history());
	}
	// Acknowledge the latest message once it's on screen and the window is focused.
	if solace.scroll.at_bottom
		&& ui.ctx().input(|i| i.focused)
		&& let Some(latest) = latest
		&& solace.last_read_request != Some(latest)
		&& state.can_mark_read(latest)
	{
		solace.last_read_request = Some(latest);
		commands.extend(state.prepare_mark_read(latest));
	}
	if !solace.scroll.at_bottom && total > viewport_h * 1.6 {
		let button = Rect::from_center_size(
			pos2(rect.center().x, rect.bottom() - 26.0),
			vec2(150.0, 34.0),
		);
		if paint::pill_button(
			ui,
			ui.scope_id().with("jump-latest"),
			button,
			Some(Icon::ChevronDown),
			"Jump to latest",
			ButtonKind::Ghost,
			true,
		)
		.clicked()
		{
			solace.scroll.jump_to_bottom = true;
		}
	}
	let _ = hovered_message;
}

fn names<'a>(state: &'a State, message: &'a Message) -> Names<'a> {
	Names {
		state,
		guild: state.channel(message.channel).and_then(|c| c.guild),
		mentions: &message.mentions,
	}
}

fn row_height(
	solace: &mut Solace,
	ui: &Ui,
	state: &State,
	row: &Row,
	width: f32,
	replies: &HashMap<Id, usize>,
) -> f32 {
	match row {
		Row::Hero => HERO_HEIGHT + 26.0,
		Row::Intro => 86.0,
		Row::Loading => 54.0,
		Row::Day(_) => 44.0,
		Row::Pending(index) => {
			let Some(pending) = state.pending.get(*index) else {
				return 0.0;
			};
			let style = Style::body(&theme::palette(ui.ctx()));
			let laid = text::layout(
				ui.painter(),
				&pending.content,
				&Names {
					state,
					guild: None,
					mentions: &[],
				},
				&style,
				(width - GUTTER).min(MAX_TEXT_WIDTH),
				true,
			);
			laid.size.y
				+ 14.0 + if pending.attachments.is_empty() {
				0.0
			} else {
				40.0
			}
		}
		Row::Message { id, continuation } => {
			let Some(message) = state.timeline.get_display(*id) else {
				return 0.0;
			};
			let key = cache_key(
				solace,
				message,
				*continuation,
				width,
				replies.get(id).copied().unwrap_or(0),
				state,
			);
			if let Some((cached, height)) = solace.heights.get(id)
				&& *cached == key
			{
				return *height;
			}
			let height = layout_message(
				solace,
				ui.painter(),
				state,
				message,
				*continuation,
				width,
				replies.get(id).copied().unwrap_or(0),
				None,
			)
			.height;
			if solace.heights.len() > 2048 {
				solace.heights.clear();
			}
			solace.heights.insert(*id, (key, height));
			height
		}
	}
}

fn cache_key(
	solace: &Solace,
	message: &Message,
	continuation: bool,
	width: f32,
	replies: usize,
	state: &State,
) -> u64 {
	use std::hash::{Hash, Hasher};
	let mut hasher = std::collections::hash_map::DefaultHasher::new();
	message.revision.hash(&mut hasher);
	message.content.hash(&mut hasher);
	continuation.hash(&mut hasher);
	(width as u32).hash(&mut hasher);
	replies.hash(&mut hasher);
	solace.revealed.contains(&message.id).hash(&mut hasher);
	state
		.reactions
		.display(message)
		.map(|r| r.len())
		.hash(&mut hasher);
	solace
		.editing
		.as_ref()
		.map(|(id, _)| *id == message.id)
		.hash(&mut hasher);
	message.attachments.len().hash(&mut hasher);
	hasher.finish()
}

struct MessageLayout {
	height: f32,
}

#[derive(Clone)]
enum Media {
	Image {
		key: Option<String>,
		size: egui::Vec2,
		alt: String,
		attachment: Option<model::Attachment>,
	},
	File(model::Attachment),
}

fn media_of(message: &Message) -> Vec<Media> {
	let mut media = Vec::new();
	for attachment in &message.attachments {
		if attachment.is_image() && !attachment.spoiler {
			media.push(Media::Image {
				key: images::media_key(&attachment.media),
				size: vec2(
					attachment.media.width.max(1) as f32,
					attachment.media.height.max(1) as f32,
				),
				alt: attachment.filename.clone(),
				attachment: Some(attachment.clone()),
			});
		} else {
			media.push(Media::File(attachment.clone()));
		}
	}
	if !message.embeds_suppressed {
		for embed in &message.embeds {
			if let Some(image) = embed.image.as_ref().or(embed.thumbnail.as_ref())
				&& embed.title.is_none()
				&& embed.description.is_none()
			{
				media.push(Media::Image {
					key: images::media_key(image),
					size: vec2(image.width.max(1) as f32, image.height.max(1) as f32),
					alt: embed.url.clone().unwrap_or_default(),
					attachment: None,
				});
			}
		}
	}
	media
}

fn is_audio(attachment: &model::Attachment) -> bool {
	attachment.is_audio() || attachment.is_voice_message()
}

/// Rects for each media item (by index), and the total height used.
fn media_rects(media: &[Media], left: f32, top: f32, width: f32) -> (Vec<(usize, Rect)>, f32) {
	let images: Vec<usize> = media
		.iter()
		.enumerate()
		.filter(|(_, m)| matches!(m, Media::Image { .. }))
		.map(|(i, _)| i)
		.collect();
	let mut rects = Vec::new();
	let mut y = top;
	if images.len() == 1 {
		if let Media::Image { size, .. } = &media[images[0]] {
			let max = vec2(width.min(400.0), 280.0);
			let scale = (max.x / size.x).min(max.y / size.y).min(1.0);
			let s = (*size * scale).max(vec2(120.0, 80.0));
			rects.push((images[0], Rect::from_min_size(pos2(left, y), s)));
			y += s.y + 8.0;
		}
	} else if !images.is_empty() {
		// A mood-board strip of equal tiles.
		let tile = vec2(88.0, 68.0) * if images.len() <= 2 { 1.6 } else { 1.0 };
		let per_row = ((width + 6.0) / (tile.x + 6.0)).floor().max(1.0) as usize;
		for (n, index) in images.iter().enumerate() {
			let col = n % per_row;
			let row = n / per_row;
			rects.push((
				*index,
				Rect::from_min_size(
					pos2(
						left + col as f32 * (tile.x + 6.0),
						y + row as f32 * (tile.y + 6.0),
					),
					tile,
				),
			));
		}
		let rows = images.len().div_ceil(per_row);
		y += rows as f32 * (tile.y + 6.0) + 2.0;
	}
	for (index, m) in media.iter().enumerate() {
		if let Media::File(attachment) = m {
			let w = if is_audio(attachment) { 360.0 } else { 320.0 };
			rects.push((
				index,
				Rect::from_min_size(pos2(left, y), vec2(width.min(w), 54.0)),
			));
			y += 60.0;
		}
	}
	(rects, y - top)
}

#[allow(clippy::too_many_arguments)]
fn layout_message(
	solace: &mut Solace,
	painter: &egui::Painter,
	state: &State,
	message: &Message,
	continuation: bool,
	width: f32,
	replies: usize,
	mut paint_at: Option<(&mut Ui, &mut Vec<Deferred>, Pos2)>,
) -> MessageLayout {
	let p = theme::palette(painter.ctx());
	let content_left = GUTTER;
	let content_w = (width - GUTTER - 8.0).min(MAX_TEXT_WIDTH);
	let mut y = if continuation { 2.0 } else { 12.0 };

	if message.is_system() {
		let summary = message
			.system_summary()
			.unwrap_or_else(|| message.display_text().into_owned());
		if let Some((ui, _, origin)) = paint_at.as_mut() {
			let pos = *origin + vec2(content_left, y + 10.0);
			icons::paint_at(
				ui.painter(),
				Icon::Sparkle,
				pos - vec2(26.0, 0.0),
				14.0,
				p.muted,
			);
			paint::text_left(
				ui.painter(),
				pos,
				&summary,
				theme::sans(13.0),
				p.muted,
				content_w,
			);
			paint::label(
				ui.painter(),
				pos2(origin.x + width - 8.0, pos.y),
				Align2::RIGHT_CENTER,
				&clock::clock(clock::snowflake(message.id)),
				theme::sans(11.0),
				p.faint,
			);
		}
		return MessageLayout { height: y + 28.0 };
	}

	// Reply context line.
	if let Some(target) = message.reply_to {
		if let Some((ui, _, origin)) = paint_at.as_mut() {
			let line_y = origin.y + y + 9.0;
			let start = pos2(origin.x + AVATAR / 2.0, line_y + 10.0);
			ui.painter().add(egui::Shape::line(
				vec![
					start,
					start - vec2(0.0, 6.0),
					start + vec2(4.0, -10.0),
					pos2(origin.x + content_left - 6.0, line_y),
				],
				Stroke::new(1.5, p.border),
			));
			let (label, preview) = match state.timeline.get_display(target) {
				Some(parent) => (
					display_name(state, &parent.author),
					text::preview(&parent.display_text(), &names(state, parent), 80),
				),
				None if message.reply_deleted => {
					(String::new(), "Original message was deleted".into())
				}
				None => (String::new(), "Earlier message".into()),
			};
			let mut x = origin.x + content_left;
			if !label.is_empty() {
				let r = paint::label(
					ui.painter(),
					pos2(x, line_y),
					Align2::LEFT_CENTER,
					&format!("@{label}"),
					theme::medium(12.0),
					p.text_soft,
				);
				x = r.right() + 6.0;
			}
			paint::text_left(
				ui.painter(),
				pos2(x, line_y),
				&preview,
				theme::sans(12.0),
				p.muted,
				origin.x + content_left + content_w - x,
			);
		}
		y += 20.0;
	}

	let header_y = y;
	if !continuation {
		if let Some((ui, _, origin)) = paint_at.as_mut() {
			let avatar =
				Rect::from_min_size(*origin + vec2(0.0, header_y + 2.0), vec2(AVATAR, AVATAR));
			let texture = solace.images.get(&message.author.avatar_key());
			paint::avatar(
				ui.painter(),
				avatar,
				AvatarShape::Circle,
				texture,
				&message.author.name,
				message.author.id.0,
				&p,
			);
			let name = state.message_author_name(message).to_owned();
			let color = state
				.message_author_color(message)
				.filter(|c| *c != 0)
				.map(|c| {
					mix(
						Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8),
						p.text,
						if p.dark { 0.2 } else { 0.35 },
					)
				})
				.unwrap_or(p.text);
			let name_rect = paint::label(
				ui.painter(),
				*origin + vec2(content_left, header_y + 10.0),
				Align2::LEFT_CENTER,
				&name,
				theme::semibold(14.0),
				color,
			);
			let mut x = name_rect.right() + 10.0;
			if let Some(tag) = message.author.account_label() {
				let chip =
					Rect::from_min_size(pos2(x, name_rect.center().y - 8.0), vec2(34.0, 16.0));
				ui.painter().rect_filled(chip, 5, p.accent_soft);
				paint::label(
					ui.painter(),
					chip.center(),
					Align2::CENTER_CENTER,
					tag,
					theme::semibold(9.0),
					p.accent,
				);
				x = chip.right() + 8.0;
			}
			let at = clock::snowflake(message.id);
			let time = paint::label(
				ui.painter(),
				pos2(x, name_rect.center().y + 0.5),
				Align2::LEFT_CENTER,
				&clock::clock(at),
				theme::sans(11.5),
				p.faint,
			);
			let hover = ui.interact(
				time,
				ui.scope_id().with(("time", message.id)),
				Sense::hover(),
			);
			hover.on_hover_text(format!("{} at {}", clock::day(at), clock::clock(at)));
		}
		y += 24.0;
	}

	// Body text, or the inline editor.
	let editing = solace
		.editing
		.as_ref()
		.is_some_and(|(id, _)| *id == message.id);
	if editing {
		if let Some((ui, commands, origin)) = paint_at.as_mut() {
			let rect = Rect::from_min_size(*origin + vec2(content_left, y), vec2(content_w, 74.0));
			edit_box(solace, ui, commands, message, rect);
		}
		y += 82.0;
	} else {
		let content = message.display_text();
		if !content.trim().is_empty() {
			let style = Style::body(&p);
			let revealed = solace.revealed.contains(&message.id);
			let laid = text::layout(
				painter,
				&content,
				&names(state, message),
				&style,
				content_w,
				revealed,
			);
			if let Some((ui, _, origin)) = paint_at.as_mut() {
				let at = *origin + vec2(content_left, y);
				let interaction = text::paint(
					ui,
					at,
					&laid,
					&mut solace.images,
					ui.scope_id().with(("text", message.id)),
				);
				if let Some(link) = interaction.link {
					solace.link = Some(link);
				}
				if interaction.spoiler_clicked {
					solace.revealed.insert(message.id);
				}
				if message.edited {
					paint::label(
						ui.painter(),
						at + vec2(laid.size.x + 6.0, laid.size.y - 11.0),
						Align2::LEFT_CENTER,
						"(edited)",
						theme::sans(10.5),
						p.faint,
					);
				}
			}
			y += laid.size.y;
		}
	}

	// Stickers render as a labelled chip (artwork may be animated or Lottie).
	for sticker in &message.sticker_items {
		if let Some((ui, _, origin)) = paint_at.as_mut() {
			let rect =
				Rect::from_min_size(*origin + vec2(content_left, y + 6.0), vec2(180.0, 34.0));
			ui.painter().rect(
				rect,
				10,
				p.surface,
				Stroke::new(1.0, p.border),
				egui::StrokeKind::Inside,
			);
			icons::paint_at(
				ui.painter(),
				Icon::Sparkle,
				rect.left_center() + vec2(16.0, 0.0),
				14.0,
				p.accent,
			);
			paint::text_left(
				ui.painter(),
				rect.left_center() + vec2(32.0, 0.0),
				&sticker.name,
				theme::medium(12.5),
				p.text,
				140.0,
			);
		}
		y += 42.0;
	}

	// Rich embeds with text become compact cards.
	if !message.embeds_suppressed {
		for embed in message
			.embeds
			.iter()
			.filter(|e| e.title.is_some() || e.description.is_some())
			.take(3)
		{
			let title = embed.title.clone().unwrap_or_default();
			let description = embed.description.clone().unwrap_or_default();
			let card_w = content_w.min(460.0);
			let desc = painter.layout(
				description.chars().take(300).collect(),
				theme::sans(12.5),
				p.text_soft,
				card_w - 32.0,
			);
			let h = 24.0 + if title.is_empty() { 0.0 } else { 22.0 } + desc.size().y + 4.0;
			if let Some((ui, _, origin)) = paint_at.as_mut() {
				let rect =
					Rect::from_min_size(*origin + vec2(content_left, y + 6.0), vec2(card_w, h));
				paint::card(ui.painter(), rect, 12, &p);
				let accent = embed.color.map_or(p.accent, |c| {
					Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
				});
				ui.painter().rect_filled(
					Rect::from_min_size(rect.min, vec2(4.0, rect.height())),
					egui::CornerRadius {
						nw: 12,
						sw: 12,
						ne: 0,
						se: 0,
					},
					accent,
				);
				let mut ty = rect.top() + 12.0;
				if !title.is_empty() {
					paint::text_left(
						ui.painter(),
						pos2(rect.left() + 18.0, ty + 9.0),
						&title,
						theme::semibold(13.5),
						p.text,
						card_w - 32.0,
					);
					ty += 22.0;
				}
				ui.painter()
					.galley(pos2(rect.left() + 18.0, ty), desc, p.text_soft);
				if let Some(url) = embed.url.clone() {
					let response = ui.interact(
						rect,
						ui.scope_id().with(("embed", message.id, title.len())),
						Sense::click(),
					);
					if response.clicked() {
						solace.link = Some(url);
					}
				}
			}
			y += h + 8.0;
		}
	}

	// Attachments and image embeds.
	let media = media_of(message);
	if !media.is_empty() {
		y += 8.0;
		let left = paint_at.as_ref().map_or(0.0, |(_, _, o)| o.x) + content_left;
		let top = paint_at.as_ref().map_or(0.0, |(_, _, o)| o.y) + y;
		let (rects, h) = media_rects(&media, left, top, content_w);
		if let Some((ui, _, _)) = paint_at.as_mut() {
			for (index, rect) in &rects {
				let rect = *rect;
				match &media[*index] {
					Media::Image {
						key,
						alt,
						attachment,
						..
					} => {
						match key.as_deref().and_then(|k| solace.images.get(k)) {
							Some((texture, size)) => {
								paint::cover_image(ui.painter(), rect, 12, texture, size)
							}
							None => {
								paint::gradient(
									ui.painter(),
									rect,
									12.0,
									&[(0.0, p.surface_alt), (1.0, p.hover)],
								);
								icons::paint_at(
									ui.painter(),
									Icon::Image,
									rect.center(),
									22.0,
									p.faint,
								);
							}
						}
						let response = ui
							.interact(
								rect,
								ui.scope_id().with(("img", message.id, *index)),
								Sense::hover(),
							)
							.on_hover_text(alt.as_str());
						if response.hovered()
							&& let Some(attachment) = attachment
						{
							let button = Rect::from_center_size(
								rect.right_top() + vec2(-20.0, 20.0),
								vec2(30.0, 30.0),
							);
							ui.painter().circle_filled(
								button.center(),
								16.0,
								egui::Color32::from_black_alpha(120),
							);
							let save = ui
								.interact(
									button,
									ui.scope_id().with(("img-save", message.id, *index)),
									Sense::click(),
								)
								.on_hover_text("Download");
							icons::paint_at(
								ui.painter(),
								Icon::Download,
								button.center(),
								16.0,
								egui::Color32::WHITE,
							);
							if save.clicked() {
								solace.request(crate::Request::Download(attachment.clone()));
							}
						}
					}
					Media::File(attachment) if is_audio(attachment) => {
						audio_card(solace, ui, message.id, attachment, rect, &p);
					}
					Media::File(attachment) => {
						let response = ui
							.interact(
								rect,
								ui.scope_id().with(("file", message.id, *index)),
								Sense::click(),
							)
							.on_hover_text("Download");
						let t = paint::hover_t(ui, &response);
						ui.painter().rect(
							rect,
							12,
							mix(p.surface, p.hover, t),
							Stroke::new(1.0, p.border),
							egui::StrokeKind::Inside,
						);
						let badge = Rect::from_center_size(
							rect.left_center() + vec2(28.0, 0.0),
							vec2(34.0, 34.0),
						);
						ui.painter().rect_filled(badge, 9, p.accent_soft);
						icons::paint_at(ui.painter(), Icon::File, badge.center(), 17.0, p.accent);
						paint::text_left(
							ui.painter(),
							rect.left_center() + vec2(54.0, -8.0),
							&attachment.filename,
							theme::medium(13.0),
							p.text,
							rect.width() - 100.0,
						);
						paint::text_left(
							ui.painter(),
							rect.left_center() + vec2(54.0, 10.0),
							&human_size(attachment.size),
							theme::sans(11.5),
							p.muted,
							rect.width() - 100.0,
						);
						icons::paint_at(
							ui.painter(),
							Icon::Download,
							rect.right_center() - vec2(24.0, 0.0),
							17.0,
							mix(p.muted, p.accent, t),
						);
						if response.clicked() {
							solace.request(crate::Request::Download(attachment.clone()));
						}
					}
				}
			}
		}
		y += h;
	}

	// Reactions.
	let reactions: Vec<model::Reaction> = state
		.reactions
		.display(message)
		.map(<[_]>::to_vec)
		.unwrap_or_default();
	if !reactions.is_empty() {
		y += 6.0;
		if let Some((ui, commands, origin)) = paint_at.as_mut() {
			let mut x = origin.x + content_left;
			let row_y = origin.y + y;
			for (i, reaction) in reactions.iter().enumerate() {
				let count = reaction.count.to_string();
				let count_galley = ui.painter().layout_no_wrap(
					count,
					theme::medium(12.0),
					if reaction.me { p.accent } else { p.text_soft },
				);
				let w = 26.0 + count_galley.size().x + 16.0;
				let chip = Rect::from_min_size(pos2(x, row_y), vec2(w, 28.0));
				let response = ui.interact(
					chip,
					ui.scope_id().with(("reaction", message.id, i)),
					Sense::click(),
				);
				let t = paint::hover_t(ui, &response);
				let fill = if reaction.me {
					p.accent_soft
				} else {
					mix(p.surface, p.hover, t)
				};
				let stroke = if reaction.me {
					p.accent.gamma_multiply(0.45)
				} else {
					p.border
				};
				ui.painter().rect(
					chip,
					14,
					fill,
					Stroke::new(1.0, stroke),
					egui::StrokeKind::Inside,
				);
				let emoji_rect =
					Rect::from_center_size(chip.left_center() + vec2(17.0, 0.0), vec2(16.0, 16.0));
				match (reaction.emoji.id, reaction.emoji.name.as_deref()) {
					(Some(id), _) => {
						if let Some((texture, _)) = solace.images.get(&images::emoji_key(id)) {
							ui.painter().image(
								texture,
								emoji_rect,
								Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
								Color32::WHITE,
							);
						}
					}
					(None, Some(name)) if !crate::emoji::paint(ui.painter(), emoji_rect, name) => {
						paint::label(
							ui.painter(),
							emoji_rect.center(),
							Align2::CENTER_CENTER,
							name,
							theme::sans(13.0),
							p.text,
						);
					}
					_ => {}
				}
				ui.painter().galley(
					pos2(
						emoji_rect.right() + 6.0,
						chip.center().y - count_galley.size().y / 2.0,
					),
					count_galley,
					p.text_soft,
				);
				let response = response.on_hover_text(reaction.emoji.label());
				if response.clicked() && reaction.emoji.name.is_some() {
					commands.push(Deferred::React(message.id, reaction.emoji.clone()));
				}
				x = chip.right() + 6.0;
			}
			let add = Rect::from_min_size(pos2(x, row_y), vec2(34.0, 28.0));
			let response = ui
				.interact(
					add,
					ui.scope_id().with(("react-add", message.id)),
					Sense::click(),
				)
				.on_hover_text("Add reaction");
			let t = paint::hover_t(ui, &response);
			ui.painter().rect(
				add,
				14,
				mix(p.surface, p.hover, t),
				Stroke::new(1.0, p.border),
				egui::StrokeKind::Inside,
			);
			icons::paint_at(ui.painter(), Icon::Smile, add.center(), 15.0, p.muted);
			if response.clicked() {
				solace.picker = Some((PickerTarget::Reaction(message.id), add.left_top()));
			}
		}
		y += 30.0;
	}

	if replies > 0 {
		y += 4.0;
		if let Some((ui, _, origin)) = paint_at.as_mut() {
			let label = if replies == 1 {
				"1 reply".to_owned()
			} else {
				format!("{replies} replies")
			};
			let rect = Rect::from_min_size(*origin + vec2(content_left, y), vec2(110.0, 22.0));
			let response = ui.interact(
				rect,
				ui.scope_id().with(("replies", message.id)),
				Sense::click(),
			);
			let color = if response.hovered() {
				p.accent_hover
			} else {
				p.accent
			};
			icons::paint_at(
				ui.painter(),
				Icon::Thread,
				rect.left_center() + vec2(8.0, 0.0),
				14.0,
				color,
			);
			paint::label(
				ui.painter(),
				rect.left_center() + vec2(22.0, 0.0),
				Align2::LEFT_CENTER,
				&label,
				theme::medium(12.5),
				color,
			);
			if response.clicked() {
				solace.thread = Some(message.id);
			}
		}
		y += 24.0;
	}

	let min = if continuation { 0.0 } else { AVATAR + 14.0 };
	MessageLayout {
		height: y.max(min) + 6.0,
	}
}

/// State changes requested while painting a message, applied once painting is done.
pub(crate) enum Deferred {
	React(Id, model::ReactionEmoji),
	Edit(Id, Id, String),
}

pub(crate) fn apply_deferred(
	state: &mut State,
	commands: &mut Vec<Command>,
	deferred: Vec<Deferred>,
) {
	for action in deferred {
		match action {
			Deferred::React(message, emoji) => {
				commands.extend(state.prepare_reaction(message, emoji))
			}
			Deferred::Edit(channel, message, content) => {
				commands.extend(state.prepare_edit(channel, message, content))
			}
		}
	}
}

fn clock_text(seconds: f32) -> String {
	let seconds = seconds.max(0.0) as u64;
	format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn audio_card(
	solace: &mut Solace,
	ui: &mut Ui,
	message: Id,
	attachment: &model::Attachment,
	rect: Rect,
	p: &Palette,
) {
	ui.painter().rect(
		rect,
		14,
		p.surface,
		Stroke::new(1.0, p.border),
		egui::StrokeKind::Inside,
	);
	let active = solace.audio.active == Some(attachment.id);
	let playing = active && solace.audio.playing;
	let button = Rect::from_center_size(rect.left_center() + vec2(28.0, 0.0), vec2(36.0, 36.0));
	let response = paint::icon_button(
		ui,
		ui.scope_id().with(("audio", message, attachment.id)),
		button,
		if playing { Icon::Pause } else { Icon::Play },
		ButtonKind::Primary,
		false,
		if playing { "Pause" } else { "Play" },
	);
	if response.clicked() {
		let request = if active && !solace.audio.ended {
			crate::Request::PauseAudio(playing)
		} else {
			crate::Request::PlayAudio(attachment.clone())
		};
		solace.request(request);
	}
	let left = button.right() + 12.0;
	let bar = Rect::from_min_size(
		pos2(left, rect.center().y + 6.0),
		vec2(rect.right() - left - 70.0, 4.0),
	);
	let title = if attachment.is_voice_message() {
		"Voice message"
	} else {
		attachment.filename.as_str()
	};
	paint::text_left(
		ui.painter(),
		pos2(left, rect.center().y - 9.0),
		title,
		theme::medium(12.5),
		p.text,
		bar.width(),
	);
	ui.painter().rect_filled(bar, 2, p.surface_alt);
	let duration = if active && solace.audio.duration > 0.0 {
		solace.audio.duration
	} else {
		attachment.duration_ms.map_or(0.0, |ms| ms as f32 / 1000.0)
	};
	let position = if active { solace.audio.position } else { 0.0 };
	if duration > 0.0 {
		let fill = Rect::from_min_size(
			bar.min,
			vec2(
				bar.width() * (position / duration).clamp(0.0, 1.0),
				bar.height(),
			),
		);
		ui.painter().rect_filled(fill, 2, p.accent);
		let seek = ui.interact(
			bar.expand2(vec2(0.0, 8.0)),
			ui.scope_id().with(("seek", attachment.id)),
			Sense::click(),
		);
		if seek.clicked()
			&& active && let Some(pos) = seek.interact_pointer_pos()
		{
			let fraction = ((pos.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
			solace.request(crate::Request::SeekAudio(fraction * duration));
		}
	}
	let label = match (active, solace.audio.error) {
		(true, Some(error)) => error.to_owned(),
		(true, None) if solace.audio.loading => "Loading".into(),
		_ => format!("{} / {}", clock_text(position), clock_text(duration)),
	};
	paint::text_left(
		ui.painter(),
		pos2(bar.right() + 10.0, rect.center().y + 6.0),
		&label,
		theme::sans(11.0),
		p.muted,
		64.0,
	);
	if playing {
		ui.ctx()
			.request_repaint_after(std::time::Duration::from_millis(200));
	}
}

pub(crate) fn human_size(bytes: u64) -> String {
	match bytes {
		0..=1023 => format!("{bytes} B"),
		1024..=1_048_575 => format!("{:.1} KB", bytes as f64 / 1024.0),
		_ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
	}
}

fn edit_box(
	solace: &mut Solace,
	ui: &mut Ui,
	deferred: &mut Vec<Deferred>,
	message: &Message,
	rect: Rect,
) {
	let p = theme::palette(ui.ctx());
	ui.painter().rect(
		rect,
		12,
		p.surface,
		Stroke::new(1.0, p.accent.gamma_multiply(0.5)),
		egui::StrokeKind::Inside,
	);
	let Some((_, draft)) = solace.editing.as_mut() else {
		return;
	};
	let id = ui.scope_id().with(("edit", message.id));
	let edit_rect = Rect::from_min_max(rect.min + vec2(12.0, 8.0), rect.max - vec2(12.0, 26.0));
	let response = ui.put(
		edit_rect,
		egui::TextEdit::multiline(draft)
			.id(id)
			.frame(egui::Frame::NONE)
			.font(theme::sans(14.0))
			.text_color(p.text)
			.desired_rows(2)
			.desired_width(edit_rect.width()),
	);
	if !response.has_focus() && ui.memory(|m| m.focused().is_none()) {
		response.request_focus();
	}
	paint::label(
		ui.painter(),
		rect.left_bottom() + vec2(12.0, -12.0),
		Align2::LEFT_CENTER,
		"Enter to save · Esc to cancel",
		theme::sans(11.0),
		p.muted,
	);
	let enter =
		response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
	let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
	if enter {
		let content = draft.trim_end_matches('\n').to_owned();
		deferred.push(Deferred::Edit(message.channel, message.id, content));
		solace.editing = None;
	} else if escape {
		solace.editing = None;
	}
}

#[allow(clippy::too_many_arguments)]
fn paint_row(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	row: &Row,
	rect: Rect,
	replies: &HashMap<Id, usize>,
	p: &Palette,
) -> Option<Id> {
	match row {
		Row::Hero => {
			hero(
				solace,
				ui,
				state,
				Rect::from_min_size(rect.min + vec2(0.0, 2.0), vec2(rect.width(), HERO_HEIGHT)),
				p,
			);
			None
		}
		Row::Intro => {
			let channel = nav::selected_channel(state);
			let title = channel.map_or(String::new(), |c| {
				if c.guild.is_some() {
					format!("Welcome to #{}", c.name)
				} else {
					conversation_title(state, c)
				}
			});
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(0.0, 30.0),
				Align2::LEFT_CENTER,
				&title,
				theme::serif(22.0),
				p.text,
			);
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(0.0, 58.0),
				Align2::LEFT_CENTER,
				"This is the very beginning of the conversation.",
				theme::sans(13.0),
				p.muted,
			);
			None
		}
		Row::Loading => {
			let t = ui.input(|i| i.time) as f32;
			for i in 0..3 {
				let phase = ((t * 3.0 - i as f32 * 0.5).sin() * 0.5 + 0.5).powf(1.5);
				ui.painter().circle_filled(
					pos2(rect.center().x - 14.0 + i as f32 * 14.0, rect.center().y),
					3.5,
					mix(p.border, p.muted, phase),
				);
			}
			ui.ctx()
				.request_repaint_after(std::time::Duration::from_millis(50));
			None
		}
		Row::Day(label) => {
			let y = rect.center().y;
			let galley = ui
				.painter()
				.layout_no_wrap(label.clone(), theme::medium(11.5), p.muted);
			let w = galley.size().x + 24.0;
			paint::hairline(
				ui.painter(),
				pos2(rect.left(), y),
				pos2(rect.center().x - w / 2.0, y),
				p,
			);
			paint::hairline(
				ui.painter(),
				pos2(rect.center().x + w / 2.0, y),
				pos2(rect.right(), y),
				p,
			);
			ui.painter().galley(
				pos2(rect.center().x, y) - galley.size() / 2.0,
				galley,
				p.muted,
			);
			None
		}
		Row::Pending(index) => {
			let pending = state.pending.get(*index)?;
			let style = Style {
				color: p.muted,
				..Style::body(p)
			};
			let content = pending.content.clone();
			let failed = matches!(pending.delivery, Delivery::Rejected | Delivery::Ambiguous);
			let laid = text::layout(
				ui.painter(),
				&content,
				&Names {
					state,
					guild: None,
					mentions: &[],
				},
				&style,
				(rect.width() - GUTTER).min(MAX_TEXT_WIDTH),
				true,
			);
			let at = rect.min + vec2(GUTTER, 4.0);
			text::paint(
				ui,
				at,
				&laid,
				&mut solace.images,
				ui.scope_id().with(("pending", *index)),
			);
			let note = match pending.delivery {
				Delivery::Sending if !pending.attachments.is_empty() => {
					match solace.upload_progress {
						Some(progress) => format!("Uploading {}%", (progress * 100.0).round()),
						None => "Uploading...".into(),
					}
				}
				Delivery::Sending => "Sending...".into(),
				Delivery::Rejected => "Not sent".into(),
				Delivery::Ambiguous => "Delivery unknown — check before resending".into(),
				Delivery::Confirmed => String::new(),
			};
			paint::label(
				ui.painter(),
				at + vec2(0.0, laid.size.y + 4.0),
				Align2::LEFT_TOP,
				&note,
				theme::sans(11.0),
				if failed { p.danger } else { p.faint },
			);
			None
		}
		Row::Message { id, continuation } => {
			let message = state.timeline.get_display(*id)?.clone();
			let hover_rect = rect.expand2(vec2(8.0, 0.0));
			let hovered = ui.rect_contains_pointer(hover_rect) && solace.picker.is_none();
			if hovered {
				ui.painter()
					.rect_filled(hover_rect, 12, p.hover.gamma_multiply(0.55));
			}
			let painter = ui.painter().clone();
			let mut deferred = Vec::new();
			layout_message(
				solace,
				&painter,
				state,
				&message,
				*continuation,
				rect.width(),
				replies.get(id).copied().unwrap_or(0),
				Some((ui, &mut deferred, rect.min)),
			);
			apply_deferred(state, commands, deferred);
			if hovered && !message.is_system() && solace.editing.is_none() {
				actions(solace, ui, state, commands, &message, rect);
			}
			hovered.then_some(*id)
		}
	}
}

fn hero(solace: &mut Solace, ui: &mut Ui, state: &State, rect: Rect, _p: &Palette) {
	let Some(channel) = nav::selected_channel(state) else {
		return;
	};
	let Some(guild) = channel.guild.and_then(|g| state.guild(g)) else {
		return;
	};
	let texture = solace
		.scenery
		.texture(ui.ctx(), guild.id.0, Mood::Dawn)
		.id();
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
			(0.0, Color32::from_black_alpha(10)),
			(0.45, Color32::TRANSPARENT),
			(1.0, Color32::from_black_alpha(120)),
		],
	);
	let title = paint::label(
		ui.painter(),
		rect.left_top() + vec2(30.0, rect.height() * 0.56),
		Align2::LEFT_CENTER,
		&guild.name,
		theme::serif(32.0),
		Color32::WHITE,
	);
	let sections = nav::sections(state, guild.id);
	let channels = sections
		.iter()
		.flat_map(|s| &s.channels)
		.filter(|c| nav::is_text(c.kind))
		.count();
	let voice = sections
		.iter()
		.flat_map(|s| &s.channels)
		.filter(|c| nav::is_voice(c.kind))
		.count();
	let mut line = format!("You're in #{}.", channel.name);
	match nav::member_counts(state, guild.id) {
		Some((total, online)) if total > 0 => line.push_str(&format!(
			" {} members, {} here right now.",
			nav::compact(total),
			nav::compact(online)
		)),
		_ => line.push_str(&format!(
			" {channels} channels and {voice} voice rooms to explore."
		)),
	}
	let galley = ui.painter().layout(
		line,
		theme::sans(14.5),
		Color32::from_white_alpha(235),
		(rect.width() * 0.55).max(260.0),
	);
	ui.painter().galley(
		pos2(rect.left() + 30.0, title.bottom() + 12.0),
		galley,
		Color32::WHITE,
	);
}

fn actions(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	message: &Message,
	rect: Rect,
) {
	let p = theme::palette(ui.ctx());
	let mut buttons: Vec<(Icon, &str, u8)> = Vec::new();
	let can_react = state.can_react(message.id, None, true) && message.reactions.is_some();
	if can_react {
		buttons.push((Icon::Smile, "Add reaction", 0));
	}
	if state.can_send(message.channel) {
		buttons.push((Icon::Reply, "Reply", 1));
	}
	buttons.push((Icon::Thread, "Open thread", 2));
	if state.can_edit(message.channel, message.id) {
		buttons.push((Icon::Pencil, "Edit", 3));
	}
	if state.can_delete(message.channel, message.id) {
		buttons.push((Icon::Trash, "Delete", 4));
	}
	let quick: &[&str] = if can_react { &["❤️", "👍"] } else { &[] };
	let w = (buttons.len() + quick.len()) as f32 * 32.0 + 10.0;
	let bar = Rect::from_min_size(
		pos2(rect.right() - w - 4.0, rect.top() - 10.0),
		vec2(w, 38.0),
	);
	paint::card(ui.painter(), bar, 12, &p);
	let mut x = bar.left() + 5.0;
	for emoji in quick {
		let cell = Rect::from_min_size(pos2(x, bar.top() + 3.0), vec2(32.0, 32.0));
		let response = ui.interact(
			cell,
			ui.scope_id().with(("quick", message.id, *emoji)),
			Sense::click(),
		);
		if response.hovered() {
			ui.painter().rect_filled(cell.shrink(2.0), 8, p.hover);
		}
		crate::emoji::paint(
			ui.painter(),
			Rect::from_center_size(cell.center(), vec2(18.0, 18.0)),
			emoji,
		);
		if response.clicked() {
			let emoji = model::ReactionEmoji {
				id: None,
				name: Some((*emoji).to_owned()),
			};
			commands.extend(state.prepare_reaction(message.id, emoji));
		}
		x += 32.0;
	}
	for (icon, label, key) in buttons {
		let cell = Rect::from_min_size(pos2(x, bar.top() + 3.0), vec2(32.0, 32.0));
		let response = ui
			.interact(
				cell,
				ui.scope_id().with(("act", message.id, key)),
				Sense::click(),
			)
			.on_hover_text(label);
		if response.hovered() {
			ui.painter().rect_filled(cell.shrink(2.0), 8, p.hover);
		}
		let color = if key == 4 {
			p.danger
		} else if response.hovered() {
			p.text
		} else {
			p.muted
		};
		icons::paint_at(ui.painter(), icon, cell.center(), 16.0, color);
		if response.clicked() {
			match key {
				0 => solace.picker = Some((PickerTarget::Reaction(message.id), cell.left_bottom())),
				1 => {
					state.reply = Some(client_core::Reply::to(message.id));
					ui.ctx()
						.memory_mut(|m| m.request_focus(composer::input_id(message.channel)));
				}
				2 => solace.thread = Some(message.id),
				3 => solace.editing = Some((message.id, message.content.clone())),
				_ => commands.extend(state.prepare_delete(message.channel, message.id)),
			}
		}
		x += 32.0;
	}
}
