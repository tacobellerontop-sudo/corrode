//! Right-click menus for messages, conversations, communities and people, and the
//! confirmation dialog for the actions in them that are hard to take back.
use crate::{
	PickerTarget, Solace, composer,
	paint::{self, ButtonKind},
	theme,
};
use client_core::{Command, State};
use egui::{Align2, Response, Ui, pos2, vec2};
use model::{Id, Message};

/// Actions that wait for a second, explicit click.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Confirm {
	LeaveServer(Id),
	LeaveGroup(Id),
	CloseDm(Id),
	Block(Id),
	RemoveFriend(Id),
}

fn item(ui: &mut Ui, label: &str) -> bool {
	let clicked = ui
		.add(
			egui::Button::new(egui::RichText::new(label).font(theme::sans(13.0)))
				.frame(false)
				.min_size(vec2(200.0, 28.0)),
		)
		.clicked();
	if clicked {
		ui.close();
	}
	clicked
}

fn danger(ui: &mut Ui, label: &str) -> bool {
	let p = theme::palette(ui.ctx());
	let clicked = ui
		.add(
			egui::Button::new(
				egui::RichText::new(label)
					.font(theme::sans(13.0))
					.color(p.danger),
			)
			.frame(false)
			.min_size(vec2(200.0, 28.0)),
		)
		.clicked();
	if clicked {
		ui.close();
	}
	clicked
}

fn style(ui: &mut Ui) {
	ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
	ui.spacing_mut().button_padding = vec2(10.0, 4.0);
	ui.set_min_width(200.0);
}

#[derive(Clone, Copy)]
enum MessageAction {
	React,
	Reply,
	Thread,
	Edit,
	Pin(bool),
	CopyText,
	CopyLink,
	CopyId,
	Profile,
	Delete,
}

fn message_items(ui: &mut Ui, state: &State, message: &Message) -> Option<MessageAction> {
	style(ui);
	let mut chosen = None;
	if state.can_react(message.id, None, true)
		&& message.reactions.is_some()
		&& item(ui, "Add reaction")
	{
		chosen = Some(MessageAction::React);
	}
	if state.can_send(message.channel) && item(ui, "Reply") {
		chosen = Some(MessageAction::Reply);
	}
	if item(ui, "Open thread") {
		chosen = Some(MessageAction::Thread);
	}
	if state.can_edit(message.channel, message.id) && item(ui, "Edit message") {
		chosen = Some(MessageAction::Edit);
	}
	if state.can_pin(message.channel, message.id) {
		let pinned = state.is_pinned(message.channel, message.id);
		if item(
			ui,
			if pinned {
				"Unpin message"
			} else {
				"Pin message"
			},
		) {
			chosen = Some(MessageAction::Pin(!pinned));
		}
	}
	ui.separator();
	if !message.content.is_empty() && item(ui, "Copy text") {
		chosen = Some(MessageAction::CopyText);
	}
	if item(ui, "Copy message link") {
		chosen = Some(MessageAction::CopyLink);
	}
	if item(ui, "Copy message ID") {
		chosen = Some(MessageAction::CopyId);
	}
	if item(ui, "View profile") {
		chosen = Some(MessageAction::Profile);
	}
	if state.can_delete(message.channel, message.id) {
		ui.separator();
		if danger(ui, "Delete message") {
			chosen = Some(MessageAction::Delete);
		}
	}
	chosen
}

/// The menu a message opens on right-click.
pub(crate) fn message(
	solace: &mut Solace,
	state: &mut State,
	commands: &mut Vec<Command>,
	response: &Response,
	message: &Message,
) {
	let mut chosen = None;
	response.context_menu(|ui| chosen = message_items(ui, state, message));
	if let Some(action) = chosen {
		let at = response.rect.left_bottom();
		apply(solace, state, commands, &response.ctx, message, action, at);
	}
}

/// The same menu, opened from the "More" button of a message's hover bar.
pub(crate) fn message_popup(
	solace: &mut Solace,
	ctx: &egui::Context,
	state: &mut State,
	commands: &mut Vec<Command>,
	message: &Message,
	anchor: egui::Rect,
) {
	let mut chosen = None;
	let area = egui::Area::new(egui::Id::unique(("message-menu", message.id)))
		.order(egui::Order::Foreground)
		.pivot(Align2::RIGHT_TOP)
		.fixed_pos(anchor.right_bottom() + vec2(0.0, 6.0))
		.constrain(true)
		.show(ctx, |ui| {
			egui::Frame::menu(ui.style()).show(ui, |ui| {
				chosen = message_items(ui, state, message);
			});
		});
	let outside = ctx.input(|i| {
		i.pointer.any_pressed()
			&& i.pointer
				.interact_pos()
				.is_some_and(|pos| !area.response.rect.contains(pos) && !anchor.contains(pos))
	});
	if chosen.is_some() || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
		solace.message_menu = None;
	}
	if let Some(action) = chosen {
		apply(
			solace,
			state,
			commands,
			ctx,
			message,
			action,
			anchor.left_bottom(),
		);
	}
}

fn apply(
	solace: &mut Solace,
	state: &mut State,
	commands: &mut Vec<Command>,
	ctx: &egui::Context,
	message: &Message,
	action: MessageAction,
	at: egui::Pos2,
) {
	match action {
		MessageAction::React => {
			let pointer = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(at);
			solace.picker = Some((PickerTarget::Reaction(message.id), pointer));
		}
		MessageAction::Reply => {
			state.reply = Some(client_core::Reply::to(message.id));
			ctx.memory_mut(|m| m.request_focus(composer::input_id(message.channel)));
		}
		MessageAction::Thread => solace.thread = Some(message.id),
		MessageAction::Edit => solace.editing = Some((message.id, message.content.clone())),
		MessageAction::Pin(pin) => match state.prepare_pin(message.channel, message.id, pin) {
			Some(command) => {
				commands.push(command);
				solace.toasts.push(
					crate::Level::Info,
					if pin {
						"Message pinned"
					} else {
						"Message unpinned"
					},
				);
			}
			None => solace.toasts.push(crate::Level::Warning, state.status),
		},
		MessageAction::CopyText => {
			ctx.copy_text(message.content.clone());
			solace.toasts.push(crate::Level::Info, "Copied text");
		}
		MessageAction::CopyLink => {
			ctx.copy_text(message_link(state, message));
			solace
				.toasts
				.push(crate::Level::Info, "Copied message link");
		}
		MessageAction::CopyId => {
			ctx.copy_text(message.id.0.to_string());
			solace.toasts.push(crate::Level::Info, "Copied message ID");
		}
		MessageAction::Profile => {
			let pointer = ctx.input(|i| i.pointer.interact_pos()).unwrap_or(at);
			crate::people::open(solace, state, commands, message.author.id, pointer);
		}
		MessageAction::Delete => commands.extend(state.prepare_delete(message.channel, message.id)),
	}
}

fn message_link(state: &State, message: &Message) -> String {
	let place = state
		.channel(message.channel)
		.and_then(|c| c.guild)
		.map_or_else(|| "@me".to_owned(), |g| g.0.to_string());
	format!(
		"https://discord.com/channels/{place}/{}/{}",
		message.channel.0, message.id.0
	)
}

/// Right-click menu for a direct or group conversation in the sidebar.
pub(crate) fn conversation(
	solace: &mut Solace,
	state: &mut State,
	commands: &mut Vec<Command>,
	response: &Response,
	channel: &model::Channel,
) {
	#[derive(Clone, Copy)]
	enum Action {
		Read,
		Mute(bool),
		Profile(Id),
		Close,
		Leave,
		CopyId,
	}
	let mut chosen = None;
	let peer = (channel.kind == 1)
		.then(|| channel.recipients.first().map(|u| u.id))
		.flatten();
	response.context_menu(|ui| {
		style(ui);
		if state.can_mark_channel_read(channel.id) && item(ui, "Mark as read") {
			chosen = Some(Action::Read);
		}
		if let Some(peer) = peer
			&& item(ui, "View profile")
		{
			chosen = Some(Action::Profile(peer));
		}
		let muted = state.dm_muted(channel.id).unwrap_or(false);
		if item(
			ui,
			if muted {
				"Unmute conversation"
			} else {
				"Mute conversation"
			},
		) {
			chosen = Some(Action::Mute(!muted));
		}
		if item(ui, "Copy channel ID") {
			chosen = Some(Action::CopyId);
		}
		ui.separator();
		if channel.kind == 3 {
			if danger(ui, "Leave group") {
				chosen = Some(Action::Leave);
			}
		} else if danger(ui, "Close conversation") {
			chosen = Some(Action::Close);
		}
	});
	match chosen {
		Some(Action::Read) => commands.extend(state.prepare_mark_channel_read(channel.id)),
		Some(Action::Mute(muted)) => commands.extend(state.set_dm_muted(channel.id, muted)),
		Some(Action::Profile(user)) => {
			let at = response.rect.right_top();
			crate::people::open(solace, state, commands, user, at);
		}
		Some(Action::Close) => solace.confirm = Some(Confirm::CloseDm(channel.id)),
		Some(Action::Leave) => solace.confirm = Some(Confirm::LeaveGroup(channel.id)),
		Some(Action::CopyId) => {
			response.ctx.copy_text(channel.id.0.to_string());
			solace.toasts.push(crate::Level::Info, "Copied channel ID");
		}
		None => {}
	}
}

/// Right-click menu for a community (rail icon or sidebar card).
pub(crate) fn community(
	solace: &mut Solace,
	state: &mut State,
	commands: &mut Vec<Command>,
	response: &Response,
	guild: Id,
) {
	#[derive(Clone, Copy)]
	enum Action {
		Read,
		CopyId,
		Leave,
	}
	let mut chosen = None;
	response.context_menu(|ui| {
		style(ui);
		if state.can_mark_guild_read(guild) && item(ui, "Mark community as read") {
			chosen = Some(Action::Read);
		}
		if item(ui, "Copy community ID") {
			chosen = Some(Action::CopyId);
		}
		ui.separator();
		if danger(ui, "Leave community") {
			chosen = Some(Action::Leave);
		}
	});
	match chosen {
		Some(Action::Read) => commands.extend(state.prepare_mark_guild_read(guild)),
		Some(Action::CopyId) => {
			response.ctx.copy_text(guild.0.to_string());
			solace
				.toasts
				.push(crate::Level::Info, "Copied community ID");
		}
		Some(Action::Leave) => match state.leave_server_reason(guild) {
			Some(reason) => solace.toasts.push(crate::Level::Warning, reason),
			None => solace.confirm = Some(Confirm::LeaveServer(guild)),
		},
		None => {}
	}
}

/// Right-click menu for a person (member list, friend request).
pub(crate) fn person(
	solace: &mut Solace,
	state: &mut State,
	commands: &mut Vec<Command>,
	response: &Response,
	user: Id,
) {
	let mut profile = false;
	let mut copy = false;
	response.context_menu(|ui| {
		style(ui);
		profile = item(ui, "View profile");
		copy = item(ui, "Copy user ID");
	});
	if profile {
		crate::people::open(solace, state, commands, user, response.rect.left_top());
	}
	if copy {
		response.ctx.copy_text(user.0.to_string());
		solace.toasts.push(crate::Level::Info, "Copied user ID");
	}
}

/// The confirmation dialog for a pending [`Confirm`].
pub(crate) fn show_confirm(
	solace: &mut Solace,
	ui: &mut Ui,
	state: &mut State,
	commands: &mut Vec<Command>,
) {
	let Some(confirm) = solace.confirm.clone() else {
		return;
	};
	let user_name = |id: Id| {
		crate::aside::find_user(state, id)
			.map(|u| state.user_display_name(&u).to_owned())
			.unwrap_or_else(|| "this person".to_owned())
	};
	let (title, body, action) = match confirm {
		Confirm::LeaveServer(guild) => (
			"Leave community?".to_owned(),
			format!(
				"You'll leave {} and need a new invite to come back.",
				state
					.guild(guild)
					.map_or("this community", |g| g.name.as_str())
			),
			"Leave",
		),
		Confirm::LeaveGroup(channel) => (
			"Leave group?".to_owned(),
			format!(
				"You'll leave {} and need to be added again to return.",
				state
					.channel(channel)
					.map(|c| crate::sidebar::conversation_title(state, c))
					.unwrap_or_else(|| "this group".to_owned())
			),
			"Leave",
		),
		Confirm::CloseDm(_) => (
			"Close conversation?".to_owned(),
			"It leaves your list; messages are kept and it comes back when either of you writes."
				.to_owned(),
			"Close",
		),
		Confirm::Block(user) => (
			format!("Block {}?", user_name(user)),
			"They won't be able to message you, and their messages are hidden.".to_owned(),
			"Block",
		),
		Confirm::RemoveFriend(user) => (
			format!("Remove {}?", user_name(user)),
			"They'll be removed from your friends. You can send a new request later.".to_owned(),
			"Remove",
		),
	};
	let mut choice = None;
	let dismissed =
		crate::overlays::modal(ui.ctx(), "confirm", vec2(440.0, 190.0), |ui, rect, p| {
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(28.0, 40.0),
				Align2::LEFT_CENTER,
				&title,
				theme::serif(22.0),
				p.text,
			);
			let galley = ui.painter().layout(
				body.clone(),
				theme::sans(13.0),
				p.muted,
				rect.width() - 56.0,
			);
			ui.painter()
				.galley(rect.left_top() + vec2(28.0, 62.0), galley, p.muted);
			let go = egui::Rect::from_min_size(
				pos2(rect.right() - 28.0 - 110.0, rect.bottom() - 56.0),
				vec2(110.0, 36.0),
			);
			let cancel =
				egui::Rect::from_min_size(pos2(go.left() - 108.0, go.top()), vec2(96.0, 36.0));
			if paint::pill_button(
				ui,
				ui.scope_id().with("confirm-cancel"),
				cancel,
				None,
				"Cancel",
				ButtonKind::Ghost,
				true,
			)
			.clicked()
			{
				choice = Some(false);
			}
			if paint::pill_button(
				ui,
				ui.scope_id().with("confirm-go"),
				go,
				None,
				action,
				ButtonKind::Danger,
				true,
			)
			.clicked()
			{
				choice = Some(true);
			}
		});
	if choice == Some(true) {
		let command = match confirm {
			Confirm::LeaveServer(guild) => {
				let command = state.leave_server(guild);
				if command.is_none()
					&& let Some(reason) = state.server_action_status(guild)
				{
					solace.toasts.push(crate::Level::Warning, reason);
				}
				command
			}
			Confirm::LeaveGroup(channel) => {
				let command = state.leave_group(channel);
				if command.is_none()
					&& let Some(reason) = state.group_action_status(channel)
				{
					solace.toasts.push(crate::Level::Warning, reason);
				}
				command
			}
			Confirm::CloseDm(channel) => state.close_dm(channel),
			Confirm::Block(user) => state.set_user_blocked(user, true),
			Confirm::RemoveFriend(user) => state.remove_friend(user),
		};
		commands.extend(command);
	}
	if choice.is_some() || dismissed {
		solace.confirm = None;
	}
}
