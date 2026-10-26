//! Floating thread panel: a message and the replies to it, with its own reply field.
use crate::{
	PickerTarget, Solace, clock, display_name,
	icons::{self, Icon},
	images,
	paint::{self, AvatarShape, ButtonKind},
	text::{self, Names, Style},
	theme,
};
use client_core::{Command, Reply, State};
use egui::{Align2, Key, Modifiers, Rect, Sense, TextEdit, Ui, pos2, vec2};
use model::Message;

const WIDTH: f32 = 290.0;

pub fn show(
	solace: &mut Solace,
	ui: &mut Ui,
	main: Rect,
	state: &mut State,
	commands: &mut Vec<Command>,
) {
	let Some(root_id) = solace.thread else {
		return;
	};
	let Some(root) = state.timeline.get_display(root_id).cloned() else {
		solace.thread = None;
		return;
	};
	let p = theme::palette(ui.ctx());
	let replies: Vec<Message> = state
		.timeline
		.display_iter()
		.filter(|m| m.reply_to == Some(root_id))
		.cloned()
		.collect();
	let top = (main.top() + theme::size::HEADER + 210.0)
		.min(main.bottom() - 420.0)
		.max(main.top() + theme::size::HEADER + 8.0);
	// The panel slides in from the right edge whenever a different thread opens.
	let a = crate::motion::appear(
		ui.ctx(),
		egui::Id::unique(("solace-thread-enter", root_id)),
		0.28,
	);
	let rect = Rect::from_min_max(
		pos2(main.right() - 24.0 - WIDTH, top),
		pos2(main.right() - 24.0, main.bottom() - 130.0),
	)
	.translate(vec2((1.0 - a) * 36.0, 0.0));
	let mut close = false;
	egui::Area::new(egui::Id::unique("solace-thread"))
		.order(egui::Order::Middle)
		.fixed_pos(rect.min)
		.show(ui.ctx(), |ui| {
			ui.multiply_opacity(a);
			ui.set_clip_rect(rect.expand(60.0));
			let (_, _) = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
			paint::floating(ui.painter(), rect, 16, &p);
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(18.0, 28.0),
				Align2::LEFT_CENTER,
				"Thread",
				theme::serif(17.0),
				p.text,
			);
			let x = Rect::from_center_size(rect.right_top() + vec2(-26.0, 28.0), vec2(28.0, 28.0));
			if paint::icon_button(
				ui,
				ui.scope_id().with("thread-close"),
				x,
				Icon::Close,
				ButtonKind::Ghost,
				false,
				"Close",
			)
			.clicked()
			{
				close = true;
			}
			let input_h = 46.0;
			let body = Rect::from_min_max(
				rect.min + vec2(0.0, 52.0),
				pos2(rect.right(), rect.bottom() - input_h - 18.0),
			);
			let mut child =
				ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt("thread-body"));
			child.set_clip_rect(body);
			egui::ScrollArea::vertical()
				.id_salt(("thread-scroll", root_id))
				.auto_shrink([false, false])
				.show(&mut child, |ui| {
					entry(solace, ui, state, &root, true);
					let (line, _) = ui.allocate_exact_size(vec2(WIDTH, 30.0), Sense::hover());
					let label = match replies.len() {
						0 => "No replies yet".to_owned(),
						1 => "1 reply".to_owned(),
						n => format!("{n} replies"),
					};
					paint::label(
						ui.painter(),
						line.left_center() + vec2(18.0, 0.0),
						Align2::LEFT_CENTER,
						&label,
						theme::sans(11.5),
						p.muted,
					);
					for reply in &replies {
						entry(solace, ui, state, reply, false);
					}
					ui.add_space(8.0);
				});

			let field = Rect::from_min_max(
				pos2(rect.left() + 12.0, rect.bottom() - input_h - 12.0),
				pos2(rect.right() - 12.0, rect.bottom() - 12.0),
			);
			ui.painter().rect(
				field,
				12,
				p.canvas,
				egui::Stroke::new(1.0, p.border),
				egui::StrokeKind::Inside,
			);
			let id = egui::Id::unique(("thread-input", root_id));
			let can_send = state.can_send(root.channel);
			let focused = ui.ctx().memory(|m| m.has_focus(id));
			let enter = focused && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
			let smile =
				Rect::from_center_size(field.right_center() - vec2(20.0, 0.0), vec2(28.0, 28.0));
			if paint::icon_button(
				ui,
				ui.scope_id().with("thread-emoji"),
				smile,
				Icon::Smile,
				ButtonKind::Ghost,
				false,
				"Emoji",
			)
			.clicked()
			{
				solace.picker = Some((PickerTarget::Thread, smile.left_top() - vec2(220.0, 0.0)));
			}
			let edit = Rect::from_min_max(
				field.min + vec2(12.0, 0.0),
				pos2(smile.left() - 4.0, field.bottom()),
			);
			ui.add_enabled_ui(can_send, |ui| {
				ui.put(
					edit,
					TextEdit::singleline(&mut solace.thread_draft)
						.id(id)
						.frame(egui::Frame::NONE)
						.font(theme::sans(13.5))
						.text_color(p.text)
						.align(egui::Align2::LEFT_CENTER)
						.hint_text(
							egui::RichText::new("Reply...")
								.color(p.muted)
								.font(theme::sans(13.5)),
						)
						.desired_width(edit.width()),
				);
			});
			if enter && !solace.thread_draft.trim().is_empty() && can_send {
				// The thread shares the conversation's send path; keep the main draft intact.
				let channel = root.channel;
				let saved = state.drafts.remove(&channel);
				let saved_reply = state.reply.replace(Reply::to(root_id));
				state
					.drafts
					.insert(channel, std::mem::take(&mut solace.thread_draft));
				match state.prepare_send() {
					Some(command) => commands.push(command),
					None => {
						solace.thread_draft = state.drafts.remove(&channel).unwrap_or_default();
						solace.toasts.push(crate::Level::Warning, state.status);
					}
				}
				state.reply = saved_reply.filter(|r| r.target() != root_id);
				if let Some(saved) = saved {
					state.drafts.insert(channel, saved);
				} else {
					state.drafts.remove(&channel);
				}
				ui.ctx().memory_mut(|m| m.request_focus(id));
			}
		});
	if close || (ui.input(|i| i.key_pressed(Key::Escape)) && solace.picker.is_none()) {
		solace.thread = None;
	}
}

fn entry(solace: &mut Solace, ui: &mut Ui, state: &State, message: &Message, root: bool) {
	let p = theme::palette(ui.ctx());
	let text_w = WIDTH - 18.0 - 46.0 - 14.0;
	let names = Names {
		state,
		guild: state.channel(message.channel).and_then(|c| c.guild),
		mentions: &message.mentions,
	};
	let style = Style {
		size: 13.0,
		line_height: 19.0,
		color: if root { p.text } else { p.text_soft },
	};
	let laid = text::layout(
		ui.painter(),
		&message.display_text(),
		&names,
		&style,
		text_w,
		solace.revealed.contains(&message.id),
	);
	let image = root
		.then(|| message.attachments.iter().find(|a| a.is_image()))
		.flatten()
		.and_then(|a| {
			images::media_key(&a.media).map(|k| {
				(
					k,
					a.media.width.max(1) as f32 / a.media.height.max(1) as f32,
				)
			})
		});
	let image_h = if image.is_some() { 96.0 } else { 0.0 };
	let height = 28.0 + laid.size.y + image_h + 14.0;
	let (rect, _) = ui.allocate_exact_size(vec2(WIDTH, height), Sense::hover());
	let avatar = Rect::from_min_size(rect.min + vec2(18.0, 6.0), vec2(30.0, 30.0));
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
	let left = avatar.right() + 12.0;
	let name = paint::label(
		ui.painter(),
		pos2(left, avatar.top() + 8.0),
		Align2::LEFT_CENTER,
		&display_name(state, &message.author),
		theme::semibold(12.5),
		p.text,
	);
	paint::label(
		ui.painter(),
		pos2(name.right() + 8.0, name.center().y),
		Align2::LEFT_CENTER,
		&clock::clock(clock::snowflake(message.id)),
		theme::sans(10.5),
		p.faint,
	);
	let interaction = text::paint(
		ui,
		pos2(left, avatar.top() + 20.0),
		&laid,
		&mut solace.images,
		ui.scope_id().with(("thread-text", message.id)),
	);
	if let Some(link) = interaction.link {
		solace.link = Some(link);
	}
	if interaction.spoiler_clicked {
		solace.revealed.insert(message.id);
	}
	if let Some((key, _)) = image {
		let frame = Rect::from_min_size(
			pos2(left, avatar.top() + 26.0 + laid.size.y),
			vec2(text_w, image_h - 6.0),
		);
		match solace.images.get(&key) {
			Some((texture, size)) => paint::cover_image(ui.painter(), frame, 10, texture, size),
			None => {
				ui.painter().rect_filled(frame, 10, p.surface_alt);
				icons::paint_at(ui.painter(), Icon::Image, frame.center(), 20.0, p.faint);
			}
		}
	}
}
