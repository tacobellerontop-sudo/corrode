//! Popovers and dialogs: emoji picker, account menu, link confirmation, joining a
//! community and the settings dialog.
use crate::{
	PickerTarget, Request, SettingsPage, Solace, emoji,
	icons::{self, Icon},
	motion,
	paint::{self, AvatarShape, ButtonKind},
	theme::{self, Palette, mix},
};
use client_core::{Command, State};
use egui::{Align2, Area, Context, Id, Key, Order, Pos2, Rect, Sense, TextEdit, Ui, pos2, vec2};

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let ctx = ui.ctx().clone();
	if solace.picker.is_some() {
		picker(solace, &ctx, state, commands);
	}
	if solace.account_menu {
		account_menu(solace, &ctx, state);
	}
	if solace.link.is_some() {
		link_dialog(solace, &ctx);
	}
	if solace.join.is_some() {
		join_dialog(solace, &ctx, state, commands);
	}
	solace.media.settings_voice_open = solace.settings == Some(SettingsPage::Voice);
	if solace.settings.is_some() {
		settings(solace, &ctx, state);
	}
}

fn clicked_outside(ctx: &Context, rect: Rect) -> bool {
	ctx.input(|i| {
		i.pointer.any_pressed()
			&& i.pointer
				.interact_pos()
				.is_some_and(|pos| !rect.contains(pos))
	})
}

fn picker(solace: &mut Solace, ctx: &Context, state: &mut State, commands: &mut Vec<Command>) {
	let Some((target, anchor)) = solace.picker else {
		return;
	};
	let p = theme::palette(ctx);
	let size = vec2(332.0, 320.0);
	let screen = ctx.content_rect();
	let mut min = anchor - vec2(0.0, size.y + 8.0);
	min.x = min
		.x
		.clamp(screen.left() + 8.0, screen.right() - size.x - 8.0);
	min.y = min
		.y
		.clamp(screen.top() + 8.0, screen.bottom() - size.y - 8.0);
	let a = motion::appear(ctx, Id::unique("solace-picker-enter"), 0.18);
	let rect = Rect::from_min_size(min + vec2(0.0, (1.0 - a) * 10.0), size);
	let mut chosen: Option<&str> = None;
	Area::new(Id::unique("solace-picker"))
		.order(Order::Foreground)
		.fixed_pos(rect.min)
		.show(ctx, |ui| {
			ui.multiply_opacity(a);
			ui.set_clip_rect(rect.expand(60.0));
			let _ = ui.allocate_exact_size(size, Sense::click());
			paint::floating(ui.painter(), rect, 16, &p);
			let body = rect.shrink2(vec2(10.0, 10.0));
			let mut child =
				ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt("picker-body"));
			child.set_clip_rect(body);
			egui::ScrollArea::vertical()
				.auto_shrink([false, false])
				.show(&mut child, |ui| {
					for (group, items) in emoji::PICKER {
						let (header, _) =
							ui.allocate_exact_size(vec2(body.width(), 26.0), Sense::hover());
						paint::label(
							ui.painter(),
							header.left_center() + vec2(6.0, 0.0),
							Align2::LEFT_CENTER,
							group,
							theme::medium(11.5),
							p.muted,
						);
						let per_row = 8usize;
						for chunk in items.chunks(per_row) {
							let (row, _) =
								ui.allocate_exact_size(vec2(body.width(), 38.0), Sense::hover());
							for (i, item) in chunk.iter().enumerate() {
								let cell = Rect::from_min_size(
									row.min + vec2(i as f32 * 38.0, 0.0),
									vec2(38.0, 38.0),
								);
								let response = ui.interact(
									cell,
									ui.scope_id().with(("emoji", group, *item)),
									Sense::click(),
								);
								if response.hovered() {
									ui.painter().rect_filled(cell.shrink(2.0), 9, p.hover);
								}
								let size = if response.hovered() { 26.0 } else { 24.0 };
								emoji::paint(
									ui.painter(),
									Rect::from_center_size(cell.center(), vec2(size, size)),
									item,
								);
								if response.clicked() {
									chosen = Some(item);
								}
							}
						}
					}
				});
		});
	if let Some(item) = chosen {
		match target {
			PickerTarget::Composer => {
				if let Some(channel) = state.selected {
					state.drafts.entry(channel).or_default().push_str(item);
					ctx.memory_mut(|m| m.request_focus(crate::composer::input_id(channel)));
				}
			}
			PickerTarget::Thread => solace.thread_draft.push_str(item),
			PickerTarget::Reaction(message) => {
				let emoji = model::ReactionEmoji {
					id: None,
					name: Some(item.to_owned()),
				};
				commands.extend(state.prepare_reaction(message, emoji));
			}
		}
		if !ctx.input(|i| i.modifiers.shift) {
			solace.picker = None;
		}
	} else if ctx.input(|i| i.key_pressed(Key::Escape)) || clicked_outside(ctx, rect) {
		solace.picker = None;
	}
}

fn account_menu(solace: &mut Solace, ctx: &Context, state: &State) {
	let Some(user) = state.user.clone() else {
		solace.account_menu = false;
		return;
	};
	let p = theme::palette(ctx);
	let screen = ctx.content_rect();
	let others: Vec<model::SavedAccount> = solace
		.accounts
		.iter()
		.filter(|a| a.id != user.id)
		.cloned()
		.collect();
	let items = 4 + others.len();
	let size = vec2(260.0, 78.0 + items as f32 * 38.0 + 10.0);
	let a = motion::appear(ctx, Id::unique("solace-account-enter"), 0.18);
	let rect = Rect::from_min_size(
		pos2(
			screen.left() + 72.0 - (1.0 - a) * 8.0,
			screen.bottom() - size.y - 14.0,
		),
		size,
	);
	let mut action = None;
	Area::new(Id::unique("solace-account"))
		.order(Order::Foreground)
		.fixed_pos(rect.min)
		.show(ctx, |ui| {
			ui.multiply_opacity(a);
			ui.set_clip_rect(rect.expand(60.0));
			let _ = ui.allocate_exact_size(size, Sense::click());
			paint::floating(ui.painter(), rect, 16, &p);
			let avatar = Rect::from_min_size(rect.min + vec2(16.0, 16.0), vec2(42.0, 42.0));
			let texture = solace.images.get(&user.avatar_key());
			paint::avatar(
				ui.painter(),
				avatar,
				AvatarShape::Circle,
				texture,
				&user.name,
				user.id.0,
				&p,
			);
			paint::text_left(
				ui.painter(),
				pos2(avatar.right() + 12.0, avatar.center().y - 9.0),
				state.user_display_name(&user),
				theme::serif(16.0),
				p.text,
				170.0,
			);
			let status = if state.gateway_connected {
				"Online"
			} else {
				"Connecting..."
			};
			paint::label(
				ui.painter(),
				pos2(avatar.right() + 12.0, avatar.center().y + 11.0),
				Align2::LEFT_CENTER,
				status,
				theme::sans(12.0),
				if state.gateway_connected {
					p.online
				} else {
					p.muted
				},
			);
			paint::hairline(
				ui.painter(),
				pos2(rect.left() + 12.0, rect.top() + 72.0),
				pos2(rect.right() - 12.0, rect.top() + 72.0),
				&p,
			);
			let dark = ctx.theme() == egui::Theme::Dark;
			let mut rows: Vec<(Icon, String, u8)> = vec![
				(Icon::Settings, "Settings".into(), 0),
				(
					if dark { Icon::Sun } else { Icon::Moon },
					if dark {
						"Light mode".into()
					} else {
						"Dark mode".into()
					},
					1,
				),
			];
			for (i, other) in others.iter().enumerate() {
				rows.push((
					Icon::People,
					format!(
						"Switch to {}",
						other.display.as_deref().unwrap_or(&other.name)
					),
					10 + i as u8,
				));
			}
			rows.push((Icon::Plus, "Add an account".into(), 2));
			rows.push((Icon::Logout, "Log out".into(), 3));
			for (i, (icon, label, key)) in rows.iter().enumerate() {
				let row = Rect::from_min_size(
					pos2(rect.left() + 8.0, rect.top() + 80.0 + i as f32 * 38.0),
					vec2(size.x - 16.0, 36.0),
				);
				let response = ui.interact(row, ui.scope_id().with(("account", i)), Sense::click());
				if response.hovered() {
					ui.painter().rect_filled(row, 9, p.hover);
				}
				let color = if *key == 3 { p.danger } else { p.text };
				icons::paint_at(
					ui.painter(),
					*icon,
					row.left_center() + vec2(18.0, 0.0),
					15.0,
					if *key == 3 { p.danger } else { p.text_soft },
				);
				paint::text_left(
					ui.painter(),
					row.left_center() + vec2(38.0, 0.0),
					label,
					theme::sans(13.5),
					color,
					row.width() - 48.0,
				);
				if response.clicked() {
					action = Some(*key);
				}
			}
		});
	match action {
		Some(0) => solace.settings = Some(SettingsPage::Appearance),
		Some(1) => {
			let next = if ctx.theme() == egui::Theme::Dark {
				egui::ThemePreference::Light
			} else {
				egui::ThemePreference::Dark
			};
			solace.theme = next;
			ctx.set_theme(next);
			solace.request(Request::Theme(next));
		}
		Some(2) => solace.request(Request::AddAccount),
		Some(3) => solace.request(Request::Logout),
		Some(key) if key >= 10 => {
			if let Some(other) = others.get((key - 10) as usize) {
				solace.request(Request::SwitchAccount(other.id));
			}
		}
		_ => {}
	}
	if action.is_some()
		|| ctx.input(|i| i.key_pressed(Key::Escape))
		|| (clicked_outside(ctx, rect)
			&& !ctx.input(|i| {
				i.pointer
					.interact_pos()
					.is_some_and(|pos| pos.x < 80.0 && pos.y > screen.bottom() - 90.0)
			})) {
		solace.account_menu = false;
	}
}

/// Dim the window and return the centered dialog rect; `None` once dismissed by the scrim.
fn modal(
	ctx: &Context,
	id: &str,
	size: egui::Vec2,
	add: impl FnOnce(&mut Ui, Rect, &Palette),
) -> bool {
	let p = theme::palette(ctx);
	let screen = ctx.content_rect();
	// Dialogs fade in over a darkening scrim and settle from slightly below.
	let a = motion::appear(ctx, Id::unique((id, "enter")), 0.22);
	let rect = Rect::from_center_size(screen.center() + vec2(0.0, (1.0 - a) * 16.0), size);
	let mut dismissed = false;
	Area::new(Id::unique((id, "scrim")))
		.order(Order::Foreground)
		.fixed_pos(screen.min)
		.show(ctx, |ui| {
			let (_, response) = ui.allocate_exact_size(screen.size(), Sense::click());
			ui.painter()
				.rect_filled(screen, 0, p.scrim.gamma_multiply(a));
			if response.clicked()
				&& !rect.contains(response.interact_pointer_pos().unwrap_or(Pos2::ZERO))
			{
				dismissed = true;
			}
		});
	Area::new(Id::unique((id, "dialog")))
		.order(Order::Tooltip)
		.fixed_pos(rect.min)
		.show(ctx, |ui| {
			ui.multiply_opacity(a);
			ui.set_clip_rect(rect.expand(60.0));
			let _ = ui.allocate_exact_size(size, Sense::click());
			paint::floating(ui.painter(), rect, 20, &p);
			add(ui, rect, &p);
		});
	dismissed || ctx.input(|i| i.key_pressed(Key::Escape))
}

fn link_dialog(solace: &mut Solace, ctx: &Context) {
	let Some(url) = solace.link.clone() else {
		return;
	};
	let mut choice = None;
	let dismissed = modal(ctx, "link", vec2(440.0, 196.0), |ui, rect, p| {
		paint::label(
			ui.painter(),
			rect.left_top() + vec2(28.0, 40.0),
			Align2::LEFT_CENTER,
			"Leaving Solace",
			theme::serif(22.0),
			p.text,
		);
		paint::label(
			ui.painter(),
			rect.left_top() + vec2(28.0, 70.0),
			Align2::LEFT_CENTER,
			"This link opens in your browser:",
			theme::sans(13.0),
			p.muted,
		);
		let field = Rect::from_min_size(
			rect.left_top() + vec2(28.0, 86.0),
			vec2(rect.width() - 56.0, 34.0),
		);
		ui.painter().rect_filled(field, 10, p.surface_alt);
		paint::text_left(
			ui.painter(),
			field.left_center() + vec2(12.0, 0.0),
			&url,
			theme::mono(12.5),
			p.text,
			field.width() - 24.0,
		);
		let open = Rect::from_min_size(
			pos2(rect.right() - 28.0 - 120.0, rect.bottom() - 56.0),
			vec2(120.0, 36.0),
		);
		let cancel = Rect::from_min_size(pos2(open.left() - 108.0, open.top()), vec2(96.0, 36.0));
		if paint::pill_button(
			ui,
			ui.scope_id().with("link-cancel"),
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
			ui.scope_id().with("link-open"),
			open,
			Some(Icon::Globe),
			"Open link",
			ButtonKind::Primary,
			true,
		)
		.clicked()
		{
			choice = Some(true);
		}
	});
	if choice == Some(true) {
		solace.request(Request::OpenLink(url));
	}
	if choice.is_some() || dismissed {
		solace.link = None;
	}
}

fn invite_code(input: &str) -> Option<String> {
	let trimmed = input.trim().trim_end_matches('/');
	let code = [
		"discord.gg/",
		"discord.com/invite/",
		"discordapp.com/invite/",
	]
	.iter()
	.find_map(|prefix| trimmed.split_once(prefix).map(|(_, code)| code))
	.unwrap_or(trimmed);
	let code = code.split(['?', '#']).next().unwrap_or(code);
	(!code.is_empty()
		&& code.len() <= 64
		&& code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
	.then(|| code.to_owned())
}

fn join_dialog(solace: &mut Solace, ctx: &Context, state: &mut State, commands: &mut Vec<Command>) {
	let Some(mut input) = solace.join.clone() else {
		return;
	};
	let mut submit = false;
	let mut cancel = false;
	let code = invite_code(&input);
	let dismissed = modal(ctx, "join", vec2(460.0, 236.0), |ui, rect, p| {
		paint::label(
			ui.painter(),
			rect.left_top() + vec2(28.0, 40.0),
			Align2::LEFT_CENTER,
			"Join a community",
			theme::serif(22.0),
			p.text,
		);
		paint::label(
			ui.painter(),
			rect.left_top() + vec2(28.0, 70.0),
			Align2::LEFT_CENTER,
			"Paste an invite link or code.",
			theme::sans(13.0),
			p.muted,
		);
		let field = Rect::from_min_size(
			rect.left_top() + vec2(28.0, 92.0),
			vec2(rect.width() - 56.0, 42.0),
		);
		ui.painter().rect(
			field,
			12,
			p.canvas,
			egui::Stroke::new(1.0, p.border),
			egui::StrokeKind::Inside,
		);
		let id = Id::unique("join-input");
		let edit = field.shrink2(vec2(12.0, 0.0));
		let response = ui.put(
			edit,
			TextEdit::singleline(&mut input)
				.id(id)
				.frame(egui::Frame::NONE)
				.font(theme::sans(14.0))
				.text_color(p.text)
				.align(egui::Align2::LEFT_CENTER)
				.hint_text(
					egui::RichText::new("discord.gg/horizon")
						.color(p.faint)
						.font(theme::sans(14.0)),
				)
				.desired_width(edit.width()),
		);
		if !response.has_focus() && ctx.memory(|m| m.focused().is_none()) {
			response.request_focus();
		}
		if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
			submit = true;
		}
		let join = Rect::from_min_size(
			pos2(rect.right() - 28.0 - 120.0, rect.bottom() - 58.0),
			vec2(120.0, 38.0),
		);
		let back = Rect::from_min_size(pos2(join.left() - 108.0, join.top()), vec2(96.0, 38.0));
		if paint::pill_button(
			ui,
			ui.scope_id().with("join-cancel"),
			back,
			None,
			"Cancel",
			ButtonKind::Ghost,
			true,
		)
		.clicked()
		{
			cancel = true;
		}
		let enabled = code.as_ref().is_some_and(|c| state.can_join_invite(c));
		if paint::pill_button(
			ui,
			ui.scope_id().with("join-go"),
			join,
			None,
			"Join",
			ButtonKind::Primary,
			enabled,
		)
		.clicked()
		{
			submit = true;
		}
	});
	solace.join = Some(input);
	if submit && let Some(code) = code {
		match state.join_invite(code) {
			Some(command) => {
				commands.push(command);
				solace.toasts.push(crate::Level::Info, "Joining...");
				solace.join = None;
			}
			None => solace
				.toasts
				.push(crate::Level::Warning, "That invite can't be used right now"),
		}
	}
	if cancel || dismissed {
		solace.join = None;
	}
}

fn settings(solace: &mut Solace, ctx: &Context, state: &State) {
	let Some(page) = solace.settings else {
		return;
	};
	let screen = ctx.content_rect();
	let size = vec2(
		720.0f32.min(screen.width() - 40.0),
		500.0f32.min(screen.height() - 40.0),
	);
	let mut next = None;
	let mut close = false;
	let dismissed = modal(ctx, "settings", size, |ui, rect, p| {
		let nav = Rect::from_min_size(rect.min, vec2(196.0, rect.height()));
		ui.painter().rect_filled(
			nav,
			egui::CornerRadius {
				nw: 20,
				sw: 20,
				ne: 0,
				se: 0,
			},
			p.sidebar,
		);
		paint::label(
			ui.painter(),
			nav.left_top() + vec2(24.0, 40.0),
			Align2::LEFT_CENTER,
			"Settings",
			theme::serif(22.0),
			p.text,
		);
		for (i, (item, icon, label)) in [
			(SettingsPage::Appearance, Icon::Sun, "Appearance"),
			(SettingsPage::Account, Icon::People, "Account"),
			(SettingsPage::Voice, Icon::Mic, "Voice"),
			(SettingsPage::About, Icon::Logo, "About"),
		]
		.into_iter()
		.enumerate()
		{
			let row = Rect::from_min_size(
				nav.left_top() + vec2(12.0, 74.0 + i as f32 * 42.0),
				vec2(nav.width() - 24.0, 38.0),
			);
			let response =
				ui.interact(row, ui.scope_id().with(("settings-nav", i)), Sense::click());
			let active = page == item;
			if active || response.hovered() {
				ui.painter()
					.rect_filled(row, 10, if active { p.selected } else { p.hover });
			}
			icons::paint_at(
				ui.painter(),
				icon,
				row.left_center() + vec2(18.0, 0.0),
				16.0,
				if active { p.text } else { p.muted },
			);
			paint::label(
				ui.painter(),
				row.left_center() + vec2(38.0, 0.0),
				Align2::LEFT_CENTER,
				label,
				theme::sans(13.5),
				if active { p.text } else { p.text_soft },
			);
			if response.clicked() {
				next = Some(item);
			}
		}
		let x = Rect::from_center_size(rect.right_top() + vec2(-30.0, 32.0), vec2(32.0, 32.0));
		if paint::icon_button(
			ui,
			ui.scope_id().with("settings-close"),
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
		let body = Rect::from_min_max(
			pos2(nav.right() + 32.0, rect.top() + 24.0),
			rect.max - vec2(32.0, 24.0),
		);
		let mut child = ui.new_child(
			egui::UiBuilder::new()
				.max_rect(body)
				.id_salt(("settings-body", page as u8)),
		);
		child.set_clip_rect(body.expand(4.0));
		match page {
			SettingsPage::Appearance => appearance(solace, &mut child, body, p),
			SettingsPage::Account => account(solace, &mut child, body, state, p),
			SettingsPage::Voice => voice(solace, &mut child, body, p),
			SettingsPage::About => about(solace, &mut child, body, p),
		}
	});
	if let Some(next) = next {
		if next == SettingsPage::Voice {
			solace.media.voice_refresh_devices = true;
		}
		solace.settings = Some(next);
	}
	if close || dismissed {
		solace.settings = None;
		solace.media.voice_preview_requested = false;
	}
}

fn heading(ui: &Ui, at: Pos2, title: &str, subtitle: &str, p: &Palette) {
	paint::label(
		ui.painter(),
		at + vec2(0.0, 16.0),
		Align2::LEFT_CENTER,
		title,
		theme::serif(20.0),
		p.text,
	);
	paint::label(
		ui.painter(),
		at + vec2(0.0, 42.0),
		Align2::LEFT_CENTER,
		subtitle,
		theme::sans(12.5),
		p.muted,
	);
}

fn appearance(solace: &mut Solace, ui: &mut Ui, body: Rect, p: &Palette) {
	heading(
		ui,
		body.min,
		"Appearance",
		"Solace paints every pixel itself on the GPU.",
		p,
	);
	let ctx = ui.ctx().clone();
	let options = [
		(
			egui::ThemePreference::Light,
			"Paper",
			Icon::Sun,
			theme::LIGHT,
		),
		(
			egui::ThemePreference::Dark,
			"Night",
			Icon::Moon,
			theme::DARK,
		),
		(
			egui::ThemePreference::System,
			"Match system",
			Icon::Settings,
			theme::LIGHT,
		),
	];
	let card_w = ((body.width() - 24.0) / 3.0).min(150.0);
	for (i, (pref, label, icon, preview)) in options.into_iter().enumerate() {
		let card = Rect::from_min_size(
			body.min + vec2(i as f32 * (card_w + 12.0), 78.0),
			vec2(card_w, 132.0),
		);
		let response = ui.interact(card, ui.scope_id().with(("theme", i)), Sense::click());
		let active = solace.theme == pref;
		ui.painter().rect(
			card,
			14,
			p.surface,
			egui::Stroke::new(
				if active { 2.0 } else { 1.0 },
				if active { p.accent } else { p.border },
			),
			egui::StrokeKind::Inside,
		);
		let swatch = Rect::from_min_size(card.min + vec2(10.0, 10.0), vec2(card_w - 20.0, 76.0));
		if pref == egui::ThemePreference::System {
			let half = Rect::from_min_size(swatch.min, vec2(swatch.width() / 2.0, swatch.height()));
			ui.painter().rect_filled(
				half,
				egui::CornerRadius {
					nw: 10,
					sw: 10,
					ne: 0,
					se: 0,
				},
				theme::LIGHT.canvas,
			);
			ui.painter().rect_filled(
				half.translate(vec2(half.width(), 0.0)),
				egui::CornerRadius {
					nw: 0,
					sw: 0,
					ne: 10,
					se: 10,
				},
				theme::DARK.canvas,
			);
		} else {
			ui.painter().rect_filled(swatch, 10, preview.canvas);
			ui.painter().rect_filled(
				Rect::from_min_size(swatch.min + vec2(10.0, 10.0), vec2(40.0, 56.0)),
				6,
				preview.sidebar,
			);
			ui.painter().rect_filled(
				Rect::from_min_size(
					swatch.min + vec2(58.0, 14.0),
					vec2(swatch.width() - 70.0, 10.0),
				),
				5,
				preview.border,
			);
			ui.painter().rect_filled(
				Rect::from_min_size(
					swatch.min + vec2(58.0, 52.0),
					vec2(swatch.width() - 70.0, 14.0),
				),
				7,
				preview.accent,
			);
		}
		icons::paint_at(
			ui.painter(),
			icon,
			pos2(card.left() + 22.0, card.bottom() - 22.0),
			15.0,
			p.text_soft,
		);
		paint::label(
			ui.painter(),
			pos2(card.left() + 38.0, card.bottom() - 22.0),
			Align2::LEFT_CENTER,
			label,
			theme::medium(13.0),
			p.text,
		);
		if response.clicked() {
			solace.theme = pref;
			ctx.set_theme(pref);
			solace.request(Request::Theme(pref));
		}
	}
	paint::label(
		ui.painter(),
		body.min + vec2(0.0, 240.0),
		Align2::LEFT_CENTER,
		"Typography",
		theme::medium(14.0),
		p.text,
	);
	paint::label(
		ui.painter(),
		body.min + vec2(0.0, 270.0),
		Align2::LEFT_CENTER,
		"Titles · Source Serif 4 Display",
		theme::serif(16.0),
		p.text_soft,
	);
	paint::label(
		ui.painter(),
		body.min + vec2(0.0, 296.0),
		Align2::LEFT_CENTER,
		"Interface · Inter",
		theme::sans(14.0),
		p.text_soft,
	);
}

fn account(solace: &mut Solace, ui: &mut Ui, body: Rect, state: &State, p: &Palette) {
	heading(
		ui,
		body.min,
		"Account",
		"Signed in with your Discord account.",
		p,
	);
	let Some(user) = state.user.clone() else {
		return;
	};
	let card = Rect::from_min_size(body.min + vec2(0.0, 76.0), vec2(body.width(), 96.0));
	paint::card(ui.painter(), card, 16, p);
	let avatar = Rect::from_min_size(card.min + vec2(18.0, 18.0), vec2(60.0, 60.0));
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
		pos2(avatar.right() + 16.0, avatar.center().y - 10.0),
		state.user_display_name(&user),
		theme::serif(20.0),
		p.text,
		card.width() - 120.0,
	);
	paint::label(
		ui.painter(),
		pos2(avatar.right() + 16.0, avatar.center().y + 14.0),
		Align2::LEFT_CENTER,
		&format!("@{}", user.name),
		theme::sans(12.5),
		p.muted,
	);
	let mut y = card.bottom() + 24.0;
	let others: Vec<model::SavedAccount> = solace
		.accounts
		.iter()
		.filter(|a| a.id != user.id)
		.cloned()
		.collect();
	if !others.is_empty() {
		paint::label(
			ui.painter(),
			pos2(body.left(), y + 8.0),
			Align2::LEFT_CENTER,
			"Other accounts on this device",
			theme::medium(13.5),
			p.text,
		);
		y += 24.0;
		for other in others {
			let row = Rect::from_min_size(pos2(body.left(), y), vec2(body.width(), 44.0));
			let response = ui.interact(
				row,
				ui.scope_id().with(("switch", other.id)),
				Sense::click(),
			);
			if response.hovered() {
				ui.painter().rect_filled(row, 10, p.hover);
			}
			let user = model::User {
				id: other.id,
				name: other.name.clone(),
				avatar: other.avatar.clone(),
				discriminator: other.discriminator,
				kind: Default::default(),
				webhook: false,
				primary_guild: None,
			};
			let a = Rect::from_min_size(row.min + vec2(8.0, 6.0), vec2(32.0, 32.0));
			let texture = solace.images.get(&user.avatar_key());
			paint::avatar(
				ui.painter(),
				a,
				AvatarShape::Circle,
				texture,
				&user.name,
				user.id.0,
				p,
			);
			paint::label(
				ui.painter(),
				pos2(a.right() + 12.0, row.center().y),
				Align2::LEFT_CENTER,
				other.display.as_deref().unwrap_or(&other.name),
				theme::sans(13.5),
				p.text,
			);
			paint::label(
				ui.painter(),
				pos2(row.right() - 12.0, row.center().y),
				Align2::RIGHT_CENTER,
				"Switch",
				theme::medium(12.5),
				p.accent,
			);
			if response.clicked() {
				solace.request(Request::SwitchAccount(other.id));
			}
			y += 46.0;
		}
	}
	let add = Rect::from_min_size(pos2(body.left(), body.bottom() - 40.0), vec2(150.0, 38.0));
	if paint::pill_button(
		ui,
		ui.scope_id().with("add-account"),
		add,
		Some(Icon::Plus),
		"Add account",
		ButtonKind::Ghost,
		true,
	)
	.clicked()
	{
		solace.request(Request::AddAccount);
	}
	let logout = Rect::from_min_size(
		pos2(body.right() - 130.0, body.bottom() - 40.0),
		vec2(130.0, 38.0),
	);
	if paint::pill_button(
		ui,
		ui.scope_id().with("logout"),
		logout,
		Some(Icon::Logout),
		"Log out",
		ButtonKind::Danger,
		true,
	)
	.clicked()
	{
		solace.request(Request::Logout);
		solace.settings = None;
	}
}

fn device_list(
	ui: &mut Ui,
	at: Pos2,
	width: f32,
	title: &str,
	devices: &[(String, String)],
	selected: &mut Option<String>,
	p: &Palette,
) -> f32 {
	paint::label(
		ui.painter(),
		at + vec2(0.0, 8.0),
		Align2::LEFT_CENTER,
		title,
		theme::medium(13.5),
		p.text,
	);
	let mut y = at.y + 24.0;
	let mut options: Vec<(Option<String>, String)> = vec![(None, "System default".into())];
	options.extend(
		devices
			.iter()
			.map(|(id, label)| (Some(id.clone()), label.clone())),
	);
	for (i, (id, label)) in options.into_iter().take(6).enumerate() {
		let row = Rect::from_min_size(pos2(at.x, y), vec2(width, 34.0));
		let response = ui.interact(row, ui.scope_id().with((title, i)), Sense::click());
		let active = *selected == id;
		if active || response.hovered() {
			ui.painter()
				.rect_filled(row, 9, if active { p.accent_soft } else { p.hover });
		}
		ui.painter().circle_stroke(
			row.left_center() + vec2(16.0, 0.0),
			6.5,
			egui::Stroke::new(1.5, if active { p.accent } else { p.faint }),
		);
		if active {
			ui.painter()
				.circle_filled(row.left_center() + vec2(16.0, 0.0), 3.5, p.accent);
		}
		paint::text_left(
			ui.painter(),
			row.left_center() + vec2(32.0, 0.0),
			&label,
			theme::sans(13.0),
			p.text,
			width - 44.0,
		);
		if response.clicked() {
			*selected = id;
		}
		y += 36.0;
	}
	y - at.y
}

fn voice(solace: &mut Solace, ui: &mut Ui, body: Rect, p: &Palette) {
	// The page is taller than the dialog; it scrolls inside the settings body.
	egui::ScrollArea::vertical()
		.id_salt("voice-settings")
		.auto_shrink([false, false])
		.show(ui, |ui| {
			let (content, _) =
				ui.allocate_exact_size(vec2(body.width() - 10.0, VOICE_PAGE), Sense::hover());
			voice_page(solace, ui, content, p);
		});
}

const VOICE_PAGE: f32 = 640.0;

/// One labelled volume slider row.
fn volume_row(ui: &mut Ui, at: Pos2, width: f32, label: &str, value: &mut u16, p: &Palette) {
	paint::label(
		ui.painter(),
		at + vec2(0.0, 10.0),
		Align2::LEFT_CENTER,
		label,
		theme::medium(13.5),
		p.text,
	);
	paint::label(
		ui.painter(),
		at + vec2(width, 10.0),
		Align2::RIGHT_CENTER,
		&format!("{value}%"),
		theme::sans(12.5),
		p.muted,
	);
	let track = Rect::from_min_size(at + vec2(-8.0, 22.0), vec2(width + 16.0, 24.0));
	paint::slider(
		ui,
		ui.scope_id().with(("volume", label)),
		track,
		value,
		crate::media::MAX_VOLUME,
		p.surface_alt,
		p.accent,
		p.surface,
	);
}

fn voice_page(solace: &mut Solace, ui: &mut Ui, body: Rect, p: &Palette) {
	heading(
		ui,
		body.min,
		"Voice",
		"Choose the devices Solace uses for calls.",
		p,
	);
	let half = (body.width() - 20.0) / 2.0;
	let media = &mut solace.media;
	let inputs = media.voice_inputs.clone();
	let outputs = media.voice_outputs.clone();
	let h1 = device_list(
		ui,
		body.min + vec2(0.0, 74.0),
		half,
		"Microphone",
		&inputs,
		&mut media.voice_input,
		p,
	);
	let h2 = device_list(
		ui,
		body.min + vec2(half + 20.0, 74.0),
		half,
		"Speakers",
		&outputs,
		&mut media.voice_output,
		p,
	);
	let y = body.top() + 74.0 + h1.max(h2) + 16.0;
	let meter = Rect::from_min_size(pos2(body.left(), y), vec2(body.width() - 150.0, 10.0));
	ui.painter().rect_filled(meter, 5, p.surface_alt);
	if let Some(level) = media.voice_preview_level {
		let fill = Rect::from_min_size(
			meter.min,
			vec2(meter.width() * level.clamp(0.0, 1.0), meter.height()),
		);
		ui.painter()
			.rect_filled(fill, 5, mix(p.online, p.accent, 0.3));
		// The engine refreshes the level about 20 times a second; drawing faster is waste.
		ui.ctx()
			.request_repaint_after(std::time::Duration::from_millis(50));
	}
	let test = Rect::from_min_size(pos2(meter.right() + 16.0, y - 14.0), vec2(134.0, 36.0));
	let label = if media.voice_preview_requested {
		"Stop test"
	} else {
		"Test mic"
	};
	if paint::pill_button(
		ui,
		ui.scope_id().with("mic-test"),
		test,
		Some(Icon::Mic),
		label,
		ButtonKind::Ghost,
		true,
	)
	.clicked()
	{
		media.voice_preview_requested = !media.voice_preview_requested;
	}
	let status = [media.voice_preview_status, media.voice_device_status]
		.into_iter()
		.find(|s| !s.is_empty())
		.unwrap_or("");
	if !status.is_empty() {
		paint::text_left(
			ui.painter(),
			pos2(body.left(), y + 34.0),
			status,
			theme::sans(12.0),
			p.muted,
			body.width(),
		);
	}
	let y = y + 66.0;
	paint::label(
		ui.painter(),
		pos2(body.left(), y),
		Align2::LEFT_CENTER,
		"Volume",
		theme::serif(17.0),
		p.text,
	);
	let half = (body.width() - 32.0) / 2.0;
	volume_row(
		ui,
		pos2(body.left(), y + 20.0),
		half,
		"Input volume",
		&mut media.voice_gain.input_percent,
		p,
	);
	volume_row(
		ui,
		pos2(body.left() + half + 32.0, y + 20.0),
		half,
		"Output volume",
		&mut media.voice_gain.output_percent,
		p,
	);
	volume_row(
		ui,
		pos2(body.left(), y + 84.0),
		half,
		"Stream volume",
		&mut media.stream_volume,
		p,
	);
	let adjusted = media.user_volumes.len() + media.muted_users.len();
	let reset = Rect::from_min_size(
		pos2(body.left() + half + 32.0, y + 100.0),
		vec2(half.min(240.0), 34.0),
	);
	let note = match adjusted {
		0 => "Everyone plays at 100%".to_owned(),
		1 => "Reset 1 person's volume".to_owned(),
		n => format!("Reset {n} people's volumes"),
	};
	if paint::pill_button(
		ui,
		ui.scope_id().with("reset-volumes"),
		reset,
		Some(Icon::Speaker),
		&note,
		ButtonKind::Ghost,
		adjusted > 0,
	)
	.clicked()
	{
		media.user_volumes.clear();
		media.muted_users.clear();
	}
	paint::text_left(
		ui.painter(),
		pos2(body.left(), y + 152.0),
		"Set one person's volume from their tile in a call.",
		theme::sans(12.0),
		p.muted,
		body.width(),
	);
	let toggle = Rect::from_min_size(pos2(body.left(), y + 180.0), vec2(body.width(), 36.0));
	let response = ui.interact(toggle, ui.scope_id().with("ptt"), Sense::click());
	paint::label(
		ui.painter(),
		toggle.left_center(),
		Align2::LEFT_CENTER,
		"Push to talk (hold V while Solace is focused)",
		theme::sans(13.5),
		p.text,
	);
	let switch = Rect::from_center_size(toggle.right_center() - vec2(24.0, 0.0), vec2(44.0, 24.0));
	let t = ui.ctx().animate_bool_with_time(
		ui.scope_id().with("ptt-anim"),
		media.voice_push_to_talk,
		0.15,
	);
	ui.painter()
		.rect_filled(switch, 12, mix(p.border, p.accent, t));
	ui.painter().circle_filled(
		pos2(switch.left() + 12.0 + t * 20.0, switch.center().y),
		9.0,
		egui::Color32::WHITE,
	);
	if response.clicked() {
		media.voice_push_to_talk = !media.voice_push_to_talk;
	}
}

fn about(solace: &mut Solace, ui: &mut Ui, body: Rect, p: &Palette) {
	heading(
		ui,
		body.min,
		"About Solace",
		"A calm, custom client for your communities.",
		p,
	);
	icons::paint_weighted(
		ui.painter(),
		Icon::Logo,
		Rect::from_min_size(body.min + vec2(0.0, 84.0), vec2(48.0, 48.0)),
		p.text,
		1.2,
	);
	paint::label(
		ui.painter(),
		body.min + vec2(64.0, 98.0),
		Align2::LEFT_CENTER,
		&format!("Solace {}", env!("CARGO_PKG_VERSION")),
		theme::serif(18.0),
		p.text,
	);
	paint::label(
		ui.painter(),
		body.min + vec2(64.0, 122.0),
		Align2::LEFT_CENTER,
		"Custom-painted interface · GPU rendered with wgpu",
		theme::sans(12.5),
		p.muted,
	);
	let lines = [
		"Solace connects to Discord directly from this device.",
		"There is no Solace server: messages, drafts and images stay in local, bounded caches.",
		"Fonts: Inter and Source Serif 4 (SIL OFL). Emoji: Twemoji (CC-BY 4.0).",
	];
	for (i, line) in lines.iter().enumerate() {
		paint::text_left(
			ui.painter(),
			body.min + vec2(0.0, 166.0 + i as f32 * 24.0),
			line,
			theme::sans(13.0),
			p.text_soft,
			body.width(),
		);
	}
	if !solace.storage_status.is_empty() {
		paint::text_left(
			ui.painter(),
			body.min + vec2(0.0, 250.0),
			solace.storage_status,
			theme::sans(12.0),
			p.muted,
			body.width(),
		);
	}
	let clear = Rect::from_min_size(pos2(body.left(), body.bottom() - 40.0), vec2(190.0, 38.0));
	if paint::pill_button(
		ui,
		ui.scope_id().with("clear-cache"),
		clear,
		Some(Icon::Trash),
		"Clear cached history",
		ButtonKind::Ghost,
		true,
	)
	.clicked()
	{
		solace.request(Request::ClearCache);
		solace
			.toasts
			.push(crate::Level::Success, "Cached history cleared");
	}
}

#[cfg(test)]
mod tests {
	use super::invite_code;

	#[test]
	fn invite_codes_are_extracted_and_bounded() {
		assert_eq!(
			invite_code("https://discord.gg/horizon").as_deref(),
			Some("horizon")
		);
		assert_eq!(
			invite_code("discord.com/invite/AbC-12?x=1").as_deref(),
			Some("AbC-12")
		);
		assert_eq!(invite_code(" plain "), Some("plain".into()));
		assert_eq!(invite_code("bad code"), None);
		assert_eq!(invite_code(""), None);
	}
}
