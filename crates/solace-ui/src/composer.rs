//! Message composer: a floating card with attach, emoji and send, plus reply, attachment and
//! typing strips above it.
use crate::{
	PickerTarget, Request, Solace,
	conversation::human_size,
	display_name,
	icons::{self, Icon},
	paint::{self, ButtonKind},
	sidebar::conversation_title,
	theme::{self, mix},
};
use client_core::{Command, State};
use egui::{Align2, Key, Modifiers, Rect, Stroke, TextEdit, Ui, pos2, vec2};
use model::Id;

const LINE: f32 = 20.0;
const MAX_LINES: usize = 8;
const REPLY_STRIP: f32 = 34.0;
const FILE_STRIP: f32 = 56.0;
const TYPING_STRIP: f32 = 22.0;

pub fn input_id(channel: Id) -> egui::Id {
	egui::Id::unique(("solace-composer", channel))
}

fn card_height(state: &State, channel: Id, width: f32, ui: &Ui) -> f32 {
	let draft = state.drafts.get(&channel).map_or("", String::as_str);
	let lines = if draft.is_empty() {
		1
	} else {
		let galley = ui.painter().layout(
			draft.to_owned(),
			theme::sans(14.0),
			egui::Color32::WHITE,
			(width - 200.0).max(80.0),
		);
		galley.rows.len().max(1)
	};
	32.0 + lines.min(MAX_LINES) as f32 * LINE
}

fn typing_names(state: &State) -> Vec<String> {
	let now = std::time::Instant::now();
	let ids: Vec<Id> = state.typing_users(now).collect();
	ids.iter()
		.filter(|id| state.user.as_ref().is_none_or(|u| u.id != **id))
		.filter_map(|id| {
			let channel = state.channel(state.selected?)?;
			channel
				.recipients
				.iter()
				.find(|u| u.id == *id)
				.map(|u| display_name(state, u))
				.or_else(|| {
					crate::nav::people(state)
						.into_iter()
						.find(|m| m.user.id == *id)
						.map(|m| m.nick.clone().unwrap_or_else(|| m.user.name.clone()))
				})
				.or_else(|| {
					state
						.timeline
						.iter()
						.rev()
						.find(|m| m.author.id == *id)
						.map(|m| display_name(state, &m.author))
				})
		})
		.collect()
}

pub fn height(solace: &Solace, ui: &Ui, state: &State, width: f32) -> f32 {
	let Some(channel) = state.selected else {
		return 0.0;
	};
	let mut h = card_height(state, channel, width, ui);
	if state.reply.is_some() {
		h += REPLY_STRIP;
	}
	if !solace.attachments.is_empty() {
		h += FILE_STRIP;
	}
	if !typing_names(state).is_empty() {
		h += TYPING_STRIP;
	}
	h
}

pub fn show(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
	channel: &model::Channel,
	rect: Rect,
) {
	let p = theme::palette(ui.ctx());
	let card_h = card_height(state, channel.id, rect.width(), ui);
	let card = Rect::from_min_max(pos2(rect.left(), rect.bottom() - card_h), rect.max);
	let mut top = rect.top();

	let typing = typing_names(state);
	if !typing.is_empty() {
		let text = match typing.as_slice() {
			[one] => format!("{one} is typing"),
			[a, b] => format!("{a} and {b} are typing"),
			_ => "Several people are typing".into(),
		};
		let t = ui.input(|i| i.time) as f32;
		let y = top + TYPING_STRIP / 2.0 - 2.0;
		for i in 0..3 {
			let phase = ((t * 4.0 - i as f32 * 0.7).sin() * 0.5 + 0.5).powf(2.0);
			ui.painter().circle_filled(
				pos2(rect.left() + 10.0 + i as f32 * 7.0, y),
				2.6,
				mix(p.faint, p.accent, phase),
			);
		}
		paint::label(
			ui.painter(),
			pos2(rect.left() + 34.0, y),
			Align2::LEFT_CENTER,
			&format!("{text}..."),
			theme::sans(12.0),
			p.muted,
		);
		ui.ctx()
			.request_repaint_after(std::time::Duration::from_millis(60));
		top += TYPING_STRIP;
	}

	if let Some(reply) = state.reply {
		let strip = Rect::from_min_size(
			pos2(rect.left() + 8.0, top),
			vec2(rect.width() - 16.0, REPLY_STRIP - 4.0),
		);
		ui.painter().rect_filled(
			strip,
			egui::CornerRadius {
				nw: 12,
				ne: 12,
				sw: 4,
				se: 4,
			},
			p.surface_alt,
		);
		icons::paint_at(
			ui.painter(),
			Icon::Reply,
			strip.left_center() + vec2(16.0, 0.0),
			14.0,
			p.muted,
		);
		let target = state.timeline.get_display(reply.target());
		let who = target.map_or("a message".to_owned(), |m| display_name(state, &m.author));
		paint::text_left(
			ui.painter(),
			strip.left_center() + vec2(32.0, 0.0),
			&format!("Replying to {who}"),
			theme::medium(12.5),
			p.text_soft,
			strip.width() - 80.0,
		);
		let close =
			Rect::from_center_size(strip.right_center() - vec2(18.0, 0.0), vec2(24.0, 24.0));
		if paint::icon_button(
			ui,
			ui.scope_id().with("reply-cancel"),
			close,
			Icon::Close,
			ButtonKind::Ghost,
			false,
			"Cancel reply",
		)
		.clicked()
		{
			state.reply = None;
		}
		top += REPLY_STRIP;
	}

	if !solace.attachments.is_empty() {
		let mut x = rect.left() + 4.0;
		let mut removed = None;
		for (index, (name, size)) in solace.attachments.iter().enumerate() {
			let chip = Rect::from_min_size(pos2(x, top + 4.0), vec2(210.0, FILE_STRIP - 12.0));
			if chip.right() > rect.right() {
				break;
			}
			paint::card(ui.painter(), chip, 12, &p);
			let badge =
				Rect::from_center_size(chip.left_center() + vec2(24.0, 0.0), vec2(30.0, 30.0));
			ui.painter().rect_filled(badge, 8, p.accent_soft);
			icons::paint_at(ui.painter(), Icon::File, badge.center(), 15.0, p.accent);
			paint::text_left(
				ui.painter(),
				chip.left_center() + vec2(46.0, -8.0),
				name,
				theme::medium(12.5),
				p.text,
				120.0,
			);
			paint::text_left(
				ui.painter(),
				chip.left_center() + vec2(46.0, 9.0),
				&human_size(*size),
				theme::sans(11.0),
				p.muted,
				120.0,
			);
			let close =
				Rect::from_center_size(chip.right_center() - vec2(18.0, 0.0), vec2(24.0, 24.0));
			if paint::icon_button(
				ui,
				ui.scope_id().with(("attachment-remove", index)),
				close,
				Icon::Close,
				ButtonKind::Ghost,
				false,
				"Remove",
			)
			.clicked()
			{
				removed = Some(index);
			}
			x = chip.right() + 8.0;
		}
		if let Some(index) = removed {
			solace.request(Request::RemoveAttachment(index));
		}
	}

	// The card itself.
	let id = input_id(channel.id);
	let focused = ui.ctx().memory(|m| m.has_focus(id));
	ui.painter().add(theme::card_shadow(&p).as_shape(card, 18));
	ui.painter().rect(
		card,
		18,
		p.surface,
		Stroke::new(
			1.0,
			if focused {
				mix(p.border, p.accent, 0.35)
			} else {
				p.hairline
			},
		),
		egui::StrokeKind::Inside,
	);
	let can_send = state.can_send(channel.id);
	let can_attach = state.can_attach(channel.id);
	let row_y = card.bottom() - 28.0;
	let plus = Rect::from_center_size(pos2(card.left() + 28.0, row_y), vec2(34.0, 34.0));
	if paint::icon_button(
		ui,
		ui.scope_id().with("composer-plus"),
		plus,
		Icon::Plus,
		ButtonKind::Ghost,
		false,
		if can_attach { "Attach files" } else { "" },
	)
	.clicked()
		&& can_attach
	{
		solace.request(Request::Attach);
	}
	let draft_empty = state
		.drafts
		.get(&channel.id)
		.is_none_or(|d| d.trim().is_empty());
	let sendable =
		can_send && (!draft_empty || !solace.attachments.is_empty()) && !solace.upload_busy;
	let send = Rect::from_center_size(pos2(card.right() - 28.0, row_y), vec2(36.0, 36.0));
	let send_clicked = paint::icon_button(
		ui,
		ui.scope_id().with("composer-send"),
		send,
		Icon::Send,
		if sendable {
			ButtonKind::Primary
		} else {
			ButtonKind::Ghost
		},
		false,
		"Send",
	)
	.clicked();
	let image = Rect::from_center_size(pos2(send.center().x - 42.0, row_y), vec2(34.0, 34.0));
	if paint::icon_button(
		ui,
		ui.scope_id().with("composer-image"),
		image,
		Icon::Image,
		ButtonKind::Ghost,
		false,
		if can_attach { "Share an image" } else { "" },
	)
	.clicked()
		&& can_attach
	{
		solace.request(Request::Attach);
	}
	let smile = Rect::from_center_size(pos2(image.center().x - 40.0, row_y), vec2(34.0, 34.0));
	if paint::icon_button(
		ui,
		ui.scope_id().with("composer-emoji"),
		smile,
		Icon::Smile,
		ButtonKind::Ghost,
		false,
		"Emoji",
	)
	.clicked()
	{
		solace.picker = Some((PickerTarget::Composer, smile.left_top()));
	}

	let edit = Rect::from_min_max(
		pos2(plus.right() + 10.0, card.top() + 14.0),
		pos2(smile.left() - 10.0, card.bottom() - 12.0),
	);
	let placeholder = if !can_send {
		"You can't send messages here".to_owned()
	} else if channel.guild.is_some() {
		let guild = channel
			.guild
			.and_then(|g| state.guild(g))
			.map(|g| g.name.clone());
		match guild {
			Some(guild) => format!("Message #{} in {guild}...", channel.name),
			None => format!("Message #{}...", channel.name),
		}
	} else {
		format!("Message {}...", conversation_title(state, channel))
	};
	let mut enter = false;
	if focused && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
		enter = true;
	}
	if focused && ui.input(|i| i.key_pressed(Key::Escape)) && state.reply.is_some() {
		state.reply = None;
	}
	let mut draft = state.drafts.get(&channel.id).cloned().unwrap_or_default();
	let before = draft.clone();
	ui.add_enabled_ui(can_send, |ui| {
		ui.put(
			edit,
			TextEdit::multiline(&mut draft)
				.id(id)
				.frame(egui::Frame::NONE)
				.font(theme::sans(14.0))
				.text_color(p.text)
				.hint_text(
					egui::RichText::new(placeholder)
						.color(p.muted)
						.font(theme::sans(14.0)),
				)
				.desired_rows(1)
				.lock_focus(true)
				.desired_width(edit.width()),
		);
	});
	if draft != before {
		if draft.is_empty() {
			state.drafts.remove(&channel.id);
		} else {
			state.drafts.insert(channel.id, draft);
		}
	}
	if (enter || send_clicked) && sendable {
		let names: Vec<String> = solace.attachments.iter().map(|(n, _)| n.clone()).collect();
		let names: Vec<&str> = names.iter().map(String::as_str).collect();
		let command = if names.is_empty() {
			state.prepare_send()
		} else {
			state.prepare_send_with_attachments(&names)
		};
		match command {
			Some(command) => {
				commands.push(command);
				solace.scroll.jump_to_bottom = true;
			}
			None if !state.status.is_empty() => {
				solace.toasts.push(crate::Level::Warning, state.status)
			}
			None => {}
		}
		ui.ctx().memory_mut(|m| m.request_focus(id));
	}
	if !focused
		&& can_send
		&& solace.picker.is_none()
		&& solace.editing.is_none()
		&& solace.thread.is_none()
		&& ui.ctx().memory(|m| m.focused().is_none())
		&& ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Text(_))))
	{
		// Typing anywhere starts a message, like most chat apps.
		ui.ctx().memory_mut(|m| m.request_focus(id));
	}
}
