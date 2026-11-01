//! Settings › Profile: edit your own global profile (picture, display name, pronouns,
//! About Me and profile color) with a live preview. One bounded, session-only draft; only
//! the fields you changed are sent when you save.
use crate::{
	Request, Solace,
	icons::{self, Icon},
	paint::{self, AvatarShape, ButtonKind},
	theme::{self, Palette, mix},
};
use client_core::{Command, State};
use egui::{Align2, Color32, Rect, Sense, TextEdit, Ui, pos2, vec2};
use model::{Id, ProfileEdit, UserProfile};

/// Height of the scrolling form under the page heading.
const CONTENT: f32 = 590.0;
const BANNER: f32 = 64.0;
/// Banner tints offered next to "Default" (which follows your avatar's hue).
const SWATCHES: [u32; 9] = [
	0x5865F2, 0xEB459E, 0xED4245, 0xF57731, 0xF0B232, 0x3BA55C, 0x1ABC9C, 0x3498DB, 0x9B59B6,
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Draft {
	name: String,
	bio: String,
	pronouns: String,
	color: Option<u32>,
	/// `Some(Some(uri))` uploads a new picture, `Some(None)` removes the current one.
	avatar: Option<Option<String>>,
}

impl Draft {
	fn from_profile(profile: &UserProfile) -> Self {
		Self {
			name: profile.global_name.clone().unwrap_or_default(),
			bio: profile.bio.clone(),
			pronouns: profile.pronouns.clone(),
			color: profile.accent_color,
			avatar: None,
		}
	}

	fn changes(&self, profile: &UserProfile) -> ProfileEdit {
		let name = (!self.name.is_empty()).then(|| self.name.clone());
		ProfileEdit {
			global_name: (name != profile.global_name).then_some(name),
			bio: (self.bio != profile.bio).then(|| self.bio.clone()),
			pronouns: (self.pronouns != profile.pronouns).then(|| self.pronouns.clone()),
			accent_color: (self.color != profile.accent_color).then_some(self.color),
			avatar: match &self.avatar {
				// Removing a picture the account does not have is not a change.
				Some(None) if profile.user.avatar.is_none() => None,
				other => other.clone(),
			},
		}
	}

	/// After a reload, adopt remote values for the fields this draft has not touched.
	fn rebase(&mut self, before: &Self, fresh: &Self) {
		if self.name == before.name {
			self.name = fresh.name.clone();
		}
		if self.bio == before.bio {
			self.bio = fresh.bio.clone();
		}
		if self.pronouns == before.pronouns {
			self.pronouns = fresh.pronouns.clone();
		}
		if self.color == before.color {
			self.color = fresh.color;
		}
	}
}

#[derive(Default)]
pub(crate) struct Editor {
	generation: Option<u64>,
	draft: Option<Draft>,
	baseline: Option<Draft>,
	/// The save request whose result is awaited.
	submitted: Option<u64>,
	saved: bool,
	/// Bumped for every picker opened, so a late result from an older one is ignored.
	revision: u64,
	choosing: bool,
	/// The chosen, not yet saved picture: its preview texture and pixels.
	pending: Option<(egui::TextureHandle, egui::ColorImage)>,
}

impl Editor {
	#[cfg(test)]
	fn has_changes(&self) -> bool {
		!self.saved && self.draft != self.baseline
	}

	/// The desktop's picker finished. Returns an error to show the reader.
	pub fn accept_avatar(
		&mut self,
		ctx: &egui::Context,
		scope: (u64, Id, u64),
		result: Result<Option<(String, egui::ColorImage)>, &'static str>,
	) -> Result<(), &'static str> {
		if !self.choosing || Some(scope.0) != self.generation || scope.2 != self.revision {
			return Ok(());
		}
		self.choosing = false;
		let Some((uri, image)) = result? else {
			return Ok(());
		};
		if !model::valid_avatar_uri(&uri) || image.size[0] > 512 || image.size[1] > 512 {
			return Err("That picture is too large; choose a simpler image");
		}
		let Some(draft) = self.draft.as_mut() else {
			return Ok(());
		};
		draft.avatar = Some(Some(uri));
		let texture = ctx.load_texture(
			"solace-profile-avatar-preview",
			image.clone(),
			egui::TextureOptions::LINEAR,
		);
		self.pending = Some((texture, image));
		self.saved = false;
		Ok(())
	}
}

/// Leaving the page: forget the loaded snapshot (the next visit reloads it) but keep the draft.
pub(crate) fn leave(state: &mut State) {
	state.clear_own_profile();
}

pub(crate) fn page(
	solace: &mut Solace,
	ui: &mut Ui,
	body: Rect,
	state: &mut State,
	commands: &mut Vec<Command>,
	p: &Palette,
) {
	crate::overlays::heading(
		ui,
		body.min,
		"Profile",
		"How you appear to everyone on Discord.",
		p,
	);
	let editor = &mut solace.profile_editor;
	if editor.generation != Some(state.generation) {
		*editor = Editor {
			generation: Some(state.generation),
			..Editor::default()
		};
	}
	let own = &state.own_profile;
	if own.data.is_none()
		&& !own.loading
		&& own.error.is_none()
		&& let Some(command) = state.load_own_profile()
	{
		commands.push(command);
	}
	// A save finished: adopt what the service returned.
	if editor.submitted.is_some()
		&& editor.submitted == Some(state.own_profile.request)
		&& !state.own_profile.saving
	{
		editor.submitted = None;
		if state.own_profile.error.is_none()
			&& let Some(profile) = &state.own_profile.data
		{
			let uploaded = editor
				.draft
				.as_ref()
				.is_some_and(|d| matches!(d.avatar, Some(Some(_))));
			// Show the new picture everywhere right away instead of waiting for the CDN.
			if let (true, Some((_, image))) = (uploaded, editor.pending.take())
				&& profile.user.avatar.is_some()
			{
				solace
					.images
					.insert(ui.ctx(), profile.user.avatar_key(), image);
			}
			editor.pending = None;
			editor.draft = Some(Draft::from_profile(profile));
			editor.saved = true;
		}
	}
	let editor = &mut solace.profile_editor;
	let scroll = Rect::from_min_max(pos2(body.left(), body.top() + 66.0), body.max);
	let bar = Rect::from_min_max(pos2(body.left(), body.bottom() - 40.0), body.max);
	let Some(profile) = state.own_profile.data.clone() else {
		let (text, color) = match state.own_profile.error {
			Some(error) => (error, p.danger),
			None => ("Loading your profile...", p.muted),
		};
		paint::text_left(
			ui.painter(),
			pos2(scroll.left(), scroll.top() + 20.0),
			text,
			theme::sans(13.0),
			color,
			scroll.width(),
		);
		if state.own_profile.error.is_some()
			&& !state.own_profile.loading
			&& paint::pill_button(
				ui,
				ui.scope_id().with("profile-reload"),
				Rect::from_min_size(pos2(scroll.left(), scroll.top() + 44.0), vec2(150.0, 36.0)),
				None,
				"Reload profile",
				ButtonKind::Ghost,
				true,
			)
			.clicked()
			&& let Some(command) = state.load_own_profile()
		{
			commands.push(command);
		}
		return;
	};
	let fresh = Draft::from_profile(&profile);
	if let (Some(draft), Some(before)) = (&mut editor.draft, &editor.baseline) {
		draft.rebase(before, &fresh);
	}
	editor.baseline = Some(fresh);
	let draft = editor
		.draft
		.get_or_insert_with(|| Draft::from_profile(&profile))
		.clone();
	let editable = !state.own_profile.loading && !state.own_profile.saving;
	let mut edited = draft.clone();
	let mut pick = false;
	let form = Rect::from_min_max(scroll.min, pos2(scroll.right(), bar.top() - 12.0));
	let mut child = ui.new_child(
		egui::UiBuilder::new()
			.max_rect(form)
			.id_salt("profile-form"),
	);
	child.set_clip_rect(form);
	egui::ScrollArea::vertical()
		.id_salt("profile-settings")
		.auto_shrink([false, false])
		.show(&mut child, |ui| {
			let (content, _) =
				ui.allocate_exact_size(vec2(form.width() - 10.0, CONTENT), Sense::hover());
			ui.add_enabled_ui(editable, |ui| {
				pick = fields(
					&mut solace.images,
					ui,
					content,
					&mut edited,
					&profile,
					(editor.pending.as_ref().map(|(t, _)| t), editor.choosing),
					p,
				);
			});
		});
	let editor = &mut solace.profile_editor;
	if pick && editable && !editor.choosing {
		editor.revision = editor.revision.wrapping_add(1);
		editor.choosing = true;
		let scope = (state.generation, profile.user.id, editor.revision);
		solace.requests_profile_picture(scope);
	}
	let editor = &mut solace.profile_editor;
	if edited.avatar.is_none() || edited.avatar == Some(None) {
		// Undo or remove drops a chosen picture.
		editor.pending = None;
	}
	if edited != draft {
		editor.saved = false;
	}
	editor.draft = Some(edited.clone());
	let changes = edited.changes(&profile);
	let changed = changes != ProfileEdit::default();
	save_bar(editor, ui, bar, state, commands, changes, changed, p);
}

/// The preview card and the form. Returns true when the picture should be changed.
fn fields(
	images: &mut crate::images::Images,
	ui: &mut Ui,
	content: Rect,
	draft: &mut Draft,
	profile: &UserProfile,
	(pending, choosing): (Option<&egui::TextureHandle>, bool),
	p: &Palette,
) -> bool {
	let mut pick = false;
	let left = content.left();
	let width = content.width();
	let user = &profile.user;

	// Live preview: banner in the chosen color, picture, name and pronouns.
	let card = Rect::from_min_size(content.min, vec2(width, 168.0));
	paint::card(ui.painter(), card, 16, p);
	let tint = banner_color(draft.color, profile, p);
	let banner = Rect::from_min_size(card.min, vec2(width, BANNER));
	let bottom = mix(tint, p.surface, 0.45);
	paint::gradient(ui.painter(), banner, 16.0, &[(0.0, tint), (1.0, bottom)]);
	ui.painter().rect_filled(
		Rect::from_min_max(
			pos2(banner.left(), banner.bottom() - 16.0),
			banner.right_bottom(),
		),
		0,
		bottom,
	);
	let avatar = Rect::from_min_size(
		pos2(card.left() + 18.0, banner.bottom() - 34.0),
		vec2(72.0, 72.0),
	);
	ui.painter().circle_filled(avatar.center(), 40.0, p.surface);
	let texture = match (&draft.avatar, pending) {
		(Some(Some(_)), Some(texture)) => Some((texture.id(), texture.size_vec2())),
		(Some(None), _) => None,
		_ => images.get(&user.avatar_key()),
	};
	paint::avatar(
		ui.painter(),
		avatar,
		AvatarShape::Circle,
		texture,
		&user.name,
		user.id.0,
		p,
	);
	let hit = ui.interact(avatar, ui.scope_id().with("profile-avatar"), Sense::click());
	if ui.is_enabled() && !choosing && (hit.hovered() || hit.has_focus()) {
		ui.painter()
			.circle_filled(avatar.center(), 36.0, Color32::from_black_alpha(120));
		icons::paint_at(
			ui.painter(),
			Icon::Pencil,
			avatar.center(),
			22.0,
			Color32::WHITE,
		);
	}
	if hit.on_hover_text("Change profile picture").clicked() {
		pick = true;
	}
	let text_x = avatar.right() + 14.0;
	let name = if draft.name.trim().is_empty() {
		&profile.username
	} else {
		&draft.name
	};
	paint::text_left(
		ui.painter(),
		pos2(text_x, banner.bottom() + 16.0),
		name,
		theme::serif(20.0),
		p.text,
		card.right() - text_x - 16.0,
	);
	let handle = if draft.pronouns.trim().is_empty() {
		format!("@{}", profile.username)
	} else {
		format!("@{}  ·  {}", profile.username, draft.pronouns)
	};
	paint::text_left(
		ui.painter(),
		pos2(text_x, banner.bottom() + 40.0),
		&handle,
		theme::sans(12.5),
		p.muted,
		card.right() - text_x - 16.0,
	);
	let change = Rect::from_min_size(
		pos2(card.left() + 18.0, card.bottom() - 46.0),
		vec2(148.0, 32.0),
	);
	if paint::pill_button(
		ui,
		ui.scope_id().with("profile-change-picture"),
		change,
		Some(Icon::Image),
		if choosing {
			"Choosing..."
		} else {
			"Change picture"
		},
		ButtonKind::Soft,
		ui.is_enabled() && !choosing,
	)
	.clicked()
	{
		pick = true;
	}
	let has_picture = match &draft.avatar {
		Some(value) => value.is_some(),
		None => user.avatar.is_some(),
	};
	let second = Rect::from_min_size(pos2(change.right() + 8.0, change.top()), vec2(84.0, 32.0));
	if draft.avatar.is_some() {
		if paint::pill_button(
			ui,
			ui.scope_id().with("profile-undo-picture"),
			second,
			None,
			"Undo",
			ButtonKind::Ghost,
			ui.is_enabled(),
		)
		.clicked()
		{
			draft.avatar = None;
		}
	} else if has_picture
		&& paint::pill_button(
			ui,
			ui.scope_id().with("profile-remove-picture"),
			second,
			None,
			"Remove",
			ButtonKind::Ghost,
			ui.is_enabled(),
		)
		.clicked()
	{
		draft.avatar = Some(None);
	}
	let hint = match &draft.avatar {
		Some(Some(_)) => "Save to upload it.",
		Some(None) => "Removed when you save.",
		None => "PNG, JPEG, GIF or WebP",
	};
	paint::text_left(
		ui.painter(),
		pos2(second.right() + 12.0, change.center().y),
		hint,
		theme::sans(12.0),
		p.muted,
		(card.right() - second.right() - 28.0).max(0.0),
	);

	let mut y = card.bottom() + 22.0;
	text_field(
		ui,
		pos2(left, y),
		width,
		("Display name", "profile-display-name"),
		&mut draft.name,
		(model::MAX_PROFILE_NAME_CHARS, false),
		&profile.username,
		p,
	);
	y += 78.0;
	paint::text_left(
		ui.painter(),
		pos2(left, y - 6.0),
		"Leave blank to use your username.",
		theme::sans(12.0),
		p.muted,
		width,
	);
	y += 18.0;
	text_field(
		ui,
		pos2(left, y),
		width,
		("Pronouns", "profile-pronouns"),
		&mut draft.pronouns,
		(model::MAX_PROFILE_PRONOUNS_CHARS, false),
		"they/them",
		p,
	);
	y += 82.0;
	text_field(
		ui,
		pos2(left, y),
		width,
		("About me", "profile-about-me"),
		&mut draft.bio,
		(model::MAX_PROFILE_BIO_CHARS, true),
		"Say a little about yourself",
		p,
	);
	y += 146.0;
	paint::label(
		ui.painter(),
		pos2(left, y + 8.0),
		Align2::LEFT_CENTER,
		"Profile color",
		theme::medium(13.5),
		p.text,
	);
	paint::label(
		ui.painter(),
		pos2(left + 104.0, y + 8.0),
		Align2::LEFT_CENTER,
		"Tints your banner when you have no banner image.",
		theme::sans(12.0),
		p.muted,
	);
	y += 26.0;
	let mut options: Vec<Option<u32>> = vec![None];
	options.extend(SWATCHES.iter().map(|&c| Some(c)));
	if let Some(custom) = draft.color.filter(|c| !SWATCHES.contains(c)) {
		options.push(Some(custom));
	}
	for (i, option) in options.into_iter().enumerate() {
		let center = pos2(left + 16.0 + i as f32 * 38.0, y + 16.0);
		let rect = Rect::from_center_size(center, vec2(32.0, 32.0));
		let response = ui.interact(
			rect,
			ui.scope_id().with(("profile-color", i)),
			Sense::click(),
		);
		let active = draft.color == option;
		let fill = option.map_or_else(|| theme::seeded_hue(user.id.0, p.dark), rgb);
		if active {
			ui.painter()
				.circle_stroke(center, 16.0, egui::Stroke::new(2.0, p.accent));
		} else if response.hovered() {
			ui.painter()
				.circle_stroke(center, 16.0, egui::Stroke::new(1.5, p.border));
		}
		ui.painter().circle_filled(center, 12.0, fill);
		if option.is_none() {
			// "Default" is marked with a diagonal so it reads as "no color chosen".
			ui.painter().line_segment(
				[center + vec2(-6.0, 6.0), center + vec2(6.0, -6.0)],
				egui::Stroke::new(1.5, Color32::from_white_alpha(220)),
			);
		} else if active {
			icons::paint_at(ui.painter(), Icon::Check, center, 13.0, Color32::WHITE);
		}
		let tip = match option {
			None => "Default".to_owned(),
			Some(c) => format!("#{c:06X}"),
		};
		if response.on_hover_text(tip).clicked() && ui.is_enabled() {
			draft.color = option;
		}
	}
	pick
}

#[allow(clippy::too_many_arguments)]
fn text_field(
	ui: &mut Ui,
	at: egui::Pos2,
	width: f32,
	(label, id): (&str, &str),
	value: &mut String,
	(limit, multiline): (usize, bool),
	hint: &str,
	p: &Palette,
) {
	paint::label(
		ui.painter(),
		at + vec2(0.0, 8.0),
		Align2::LEFT_CENTER,
		label,
		theme::medium(13.5),
		p.text,
	);
	let count = value.chars().count();
	paint::label(
		ui.painter(),
		pos2(at.x + width, at.y + 8.0),
		Align2::RIGHT_CENTER,
		&format!("{count} / {limit}"),
		theme::sans(11.5),
		if count > limit { p.danger } else { p.faint },
	);
	let height = if multiline { 104.0 } else { 40.0 };
	let field = Rect::from_min_size(at + vec2(0.0, 22.0), vec2(width, height));
	let id = egui::Id::unique(id);
	let focused = ui.memory(|m| m.has_focus(id));
	ui.painter().rect(
		field,
		12,
		p.canvas,
		egui::Stroke::new(
			if focused { 1.5 } else { 1.0 },
			if focused { p.accent } else { p.border },
		),
		egui::StrokeKind::Inside,
	);
	let hint = egui::RichText::new(hint)
		.color(p.faint)
		.font(theme::sans(14.0));
	let (edit, inner) = if multiline {
		(
			TextEdit::multiline(value).desired_rows(4),
			field.shrink2(vec2(12.0, 10.0)),
		)
	} else {
		(
			TextEdit::singleline(value).align(Align2::LEFT_CENTER),
			field.shrink2(vec2(12.0, 0.0)),
		)
	};
	ui.put(
		inner,
		edit.id(id)
			.char_limit(limit)
			.frame(egui::Frame::NONE)
			.font(theme::sans(14.0))
			.text_color(p.text)
			.hint_text(hint)
			.desired_width(inner.width()),
	);
}

#[allow(clippy::too_many_arguments)]
fn save_bar(
	editor: &mut Editor,
	ui: &mut Ui,
	bar: Rect,
	state: &mut State,
	commands: &mut Vec<Command>,
	changes: ProfileEdit,
	changed: bool,
	p: &Palette,
) {
	paint::hairline(
		ui.painter(),
		pos2(bar.left(), bar.top() - 8.0),
		pos2(bar.right(), bar.top() - 8.0),
		p,
	);
	let own = &state.own_profile;
	let (status, color) = if own.saving {
		("Saving your profile...".to_owned(), p.muted)
	} else if let Some(error) = own.error {
		(error.to_owned(), p.danger)
	} else if !changes.valid() {
		(
			"Check character limits; a display name cannot be only spaces.".to_owned(),
			p.danger,
		)
	} else if changed {
		("You have unsaved changes.".to_owned(), p.text_soft)
	} else if editor.saved {
		(
			if state.demo {
				"Saved in the showcase".to_owned()
			} else {
				"Profile saved".to_owned()
			},
			p.online,
		)
	} else if !state.demo && !state.gateway_connected {
		("Reconnect to save your profile.".to_owned(), p.muted)
	} else {
		(String::new(), p.muted)
	};
	let save = Rect::from_min_size(pos2(bar.right() - 132.0, bar.top()), vec2(132.0, 38.0));
	let second = Rect::from_min_size(pos2(save.left() - 100.0, bar.top()), vec2(92.0, 38.0));
	paint::text_left(
		ui.painter(),
		pos2(bar.left(), bar.center().y),
		&status,
		theme::sans(12.5),
		color,
		(second.left() - bar.left() - 12.0).max(0.0),
	);
	let reload = own.reload_required && own.error.is_some() && !own.loading && !own.saving;
	if reload {
		if paint::pill_button(
			ui,
			ui.scope_id().with("profile-reload-bar"),
			second,
			None,
			"Reload",
			ButtonKind::Ghost,
			true,
		)
		.clicked() && let Some(command) = state.load_own_profile()
		{
			commands.push(command);
		}
	} else if paint::pill_button(
		ui,
		ui.scope_id().with("profile-cancel"),
		second,
		None,
		"Cancel",
		ButtonKind::Ghost,
		changed && !own.saving,
	)
	.clicked()
	{
		editor.draft = state.own_profile.data.as_ref().map(Draft::from_profile);
		editor.pending = None;
		editor.saved = false;
	}
	let enabled = changed && changes.valid() && state.can_save_own_profile();
	if paint::pill_button(
		ui,
		ui.scope_id().with("profile-save"),
		save,
		None,
		"Save changes",
		ButtonKind::Primary,
		enabled,
	)
	.clicked()
		&& let Some(command) = state.save_own_profile(changes)
	{
		editor.submitted = Some(state.own_profile.request);
		editor.saved = false;
		commands.push(command);
	}
}

fn banner_color(color: Option<u32>, profile: &UserProfile, p: &Palette) -> Color32 {
	color
		.or_else(|| profile.theme_colors.map(|c| c[0]))
		.map_or_else(|| theme::seeded_hue(profile.user.id.0, p.dark), rgb)
}

fn rgb(color: u32) -> Color32 {
	Color32::from_rgb((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

impl Solace {
	fn requests_profile_picture(&mut self, scope: (u64, Id, u64)) {
		self.request(Request::ChooseProfilePicture(scope));
	}

	/// The result of [`Request::ChooseProfilePicture`]: a PNG data URI and its pixels, `None`
	/// when the reader cancelled, or an error to show.
	pub fn accept_profile_picture(
		&mut self,
		ctx: &egui::Context,
		scope: (u64, Id, u64),
		result: Result<Option<(String, egui::ColorImage)>, &'static str>,
	) {
		if let Err(error) = self.profile_editor.accept_avatar(ctx, scope, result) {
			self.toasts.push(crate::Level::Error, error);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn synthetic(user: &model::User) -> UserProfile {
		UserProfile {
			user: user.clone(),
			username: user.name.clone(),
			global_name: None,
			banner: None,
			accent_color: None,
			bio: String::new(),
			pronouns: String::new(),
			badges: vec![],
			connections: vec![],
			mutual_guilds: vec![],
			guild: None,
			theme_colors: None,
			clan: None,
			limited: false,
		}
	}

	#[test]
	fn drafts_send_only_changed_fields_and_can_clear_values() {
		let user = model::User {
			id: Id(1),
			name: "synthetic".into(),
			avatar: None,
			webhook: false,
			kind: Default::default(),
			discriminator: 0,
			primary_guild: None,
		};
		let mut profile = synthetic(&user);
		profile.global_name = Some("Before".into());
		profile.accent_color = Some(0x123456);
		let mut draft = Draft::from_profile(&profile);
		assert!(draft.changes(&profile) == ProfileEdit::default());
		draft.name.clear();
		draft.bio.clear();
		draft.color = None;
		// Removing a picture the account does not have changes nothing.
		draft.avatar = Some(None);
		let changes = draft.changes(&profile);
		assert_eq!(changes.global_name, Some(None));
		assert_eq!(changes.accent_color, Some(None));
		assert_eq!(changes.pronouns, None);
		assert_eq!(changes.avatar, None);
		assert!(changes.valid());
		// A reload keeps edited values but adopts remote changes to untouched fields.
		let before = Draft::from_profile(&profile);
		profile.pronouns = "she/her".into();
		draft.rebase(&before, &Draft::from_profile(&profile));
		assert_eq!(draft.pronouns, "she/her");
		assert!(draft.name.is_empty());
		assert_eq!(draft.changes(&profile).pronouns, None);
	}

	fn run(
		ctx: &egui::Context,
		solace: &mut Solace,
		state: &mut State,
		events: Vec<egui::Event>,
	) -> (Vec<Command>, Vec<(String, Rect)>) {
		let input = egui::RawInput {
			screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, 860.0))),
			events,
			..Default::default()
		};
		let mut commands = vec![];
		let mut output = ctx.run_ui(input, |ui| commands = solace.show(ui, state));
		output.textures_delta.clear();
		fn collect(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
			match shape {
				egui::Shape::Text(text) => out.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, out)),
				_ => {}
			}
		}
		let mut labels = vec![];
		for shape in &output.shapes {
			collect(&shape.shape, &mut labels);
		}
		(commands, labels)
	}

	fn click(at: egui::Pos2) -> Vec<Vec<egui::Event>> {
		[true, false]
			.into_iter()
			.map(|pressed| {
				vec![
					egui::Event::PointerMoved(at),
					egui::Event::PointerButton {
						pos: at,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				]
			})
			.collect()
	}

	#[test]
	fn the_profile_page_loads_edits_and_saves_once_in_both_themes() {
		for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
			let ctx = egui::Context::default();
			crate::install(&ctx).unwrap();
			ctx.set_theme(theme);
			let mut solace = Solace::default();
			let mut state = test_support::demo_state();
			solace.open_settings(crate::SettingsPage::Profile);
			let (commands, _) = run(&ctx, &mut solace, &mut state, vec![]);
			let Some(Command::EditProfile {
				user,
				request,
				changes: None,
			}) = commands
				.into_iter()
				.find(|c| matches!(c, Command::EditProfile { .. }))
			else {
				panic!("the page loads your profile")
			};
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::ProfileEdited {
					user,
					request,
					result: Ok(Box::new(synthetic(state.user.as_ref().unwrap()))),
				},
			});
			run(&ctx, &mut solace, &mut state, vec![]);
			ctx.memory_mut(|m| m.request_focus(egui::Id::unique("profile-pronouns")));
			run(
				&ctx,
				&mut solace,
				&mut state,
				vec![egui::Event::Text("they/them".into())],
			);
			let (_, labels) = run(&ctx, &mut solace, &mut state, vec![]);
			assert!(labels.iter().any(|(t, _)| t == "You have unsaved changes."));
			// The preview follows the draft.
			assert!(labels.iter().any(|(t, _)| t.ends_with("·  they/them")));
			let save = labels
				.iter()
				.find(|(t, _)| t == "Save changes")
				.expect("save button")
				.1
				.center();
			let mut sent = vec![];
			for events in click(save) {
				sent.extend(run(&ctx, &mut solace, &mut state, events).0);
			}
			let saves: Vec<_> = sent
				.into_iter()
				.filter_map(|c| match c {
					Command::EditProfile {
						user,
						request,
						changes: Some(changes),
					} => Some((user, request, changes)),
					_ => None,
				})
				.collect();
			assert_eq!(saves.len(), 1);
			let (user, request, changes) = saves.into_iter().next().unwrap();
			assert_eq!(changes.pronouns.as_deref(), Some("they/them"));
			assert_eq!(changes.global_name, None);
			let mut saved = synthetic(state.user.as_ref().unwrap());
			saved.pronouns = "they/them".into();
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::ProfileEdited {
					user,
					request,
					result: Ok(Box::new(saved)),
				},
			});
			let (_, labels) = run(&ctx, &mut solace, &mut state, vec![]);
			assert!(labels.iter().any(|(t, _)| t == "Saved in the showcase"));
			assert!(!solace.profile_editor.has_changes());
		}
	}

	#[test]
	fn a_picture_from_an_older_picker_is_ignored() {
		let ctx = egui::Context::default();
		let mut editor = Editor {
			generation: Some(3),
			revision: 2,
			choosing: true,
			draft: Some(Draft {
				name: String::new(),
				bio: String::new(),
				pronouns: String::new(),
				color: None,
				avatar: None,
			}),
			..Editor::default()
		};
		let image = egui::ColorImage::new([4, 4], vec![Color32::RED; 16]);
		let uri = "data:image/png;base64,iVBORw0KGgo=".to_owned();
		editor
			.accept_avatar(&ctx, (3, Id(1), 1), Ok(Some((uri.clone(), image.clone()))))
			.unwrap();
		assert!(editor.pending.is_none() && editor.choosing);
		editor
			.accept_avatar(&ctx, (3, Id(1), 2), Ok(Some((uri.clone(), image))))
			.unwrap();
		assert!(editor.pending.is_some() && !editor.choosing);
		assert_eq!(editor.draft.unwrap().avatar, Some(Some(uri)));
	}
}
