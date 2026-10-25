//! Solace design tokens: a warm paper palette (with a matching night palette), type scale,
//! radii and elevation. Every view paints from these tokens; no stock egui chrome is used.
use crate::fonts::{MEDIUM, SEMIBOLD, SERIF_FAMILY, SERIF_STRONG};
use egui::{Color32, Context, FontFamily, FontId, Shadow};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
	pub dark: bool,
	/// Main conversation canvas.
	pub canvas: Color32,
	/// Navigation column behind the community and message lists.
	pub sidebar: Color32,
	/// Icon rail at the far left.
	pub rail: Color32,
	/// Right-hand context column.
	pub aside: Color32,
	/// Raised cards, composer and popovers.
	pub surface: Color32,
	/// Quiet fills: icon buttons, chips, search field.
	pub surface_alt: Color32,
	pub hover: Color32,
	pub selected: Color32,
	pub border: Color32,
	pub hairline: Color32,
	pub text: Color32,
	pub text_soft: Color32,
	pub muted: Color32,
	pub faint: Color32,
	pub accent: Color32,
	pub accent_hover: Color32,
	pub accent_text: Color32,
	pub accent_soft: Color32,
	pub online: Color32,
	pub idle: Color32,
	pub dnd: Color32,
	pub listening: Color32,
	pub danger: Color32,
	pub mention: Color32,
	pub shadow: Color32,
	pub scrim: Color32,
}

const fn hex(value: u32) -> Color32 {
	Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

pub const LIGHT: Palette = Palette {
	dark: false,
	canvas: hex(0xf8f6f3),
	sidebar: hex(0xf3f1ed),
	rail: hex(0xf3f1ed),
	aside: hex(0xf5f3f0),
	surface: hex(0xffffff),
	surface_alt: hex(0xf0ede9),
	hover: hex(0xeeebe6),
	selected: hex(0xe8e4de),
	border: hex(0xe7e2dc),
	hairline: hex(0xeeeae5),
	text: hex(0x272523),
	text_soft: hex(0x4f4b46),
	muted: hex(0x8d8780),
	faint: hex(0xb4aea6),
	accent: hex(0x2f6a5a),
	accent_hover: hex(0x285c4e),
	accent_text: hex(0xffffff),
	accent_soft: hex(0xe2ece7),
	online: hex(0x3ea96c),
	idle: hex(0xe9a13b),
	dnd: hex(0xd4574e),
	listening: hex(0x9b82d6),
	danger: hex(0xc4554a),
	mention: hex(0xf6ead2),
	shadow: Color32::from_rgba_premultiplied(40, 28, 14, 16),
	scrim: Color32::from_rgba_premultiplied(24, 20, 16, 90),
};

pub const DARK: Palette = Palette {
	dark: true,
	canvas: hex(0x18191b),
	sidebar: hex(0x141517),
	rail: hex(0x141517),
	aside: hex(0x161719),
	surface: hex(0x202124),
	surface_alt: hex(0x26272b),
	hover: hex(0x2a2b2f),
	selected: hex(0x313237),
	border: hex(0x2e2f33),
	hairline: hex(0x26272a),
	text: hex(0xecebe7),
	text_soft: hex(0xc9c5bf),
	muted: hex(0x8f8b85),
	faint: hex(0x67635e),
	accent: hex(0x5ea58c),
	accent_hover: hex(0x6db59b),
	accent_text: hex(0x0f1a16),
	accent_soft: hex(0x223530),
	online: hex(0x4cc282),
	idle: hex(0xf0b04f),
	dnd: hex(0xe0675e),
	listening: hex(0xae98e6),
	danger: hex(0xe0675e),
	mention: hex(0x3a3324),
	shadow: Color32::from_rgba_premultiplied(0, 0, 0, 70),
	scrim: Color32::from_rgba_premultiplied(0, 0, 0, 140),
};

pub fn palette(ctx: &Context) -> Palette {
	if ctx.theme() == egui::Theme::Dark {
		DARK
	} else {
		LIGHT
	}
}

/// Layout constants shared by the shell.
pub mod size {
	pub const RAIL: f32 = 80.0;
	pub const SIDEBAR: f32 = 272.0;
	pub const ASIDE: f32 = 326.0;
	pub const HEADER: f32 = 84.0;
	pub const GUTTER: f32 = 24.0;
	pub const RADIUS_CARD: u8 = 18;
	pub const RADIUS_ITEM: u8 = 12;
}

pub fn sans(size: f32) -> FontId {
	FontId::new(size, FontFamily::Proportional)
}
pub fn medium(size: f32) -> FontId {
	FontId::new(size, FontFamily::Name(MEDIUM.into()))
}
pub fn semibold(size: f32) -> FontId {
	FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn serif(size: f32) -> FontId {
	FontId::new(size, FontFamily::Name(SERIF_FAMILY.into()))
}
pub fn serif_strong(size: f32) -> FontId {
	FontId::new(size, FontFamily::Name(SERIF_STRONG.into()))
}
pub fn mono(size: f32) -> FontId {
	FontId::monospace(size)
}

/// Soft ambient elevation used by cards and the composer.
pub fn card_shadow(p: &Palette) -> Shadow {
	Shadow {
		offset: [0, 6],
		blur: 24,
		spread: 0,
		color: p.shadow,
	}
}

/// Deeper elevation for floating panels (thread, menus, dialogs).
pub fn float_shadow(p: &Palette) -> Shadow {
	Shadow {
		offset: [0, 14],
		blur: 40,
		spread: 2,
		color: p.shadow.gamma_multiply(1.6),
	}
}

/// Configure egui so the few stock primitives we still lean on (text editing, scroll
/// physics, selection) inherit Solace tokens instead of egui's default look.
pub fn apply(ctx: &Context) {
	for (theme, p) in [(egui::Theme::Light, LIGHT), (egui::Theme::Dark, DARK)] {
		ctx.style_mut_of(theme, |style| {
			let visuals = &mut style.visuals;
			*visuals = if p.dark {
				egui::Visuals::dark()
			} else {
				egui::Visuals::light()
			};
			visuals.panel_fill = p.canvas;
			visuals.window_fill = p.surface;
			visuals.window_stroke = egui::Stroke::new(1.0, p.border);
			visuals.window_corner_radius = egui::CornerRadius::same(size::RADIUS_CARD);
			visuals.window_shadow = float_shadow(&p);
			visuals.popup_shadow = card_shadow(&p);
			visuals.menu_corner_radius = egui::CornerRadius::same(12);
			visuals.extreme_bg_color = p.surface;
			visuals.faint_bg_color = p.surface_alt;
			visuals.code_bg_color = p.surface_alt;
			visuals.hyperlink_color = p.accent;
			visuals.override_text_color = None;
			visuals.selection.bg_fill = p.accent.gamma_multiply(0.28);
			visuals.selection.stroke = egui::Stroke::new(1.0, p.accent);
			visuals.text_cursor.stroke = egui::Stroke::new(1.6, p.accent);
			for widget in [
				&mut visuals.widgets.noninteractive,
				&mut visuals.widgets.inactive,
				&mut visuals.widgets.hovered,
				&mut visuals.widgets.active,
				&mut visuals.widgets.open,
			] {
				widget.corner_radius = egui::CornerRadius::same(10);
				widget.fg_stroke.color = p.text;
				widget.bg_stroke = egui::Stroke::NONE;
				widget.expansion = 0.0;
			}
			visuals.widgets.noninteractive.bg_fill = p.surface;
			visuals.widgets.noninteractive.fg_stroke.color = p.text_soft;
			visuals.widgets.inactive.bg_fill = p.surface_alt;
			visuals.widgets.inactive.weak_bg_fill = p.surface_alt;
			visuals.widgets.hovered.bg_fill = p.hover;
			visuals.widgets.hovered.weak_bg_fill = p.hover;
			visuals.widgets.active.bg_fill = p.selected;
			visuals.widgets.active.weak_bg_fill = p.selected;

			let spacing = &mut style.spacing;
			spacing.item_spacing = egui::vec2(8.0, 6.0);
			spacing.button_padding = egui::vec2(12.0, 6.0);
			let scroll = &mut spacing.scroll;
			scroll.floating = true;
			scroll.bar_width = 6.0;
			scroll.floating_width = 2.0;
			scroll.floating_allocated_width = 0.0;
			scroll.bar_inner_margin = 3.0;
			scroll.bar_outer_margin = 2.0;
			scroll.foreground_color = false;
			scroll.dormant_handle_opacity = 0.0;
			scroll.active_handle_opacity = 0.45;
			scroll.interact_handle_opacity = 0.7;
			scroll.dormant_background_opacity = 0.0;
			scroll.active_background_opacity = 0.0;
			scroll.interact_background_opacity = 0.0;
			scroll.handle_min_length = 24.0;
			style.text_styles.insert(egui::TextStyle::Body, sans(14.0));
			style
				.text_styles
				.insert(egui::TextStyle::Button, medium(13.5));
			style.text_styles.insert(egui::TextStyle::Small, sans(11.5));
			style
				.text_styles
				.insert(egui::TextStyle::Heading, serif(22.0));
			style
				.text_styles
				.insert(egui::TextStyle::Monospace, mono(13.0));
			style.interaction.selectable_labels = false;
			style.animation_time = 0.14;
		});
	}
}

/// Linear blend in gamma space; adequate for hover tints and gradients.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
	let t = t.clamp(0.0, 1.0);
	let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
	Color32::from_rgba_premultiplied(
		lerp(a.r(), b.r()),
		lerp(a.g(), b.g()),
		lerp(a.b(), b.b()),
		lerp(a.a(), b.a()),
	)
}

/// Stable, gentle hue for a seed (user or community id) used for placeholder art.
pub fn seeded_hue(seed: u64, dark: bool) -> Color32 {
	const TONES: [u32; 8] = [
		0x7c9a8e, 0xb08968, 0x8a9bb5, 0xc29a8a, 0x9a8bb8, 0x8fae9b, 0xb5a27c, 0x7f97a8,
	];
	let base = hex(TONES[(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 61) as usize]);
	if dark {
		mix(base, Color32::BLACK, 0.25)
	} else {
		base
	}
}
