//! Procedural misty-mountain artwork for community banners and the voice room card.
//! Each picture is rasterized once per community on the CPU and then lives on the GPU as a
//! texture, so repainting costs one textured quad.
use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mood {
	/// Warm dawn haze above layered ridges; used for community banners.
	Dawn,
	/// Pale sage mist over a still lake; used for voice rooms.
	Lake,
}

#[derive(Default)]
pub struct Scenery {
	textures: HashMap<(u64, Mood), TextureHandle>,
}

impl Scenery {
	pub fn texture(&mut self, ctx: &Context, seed: u64, mood: Mood) -> &TextureHandle {
		self.textures.entry((seed, mood)).or_insert_with(|| {
			let (w, h) = match mood {
				Mood::Dawn => (1280, 380),
				Mood::Lake => (720, 480),
			};
			ctx.load_texture(
				format!("scenery-{seed}-{mood:?}"),
				render(w, h, seed, mood),
				TextureOptions::LINEAR,
			)
		})
	}
}

fn hash(n: i64, seed: u64) -> f32 {
	let mut x =
		(n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ seed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
	x ^= x >> 31;
	x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
	x ^= x >> 29;
	(x >> 40) as f32 / (1u64 << 24) as f32
}

fn noise(x: f32, seed: u64) -> f32 {
	let i = x.floor();
	let f = x - i;
	let t = f * f * (3.0 - 2.0 * f);
	let a = hash(i as i64, seed);
	let b = hash(i as i64 + 1, seed);
	a + (b - a) * t
}

/// Ridged fractal noise: sharp peaks, soft valleys.
fn ridge(x: f32, seed: u64, octaves: u32) -> f32 {
	let mut sum = 0.0;
	let mut amp = 0.55;
	let mut freq = 1.0;
	let mut norm = 0.0;
	for o in 0..octaves {
		let n = noise(x * freq, seed.wrapping_add(o as u64 * 977));
		let r = 1.0 - (2.0 * n - 1.0).abs();
		sum += r * r * amp;
		norm += amp;
		amp *= 0.5;
		freq *= 2.1;
	}
	sum / norm
}

#[derive(Clone, Copy)]
struct Rgb(f32, f32, f32);

impl Rgb {
	const fn hex(v: u32) -> Self {
		Self(
			((v >> 16) & 0xff) as f32 / 255.0,
			((v >> 8) & 0xff) as f32 / 255.0,
			(v & 0xff) as f32 / 255.0,
		)
	}
	fn mix(self, o: Self, t: f32) -> Self {
		let t = t.clamp(0.0, 1.0);
		Self(
			self.0 + (o.0 - self.0) * t,
			self.1 + (o.1 - self.1) * t,
			self.2 + (o.2 - self.2) * t,
		)
	}
	fn color(self) -> Color32 {
		let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
		Color32::from_rgb(c(self.0), c(self.1), c(self.2))
	}
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
	let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
	t * t * (3.0 - 2.0 * t)
}

struct Recipe {
	sky_top: Rgb,
	sky_horizon: Rgb,
	sun: Rgb,
	sun_pos: (f32, f32),
	fog: Rgb,
	layers: [(f32, f32, Rgb); 5],
	lake: Option<(f32, Rgb)>,
}

fn recipe(seed: u64, mood: Mood) -> Recipe {
	let shift = hash(7, seed);
	match mood {
		Mood::Dawn => Recipe {
			sky_top: Rgb::hex(0x9aa3ad).mix(Rgb::hex(0xa79e9a), shift),
			sky_horizon: Rgb::hex(0xe8c9a8).mix(Rgb::hex(0xf0d3b8), shift),
			sun: Rgb::hex(0xfff1dc),
			sun_pos: (0.55 + 0.2 * (hash(3, seed) - 0.5), 0.36),
			fog: Rgb::hex(0xd9c6b4),
			layers: [
				(0.34, 0.16, Rgb::hex(0x9c97a0)),
				(0.44, 0.17, Rgb::hex(0x7d7884)),
				(0.56, 0.18, Rgb::hex(0x58535d)),
				(0.70, 0.20, Rgb::hex(0x3a3640)),
				(0.84, 0.20, Rgb::hex(0x24212a)),
			],
			lake: None,
		},
		Mood::Lake => Recipe {
			sky_top: Rgb::hex(0xc9d8d2),
			sky_horizon: Rgb::hex(0xeef2ee),
			sun: Rgb::hex(0xfbfdf9),
			sun_pos: (0.3 + 0.3 * hash(5, seed), 0.42),
			fog: Rgb::hex(0xe4ece8),
			layers: [
				(0.30, 0.14, Rgb::hex(0xb7cbc3)),
				(0.38, 0.16, Rgb::hex(0x9fbab0)),
				(0.47, 0.16, Rgb::hex(0x86a99c)),
				(0.55, 0.12, Rgb::hex(0x6f9587)),
				(0.62, 0.08, Rgb::hex(0x5d8475)),
			],
			lake: Some((0.66, Rgb::hex(0xc4d6cf))),
		},
	}
}

/// Rasterize one scene. Deterministic for a given seed and mood.
pub fn render(width: usize, height: usize, seed: u64, mood: Mood) -> ColorImage {
	let recipe = recipe(seed, mood);
	let mut ridges = vec![[0.0f32; 5]; width];
	for (x, column) in ridges.iter_mut().enumerate() {
		let u = x as f32 / width as f32;
		for (l, (base, amp, _)) in recipe.layers.iter().enumerate() {
			let frequency = 2.2 + l as f32 * 1.1;
			let r = ridge(
				u * frequency + l as f32 * 13.7,
				seed.wrapping_add(l as u64 * 31),
				5,
			);
			column[l] = base - amp * (r - 0.35);
		}
	}
	let aspect = width as f32 / height as f32;
	let sample = |u: f32, v: f32, column: &[f32; 5]| -> Rgb {
		let mut color = recipe
			.sky_top
			.mix(recipe.sky_horizon, smoothstep(0.0, 0.62, v));
		let dx = (u - recipe.sun_pos.0) * aspect;
		let dy = v - recipe.sun_pos.1;
		let d = (dx * dx + dy * dy).sqrt();
		color = color.mix(recipe.sun, (1.0 - d / 0.55).max(0.0).powf(2.2) * 0.85);
		for (l, (_, _, layer)) in recipe.layers.iter().enumerate() {
			let top = column[l];
			if v >= top {
				let depth = v - top;
				// Mist pools in the valley below each ridge line.
				let haze = smoothstep(0.0, 0.16, depth) * (0.55 - l as f32 * 0.09).max(0.12);
				let atmosphere = 0.18 * (4 - l) as f32 / 4.0;
				color = layer.mix(recipe.fog, atmosphere).mix(recipe.fog, haze).mix(
					recipe.sun,
					(1.0 - d / 0.8).max(0.0) * 0.12 * (4 - l) as f32 / 4.0,
				);
			}
		}
		color
	};
	let mut image = ColorImage::filled([width, height], Color32::BLACK);
	for y in 0..height {
		let v = y as f32 / height as f32;
		for (x, column) in ridges.iter().enumerate() {
			let u = x as f32 / width as f32;
			let color = match recipe.lake {
				Some((line, water)) if v > line => {
					let mirrored = line - (v - line) * 0.9;
					let ripple = (noise(v * 140.0 + (u * 6.0).sin(), seed) - 0.5) * 0.012;
					let reflected = sample(u + ripple, mirrored.max(0.0), column);
					let fade = smoothstep(line, 1.0, v);
					reflected.mix(water, 0.35 + fade * 0.35)
				}
				_ => sample(u, v, column),
			};
			// A gentle grain keeps large gradients from banding on 8-bit displays.
			let grain = (hash((y * width + x) as i64, seed ^ 0x51) - 0.5) / 255.0 * 2.0;
			image.pixels[y * width + x] =
				Rgb(color.0 + grain, color.1 + grain, color.2 + grain).color();
		}
	}
	image
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn scenes_are_deterministic_and_varied() {
		let a = render(64, 24, 1, Mood::Dawn);
		let b = render(64, 24, 1, Mood::Dawn);
		let c = render(64, 24, 2, Mood::Dawn);
		assert_eq!(a.pixels, b.pixels);
		assert_ne!(a.pixels, c.pixels);
		// Sky is lighter than the foreground ridges.
		let luma = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
		assert!(luma(a.pixels[2]) > luma(a.pixels[a.pixels.len() - 2]));
		let lake = render(48, 32, 3, Mood::Lake);
		assert_eq!(lake.pixels.len(), 48 * 32);
	}
}
