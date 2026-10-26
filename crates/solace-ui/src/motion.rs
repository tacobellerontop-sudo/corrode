//! Small motion helpers. Entrances are measured from when something first appears on screen,
//! so they replay whenever it comes back after being hidden, and they only request repaints
//! while they are running.
use egui::{Context, Id};

/// Cubic ease-out: quick start, soft landing.
pub fn ease_out(t: f32) -> f32 {
	1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Eased progress of an animation that began at egui time `since`.
pub fn progress(ctx: &Context, since: f64, duration: f32) -> f32 {
	let elapsed = (ctx.input(|i| i.time) - since) as f32;
	let t = (elapsed / duration.max(0.001)).clamp(0.0, 1.0);
	if t < 1.0 {
		ctx.request_repaint();
	}
	ease_out(t)
}

/// Entrance progress (0..=1) for something identified by `id`. It restarts when `id` was not
/// shown on the previous pass.
pub fn appear(ctx: &Context, id: Id, duration: f32) -> f32 {
	appear_after(ctx, id, 0.0, duration)
}

/// Like [`appear`], but waits `delay` seconds first (for staggered groups).
pub fn appear_after(ctx: &Context, id: Id, delay: f32, duration: f32) -> f32 {
	let since = shown_since(ctx, id);
	progress(ctx, since + f64::from(delay), duration)
}

/// Egui time at which `id` started its current uninterrupted run of visible passes.
pub fn shown_since(ctx: &Context, id: Id) -> f64 {
	let time = ctx.input(|i| i.time);
	let pass = ctx.cumulative_pass_nr();
	ctx.data_mut(|data| {
		let entry = data.get_temp_mut_or_insert_with(id.with("motion"), || (time, pass));
		if pass > entry.1 + 1 {
			*entry = (time, pass);
		} else {
			entry.1 = pass;
		}
		entry.0
	})
}

/// Entrance progress that plays only the first time `id` is ever shown (new messages).
pub fn once(ctx: &Context, id: Id, duration: f32) -> f32 {
	let time = ctx.input(|i| i.time);
	let since = ctx.data_mut(|data| *data.get_temp_mut_or_insert_with(id.with("once"), || time));
	progress(ctx, since, duration)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn easing_is_bounded_and_monotonic() {
		assert_eq!(ease_out(0.0), 0.0);
		assert_eq!(ease_out(1.0), 1.0);
		assert_eq!(ease_out(2.0), 1.0);
		let mut last = 0.0;
		for i in 0..=20 {
			let v = ease_out(i as f32 / 20.0);
			assert!(v >= last);
			last = v;
		}
	}
}
