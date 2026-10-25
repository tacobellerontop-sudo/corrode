//! Solace's painted primitives. Everything here is plain epaint geometry that the wgpu
//! renderer batches into a handful of GPU draws per frame.
use crate::{
	icons::{self, Icon},
	theme::{self, Palette, mix},
};
use egui::{
	Align2, Color32, CornerRadius, FontId, Galley, Id, Mesh, Painter, Pos2, Rect, Response, Sense,
	Shape, Stroke, StrokeKind, TextureId, Ui, Vec2,
	epaint::{RectShape, text::LayoutJob},
	pos2, vec2,
};
use std::sync::Arc;

/// A raised surface: soft shadow, fill and a hairline border.
pub fn card(painter: &Painter, rect: Rect, radius: u8, p: &Palette) {
	painter.add(theme::card_shadow(p).as_shape(rect, radius));
	painter.rect(
		rect,
		radius,
		p.surface,
		Stroke::new(1.0, p.hairline),
		StrokeKind::Inside,
	);
}

/// A floating surface (thread pane, menus, dialogs) with a deeper shadow.
pub fn floating(painter: &Painter, rect: Rect, radius: u8, p: &Palette) {
	painter.add(theme::float_shadow(p).as_shape(rect, radius));
	painter.rect(
		rect,
		radius,
		p.surface,
		Stroke::new(1.0, p.border),
		StrokeKind::Inside,
	);
}

/// Vertical gradient clipped to a rounded rectangle. `stops` are `(t, color)` with t in 0..=1.
pub fn gradient(painter: &Painter, rect: Rect, radius: f32, stops: &[(f32, Color32)]) {
	if stops.is_empty() || !rect.is_positive() {
		return;
	}
	let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
	let color_at = |t: f32| -> Color32 {
		let t = t.clamp(0.0, 1.0);
		let mut previous = stops[0];
		for stop in stops {
			if t <= stop.0 {
				let span = (stop.0 - previous.0).max(f32::EPSILON);
				return mix(previous.1, stop.1, (t - previous.0) / span);
			}
			previous = *stop;
		}
		previous.1
	};
	// Rows: every stop plus dense samples through the rounded corners.
	let mut rows: Vec<f32> = stops.iter().map(|s| s.0 * rect.height()).collect();
	for i in 0..=10 {
		let d = radius * i as f32 / 10.0;
		rows.push(d);
		rows.push(rect.height() - d);
	}
	rows.push(0.0);
	rows.push(rect.height());
	rows.sort_by(f32::total_cmp);
	rows.dedup_by(|a, b| (*a - *b).abs() < 0.25);
	let inset = |y: f32| -> f32 {
		let d = if y < radius {
			radius - y
		} else if y > rect.height() - radius {
			y - (rect.height() - radius)
		} else {
			return 0.0;
		};
		radius - (radius * radius - d * d).max(0.0).sqrt()
	};
	let mut mesh = Mesh::default();
	for (i, y) in rows.iter().enumerate() {
		let x = inset(*y);
		let color = color_at(*y / rect.height());
		mesh.colored_vertex(pos2(rect.left() + x, rect.top() + y), color);
		mesh.colored_vertex(pos2(rect.right() - x, rect.top() + y), color);
		if i > 0 {
			let base = (i as u32 - 1) * 2;
			mesh.add_triangle(base, base + 1, base + 2);
			mesh.add_triangle(base + 1, base + 3, base + 2);
		}
	}
	painter.add(Shape::mesh(mesh));
}

/// Radial glow: opaque `color` at the center fading to transparent at `radius`.
pub fn glow(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
	let mut mesh = Mesh::default();
	mesh.colored_vertex(center, color);
	let steps = 48;
	for i in 0..=steps {
		let a = i as f32 / steps as f32 * std::f32::consts::TAU;
		mesh.colored_vertex(
			center + vec2(a.cos(), a.sin()) * radius,
			Color32::TRANSPARENT,
		);
		if i > 0 {
			mesh.add_triangle(0, i, i + 1);
		}
	}
	painter.add(Shape::mesh(mesh));
}

/// Paint a texture into a rounded rectangle, cropping to fill (like CSS `object-fit: cover`).
pub fn cover_image(
	painter: &Painter,
	rect: Rect,
	radius: impl Into<CornerRadius>,
	texture: TextureId,
	size: Vec2,
) {
	let uv = cover_uv(rect.size(), size);
	painter.add(RectShape::filled(rect, radius, Color32::WHITE).with_texture(texture, uv));
}

pub fn cover_uv(target: Vec2, source: Vec2) -> Rect {
	if source.x <= 0.0 || source.y <= 0.0 || target.x <= 0.0 || target.y <= 0.0 {
		return Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
	}
	let target_ratio = target.x / target.y;
	let source_ratio = source.x / source.y;
	if source_ratio > target_ratio {
		let w = target_ratio / source_ratio;
		Rect::from_min_max(pos2((1.0 - w) / 2.0, 0.0), pos2((1.0 + w) / 2.0, 1.0))
	} else {
		let h = source_ratio / target_ratio;
		Rect::from_min_max(pos2(0.0, (1.0 - h) / 2.0), pos2(1.0, (1.0 + h) / 2.0))
	}
}

#[derive(Clone, Copy)]
pub enum AvatarShape {
	Circle,
	Rounded(f32),
}

/// Avatar or community icon: a texture when loaded, otherwise a soft tinted monogram.
pub fn avatar(
	painter: &Painter,
	rect: Rect,
	shape: AvatarShape,
	texture: Option<(TextureId, Vec2)>,
	name: &str,
	seed: u64,
	p: &Palette,
) {
	let radius = match shape {
		AvatarShape::Circle => rect.width() / 2.0,
		AvatarShape::Rounded(r) => r,
	};
	if let Some((texture, size)) = texture {
		cover_image(painter, rect, radius, texture, size);
		return;
	}
	let tone = theme::seeded_hue(seed, p.dark);
	let top = mix(tone, Color32::WHITE, if p.dark { 0.12 } else { 0.35 });
	let bottom = mix(tone, Color32::BLACK, if p.dark { 0.25 } else { 0.08 });
	gradient(painter, rect, radius, &[(0.0, top), (1.0, bottom)]);
	let initial: String = name
		.chars()
		.find(|c| c.is_alphanumeric())
		.map(|c| c.to_uppercase().collect())
		.unwrap_or_else(|| "·".into());
	painter.text(
		rect.center() + vec2(0.0, 0.5),
		Align2::CENTER_CENTER,
		initial,
		theme::serif_strong(rect.height() * 0.46),
		Color32::from_white_alpha(240),
	);
}

/// Presence indicator ringed with the background so it reads as cut out of the avatar.
pub fn presence(painter: &Painter, center: Pos2, radius: f32, color: Color32, ring: Color32) {
	painter.circle_filled(center, radius + 2.0, ring);
	painter.circle_filled(center, radius, color);
}

pub fn status_color(status: Option<&str>, p: &Palette) -> Option<Color32> {
	match status? {
		"online" => Some(p.online),
		"idle" => Some(p.idle),
		"dnd" => Some(p.dnd),
		_ => None,
	}
}

pub fn status_label(status: Option<&str>) -> &'static str {
	match status {
		Some("online") => "Online",
		Some("idle") => "Away",
		Some("dnd") => "Do not disturb",
		_ => "Offline",
	}
}

/// Lay out a single line, eliding with an ellipsis beyond `max_width`.
pub fn line(
	painter: &Painter,
	text: &str,
	font: FontId,
	color: Color32,
	max_width: f32,
) -> Arc<Galley> {
	let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
	job.wrap = egui::text::TextWrapping::truncate_at_width(max_width.max(1.0));
	painter.layout_job(job)
}

/// Paint a single elided line with its left edge at `pos` and vertical center on `pos.y`.
pub fn text_left(
	painter: &Painter,
	pos: Pos2,
	text: &str,
	font: FontId,
	color: Color32,
	max_width: f32,
) -> Rect {
	let galley = line(painter, text, font, color, max_width);
	let rect = Align2::LEFT_CENTER.anchor_size(pos, galley.size());
	painter.galley(rect.min, galley, color);
	rect
}

/// Single-line text helper without elision.
pub fn label(
	painter: &Painter,
	pos: Pos2,
	anchor: Align2,
	text: &str,
	font: FontId,
	color: Color32,
) -> Rect {
	let galley = painter.layout_no_wrap(text.to_owned(), font, color);
	let rect = anchor.anchor_size(pos, galley.size());
	painter.galley(rect.min, galley, color);
	rect
}

/// Hover/press animation in 0..=1 for an interactive element.
pub fn hover_t(ui: &Ui, response: &Response) -> f32 {
	ui.ctx()
		.animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
	/// Round, tinted with the quiet surface color (header actions).
	Soft,
	/// No fill until hovered (inline actions).
	Ghost,
	/// Accent fill.
	Primary,
	/// Red fill for destructive or leave actions.
	Danger,
}

/// Circular icon button.
pub fn icon_button(
	ui: &mut Ui,
	id: Id,
	rect: Rect,
	icon: Icon,
	kind: ButtonKind,
	active: bool,
	tooltip: &str,
) -> Response {
	let response = ui.interact(rect, id, Sense::click());
	let p = theme::palette(ui.ctx());
	let t = hover_t(ui, &response);
	let painter = ui.painter();
	let (fill, color) = match kind {
		ButtonKind::Soft => (
			mix(
				if active { p.selected } else { p.surface_alt },
				p.hover,
				t * 0.8,
			),
			if active { p.text } else { p.text_soft },
		),
		ButtonKind::Ghost => (
			mix(
				if active {
					p.selected
				} else {
					p.selected.gamma_multiply(0.0)
				},
				p.hover,
				t,
			),
			mix(if active { p.text } else { p.muted }, p.text, t),
		),
		ButtonKind::Primary => (mix(p.accent, p.accent_hover, t), p.accent_text),
		ButtonKind::Danger => (mix(p.danger, Color32::BLACK, t * 0.12), Color32::WHITE),
	};
	painter.circle_filled(rect.center(), rect.width() / 2.0, fill);
	let glyph = (rect.width() * 0.46).clamp(14.0, 22.0);
	icons::paint_at(painter, icon, rect.center(), glyph, color);
	if !tooltip.is_empty() {
		response.on_hover_text(tooltip)
	} else {
		response
	}
}

/// Rounded pill button with an optional leading icon. Returns the click response.
pub fn pill_button(
	ui: &mut Ui,
	id: Id,
	rect: Rect,
	icon: Option<Icon>,
	text: &str,
	kind: ButtonKind,
	enabled: bool,
) -> Response {
	let sense = if enabled {
		Sense::click()
	} else {
		Sense::hover()
	};
	let response = ui.interact(rect, id, sense);
	let p = theme::palette(ui.ctx());
	let t = if enabled { hover_t(ui, &response) } else { 0.0 };
	let (fill, color, stroke) = match kind {
		ButtonKind::Primary => (
			mix(p.accent, p.accent_hover, t),
			p.accent_text,
			Stroke::NONE,
		),
		ButtonKind::Danger => (
			mix(p.danger, Color32::BLACK, t * 0.12),
			Color32::WHITE,
			Stroke::NONE,
		),
		ButtonKind::Soft => (mix(p.surface_alt, p.hover, t), p.text, Stroke::NONE),
		ButtonKind::Ghost => (
			mix(p.surface, p.hover, t),
			p.text,
			Stroke::new(1.0, p.border),
		),
	};
	let fill = if enabled {
		fill
	} else {
		fill.gamma_multiply(0.55)
	};
	let painter = ui.painter();
	painter.rect(
		rect,
		CornerRadius::same((rect.height() / 2.0) as u8),
		fill,
		stroke,
		StrokeKind::Inside,
	);
	let font = theme::medium(13.5);
	let galley = painter.layout_no_wrap(text.to_owned(), font, color);
	let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
	let total = icon_w + galley.size().x;
	let start = rect.center().x - total / 2.0;
	if let Some(icon) = icon {
		icons::paint_at(
			painter,
			icon,
			pos2(start + 8.0, rect.center().y),
			16.0,
			color,
		);
	}
	painter.galley(
		pos2(start + icon_w, rect.center().y - galley.size().y / 2.0),
		galley,
		color,
	);
	response
}

/// Thin separator line.
pub fn hairline(painter: &Painter, from: Pos2, to: Pos2, p: &Palette) {
	painter.line_segment([from, to], Stroke::new(1.0, p.hairline));
}

/// Little rounded tag chip; returns its rect.
pub fn chip(painter: &Painter, pos: Pos2, text: &str, p: &Palette) -> Rect {
	let galley = painter.layout_no_wrap(text.to_owned(), theme::sans(12.0), p.text_soft);
	let rect = Rect::from_min_size(pos, galley.size() + vec2(20.0, 12.0));
	painter.rect(
		rect,
		9,
		p.surface,
		Stroke::new(1.0, p.border),
		StrokeKind::Inside,
	);
	painter.galley(rect.min + vec2(10.0, 6.0), galley, p.text_soft);
	rect
}

/// Count badge for unread mentions.
pub fn badge(painter: &Painter, center: Pos2, count: u32, p: &Palette) {
	let text = if count > 99 {
		"99+".to_owned()
	} else {
		count.to_string()
	};
	let galley = painter.layout_no_wrap(text, theme::semibold(10.5), p.accent_text);
	let size = vec2((galley.size().x + 10.0).max(18.0), 18.0);
	let rect = Rect::from_center_size(center, size);
	painter.rect_filled(rect, 9, p.accent);
	painter.galley(rect.center() - galley.size() / 2.0, galley, p.accent_text);
}
