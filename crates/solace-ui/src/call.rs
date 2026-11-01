//! The call stage. When you are in a call in a direct conversation, everyone in it fills a
//! dark stage above the timeline, one tile each, with their screen shares beside them. A tile
//! can be focused, streams are watched from their tile, and anyone's volume is one click away.
//! Your own screen goes live from the stage's share button and shows as a tile of its own.
use crate::{
	Solace,
	aside::find_user,
	icons::{self, Icon},
	media::MAX_VOLUME,
	motion,
	paint::{self, AvatarShape, ButtonKind},
	sidebar::conversation_title,
	theme::{self, Palette, mix},
};
use client_core::{
	Command, State,
	screen::{Settings, SourceId},
	voice::{Participant, Phase},
};
use egui::{Align2, Area, Color32, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use model::{Channel, Id};

/// Title row above the tiles.
const TOP: f32 = 52.0;
/// Control bar below the tiles.
const BAR: f32 = 78.0;
const GAP: f32 = 12.0;
pub(crate) const MIN_STAGE: f32 = 260.0;
/// Height of the row of small tiles under a focused one.
const STRIP: f32 = 108.0;
/// Room the timeline and composer keep below a stage that is not full.
const KEEP_BELOW: f32 = 230.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Tile {
	Person(Id),
	Stream(Id),
}

#[derive(Default)]
pub(crate) struct CallView {
	/// Streamer this device was watching last frame, to notice a stream that ends.
	watched: Option<Id>,
	/// Streamer whose watched stream ended; its tile stays until closed.
	ended: Option<Id>,
	/// Per-person volume popover: who, and where it opens.
	volume: Option<(Id, Pos2)>,
	/// Overall levels popover anchor.
	levels: Option<Pos2>,
	/// The stage fills the whole conversation.
	pub(crate) full: bool,
	/// The stage's own button put the window in full screen, so leaving the call (or
	/// the full-window stage) takes it out again.
	owns_fullscreen: bool,
	/// Height chosen with the divider under the stage.
	height: Option<f32>,
	focus: Option<Tile>,
	/// Go Live picker anchor, while it is open.
	share: Option<Pos2>,
	/// Quality last chosen in the picker.
	quality: Quality,
	/// Go Live was pressed and the engine has not picked the request up yet.
	share_asked: bool,
	/// You were sharing last frame, to notice a share that stopped by itself.
	share_seen: bool,
	/// You pressed stop, so the share ending needs no notice.
	share_stopping: bool,
}

/// Resolution, frame rate and sound for the next share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Quality {
	height: u32,
	fps: u32,
	audio: bool,
}

impl Default for Quality {
	fn default() -> Self {
		Self {
			height: 720,
			fps: 30,
			audio: true,
		}
	}
}

impl Quality {
	fn settings(self, source: SourceId, audio_supported: bool) -> Settings {
		let width = match self.height {
			480 => 854,
			1080 => 1920,
			_ => 1280,
		};
		Settings {
			source,
			width,
			height: self.height,
			fps: self.fps,
			cursor: true,
			audio: self.audio && audio_supported,
		}
	}
}

impl CallView {
	/// A volume popover is open (it closes on Esc before anything else does).
	pub(crate) fn popover_open(&self) -> bool {
		self.volume.is_some() || self.levels.is_some() || self.share.is_some()
	}

	/// Open the Go Live picker; it anchors itself to the share button.
	pub(crate) fn open_share_picker(&mut self) {
		self.share = Some(Pos2::ZERO);
	}

	/// Stop filling the window; full screen entered from the stage ends with it.
	pub(crate) fn leave_full(&mut self, ctx: &egui::Context) {
		self.full = false;
		if std::mem::take(&mut self.owns_fullscreen)
			&& ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
		{
			ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
		}
	}
}

/// The stage keeps night colors in both themes, like a theatre with the lights down.
struct Stage {
	back: Color32,
	tile: Color32,
	text: Color32,
	soft: Color32,
	button: Color32,
	button_hover: Color32,
	speaking: Color32,
}

fn stage(p: &Palette) -> Stage {
	Stage {
		back: if p.dark {
			Color32::from_rgb(0x0f, 0x10, 0x12)
		} else {
			Color32::from_rgb(0x17, 0x19, 0x1b)
		},
		tile: Color32::from_rgb(0x24, 0x26, 0x2a),
		text: Color32::from_rgb(0xf2, 0xf1, 0xee),
		soft: Color32::from_rgb(0xa4, 0xa6, 0xab),
		button: Color32::from_rgb(0x2c, 0x2e, 0x33),
		button_hover: Color32::from_rgb(0x3a, 0x3d, 0x43),
		speaking: Color32::from_rgb(0x4c, 0xc3, 0x84),
	}
}

/// You are in a call in this direct conversation, so it shows the stage.
pub(crate) fn joined(state: &State, channel: &Channel) -> bool {
	channel.guild.is_none()
		&& state
			.voice
			.active
			.as_ref()
			.is_some_and(|call| call.channel == channel.id)
}

/// A call is ringing or running here without you: a slim banner offers to join.
pub(crate) fn banner_wanted(state: &State, channel: &Channel) -> bool {
	channel.guild.is_none()
		&& !joined(state, channel)
		&& (state.voice.has_dm_call(channel.id) || state.voice.incoming == Some(channel.id))
}

pub(crate) const BANNER: f32 = 64.0;

/// Stage height for a conversation area `available` tall.
pub(crate) fn stage_height(solace: &Solace, available: f32) -> f32 {
	if solace.call.full {
		return available;
	}
	let most = (available - KEEP_BELOW).max(MIN_STAGE);
	solace
		.call
		.height
		.unwrap_or(available * 0.55)
		.clamp(MIN_STAGE.min(most), most)
}

/// The divider under a stage that is not full; dragging it resizes the stage.
pub(crate) fn divider(solace: &mut Solace, ui: &mut Ui, stage: Rect, available: f32, p: &Palette) {
	let zone = Rect::from_center_size(
		pos2(stage.center().x, stage.bottom() + 7.0),
		vec2(stage.width(), 12.0),
	);
	let response = ui.interact(zone, ui.scope_id().with("call-divider"), Sense::drag());
	let t = paint::hover_t(ui, &response);
	ui.painter().rect_filled(
		Rect::from_center_size(zone.center(), vec2(44.0 + t * 12.0, 4.0)),
		2,
		mix(p.border, p.faint, t),
	);
	if response.hovered() || response.dragged() {
		ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
	}
	if response.dragged() {
		let height = solace.call.height.unwrap_or(stage.height()) + response.drag_delta().y;
		solace.call.height = Some(height.clamp(MIN_STAGE, (available - KEEP_BELOW).max(MIN_STAGE)));
	}
	if response.double_clicked() {
		solace.call.height = None;
	}
}

/// Join banner for a call you are not in.
pub(crate) fn banner(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &Channel,
	rect: Rect,
) {
	let p = theme::palette(ui.ctx());
	let s = stage(&p);
	let a = motion::appear(
		ui.ctx(),
		ui.scope_id().with(("call-banner", channel.id)),
		0.3,
	);
	let rect = rect.translate(vec2(0.0, (1.0 - a) * -8.0));
	let painter = ui.painter().clone().with_clip_rect(rect.expand(30.0));
	let mut painter = painter;
	painter.multiply_opacity(a);
	painter.rect_filled(rect, 16, s.back);
	let ringing = state.voice.incoming == Some(channel.id);
	// A slow pulse while it rings; a steady dot for a call that is just running.
	let dot = rect.left_center() + vec2(26.0, 0.0);
	if ringing {
		let pulse = (motion::ticker(ui.ctx()) * 3.0).sin() as f32 * 0.5 + 0.5;
		painter.circle_filled(dot, 7.0 + pulse * 5.0, s.speaking.gamma_multiply(0.25));
	} else {
		painter.circle_filled(dot, 10.0, s.speaking.gamma_multiply(0.2));
	}
	painter.circle_filled(dot, 6.0, s.speaking);
	painter.text(
		rect.left_center() + vec2(48.0, -9.0),
		Align2::LEFT_CENTER,
		if ringing {
			"Incoming call"
		} else {
			"Call in progress"
		},
		theme::semibold(14.0),
		s.text,
	);
	painter.text(
		rect.left_center() + vec2(48.0, 11.0),
		Align2::LEFT_CENTER,
		conversation_title(state, channel),
		theme::sans(12.5),
		s.soft,
	);
	let join = Rect::from_min_size(rect.right_center() - vec2(124.0, 18.0), vec2(108.0, 36.0));
	let enabled = state.can_call(channel.id) && state.voice.active.is_none();
	let response = paint::pill_button(
		ui,
		ui.scope_id().with(("call-banner-join", channel.id)),
		join,
		Some(Icon::Waveform),
		"Join",
		ButtonKind::Primary,
		enabled,
	);
	let response = if state.voice.active.is_some() {
		response.on_hover_text("Leave your current call first")
	} else {
		response
	};
	if response.clicked()
		&& let Some(command) = state.start_call(channel.id, false)
	{
		solace.call.focus = None;
		commands.push(command);
	}
	if ringing {
		let decline = Rect::from_min_size(join.left_top() - vec2(118.0, 0.0), vec2(108.0, 36.0));
		if paint::pill_button(
			ui,
			ui.scope_id().with(("call-banner-decline", channel.id)),
			decline,
			Some(Icon::PhoneOff),
			"Decline",
			ButtonKind::Danger,
			true,
		)
		.clicked()
		{
			commands.extend(state.decline_call());
		}
	}
}

/// Everyone in the call, you first; your own state comes from this device.
fn people(state: &State) -> Vec<Participant> {
	let Some(call) = state.voice.active.as_ref() else {
		return Vec::new();
	};
	let own = state.user.as_ref().map(|u| u.id);
	let mut people: Vec<Participant> = call
		.participants
		.iter()
		.filter(|p| Some(p.user) != own)
		.copied()
		.collect();
	if let Some(own) = own {
		people.insert(
			0,
			Participant {
				user: own,
				muted: call.muted || call.server_muted,
				deafened: call.deafened || call.server_deafened,
				server_muted: call.server_muted,
				server_deafened: call.server_deafened,
				video: call.camera,
				streaming: false,
			},
		);
	}
	people
}

/// Notice a watched stream that ended without being closed here.
fn track_streams(solace: &mut Solace, state: &State, people: &[Participant]) {
	let watching = state.voice.active.as_ref().and_then(|c| c.watching);
	if let Some(previous) = solace.call.watched
		&& watching.is_none()
	{
		solace.call.ended = Some(previous);
	}
	solace.call.watched = watching;
	if let Some(ended) = solace.call.ended
		&& (watching == Some(ended)
			|| people.iter().any(|p| p.user == ended && p.streaming)
			|| !people.iter().any(|p| p.user == ended))
	{
		// They went live again (or left): the regular tile, or nothing, replaces the notice.
		solace.call.ended = None;
	}
}

fn tiles(solace: &Solace, state: &State, people: &[Participant], sharing: bool) -> Vec<Tile> {
	let own = state.user.as_ref().map(|u| u.id);
	let mut tiles: Vec<Tile> = people.iter().map(|p| Tile::Person(p.user)).collect();
	// Your own screen comes first among the streams, so you can see what everyone sees.
	if let Some(own) = own.filter(|_| sharing) {
		tiles.push(Tile::Stream(own));
	}
	for person in people {
		if person.streaming && Some(person.user) != own {
			tiles.push(Tile::Stream(person.user));
		}
	}
	if let Some(ended) = solace.call.ended
		&& !tiles.contains(&Tile::Stream(ended))
	{
		tiles.push(Tile::Stream(ended));
	}
	tiles
}

/// Rects for `n` 16:9 tiles, as large as fit in `area`, rows centered.
fn grid(n: usize, area: Rect) -> Vec<Rect> {
	if n == 0 {
		return Vec::new();
	}
	let mut best = (0.0f32, 1usize);
	for cols in 1..=n {
		let rows = n.div_ceil(cols);
		let by_width = (area.width() - GAP * (cols - 1) as f32) / cols as f32;
		let by_height = (area.height() - GAP * (rows - 1) as f32) / rows as f32 * 16.0 / 9.0;
		let w = by_width.min(by_height);
		if w > best.0 {
			best = (w, cols);
		}
	}
	let (w, cols) = (best.0.max(40.0), best.1);
	let rows = n.div_ceil(cols);
	// Spare height makes tiles a little taller, up to 4:3.
	let h = ((area.height() - (rows - 1) as f32 * GAP) / rows as f32)
		.min(w * 3.0 / 4.0)
		.max(w * 9.0 / 16.0);
	let total = rows as f32 * h + (rows - 1) as f32 * GAP;
	let top = area.center().y - total / 2.0;
	(0..n)
		.map(|i| {
			let row = i / cols;
			let in_row = if row == rows - 1 {
				n - row * cols
			} else {
				cols
			};
			let row_w = in_row as f32 * w + (in_row - 1) as f32 * GAP;
			let left = area.center().x - row_w / 2.0;
			Rect::from_min_size(
				pos2(
					left + (i % cols) as f32 * (w + GAP),
					top + row as f32 * (h + GAP),
				),
				vec2(w, h),
			)
		})
		.collect()
}

/// A focused tile fills the stage; the rest sit in a strip beneath it.
fn focused(tiles: &[Tile], focus: Tile, area: Rect) -> Vec<Rect> {
	let main_area = Rect::from_min_max(area.min, pos2(area.right(), area.bottom() - STRIP - GAP));
	let w = main_area.width().min(main_area.height() * 16.0 / 9.0);
	let main = Rect::from_center_size(main_area.center(), vec2(w, w * 9.0 / 16.0));
	let others = tiles.len() - 1;
	let small = vec2(STRIP * 16.0 / 9.0, STRIP);
	let fit = ((area.width() + GAP) / (small.x + GAP)).floor().max(1.0) as usize;
	let shown = others.min(fit);
	let row_w = shown as f32 * small.x + shown.saturating_sub(1) as f32 * GAP;
	let mut x = area.center().x - row_w / 2.0;
	tiles
		.iter()
		.map(|tile| {
			if *tile == focus {
				return main;
			}
			let rect = Rect::from_min_size(pos2(x, area.bottom() - STRIP), small);
			x += small.x + GAP;
			rect
		})
		.collect()
}

fn elapsed(call: &client_core::voice::Call) -> Option<String> {
	let seconds = call.connected_at?.elapsed().as_secs();
	Some(if seconds >= 3600 {
		format!(
			"{}:{:02}:{:02}",
			seconds / 3600,
			seconds / 60 % 60,
			seconds % 60
		)
	} else {
		format!("{}:{:02}", seconds / 60, seconds % 60)
	})
}

#[allow(clippy::too_many_arguments)]
/// Round control on the stage. `on` shows the "engaged" look (white with dark glyph).
fn control(
	ui: &mut Ui,
	id: egui::Id,
	rect: Rect,
	icon: Icon,
	on: bool,
	danger: bool,
	tip: &str,
	s: &Stage,
) -> egui::Response {
	let response = ui.interact(rect, id, Sense::click()).on_hover_text(tip);
	let t = paint::hover_t(ui, &response);
	let pressed = response.is_pointer_button_down_on();
	let (fill, ink) = if danger {
		(
			mix(
				Color32::from_rgb(0xd9, 0x4f, 0x45),
				Color32::from_rgb(0xb8, 0x3c, 0x34),
				t,
			),
			Color32::WHITE,
		)
	} else if on {
		(
			mix(Color32::from_rgb(0xee, 0xec, 0xe8), Color32::WHITE, t),
			Color32::from_rgb(0x1d, 0x1e, 0x21),
		)
	} else {
		(mix(s.button, s.button_hover, t), s.text)
	};
	let radius = rect.height() / 2.0 * if pressed { 0.94 } else { 1.0 };
	if danger {
		ui.painter().rect_filled(
			Rect::from_center_size(rect.center(), vec2(rect.width(), radius * 2.0)),
			radius as u8,
			fill,
		);
	} else {
		ui.painter().circle_filled(rect.center(), radius, fill);
	}
	icons::paint_at(ui.painter(), icon, rect.center(), 19.0, ink);
	response
}

pub(crate) fn show(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &Channel,
	rect: Rect,
) {
	let p = theme::palette(ui.ctx());
	let s = stage(&p);
	let Some((phase, muted, deafened, camera, watching, timer)) =
		state.voice.active.as_ref().map(|call| {
			(
				call.phase,
				call.muted,
				call.deafened,
				call.camera,
				call.watching,
				elapsed(call),
			)
		})
	else {
		return;
	};
	let people = people(state);
	track_streams(solace, state, &people);
	let own = state.user.as_ref().map(|u| u.id);
	let sharing = track_share(solace, state);
	let tiles = tiles(solace, state, &people, sharing);
	if solace.call.focus.is_some_and(|f| !tiles.contains(&f)) {
		solace.call.focus = None;
	}

	let a = motion::appear(
		ui.ctx(),
		ui.scope_id().with(("call-stage", channel.id)),
		0.35,
	);
	ui.painter().rect_filled(
		rect.shrink((1.0 - a) * 10.0),
		18,
		s.back.gamma_multiply(a.max(0.2)),
	);
	let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt("call-stage"));
	child.set_clip_rect(rect);
	child.multiply_opacity(a);
	let ui = &mut child;

	// Title row: what this is, how long it has run, and the full-stage toggle.
	let dot = rect.left_top() + vec2(26.0, TOP / 2.0 + 2.0);
	let live = matches!(phase, Phase::Connected | Phase::Waiting);
	ui.painter().circle_filled(
		dot,
		4.5,
		if live {
			s.speaking
		} else {
			Color32::from_rgb(0xe9, 0xa1, 0x3b)
		},
	);
	let title = conversation_title(state, channel);
	let galley = ui
		.painter()
		.layout_no_wrap(title, theme::semibold(14.5), s.text);
	let title_w = galley.size().x.min(rect.width() * 0.5);
	ui.painter().galley(
		pos2(dot.x + 14.0, dot.y - galley.size().y / 2.0),
		galley,
		s.text,
	);
	let detail = match phase {
		Phase::Connected => {
			let count = people.len();
			let who = format!("{count} {}", if count == 1 { "person" } else { "people" });
			match &timer {
				Some(time) => format!("{who}  ·  {time}"),
				None => who,
			}
		}
		other => other.label().trim_end_matches("...").to_owned(),
	};
	ui.painter().text(
		pos2(dot.x + 14.0 + title_w + 12.0, dot.y),
		Align2::LEFT_CENTER,
		detail,
		theme::sans(12.5),
		s.soft,
	);
	let timer_seconds = state
		.voice
		.active
		.as_ref()
		.and_then(|call| call.connected_at)
		.map(|at| at.elapsed().as_secs_f64());
	if let Some(seconds) = timer_seconds {
		// One frame per second for the clock, landing just after it turns over.
		let into = seconds.fract();
		ui.ctx()
			.request_repaint_after(std::time::Duration::from_secs_f64(1.0 - into + 0.01));
	}
	let full = Rect::from_center_size(
		rect.right_top() + vec2(-30.0, TOP / 2.0 + 2.0),
		vec2(34.0, 34.0),
	);
	if control(
		ui,
		ui.scope_id().with("call-full"),
		full,
		if solace.call.full {
			Icon::Collapse
		} else {
			Icon::Expand
		},
		false,
		false,
		if solace.call.full {
			"Show the conversation"
		} else {
			"Fill the window with the call (Esc to leave)"
		},
		&s,
	)
	.clicked()
	{
		if solace.call.full {
			solace.call.leave_full(ui.ctx());
		} else {
			solace.call.full = true;
		}
	}
	// True full screen hides the window's own edges too (F11 anywhere).
	let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
	let screen = full.translate(vec2(-44.0, 0.0));
	if control(
		ui,
		ui.scope_id().with("call-fullscreen"),
		screen,
		if fullscreen {
			Icon::WindowRestore
		} else {
			Icon::WindowMaximize
		},
		fullscreen,
		false,
		if fullscreen {
			"Leave full screen (F11)"
		} else {
			"Full screen (F11)"
		},
		&s,
	)
	.clicked()
	{
		solace.call.owns_fullscreen = !fullscreen;
		if !fullscreen {
			solace.call.full = true;
		}
		ui.ctx()
			.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
	}

	// Tiles.
	let area = Rect::from_min_max(
		pos2(rect.left() + 18.0, rect.top() + TOP),
		pos2(rect.right() - 18.0, rect.bottom() - BAR),
	);
	let rects = match solace.call.focus {
		Some(focus) if tiles.len() > 1 && area.height() > STRIP * 2.2 => {
			focused(&tiles, focus, area)
		}
		_ => grid(tiles.len(), area),
	};
	for (i, (tile, tile_rect)) in tiles.iter().zip(rects).enumerate() {
		if !area.expand(2.0).contains_rect(tile_rect) {
			continue;
		}
		let id = ui.scope_id().with(("call-tile", *tile));
		let enter = motion::appear_after(ui.ctx(), id, i as f32 * 0.05, 0.32);
		let shown = Rect::from_center_size(
			tile_rect.center() + vec2(0.0, (1.0 - enter) * 14.0),
			tile_rect.size() * (0.94 + 0.06 * enter),
		);
		let mut tile_ui = ui.new_child(egui::UiBuilder::new().max_rect(shown).id_salt(id));
		tile_ui.multiply_opacity(enter);
		match *tile {
			Tile::Person(user) => {
				let person = people.iter().find(|p| p.user == user).copied();
				person_tile(solace, &mut tile_ui, state, person, user, shown, &s, &p);
			}
			Tile::Stream(user) if Some(user) == own => {
				own_stream_tile(solace, &mut tile_ui, user, shown, &s);
			}
			Tile::Stream(user) => {
				stream_tile(solace, &mut tile_ui, state, user, watching, shown, &s);
			}
		}
	}

	// Controls.
	let size = 46.0;
	let leave_w = 64.0;
	let count = 6.0;
	let total = count * size + (count - 1.0) * 12.0 + 12.0 + leave_w;
	let y = rect.bottom() - BAR / 2.0;
	let mut x = rect.center().x - total / 2.0;
	let mut next = |w: f32| {
		let r = Rect::from_min_size(pos2(x, y - size / 2.0), vec2(w, size));
		x += w + 12.0;
		r
	};
	let mic = next(size);
	let ears = next(size);
	let cam = next(size);
	let share = next(size);
	let levels = next(size);
	let focus_btn = next(size);
	x += 12.0;
	let leave = Rect::from_min_size(pos2(x, y - size / 2.0), vec2(leave_w, size));
	if control(
		ui,
		ui.scope_id().with("call-mic"),
		mic,
		if muted { Icon::MicOff } else { Icon::Mic },
		muted,
		false,
		if muted { "Unmute" } else { "Mute" },
		&s,
	)
	.clicked()
	{
		solace.media.voice_muted = !muted;
		commands.extend(state.set_call_mute(!muted, deafened));
	}
	if control(
		ui,
		ui.scope_id().with("call-deafen"),
		ears,
		if deafened {
			Icon::HeadphonesOff
		} else {
			Icon::Headphones
		},
		deafened,
		false,
		if deafened { "Undeafen" } else { "Deafen" },
		&s,
	)
	.clicked()
	{
		solace.media.voice_deafened = !deafened;
		commands.extend(state.set_call_mute(muted, !deafened));
	}
	let camera_ok = state.can_camera(channel.id) || camera;
	let cam_response = control(
		ui,
		ui.scope_id().with("call-camera"),
		cam,
		Icon::Video,
		camera,
		false,
		if camera {
			"Turn camera off"
		} else if camera_ok {
			"Turn camera on"
		} else {
			"Camera isn't available here"
		},
		&s,
	);
	if cam_response.clicked() {
		match state.set_call_camera(!camera) {
			Some(command) => commands.push(command),
			None if !camera => solace.toasts.push(
				crate::Level::Info,
				"The camera can't be turned on in this call yet",
			),
			None => {}
		}
	}
	if solace.call.share.is_some() {
		// Follow the button as the stage resizes.
		solace.call.share = Some(share.center_top());
	}
	let share_response = control(
		ui,
		ui.scope_id().with("call-share"),
		share,
		Icon::ScreenShare,
		sharing || solace.call.share.is_some(),
		false,
		if sharing {
			"Stop sharing your screen"
		} else {
			"Share your screen"
		},
		&s,
	);
	if share_response.clicked() {
		if sharing {
			solace.media.screen.request = Some(crate::media::screen::Request::Stop);
			solace.call.share_stopping = true;
			if solace.call.focus == own.map(Tile::Stream) {
				solace.call.focus = None;
			}
		} else if solace.call.share.is_some() {
			solace.call.share = None;
		} else {
			solace.call.share = Some(share.center_top());
			solace.media.screen.refresh_requested = true;
			solace.call.levels = None;
			solace.call.volume = None;
		}
	}
	let levels_response = control(
		ui,
		ui.scope_id().with("call-levels"),
		levels,
		Icon::Sliders,
		solace.call.levels.is_some(),
		false,
		"Volume",
		&s,
	);
	if levels_response.clicked() {
		solace.call.levels = match solace.call.levels {
			Some(_) => None,
			None => Some(levels.center_top()),
		};
		solace.call.volume = None;
		solace.call.share = None;
	}
	let focus_on = solace.call.focus.is_some();
	if control(
		ui,
		ui.scope_id().with("call-focus"),
		focus_btn,
		Icon::Screen,
		focus_on,
		false,
		if focus_on {
			"Show everyone equally"
		} else {
			"Focus the speaker or stream"
		},
		&s,
	)
	.clicked()
	{
		solace.call.focus = if focus_on {
			None
		} else {
			// A stream you are watching first, else whoever is talking, else the first other person.
			let own = state.user.as_ref().map(|u| u.id);
			watching
				.map(Tile::Stream)
				.or_else(|| {
					people
						.iter()
						.find(|p| {
							Some(p.user) != own && solace.media.voice_speaking.contains(&p.user)
						})
						.map(|p| Tile::Person(p.user))
				})
				.or_else(|| {
					tiles
						.iter()
						.copied()
						.find(|t| *t != Tile::Person(own.unwrap_or(Id(0))))
				})
		};
	}
	if control(
		ui,
		ui.scope_id().with("call-leave"),
		leave,
		Icon::PhoneOff,
		false,
		true,
		"Leave call",
		&s,
	)
	.clicked()
	{
		solace.call.leave_full(ui.ctx());
		solace.call = CallView {
			height: solace.call.height,
			quality: solace.call.quality,
			..CallView::default()
		};
		solace.media.screen.open = false;
		commands.extend(state.leave_call());
	}

	let ctx = ui.ctx().clone();
	if let Some(anchor) = solace.call.levels {
		levels_popover(solace, &ctx, anchor, &p);
	}
	if let Some((user, anchor)) = solace.call.volume {
		volume_popover(solace, &ctx, state, user, anchor, &p);
	}
	if let Some(anchor) = solace.call.share {
		share_popover(solace, &ctx, anchor, &p);
	}
	solace.media.screen.open = solace.call.share.is_some();
}

/// Tell the engine which call a share belongs to, and notice a share that failed to start or
/// stopped by itself. Returns whether your screen is shared (or starting, or stopping).
fn track_share(solace: &mut Solace, state: &State) -> bool {
	let screen = &mut solace.media.screen;
	screen.context = state
		.voice
		.active
		.as_ref()
		.map(|call| (state.generation, call.channel, call.request));
	let busy = screen.busy;
	let status = screen.status;
	let mut notice = None;
	if solace.call.share_asked && screen.request.is_none() {
		// The engine has seen the request: it either started or said why not.
		solace.call.share_asked = false;
		if !busy {
			notice = Some(status);
		}
	}
	if solace.call.share_seen && !busy && !std::mem::take(&mut solace.call.share_stopping) {
		notice = Some(status);
	}
	solace.call.share_seen = busy;
	if let Some(message) = notice.filter(|m| !m.is_empty()) {
		solace.toasts.push(crate::Level::Warning, message);
	}
	busy
}

#[allow(clippy::too_many_arguments)]
fn person_tile(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &State,
	person: Option<Participant>,
	user: Id,
	rect: Rect,
	s: &Stage,
	p: &Palette,
) {
	let own = state.user.as_ref().is_some_and(|u| u.id == user);
	let known = find_user(state, user);
	let name = known.as_ref().map_or("Someone".to_owned(), |u| {
		state.user_display_name(u).to_owned()
	});
	let response = ui.interact(rect, ui.scope_id().with("tile"), Sense::click());
	let hover = paint::hover_t(ui, &response);
	let speaking = solace.media.voice_speaking.contains(&user)
		&& !person.is_some_and(|p| p.muted || p.server_muted);
	let talk = ui
		.ctx()
		.animate_bool_with_time(ui.scope_id().with("talk"), speaking, 0.14);
	let local_mute = !own && solace.media.is_user_muted(user);

	// Backdrop: a soft wash of the person's tone over the tile color.
	let tone = theme::seeded_hue(user.0, true);
	paint::gradient(
		ui.painter(),
		rect,
		14.0,
		&[
			(0.0, mix(s.tile, tone, 0.22 + hover * 0.05)),
			(1.0, mix(s.tile, Color32::BLACK, 0.18)),
		],
	);
	let video = solace
		.media
		.voice_remote_video
		.iter()
		.find(|(id, _)| *id == user)
		.map(|(_, texture)| (texture.id(), texture.size_vec2()));
	let own_video = own
		.then_some(solace.media.voice_camera_preview.as_ref())
		.flatten()
		.map(|texture| (texture.id(), texture.size_vec2()));
	if let Some((texture, size)) = video.or(own_video) {
		paint::cover_image(ui.painter(), rect, 14, texture, size);
	} else {
		let side = (rect.height() * 0.42).clamp(36.0, 112.0);
		let avatar = Rect::from_center_size(rect.center() - vec2(0.0, 6.0), vec2(side, side));
		if talk > 0.0 {
			// The halo grows in and out with speech; it does not loop, so a busy call does
			// not redraw every frame.
			ui.painter().circle_filled(
				avatar.center(),
				side / 2.0 + 4.0 + 6.0 * talk,
				s.speaking.gamma_multiply(0.18 * talk),
			);
		}
		let texture = known
			.as_ref()
			.and_then(|u| solace.images.get(&u.avatar_key()));
		paint::avatar(
			ui.painter(),
			avatar,
			AvatarShape::Circle,
			texture,
			&name,
			user.0,
			p,
		);
		if talk > 0.0 {
			ui.painter().circle_stroke(
				avatar.center(),
				side / 2.0 + 2.5,
				Stroke::new(3.0, s.speaking.gamma_multiply(talk)),
			);
		}
	}
	if talk > 0.0 {
		ui.painter().rect_stroke(
			rect,
			14,
			Stroke::new(2.5, s.speaking.gamma_multiply(talk)),
			StrokeKind::Inside,
		);
	}

	// Name chip with mute and deafen marks.
	let mut label = name.clone();
	if own {
		label.push_str(" (you)");
	}
	let galley = ui
		.painter()
		.layout_no_wrap(label, theme::medium(12.5), s.text);
	let marks: Vec<Icon> = [
		person
			.is_some_and(|p| p.deafened || p.server_deafened)
			.then_some(Icon::HeadphonesOff),
		person
			.is_some_and(|p| (p.muted || p.server_muted) && !(p.deafened || p.server_deafened))
			.then_some(Icon::MicOff),
		local_mute.then_some(Icon::SpeakerOff),
	]
	.into_iter()
	.flatten()
	.collect();
	let chip_w = (galley.size().x + 20.0 + marks.len() as f32 * 18.0).min(rect.width() - 20.0);
	let chip = Rect::from_min_size(rect.left_bottom() + vec2(10.0, -36.0), vec2(chip_w, 26.0));
	ui.painter()
		.rect_filled(chip, 8, Color32::from_black_alpha(150));
	let mut x = chip.left() + 9.0;
	for icon in &marks {
		icons::paint_at(
			ui.painter(),
			*icon,
			pos2(x + 7.0, chip.center().y),
			14.0,
			Color32::from_rgb(0xf0, 0x8a, 0x80),
		);
		x += 18.0;
	}
	ui.painter().with_clip_rect(chip.shrink(2.0)).galley(
		pos2(x, chip.center().y - galley.size().y / 2.0),
		galley,
		s.text,
	);

	// Volume control for anyone but you: a button on hover, or right-click the tile.
	if !own {
		let volume = solace.media.user_volume(user);
		if volume != 100 && !local_mute {
			let tag = format!("{volume}%");
			let galley = ui
				.painter()
				.layout_no_wrap(tag, theme::medium(11.0), s.text);
			let tag_rect = Rect::from_min_size(
				rect.right_bottom() + vec2(-galley.size().x - 26.0, -36.0),
				vec2(galley.size().x + 16.0, 26.0),
			);
			ui.painter()
				.rect_filled(tag_rect, 8, Color32::from_black_alpha(150));
			ui.painter()
				.galley(tag_rect.center() - galley.size() / 2.0, galley, s.soft);
		}
		let button = Rect::from_center_size(rect.right_top() + vec2(-24.0, 24.0), vec2(32.0, 32.0));
		let open = solace.call.volume.is_some_and(|(u, _)| u == user);
		let show = hover.max(if open { 1.0 } else { 0.0 });
		if show > 0.0 {
			let b = ui.interact(button, ui.scope_id().with("volume-btn"), Sense::click());
			let bt = paint::hover_t(ui, &b);
			ui.painter().circle_filled(
				button.center(),
				16.0,
				Color32::from_black_alpha((120.0 + bt * 60.0) as u8).gamma_multiply(show),
			);
			icons::paint_at(
				ui.painter(),
				if local_mute {
					Icon::SpeakerOff
				} else {
					Icon::Speaker
				},
				button.center(),
				16.0,
				s.text.gamma_multiply(show),
			);
			if b.on_hover_text(format!("{name}'s volume")).clicked() {
				solace.call.volume = if open {
					None
				} else {
					Some((user, button.left_bottom() + vec2(0.0, 6.0)))
				};
				solace.call.levels = None;
			}
		}
		if response.secondary_clicked() {
			let at = response
				.interact_pointer_pos()
				.unwrap_or(button.left_bottom());
			solace.call.volume = Some((user, at));
			solace.call.levels = None;
		}
	}
	if response.clicked() {
		let tile = Tile::Person(user);
		solace.call.focus = if solace.call.focus == Some(tile) {
			None
		} else {
			Some(tile)
		};
	}
	if response.hovered() {
		ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
	}
}

fn stream_tile(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	user: Id,
	watching: Option<Id>,
	rect: Rect,
	s: &Stage,
) {
	let p = theme::palette(ui.ctx());
	let name = find_user(state, user).map_or("Someone".to_owned(), |u| {
		state.user_display_name(&u).to_owned()
	});
	let response = ui.interact(rect, ui.scope_id().with("tile"), Sense::click());
	ui.painter()
		.rect_filled(rect, 14, Color32::from_rgb(0x0b, 0x0c, 0x0d));
	ui.painter().rect_stroke(
		rect,
		14,
		Stroke::new(1.0, Color32::from_white_alpha(14)),
		StrokeKind::Inside,
	);
	let ended = solace.call.ended == Some(user) && watching != Some(user);
	let center = rect.center();
	if ended {
		ui.painter().text(
			center - vec2(0.0, 18.0),
			Align2::CENTER_CENTER,
			"This stream has ended.",
			theme::semibold(15.0),
			s.text,
		);
		let close = Rect::from_center_size(center + vec2(0.0, 20.0), vec2(124.0, 34.0));
		if stage_pill(
			ui,
			ui.scope_id().with("close-stream"),
			close,
			"Close Stream",
		)
		.clicked()
		{
			solace.call.ended = None;
			if solace.call.focus == Some(Tile::Stream(user)) {
				solace.call.focus = None;
			}
		}
	} else if watching == Some(user) {
		if let Some(view) = solace.media.voice_stream_view {
			// Letterbox the shared screen: never crop someone's screen.
			let size = view.size;
			let scale = (rect.width() / size.x).min(rect.height() / size.y);
			let fitted = Rect::from_center_size(center, size * scale);
			ui.painter().image(
				view.texture,
				fitted,
				Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
				Color32::WHITE,
			);
		} else {
			// Three soft dots while the stream connects.
			let time = motion::ticker(ui.ctx());
			for i in 0..3 {
				let phase = ((time * 4.0 - i as f64 * 0.6).sin() * 0.5 + 0.5) as f32;
				ui.painter().circle_filled(
					center + vec2((i as f32 - 1.0) * 16.0, -10.0),
					4.0,
					s.text.gamma_multiply(0.3 + 0.7 * phase),
				);
			}
			let status = match solace.media.voice_stream_status {
				"" => "Connecting to the stream…",
				status => status,
			};
			ui.painter().text(
				center + vec2(0.0, 18.0),
				Align2::CENTER_CENTER,
				status,
				theme::sans(12.5),
				s.soft,
			);
		}
		let stop = Rect::from_center_size(rect.right_top() + vec2(-24.0, 24.0), vec2(32.0, 32.0));
		let hovered = response.hovered() || ui.rect_contains_pointer(stop);
		if hovered {
			let b = ui.interact(stop, ui.scope_id().with("stop-watch"), Sense::click());
			let t = paint::hover_t(ui, &b);
			ui.painter().circle_filled(
				stop.center(),
				16.0,
				Color32::from_black_alpha((140.0 + 60.0 * t) as u8),
			);
			icons::paint_at(ui.painter(), Icon::Close, stop.center(), 15.0, s.text);
			if b.on_hover_text("Stop watching").clicked() {
				// Closed here, so it is not "ended".
				solace.call.watched = None;
				state.stop_watching();
				if solace.call.focus == Some(Tile::Stream(user)) {
					solace.call.focus = None;
				}
			}
		}
	} else {
		// Someone is live and you are not watching yet.
		let known = find_user(state, user);
		// Small tiles drop the avatar and keep just the button.
		let roomy = rect.height() >= 170.0;
		let side = if roomy {
			(rect.height() * 0.26).clamp(28.0, 64.0)
		} else {
			0.0
		};
		let avatar =
			Rect::from_center_size(center - vec2(0.0, 18.0 + side / 2.0), vec2(side, side));
		if roomy {
			let texture = known
				.as_ref()
				.and_then(|u| solace.images.get(&u.avatar_key()));
			paint::avatar(
				ui.painter(),
				avatar,
				AvatarShape::Circle,
				texture,
				&name,
				user.0,
				&p,
			);
		}
		let watch = Rect::from_center_size(
			pos2(
				center.x,
				if roomy {
					avatar.bottom() + 30.0
				} else {
					center.y + 8.0
				},
			),
			vec2(140.0, 34.0),
		);
		let can_watch = watching.is_none() || watching != Some(user);
		if paint::pill_button(
			ui,
			ui.scope_id().with("watch"),
			watch,
			Some(Icon::Screen),
			"Watch Stream",
			ButtonKind::Primary,
			can_watch,
		)
		.clicked()
		{
			if watching.is_some() {
				state.stop_watching();
			}
			solace.call.watched = None;
			solace.call.ended = None;
			if state.watch_stream(user).is_none() {
				solace.toasts.push(
					crate::Level::Info,
					"Streams can be watched once the call is connected",
				);
			}
		}
	}
	if !ended {
		// LIVE badge.
		let badge = Rect::from_min_size(rect.left_top() + vec2(12.0, 12.0), vec2(42.0, 20.0));
		ui.painter()
			.rect_filled(badge, 6, Color32::from_rgb(0xd9, 0x4f, 0x45));
		ui.painter().text(
			badge.center(),
			Align2::CENTER_CENTER,
			"LIVE",
			theme::semibold(10.5),
			Color32::WHITE,
		);
	}
	// The owner chip, when there is room for it below the tile's content.
	if ended || rect.height() < 150.0 {
		if response.clicked() && !ended {
			let tile = Tile::Stream(user);
			solace.call.focus = if solace.call.focus == Some(tile) {
				None
			} else {
				Some(tile)
			};
		}
		return;
	}
	let label = format!("{name}'s screen");
	let galley = ui
		.painter()
		.layout_no_wrap(label, theme::medium(12.5), s.text);
	let chip = Rect::from_min_size(
		rect.left_bottom() + vec2(10.0, -36.0),
		vec2((galley.size().x + 36.0).min(rect.width() - 20.0), 26.0),
	);
	ui.painter()
		.rect_filled(chip, 8, Color32::from_black_alpha(150));
	icons::paint_at(
		ui.painter(),
		Icon::Screen,
		chip.left_center() + vec2(15.0, 0.0),
		14.0,
		s.soft,
	);
	ui.painter().with_clip_rect(chip.shrink(2.0)).galley(
		pos2(chip.left() + 28.0, chip.center().y - galley.size().y / 2.0),
		galley,
		s.text,
	);
	if response.clicked() && !ended {
		let tile = Tile::Stream(user);
		solace.call.focus = if solace.call.focus == Some(tile) {
			None
		} else {
			Some(tile)
		};
	}
}

/// Your own screen while you share it: the local preview, or how far the share has got.
fn own_stream_tile(solace: &mut Solace, ui: &mut Ui, own: Id, rect: Rect, s: &Stage) {
	let response = ui.interact(rect, ui.scope_id().with("tile"), Sense::click());
	ui.painter()
		.rect_filled(rect, 14, Color32::from_rgb(0x0b, 0x0c, 0x0d));
	ui.painter().rect_stroke(
		rect,
		14,
		Stroke::new(1.0, Color32::from_white_alpha(14)),
		StrokeKind::Inside,
	);
	let center = rect.center();
	let screen = &solace.media.screen;
	let status = screen
		.capture_status
		.filter(|s| !s.is_empty())
		.unwrap_or(screen.status);
	let previewing = if let Some(texture) = &screen.preview {
		// Letterbox like any other stream, so you see exactly what everyone else sees.
		let size = texture.size_vec2();
		let scale = (rect.width() / size.x).min(rect.height() / size.y);
		ui.painter().image(
			texture.id(),
			Rect::from_center_size(center, size * scale),
			Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
			Color32::WHITE,
		);
		true
	} else {
		let time = motion::ticker(ui.ctx());
		for i in 0..3 {
			let phase = ((time * 4.0 - i as f64 * 0.6).sin() * 0.5 + 0.5) as f32;
			ui.painter().circle_filled(
				center + vec2((i as f32 - 1.0) * 16.0, -10.0),
				4.0,
				s.text.gamma_multiply(0.3 + 0.7 * phase),
			);
		}
		let galley = paint::line(
			ui.painter(),
			if status.is_empty() {
				"Starting your stream…"
			} else {
				status
			},
			theme::sans(12.5),
			s.soft,
			rect.width() - 32.0,
		);
		ui.painter().galley(
			center + vec2(-galley.size().x / 2.0, 18.0 - galley.size().y / 2.0),
			galley,
			s.soft,
		);
		false
	};

	// Stop Sharing sits in the middle on hover, and always while there is no picture yet.
	let stop = Rect::from_center_size(
		center + vec2(0.0, if previewing { 0.0 } else { 52.0 }),
		vec2(132.0, 34.0),
	);
	let show_stop = (!previewing || response.hovered() || ui.rect_contains_pointer(stop))
		&& rect.height() >= 120.0
		&& !solace.call.share_stopping;
	let mut stopped = false;
	if show_stop
		&& paint::pill_button(
			ui,
			ui.scope_id().with("stop-share"),
			stop,
			Some(Icon::Close),
			"Stop Sharing",
			ButtonKind::Danger,
			true,
		)
		.clicked()
	{
		solace.media.screen.request = Some(crate::media::screen::Request::Stop);
		solace.call.share_stopping = true;
		stopped = true;
	}

	let badge = Rect::from_min_size(rect.left_top() + vec2(12.0, 12.0), vec2(42.0, 20.0));
	ui.painter()
		.rect_filled(badge, 6, Color32::from_rgb(0xd9, 0x4f, 0x45));
	ui.painter().text(
		badge.center(),
		Align2::CENTER_CENTER,
		"LIVE",
		theme::semibold(10.5),
		Color32::WHITE,
	);
	if rect.height() >= 150.0 {
		let galley =
			ui.painter()
				.layout_no_wrap("Your screen".to_owned(), theme::medium(12.5), s.text);
		let chip = Rect::from_min_size(
			rect.left_bottom() + vec2(10.0, -36.0),
			vec2((galley.size().x + 36.0).min(rect.width() - 20.0), 26.0),
		);
		ui.painter()
			.rect_filled(chip, 8, Color32::from_black_alpha(150));
		icons::paint_at(
			ui.painter(),
			Icon::ScreenShare,
			chip.left_center() + vec2(15.0, 0.0),
			14.0,
			s.soft,
		);
		ui.painter().with_clip_rect(chip.shrink(2.0)).galley(
			pos2(chip.left() + 28.0, chip.center().y - galley.size().y / 2.0),
			galley,
			s.text,
		);
	}
	if response.clicked() && !stopped {
		let tile = Tile::Stream(own);
		solace.call.focus = if solace.call.focus == Some(tile) {
			None
		} else {
			Some(tile)
		};
	}
}

/// Light pill on the dark stage.
fn stage_pill(ui: &mut Ui, id: egui::Id, rect: Rect, text: &str) -> egui::Response {
	let response = ui.interact(rect, id, Sense::click());
	let t = paint::hover_t(ui, &response);
	ui.painter().rect_filled(
		rect,
		(rect.height() / 2.0) as u8,
		mix(Color32::from_rgb(0xec, 0xea, 0xe6), Color32::WHITE, t),
	);
	ui.painter().text(
		rect.center(),
		Align2::CENTER_CENTER,
		text,
		theme::medium(13.5),
		Color32::from_rgb(0x1d, 0x1e, 0x21),
	);
	response
}

fn clicked_outside(ctx: &egui::Context, rect: Rect) -> bool {
	ctx.input(|i| {
		i.pointer.any_pressed()
			&& i.pointer
				.interact_pos()
				.is_some_and(|pos| !rect.contains(pos))
	})
}

/// One labelled slider inside a popover.
fn popover_slider(ui: &mut Ui, at: Pos2, width: f32, label: &str, value: &mut u16, p: &Palette) {
	ui.painter()
		.text(at, Align2::LEFT_CENTER, label, theme::medium(13.0), p.text);
	ui.painter().text(
		at + vec2(width, 0.0),
		Align2::RIGHT_CENTER,
		format!("{value}%"),
		theme::sans(12.0),
		p.muted,
	);
	paint::slider(
		ui,
		ui.scope_id().with(("popover-slider", label)),
		Rect::from_min_size(at + vec2(-8.0, 10.0), vec2(width + 16.0, 26.0)),
		value,
		MAX_VOLUME,
		p.surface_alt,
		p.accent,
		p.surface,
	);
}

/// Place a popover of `size` above `anchor`, kept on screen.
fn place(ctx: &egui::Context, anchor: Pos2, size: egui::Vec2, above: bool) -> Rect {
	let screen = ctx.content_rect().shrink(10.0);
	let min = if above {
		anchor - vec2(size.x / 2.0, size.y + 12.0)
	} else {
		anchor
	};
	let x = min
		.x
		.clamp(screen.left(), (screen.right() - size.x).max(screen.left()));
	let y = min
		.y
		.clamp(screen.top(), (screen.bottom() - size.y).max(screen.top()));
	Rect::from_min_size(pos2(x, y), size)
}

/// Rows of the source list visible before it scrolls.
const SHARE_ROWS: usize = 5;
const SHARE_ROW: f32 = 40.0;

/// One choice in a row of segments; returns whether it was clicked.
fn segment(ui: &mut Ui, id: egui::Id, rect: Rect, text: &str, on: bool, p: &Palette) -> bool {
	let response = ui.interact(rect, id, Sense::click());
	let t = paint::hover_t(ui, &response);
	if on {
		ui.painter().rect_filled(rect, 8, p.accent);
	} else {
		ui.painter()
			.rect_filled(rect, 8, mix(p.surface_alt, p.hover, t));
	}
	ui.painter().text(
		rect.center(),
		Align2::CENTER_CENTER,
		text,
		theme::medium(12.5),
		if on { p.accent_text } else { p.text },
	);
	response.clicked()
}

/// A labelled row of segments in the Go Live picker.
fn segments<T: Copy + PartialEq>(
	ui: &mut Ui,
	at: Pos2,
	width: f32,
	label: &str,
	options: &[(T, &str)],
	value: &mut T,
	p: &Palette,
) {
	ui.painter()
		.text(at, Align2::LEFT_CENTER, label, theme::medium(13.0), p.text);
	let w = 58.0;
	let mut x = at.x + width - options.len() as f32 * (w + 4.0) + 4.0;
	for (option, text) in options {
		let rect = Rect::from_min_size(pos2(x, at.y - 14.0), vec2(w, 28.0));
		if segment(
			ui,
			ui.scope_id().with(("share-segment", label, *text)),
			rect,
			text,
			*value == *option,
			p,
		) {
			*value = *option;
		}
		x += w + 4.0;
	}
}

/// Go Live: pick a screen or window and how sharp and smooth it should be.
fn share_popover(solace: &mut Solace, ctx: &egui::Context, anchor: Pos2, p: &Palette) {
	let screen = &solace.media.screen;
	let rows = screen.sources.len().clamp(1, SHARE_ROWS);
	let list_h = rows as f32 * SHARE_ROW;
	let audio_row = if screen.audio_supported { 44.0 } else { 0.0 };
	let size = vec2(380.0, 64.0 + list_h + 16.0 + 92.0 + audio_row + 62.0);
	let rect = place(ctx, anchor, size, true);
	let a = motion::appear(ctx, egui::Id::unique("call-share-pop"), 0.18);
	Area::new(egui::Id::unique("call-share-area"))
		.order(Order::Foreground)
		.fixed_pos(rect.min)
		.show(ctx, |ui| {
			ui.multiply_opacity(a);
			let shown = rect.translate(vec2(0.0, (1.0 - a) * 8.0));
			let _ = ui.allocate_exact_size(size, Sense::click());
			paint::floating(ui.painter(), shown, 16, p);
			ui.painter().text(
				shown.left_top() + vec2(18.0, 26.0),
				Align2::LEFT_CENTER,
				"Share your screen",
				theme::serif(17.0),
				p.text,
			);
			// Look again, for a window opened since the picker was.
			let refresh =
				Rect::from_center_size(shown.right_top() + vec2(-30.0, 26.0), vec2(28.0, 28.0));
			let media = &mut solace.media;
			let scanning =
				media.screen.supported && media.screen.status == "Looking for screens and windows…";
			if paint::icon_button(
				ui,
				ui.scope_id().with("share-refresh"),
				refresh,
				Icon::Refresh,
				ButtonKind::Ghost,
				false,
				"Look for screens and windows again",
			)
			.clicked() && !scanning
			{
				media.screen.refresh_requested = true;
			}
			let w = shown.width() - 36.0;
			let list = Rect::from_min_size(
				shown.left_top() + vec2(10.0, 50.0),
				vec2(shown.width() - 20.0, list_h),
			);
			if media.screen.sources.is_empty() {
				let message = if !media.screen.supported {
					"Screen sharing isn't available on this computer."
				} else if media.screen.status.is_empty() {
					"Looking for screens and windows…"
				} else {
					media.screen.status
				};
				paint::text_left(
					ui.painter(),
					list.left_center() + vec2(8.0, 0.0),
					message,
					theme::sans(13.0),
					p.muted,
					list.width() - 16.0,
				);
			} else {
				let mut list_ui = ui.new_child(
					egui::UiBuilder::new()
						.max_rect(list)
						.id_salt("share-sources"),
				);
				egui::ScrollArea::vertical()
					.max_height(list_h)
					.auto_shrink([false, true])
					.show(&mut list_ui, |ui| {
						ui.spacing_mut().item_spacing.y = 0.0;
						for source in &media.screen.sources {
							let (row, response) = ui
								.allocate_exact_size(vec2(list.width(), SHARE_ROW), Sense::click());
							let selected = media.screen.selected == Some(source.id);
							let t = paint::hover_t(ui, &response);
							let fill = if selected {
								p.accent_soft
							} else {
								p.hover.gamma_multiply(t)
							};
							ui.painter()
								.rect_filled(row.shrink2(vec2(0.0, 2.0)), 10, fill);
							let icon = match source.id {
								SourceId::Display(_) => Icon::Screen,
								SourceId::Window(_) => Icon::WindowMaximize,
							};
							icons::paint_at(
								ui.painter(),
								icon,
								row.left_center() + vec2(20.0, 0.0),
								17.0,
								if selected { p.accent } else { p.muted },
							);
							let name = if source.name.trim().is_empty() {
								"Untitled window"
							} else {
								source.name.as_str()
							};
							paint::text_left(
								ui.painter(),
								row.left_center() + vec2(40.0, 0.0),
								name,
								if selected {
									theme::medium(13.5)
								} else {
									theme::sans(13.5)
								},
								p.text,
								row.width() - 76.0,
							);
							if selected {
								icons::paint_at(
									ui.painter(),
									Icon::Check,
									row.right_center() - vec2(20.0, 0.0),
									16.0,
									p.accent,
								);
							}
							if response.clicked() {
								media.screen.selected = Some(source.id);
							}
						}
					});
			}
			let mut y = list.bottom() + 16.0;
			paint::hairline(
				ui.painter(),
				pos2(shown.left() + 18.0, y - 8.0),
				pos2(shown.right() - 18.0, y - 8.0),
				p,
			);
			let quality = &mut solace.call.quality;
			segments(
				ui,
				pos2(shown.left() + 18.0, y + 22.0),
				w,
				"Resolution",
				&[(480, "480p"), (720, "720p"), (1080, "1080p")],
				&mut quality.height,
				p,
			);
			segments(
				ui,
				pos2(shown.left() + 18.0, y + 64.0),
				w,
				"Frame rate",
				&[(15, "15"), (30, "30"), (60, "60")],
				&mut quality.fps,
				p,
			);
			y += 92.0;
			if media.screen.audio_supported {
				let row = Rect::from_min_size(
					pos2(shown.left() + 10.0, y + 2.0),
					vec2(shown.width() - 20.0, 40.0),
				);
				let response = ui.interact(row, ui.scope_id().with("share-audio"), Sense::click());
				let t = paint::hover_t(ui, &response);
				ui.painter().rect_filled(row, 10, p.hover.gamma_multiply(t));
				icons::paint_at(
					ui.painter(),
					Icon::Speaker,
					row.left_center() + vec2(18.0, 0.0),
					16.0,
					p.muted,
				);
				ui.painter().text(
					row.left_center() + vec2(36.0, 0.0),
					Align2::LEFT_CENTER,
					"Share sound too",
					theme::sans(13.5),
					p.text,
				);
				let switch =
					Rect::from_center_size(row.right_center() - vec2(28.0, 0.0), vec2(40.0, 22.0));
				let on = ui.ctx().animate_bool_with_time(
					ui.scope_id().with("share-audio-anim"),
					quality.audio,
					0.15,
				);
				ui.painter()
					.rect_filled(switch, 11, mix(p.border, p.accent, on));
				ui.painter().circle_filled(
					pos2(switch.left() + 11.0 + on * 18.0, switch.center().y),
					8.0,
					Color32::WHITE,
				);
				if response.clicked() {
					quality.audio = !quality.audio;
				}
				y += audio_row;
			}
			let go = Rect::from_min_size(
				pos2(shown.right() - 18.0 - 124.0, y + 12.0),
				vec2(124.0, 36.0),
			);
			let ready =
				media.screen.supported && !media.screen.busy && media.screen.selected.is_some();
			let response = paint::pill_button(
				ui,
				ui.scope_id().with("share-go"),
				go,
				Some(Icon::ScreenShare),
				"Go Live",
				ButtonKind::Primary,
				ready,
			);
			let hint = format!(
				"{}p · {} fps{}",
				quality.height,
				quality.fps,
				if quality.audio && media.screen.audio_supported {
					" · with sound"
				} else {
					""
				}
			);
			paint::text_left(
				ui.painter(),
				pos2(shown.left() + 18.0, go.center().y),
				&hint,
				theme::sans(12.5),
				p.muted,
				go.left() - shown.left() - 30.0,
			);
			if response.clicked()
				&& let Some(source) = media.screen.selected
			{
				let settings = quality.settings(source, media.screen.audio_supported);
				media.screen.request = Some(crate::media::screen::Request::Start(settings));
				solace.call.share_asked = true;
				solace.call.share_stopping = false;
				solace.call.share = None;
			}
		});
	let on_button = ctx.input(|i| {
		i.pointer
			.interact_pos()
			.is_some_and(|pos| pos.distance(anchor) < 30.0)
	});
	if (clicked_outside(ctx, rect.expand(4.0)) && !on_button)
		|| ctx.input(|i| i.key_pressed(egui::Key::Escape))
	{
		solace.call.share = None;
	}
}

fn levels_popover(solace: &mut Solace, ctx: &egui::Context, anchor: Pos2, p: &Palette) {
	let size = vec2(300.0, 226.0);
	let rect = place(ctx, anchor, size, true);
	let a = motion::appear(ctx, egui::Id::unique("call-levels-pop"), 0.18);
	let mut close = false;
	Area::new(egui::Id::unique("call-levels-area"))
		.order(Order::Foreground)
		.fixed_pos(rect.min)
		.show(ctx, |ui| {
			ui.multiply_opacity(a);
			let shown = rect.translate(vec2(0.0, (1.0 - a) * 8.0));
			let _ = ui.allocate_exact_size(size, Sense::click());
			paint::floating(ui.painter(), shown, 16, p);
			ui.painter().text(
				shown.left_top() + vec2(18.0, 26.0),
				Align2::LEFT_CENTER,
				"Volume",
				theme::serif(17.0),
				p.text,
			);
			let w = shown.width() - 36.0;
			let media = &mut solace.media;
			popover_slider(
				ui,
				shown.left_top() + vec2(18.0, 62.0),
				w,
				"Output",
				&mut media.voice_gain.output_percent,
				p,
			);
			popover_slider(
				ui,
				shown.left_top() + vec2(18.0, 116.0),
				w,
				"Input",
				&mut media.voice_gain.input_percent,
				p,
			);
			popover_slider(
				ui,
				shown.left_top() + vec2(18.0, 170.0),
				w,
				"Streams",
				&mut media.stream_volume,
				p,
			);
		});
	if clicked_outside(ctx, rect.expand(4.0)) || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
		close = true;
	}
	// The levels button toggles the popover itself; don't let its own click close it twice.
	if close
		&& !ctx.input(|i| {
			i.pointer
				.interact_pos()
				.is_some_and(|pos| pos.distance(anchor) < 30.0)
		}) {
		solace.call.levels = None;
	}
}

fn volume_popover(
	solace: &mut Solace,
	ctx: &egui::Context,
	state: &State,
	user: Id,
	anchor: Pos2,
	p: &Palette,
) {
	let size = vec2(280.0, 176.0);
	let rect = place(ctx, anchor - vec2(size.x - 32.0, 0.0), size, false);
	let a = motion::appear(ctx, egui::Id::unique(("call-volume-pop", user)), 0.18);
	let known = find_user(state, user);
	let name = known.as_ref().map_or("Someone".to_owned(), |u| {
		state.user_display_name(u).to_owned()
	});
	Area::new(egui::Id::unique("call-volume-area"))
		.order(Order::Foreground)
		.fixed_pos(rect.min)
		.show(ctx, |ui| {
			ui.multiply_opacity(a);
			let shown = rect.translate(vec2(0.0, (1.0 - a) * -6.0));
			let _ = ui.allocate_exact_size(size, Sense::click());
			paint::floating(ui.painter(), shown, 16, p);
			let avatar = Rect::from_min_size(shown.left_top() + vec2(16.0, 14.0), vec2(30.0, 30.0));
			let texture = known
				.as_ref()
				.and_then(|u| solace.images.get(&u.avatar_key()));
			paint::avatar(
				ui.painter(),
				avatar,
				AvatarShape::Circle,
				texture,
				&name,
				user.0,
				p,
			);
			paint::text_left(
				ui.painter(),
				pos2(avatar.right() + 10.0, avatar.center().y),
				&name,
				theme::semibold(14.0),
				p.text,
				shown.width() - 80.0,
			);
			let w = shown.width() - 36.0;
			let mut volume = solace.media.user_volume(user);
			let before = volume;
			popover_slider(
				ui,
				shown.left_top() + vec2(18.0, 66.0),
				w,
				"User volume",
				&mut volume,
				p,
			);
			if volume != before {
				solace.media.set_user_volume(user, volume);
				if solace.media.is_user_muted(user) && volume > 0 {
					solace.media.set_user_muted(user, false);
				}
			}
			// Mute for me only.
			let muted = solace.media.is_user_muted(user);
			let row = Rect::from_min_size(
				shown.left_top() + vec2(10.0, 118.0),
				vec2(shown.width() - 20.0, 40.0),
			);
			let response = ui.interact(row, ui.scope_id().with("local-mute"), Sense::click());
			let t = paint::hover_t(ui, &response);
			ui.painter().rect_filled(row, 10, p.hover.gamma_multiply(t));
			icons::paint_at(
				ui.painter(),
				Icon::SpeakerOff,
				row.left_center() + vec2(18.0, 0.0),
				16.0,
				if muted { p.danger } else { p.muted },
			);
			ui.painter().text(
				row.left_center() + vec2(36.0, 0.0),
				Align2::LEFT_CENTER,
				"Mute for me",
				theme::sans(13.5),
				p.text,
			);
			let switch =
				Rect::from_center_size(row.right_center() - vec2(28.0, 0.0), vec2(40.0, 22.0));
			let on =
				ui.ctx()
					.animate_bool_with_time(ui.scope_id().with("local-mute-anim"), muted, 0.15);
			ui.painter()
				.rect_filled(switch, 11, mix(p.border, p.danger, on));
			ui.painter().circle_filled(
				pos2(switch.left() + 11.0 + on * 18.0, switch.center().y),
				8.0,
				Color32::WHITE,
			);
			if response.clicked() {
				solace.media.set_user_muted(user, !muted);
			}
		});
	// Pressing the tile's own volume button toggles the popover; leave that press to it.
	let on_button = ctx.input(|i| {
		i.pointer
			.interact_pos()
			.is_some_and(|pos| pos.distance(anchor) < 40.0)
	});
	if (clicked_outside(ctx, rect.expand(4.0)) && !on_button)
		|| ctx.input(|i| i.key_pressed(egui::Key::Escape))
	{
		solace.call.volume = None;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_quality_in_the_picker_is_one_the_engine_accepts() {
		for height in [480, 720, 1080] {
			for fps in [15, 30, 60] {
				for audio in [false, true] {
					let quality = Quality { height, fps, audio };
					for source in [SourceId::Display(1), SourceId::Window(7)] {
						let settings = quality.settings(source, false);
						assert!(settings.valid(), "{height}p {fps} fps");
						assert!(!settings.audio, "no sound where it can't be captured");
						assert_eq!(quality.settings(source, true).audio, audio);
					}
				}
			}
		}
	}

	#[test]
	fn grid_tiles_fit_and_do_not_overlap() {
		let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
		for n in 1..=9 {
			let rects = grid(n, area);
			assert_eq!(rects.len(), n);
			for (i, a) in rects.iter().enumerate() {
				assert!(area.expand(0.5).contains_rect(*a), "{n} tiles: {a:?}");
				for b in &rects[i + 1..] {
					assert!(!a.shrink(0.5).intersects(*b), "{n} tiles overlap");
				}
			}
		}
	}
}
