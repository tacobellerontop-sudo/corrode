//! Transient notices, stacked bottom-center above the composer.
use crate::{
	icons::{self, Icon},
	paint,
	theme::{self},
};
use egui::{Align2, Area, Context, Order, Rect, vec2};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
	Info,
	Success,
	Warning,
	Error,
}

const LIFETIME: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct Toasts {
	items: Vec<(Level, String, Instant)>,
}

impl Toasts {
	pub fn push(&mut self, level: Level, text: impl Into<String>) {
		let text = text.into();
		if text.is_empty() {
			return;
		}
		self.items.retain(|(_, existing, _)| *existing != text);
		self.items.push((level, text, Instant::now()));
		if self.items.len() > 4 {
			self.items.remove(0);
		}
	}

	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}

	pub fn show(&mut self, ctx: &Context) {
		self.items.retain(|(_, _, at)| at.elapsed() < LIFETIME);
		if self.items.is_empty() {
			return;
		}
		let p = theme::palette(ctx);
		let screen = ctx.content_rect();
		let mut y = screen.bottom() - 112.0;
		let mut dismissed = None;
		for (index, (level, text, at)) in self.items.iter().enumerate().rev() {
			let age = at.elapsed().as_secs_f32();
			let fade = crate::motion::ease_out((age / 0.25).min(1.0))
				* ((LIFETIME.as_secs_f32() - age) / 0.4).clamp(0.0, 1.0);
			let galley =
				ctx.fonts_mut(|f| f.layout(text.clone(), theme::medium(13.5), p.text, 420.0));
			let size = vec2(galley.size().x + 64.0, galley.size().y + 24.0);
			let rect = Rect::from_center_size(
				egui::pos2(screen.center().x, y - size.y / 2.0 + (1.0 - fade) * 14.0),
				size,
			);
			y -= size.y + 10.0;
			Area::new(egui::Id::unique(("toast", index)))
				.order(Order::Tooltip)
				.fixed_pos(rect.min)
				.show(ctx, |ui| {
					ui.multiply_opacity(fade);
					let (_, response) = ui.allocate_exact_size(size, egui::Sense::click());
					let painter = ui.painter().with_clip_rect(rect.expand(40.0));
					paint::floating(&painter, rect, 14, &p);
					let (icon, color) = match level {
						Level::Info => (Icon::Bell, p.accent),
						Level::Success => (Icon::Check, p.online),
						Level::Warning => (Icon::Bell, p.idle),
						Level::Error => (Icon::Close, p.danger),
					};
					painter.circle_filled(
						rect.left_center() + vec2(22.0, 0.0),
						11.0,
						color.gamma_multiply(0.15),
					);
					icons::paint_at(
						&painter,
						icon,
						rect.left_center() + vec2(22.0, 0.0),
						14.0,
						color,
					);
					painter.galley(
						Align2::LEFT_CENTER
							.anchor_size(rect.left_center() + vec2(42.0, 0.0), galley.size())
							.min,
						galley.clone(),
						p.text,
					);
					if response.clicked() {
						dismissed = Some(index);
					}
				});
		}
		if let Some(index) = dismissed {
			self.items.remove(index);
		}
		// Smooth while a toast is entering or leaving; otherwise wake for the next expiry.
		let moving = self.items.iter().any(|(_, _, at)| {
			let age = at.elapsed().as_secs_f32();
			age < 0.3 || age > LIFETIME.as_secs_f32() - 0.5
		});
		if moving {
			ctx.request_repaint();
		} else {
			ctx.request_repaint_after(Duration::from_millis(100));
		}
	}
}
