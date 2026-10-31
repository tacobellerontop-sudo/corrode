//! People: the profile card opened from a name or avatar, and the add-friend dialog.
use crate::{
	SettingsPage, Solace,
	aside::find_user,
	menus::Confirm,
	nav,
	paint::{self, AvatarShape, ButtonKind},
	theme::{self, mix},
};
use client_core::{Command, State};
use egui::{Align2, Color32, Key, Pos2, Rect, Sense, TextEdit, Ui, pos2, vec2};
use model::Id;

const WIDTH: f32 = 320.0;
const BANNER: f32 = 92.0;

/// Open the profile card for `user`, anchored near `at` (screen position).
pub(crate) fn open(
	solace: &mut Solace,
	state: &mut State,
	commands: &mut Vec<Command>,
	user: Id,
	at: Pos2,
) {
	let guild = nav::selected_channel(state).and_then(|c| c.guild);
	commands.extend(state.request_profile(user, guild));
	solace.profile_at = Some(at);
}

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	if solace.profile_at.is_some() {
		card(solace, ui, state, commands);
	}
	if solace.add_friend.is_some() {
		add_friend(solace, ui, state, commands);
	}
}

fn rgb(color: u32) -> Color32 {
	Color32::from_rgb((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

#[derive(Clone, Copy)]
enum Action {
	Message,
	AddFriend,
	RemoveFriend,
	Accept,
	Block,
	Unblock,
	EditOwn,
	CopyId,
}

fn card(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let ctx = ui.ctx().clone();
	let (Some(anchor), Some(view)) = (solace.profile_at, state.profile.as_ref()) else {
		solace.profile_at = None;
		return;
	};
	let p = theme::palette(&ctx);
	let user_id = view.user;
	let data = view.data.clone();
	let loading = view.loading;
	let error = view.error;
	let Some(user) = data
		.as_ref()
		.map(|d| d.user.clone())
		.or_else(|| find_user(state, user_id))
	else {
		solace.profile_at = None;
		commands.push(state.clear_profile());
		return;
	};
	let own = state.user.as_ref().is_some_and(|u| u.id == user_id);
	let display = data
		.as_ref()
		.and_then(|d| d.guild.as_ref().and_then(|g| g.nick.clone()))
		.or_else(|| data.as_ref().and_then(|d| d.global_name.clone()))
		.unwrap_or_else(|| crate::display_name(state, &user));
	let username = data
		.as_ref()
		.map(|d| d.username.clone())
		.unwrap_or_else(|| user.name.clone());
	let pronouns = data
		.as_ref()
		.map(|d| match &d.guild {
			Some(g) if !g.pronouns.is_empty() => g.pronouns.clone(),
			_ => d.pronouns.clone(),
		})
		.unwrap_or_default();
	let bio = data
		.as_ref()
		.map(|d| match &d.guild {
			Some(g) if !g.bio.is_empty() => g.bio.clone(),
			_ => d.bio.clone(),
		})
		.unwrap_or_default();
	let mutual = data.as_ref().map_or(0, |d| d.mutual_guilds.len());
	let guild = view.guild;
	let roles: Vec<(String, u32)> = match (guild, data.as_ref().and_then(|d| d.guild.as_ref())) {
		(Some(guild), Some(member)) => state
			.guild_roles(guild)
			.map(|known| {
				known
					.iter()
					.filter(|r| member.roles.contains(&r.id))
					.take(6)
					.map(|r| (r.name.clone(), r.color))
					.collect()
			})
			.unwrap_or_default(),
		_ => Vec::new(),
	};
	let banner_color = data
		.as_ref()
		.and_then(|d| d.theme_colors.map(|c| c[0]).or(d.accent_color))
		.map(rgb)
		.unwrap_or_else(|| theme::seeded_hue(user.id.0, p.dark));
	let status = nav::status_of(state, user_id);
	let custom = nav::custom_status_of(state, user_id).map(str::to_owned);
	let friend = state.friend(user_id).is_some();
	let request = state
		.pending_friends()
		.find(|(u, _, _)| u.id == user_id)
		.map(|(_, _, incoming)| *incoming);
	let blocked = state.user_blocked(user_id) == Some(true);
	let dm = nav::dm_with(state, user_id);

	// Body text wraps within the card; measure it to size the card.
	let inner = WIDTH - 40.0;
	let bio_galley = (!bio.trim().is_empty()).then(|| {
		ctx.fonts_mut(|f| {
			f.layout(
				bio.chars().take(400).collect(),
				theme::sans(12.5),
				p.text_soft,
				inner,
			)
		})
	});
	let mut height = BANNER + 44.0 + 52.0;
	if custom.is_some() {
		height += 22.0;
	}
	if let Some(g) = &bio_galley {
		height += 26.0 + g.size().y.min(120.0) + 6.0;
	}
	if !roles.is_empty() {
		height += 26.0 + 30.0 * roles.len().div_ceil(3) as f32;
	}
	if mutual > 0 {
		height += 22.0;
	}
	if loading || error.is_some() && data.is_none() && !state.demo {
		height += 22.0;
	}
	height += 64.0;

	let screen = ctx.content_rect();
	let mut min = anchor;
	if min.x + WIDTH > screen.right() - 12.0 {
		min.x = anchor.x - WIDTH - 60.0;
	}
	min.x = min
		.x
		.clamp(screen.left() + 12.0, screen.right() - WIDTH - 12.0);
	min.y = min.y.clamp(
		screen.top() + 12.0,
		(screen.bottom() - height - 12.0).max(screen.top() + 12.0),
	);
	let a = crate::motion::appear(&ctx, egui::Id::unique(("profile-enter", user_id)), 0.18);
	let rect = Rect::from_min_size(min + vec2(0.0, (1.0 - a) * 8.0), vec2(WIDTH, height));
	let mut chosen = None;
	egui::Area::new(egui::Id::unique("solace-profile"))
		.order(egui::Order::Foreground)
		.fixed_pos(rect.min)
		.show(&ctx, |ui| {
			ui.multiply_opacity(a);
			ui.set_clip_rect(rect.expand(60.0));
			let _ = ui.allocate_exact_size(rect.size(), Sense::click());
			paint::floating(ui.painter(), rect, 18, &p);
			let banner = Rect::from_min_size(rect.min, vec2(WIDTH, BANNER));
			let bottom = data
				.as_ref()
				.and_then(|d| d.theme_colors.map(|c| rgb(c[1])))
				.unwrap_or_else(|| mix(banner_color, p.surface, 0.45));
			paint::gradient(
				ui.painter(),
				banner,
				18.0,
				&[(0.0, banner_color), (1.0, bottom)],
			);
			// Square off the banner's lower corners against the card body.
			ui.painter().rect_filled(
				Rect::from_min_max(
					pos2(rect.left(), banner.bottom() - 18.0),
					banner.right_bottom(),
				),
				0,
				bottom,
			);
			let avatar_rect = Rect::from_min_size(
				pos2(rect.left() + 18.0, banner.bottom() - 40.0),
				vec2(80.0, 80.0),
			);
			ui.painter()
				.circle_filled(avatar_rect.center(), 44.0, p.surface);
			let texture = solace.images.get(&user.avatar_key());
			paint::avatar(
				ui.painter(),
				avatar_rect,
				AvatarShape::Circle,
				texture,
				&user.name,
				user.id.0,
				&p,
			);
			if let Some(color) = paint::status_color(status, &p) {
				paint::presence(
					ui.painter(),
					avatar_rect.right_bottom() - vec2(10.0, 10.0),
					8.0,
					color,
					p.surface,
				);
			}
			let mut y = banner.bottom() + 54.0;
			let left = rect.left() + 20.0;
			paint::text_left(
				ui.painter(),
				pos2(left, y),
				&display,
				theme::serif(21.0),
				p.text,
				inner,
			);
			y += 24.0;
			let handle = if pronouns.is_empty() {
				format!("@{username}")
			} else {
				format!("@{username}  ·  {pronouns}")
			};
			paint::text_left(
				ui.painter(),
				pos2(left, y),
				&handle,
				theme::sans(12.5),
				p.muted,
				inner,
			);
			y += 22.0;
			if let Some(custom) = &custom {
				paint::text_left(
					ui.painter(),
					pos2(left, y),
					custom,
					theme::sans(12.5),
					p.text_soft,
					inner,
				);
				y += 22.0;
			}
			let section = |ui: &Ui, y: &mut f32, title: &str| {
				paint::label(
					ui.painter(),
					pos2(left, *y + 6.0),
					Align2::LEFT_CENTER,
					title,
					theme::medium(11.0),
					p.muted,
				);
				*y += 20.0;
			};
			if let Some(galley) = bio_galley.clone() {
				y += 6.0;
				section(ui, &mut y, "ABOUT");
				let clip =
					Rect::from_min_size(pos2(left, y), vec2(inner, galley.size().y.min(120.0)));
				ui.painter()
					.with_clip_rect(clip)
					.galley(clip.min, galley, p.text_soft);
				y = clip.bottom() + 6.0;
			}
			if !roles.is_empty() {
				y += 6.0;
				section(ui, &mut y, "ROLES");
				let mut x = left;
				for (name, color) in &roles {
					let galley =
						ui.painter()
							.layout_no_wrap(name.clone(), theme::sans(11.5), p.text);
					let w = (galley.size().x + 26.0).min(inner);
					if x + w > left + inner {
						x = left;
						y += 30.0;
					}
					let chip = Rect::from_min_size(pos2(x, y), vec2(w, 24.0));
					ui.painter().rect_filled(chip, 8, p.surface_alt);
					let dot = if *color == 0 { p.muted } else { rgb(*color) };
					ui.painter()
						.circle_filled(chip.left_center() + vec2(10.0, 0.0), 4.0, dot);
					ui.painter().galley(
						pos2(chip.left() + 18.0, chip.center().y - galley.size().y / 2.0),
						galley,
						p.text,
					);
					x += w + 6.0;
				}
				y += 30.0;
			}
			if mutual > 0 {
				paint::text_left(
					ui.painter(),
					pos2(left, y + 8.0),
					&format!(
						"{mutual} mutual communit{}",
						if mutual == 1 { "y" } else { "ies" }
					),
					theme::sans(12.0),
					p.muted,
					inner,
				);
				y += 22.0;
			}
			if loading {
				paint::text_left(
					ui.painter(),
					pos2(left, y + 8.0),
					"Loading profile...",
					theme::sans(12.0),
					p.faint,
					inner,
				);
			} else if let (Some(error), None, false) = (error, &data, state.demo) {
				paint::text_left(
					ui.painter(),
					pos2(left, y + 8.0),
					error,
					theme::sans(12.0),
					p.faint,
					inner,
				);
			}

			// Actions along the bottom.
			let mut buttons: Vec<(&str, Action, ButtonKind)> = Vec::new();
			if own {
				buttons.push(("Edit profile", Action::EditOwn, ButtonKind::Primary));
			} else {
				if dm.is_some() || friend {
					buttons.push(("Message", Action::Message, ButtonKind::Primary));
				}
				if blocked {
					buttons.push(("Unblock", Action::Unblock, ButtonKind::Ghost));
				} else if friend {
					buttons.push(("Remove friend", Action::RemoveFriend, ButtonKind::Ghost));
				} else if request == Some(true) {
					buttons.push(("Accept request", Action::Accept, ButtonKind::Primary));
				} else if request.is_none() && user.account_label().is_none() {
					buttons.push(("Add friend", Action::AddFriend, ButtonKind::Ghost));
				}
				if !blocked && buttons.len() < 3 {
					buttons.push(("Block", Action::Block, ButtonKind::Ghost));
				}
			}
			let row_y = rect.bottom() - 30.0;
			let count = buttons.len().max(1) as f32;
			let gap = 8.0;
			let more = 36.0;
			let w = ((inner - more - gap) - gap * (count - 1.0)) / count;
			let mut x = left;
			for (i, (label, action, kind)) in buttons.into_iter().enumerate() {
				let button = Rect::from_min_size(pos2(x, row_y - 17.0), vec2(w, 34.0));
				if paint::pill_button(
					ui,
					ui.scope_id().with(("profile-action", i)),
					button,
					None,
					label,
					kind,
					!state.user_action_pending(),
				)
				.clicked()
				{
					chosen = Some(action);
				}
				x += w + gap;
			}
			let copy = Rect::from_min_size(
				pos2(rect.right() - 20.0 - more, row_y - 17.0),
				vec2(more, 34.0),
			);
			if paint::icon_button(
				ui,
				ui.scope_id().with("profile-copy"),
				copy,
				crate::icons::Icon::More,
				ButtonKind::Soft,
				false,
				"Copy user ID",
			)
			.clicked()
			{
				chosen = Some(Action::CopyId);
			}
		});
	let outside = ctx.input(|i| {
		i.pointer.any_pressed()
			&& i.pointer
				.interact_pos()
				.is_some_and(|pos| !rect.contains(pos))
	});
	let mut close = outside || ctx.input(|i| i.key_pressed(Key::Escape)) || chosen.is_some();
	match chosen {
		Some(Action::Message) => {
			if let Some(channel) = dm {
				commands.extend(state.select(channel));
			} else {
				commands.extend(state.open_friend_dm(user_id));
			}
		}
		Some(Action::AddFriend) => {
			if let Some(command) = state.add_profile_friend(user_id) {
				commands.push(command);
			} else {
				solace.toasts.push(
					crate::Level::Info,
					"A friend request can't be sent right now",
				);
			}
		}
		Some(Action::Accept) => commands.extend(state.resolve_friend_request(user_id, true)),
		Some(Action::RemoveFriend) => solace.confirm = Some(Confirm::RemoveFriend(user_id)),
		Some(Action::Block) => solace.confirm = Some(Confirm::Block(user_id)),
		Some(Action::Unblock) => commands.extend(state.set_user_blocked(user_id, false)),
		Some(Action::EditOwn) => solace.settings = Some(SettingsPage::Account),
		Some(Action::CopyId) => {
			ctx.copy_text(user_id.0.to_string());
			solace.toasts.push(crate::Level::Info, "Copied user ID");
			close = false;
		}
		None => {}
	}
	if close {
		solace.profile_at = None;
		commands.push(state.clear_profile());
	}
}

fn add_friend(solace: &mut Solace, ui: &mut Ui, state: &mut State, commands: &mut Vec<Command>) {
	let Some(mut input) = solace.add_friend.clone() else {
		return;
	};
	let ctx = ui.ctx().clone();
	let mut submit = false;
	let mut cancel = false;
	let dismissed =
		crate::overlays::modal(&ctx, "add-friend", vec2(460.0, 236.0), |ui, rect, p| {
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(28.0, 40.0),
				Align2::LEFT_CENTER,
				"Add a friend",
				theme::serif(22.0),
				p.text,
			);
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(28.0, 70.0),
				Align2::LEFT_CENTER,
				"Send a request using their Discord username.",
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
			let id = egui::Id::unique("add-friend-input");
			let edit = field.shrink2(vec2(12.0, 0.0));
			let response = ui.put(
				edit,
				TextEdit::singleline(&mut input)
					.id(id)
					.frame(egui::Frame::NONE)
					.font(theme::sans(14.0))
					.text_color(p.text)
					.align(Align2::LEFT_CENTER)
					.hint_text(
						egui::RichText::new("username")
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
			let send = Rect::from_min_size(
				pos2(rect.right() - 28.0 - 140.0, rect.bottom() - 58.0),
				vec2(140.0, 38.0),
			);
			let back = Rect::from_min_size(pos2(send.left() - 108.0, send.top()), vec2(96.0, 38.0));
			if paint::pill_button(
				ui,
				ui.scope_id().with("add-friend-cancel"),
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
			if paint::pill_button(
				ui,
				ui.scope_id().with("add-friend-send"),
				send,
				None,
				"Send request",
				ButtonKind::Primary,
				!input.trim().is_empty() && !state.user_action_pending(),
			)
			.clicked()
			{
				submit = true;
			}
		});
	solace.add_friend = Some(input.clone());
	if submit {
		match state.add_friend(&input) {
			Some(command) => {
				commands.push(command);
				solace.add_friend = None;
			}
			None => {
				if let Some(status) = state.take_user_action_status() {
					solace.toasts.push(crate::Level::Warning, status);
				}
			}
		}
	}
	if cancel || dismissed {
		solace.add_friend = None;
	}
}
