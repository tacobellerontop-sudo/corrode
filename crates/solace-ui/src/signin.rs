//! Signed-out screens: welcome/sign-in, saved-login restore, and the strip shown above the
//! temporary Discord login window.
use crate::{
	Solace,
	icons::{self, Icon},
	paint::{self, AvatarShape, ButtonKind},
	scenery::Mood,
	theme::{self, mix},
};
use egui::{Align2, Color32, Key, Rect, Sense, TextEdit, Ui, pos2, vec2};
use model::{Id, SavedAccount};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
	/// Open Discord's own login page in a temporary window.
	Browser,
	/// Connect with the token currently in the token field.
	Token,
	/// Restore a saved account's login.
	Saved(Id),
	/// Remove a saved account from this device.
	Forget(Id),
}

pub struct Props<'a> {
	pub accounts: &'a [SavedAccount],
	pub status: &'a str,
	/// A login or restore is in progress; buttons are disabled.
	pub busy: bool,
	pub token: &'a mut String,
	/// The owner confirmed this is their account; required before a new sign-in.
	pub authorized: &'a mut bool,
}

fn backdrop(solace: &mut Solace, ui: &Ui, rect: Rect) {
	let texture = solace.scenery.texture(ui.ctx(), 11, Mood::Dawn).id();
	paint::cover_image(ui.painter(), rect, 0, texture, vec2(1280.0, 380.0));
	let p = theme::palette(ui.ctx());
	let veil = if p.dark {
		Color32::from_black_alpha(120)
	} else {
		Color32::from_white_alpha(40)
	};
	ui.painter().rect_filled(rect, 0, veil);
}

fn brand(ui: &Ui, center_x: f32, top: f32, p: &theme::Palette) {
	icons::paint_weighted(
		ui.painter(),
		Icon::Logo,
		Rect::from_center_size(pos2(center_x, top + 24.0), vec2(40.0, 40.0)),
		p.text,
		1.2,
	);
	paint::label(
		ui.painter(),
		pos2(center_x, top + 84.0),
		Align2::CENTER_CENTER,
		"Solace",
		theme::serif(40.0),
		p.text,
	);
	paint::label(
		ui.painter(),
		pos2(center_x, top + 120.0),
		Align2::CENTER_CENTER,
		"A calmer place for your communities.",
		theme::sans(14.0),
		p.muted,
	);
}

/// Welcome and sign-in.
pub fn show(solace: &mut Solace, ui: &mut Ui, props: Props) -> Option<Action> {
	let p = theme::palette(ui.ctx());
	let screen = ui.max_rect();
	backdrop(solace, ui, screen);
	let token_open = ui.ctx().data(|d| {
		d.get_temp::<bool>(egui::Id::unique("token-open"))
			.unwrap_or(false)
	});
	let saved = props.accounts.iter().filter(|a| a.has_token).count().min(4);
	let height = 374.0
		+ saved as f32 * 56.0
		+ if saved > 0 { 30.0 } else { 0.0 }
		+ if token_open { 64.0 } else { 0.0 }
		+ if props.status.is_empty() { 0.0 } else { 30.0 };
	let card = Rect::from_center_size(screen.center(), vec2(420.0, height));
	paint::floating(ui.painter(), card, 24, &p);
	brand(ui, card.center().x, card.top() + 20.0, &p);

	let mut action = None;
	let mut y = card.top() + 170.0;
	let inner = card.width() - 64.0;
	if saved > 0 {
		paint::label(
			ui.painter(),
			pos2(card.left() + 32.0, y + 6.0),
			Align2::LEFT_CENTER,
			"Continue as",
			theme::medium(12.5),
			p.muted,
		);
		y += 24.0;
		for account in props.accounts.iter().filter(|a| a.has_token).take(4) {
			let row = Rect::from_min_size(pos2(card.left() + 32.0, y), vec2(inner, 50.0));
			let response = ui.interact(
				row,
				ui.scope_id().with(("saved", account.id)),
				if props.busy {
					Sense::hover()
				} else {
					Sense::click()
				},
			);
			let t = paint::hover_t(ui, &response);
			ui.painter().rect(
				row,
				14,
				mix(p.surface, p.hover, t),
				egui::Stroke::new(1.0, p.border),
				egui::StrokeKind::Inside,
			);
			let user = model::User {
				id: account.id,
				name: account.name.clone(),
				avatar: account.avatar.clone(),
				discriminator: account.discriminator,
				kind: Default::default(),
				webhook: false,
				primary_guild: None,
			};
			let avatar = Rect::from_min_size(row.min + vec2(10.0, 9.0), vec2(32.0, 32.0));
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
			let name = account.display.as_deref().unwrap_or(&account.name);
			paint::text_left(
				ui.painter(),
				pos2(avatar.right() + 12.0, row.center().y - 7.0),
				name,
				theme::medium(14.0),
				p.text,
				inner - 110.0,
			);
			paint::text_left(
				ui.painter(),
				pos2(avatar.right() + 12.0, row.center().y + 10.0),
				&format!("@{}", account.name),
				theme::sans(11.5),
				p.muted,
				inner - 110.0,
			);
			let forget =
				Rect::from_center_size(row.right_center() - vec2(22.0, 0.0), vec2(28.0, 28.0));
			if paint::icon_button(
				ui,
				ui.scope_id().with(("forget", account.id)),
				forget,
				Icon::Close,
				ButtonKind::Ghost,
				false,
				"Remove from this device",
			)
			.clicked() && !props.busy
			{
				action = Some(Action::Forget(account.id));
			} else if response.clicked() {
				action = Some(Action::Saved(account.id));
			}
			y += 56.0;
		}
		y += 6.0;
	}

	// Ownership consent gates every new sign-in.
	let consent = Rect::from_min_size(pos2(card.left() + 32.0, y), vec2(inner, 34.0));
	let response = ui.interact(consent, ui.scope_id().with("consent"), Sense::click());
	if response.clicked() {
		*props.authorized = !*props.authorized;
	}
	let t = ui.ctx().animate_bool_with_time(
		ui.scope_id().with("consent-anim"),
		*props.authorized,
		0.12,
	);
	let box_rect =
		Rect::from_center_size(consent.left_center() + vec2(10.0, 0.0), vec2(20.0, 20.0));
	ui.painter().rect(
		box_rect,
		6,
		mix(p.surface, p.accent, t),
		egui::Stroke::new(1.5, mix(p.border, p.accent, t)),
		egui::StrokeKind::Inside,
	);
	if t > 0.5 {
		icons::paint_at(
			ui.painter(),
			Icon::Check,
			box_rect.center(),
			14.0,
			p.accent_text,
		);
	}
	paint::text_left(
		ui.painter(),
		consent.left_center() + vec2(30.0, 0.0),
		"I own this account and authorize this session",
		theme::sans(13.0),
		p.text_soft,
		inner - 30.0,
	);
	y += 44.0;
	let allowed = !props.busy && *props.authorized;

	let primary = Rect::from_min_size(pos2(card.left() + 32.0, y), vec2(inner, 46.0));
	if paint::pill_button(
		ui,
		ui.scope_id().with("browser-login"),
		primary,
		Some(Icon::Globe),
		if saved > 0 {
			"Use another account"
		} else {
			"Continue with Discord"
		},
		ButtonKind::Primary,
		allowed,
	)
	.clicked()
	{
		action = Some(Action::Browser);
	}
	y += 62.0;

	let toggle = Rect::from_min_size(pos2(card.left() + 32.0, y), vec2(inner, 24.0));
	let response = ui.interact(toggle, ui.scope_id().with("token-toggle"), Sense::click());
	paint::label(
		ui.painter(),
		toggle.center(),
		Align2::CENTER_CENTER,
		if token_open {
			"Hide token sign-in"
		} else {
			"Sign in with a token instead"
		},
		theme::medium(12.5),
		if response.hovered() { p.text } else { p.muted },
	);
	if response.clicked() {
		ui.ctx()
			.data_mut(|d| d.insert_temp(egui::Id::unique("token-open"), !token_open));
	}
	y += 34.0;
	if token_open {
		let field = Rect::from_min_size(pos2(card.left() + 32.0, y), vec2(inner - 108.0, 42.0));
		ui.painter().rect(
			field,
			12,
			p.canvas,
			egui::Stroke::new(1.0, p.border),
			egui::StrokeKind::Inside,
		);
		let edit = field.shrink2(vec2(12.0, 0.0));
		let response = ui.put(
			edit,
			TextEdit::singleline(props.token)
				.password(true)
				.frame(egui::Frame::NONE)
				.font(theme::sans(13.5))
				.text_color(p.text)
				.align(egui::Align2::LEFT_CENTER)
				.hint_text(
					egui::RichText::new("Paste your token")
						.color(p.faint)
						.font(theme::sans(13.5)),
				)
				.desired_width(edit.width()),
		);
		let connect = Rect::from_min_size(pos2(field.right() + 8.0, y), vec2(100.0, 42.0));
		let ready = !props.token.trim().is_empty() && allowed;
		if paint::pill_button(
			ui,
			ui.scope_id().with("token-connect"),
			connect,
			None,
			"Connect",
			ButtonKind::Ghost,
			ready,
		)
		.clicked() || (ready && response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
		{
			action = Some(Action::Token);
		}
		y += 58.0;
	}
	if !props.status.is_empty() {
		let galley = ui.painter().layout(
			props.status.to_owned(),
			theme::sans(12.5),
			p.text_soft,
			inner,
		);
		ui.painter().galley(
			pos2(card.center().x - galley.size().x / 2.0, y),
			galley,
			p.text_soft,
		);
	}
	paint::label(
		ui.painter(),
		pos2(card.center().x, card.bottom() - 26.0),
		Align2::CENTER_CENTER,
		"Solace talks to Discord directly. Your password stays on Discord's page.",
		theme::sans(11.5),
		p.faint,
	);
	action
}

/// Saved-login restore in progress. Returns true if the reader chose to sign in differently.
pub fn restoring(solace: &mut Solace, ui: &mut Ui, stage: &str) -> bool {
	let p = theme::palette(ui.ctx());
	let screen = ui.max_rect();
	backdrop(solace, ui, screen);
	let card = Rect::from_center_size(screen.center(), vec2(380.0, 270.0));
	paint::floating(ui.painter(), card, 24, &p);
	brand(ui, card.center().x, card.top() + 16.0, &p);
	let bar = Rect::from_center_size(pos2(card.center().x, card.top() + 176.0), vec2(220.0, 4.0));
	ui.painter().rect_filled(bar, 2, p.surface_alt);
	let t = (ui.input(|i| i.time) as f32 * 0.7).fract();
	let start = bar.left() + (bar.width() + 80.0) * t - 80.0;
	let seg = Rect::from_min_max(
		pos2(start.max(bar.left()), bar.top()),
		pos2((start + 80.0).min(bar.right()), bar.bottom()),
	);
	if seg.width() > 0.0 {
		ui.painter().rect_filled(seg, 2, p.accent);
	}
	ui.ctx().request_repaint();
	paint::label(
		ui.painter(),
		pos2(card.center().x, card.top() + 200.0),
		Align2::CENTER_CENTER,
		stage,
		theme::sans(13.0),
		p.muted,
	);
	let other = Rect::from_center_size(
		pos2(card.center().x, card.bottom() - 30.0),
		vec2(180.0, 30.0),
	);
	paint::pill_button(
		ui,
		ui.scope_id().with("restore-other"),
		other,
		None,
		"Use another account",
		ButtonKind::Ghost,
		true,
	)
	.clicked()
}

/// Header strip above Discord's login page. Returns true when the reader cancels.
pub fn login_header(ui: &mut Ui, rect: Rect) -> bool {
	let p = theme::palette(ui.ctx());
	ui.painter().rect_filled(rect, 0, p.sidebar);
	paint::hairline(ui.painter(), rect.left_bottom(), rect.right_bottom(), &p);
	icons::paint_weighted(
		ui.painter(),
		Icon::Logo,
		Rect::from_center_size(rect.left_center() + vec2(30.0, 0.0), vec2(22.0, 22.0)),
		p.text,
		1.1,
	);
	paint::label(
		ui.painter(),
		rect.left_center() + vec2(52.0, -8.0),
		Align2::LEFT_CENTER,
		"Sign in to Discord",
		theme::serif(15.0),
		p.text,
	);
	paint::label(
		ui.painter(),
		rect.left_center() + vec2(52.0, 10.0),
		Align2::LEFT_CENTER,
		"discord.com · temporary window · your password never reaches Solace",
		theme::sans(11.5),
		p.muted,
	);
	let cancel = Rect::from_min_size(rect.right_center() - vec2(110.0, 16.0), vec2(94.0, 32.0));
	paint::pill_button(
		ui,
		ui.scope_id().with("login-cancel"),
		cancel,
		None,
		"Cancel",
		ButtonKind::Ghost,
		true,
	)
	.clicked()
}
