//! Bundled OFL faces: Inter for interface text, Source Serif 4 Display for editorial titles.
//! Eframe separately provides native system-font fallback (including color emoji).
use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::sync::{
	Mutex,
	atomic::{AtomicBool, Ordering},
};

const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
const SERIF: &[u8] = include_bytes!("../../../assets/fonts/SourceSerif4Display-Regular.ttf");
const SERIF_SEMIBOLD: &[u8] =
	include_bytes!("../../../assets/fonts/SourceSerif4Display-Semibold.ttf");
const ARABIC: &[u8] = include_bytes!("../../../assets/fonts/NotoSansArabic.ttf");
const MATH: &[u8] = include_bytes!("../../../assets/fonts/NotoSansMath-Regular.otf");
/// Noto Sans CJK JP ships as a zstd archive and is inflated the first time CJK text appears.
const CJK_ZSTD: &[u8] = include_bytes!("../../../assets/fonts/NotoSansCJKjp-Regular.otf.zst");
const CJK_BYTES: usize = 16_467_736;

pub const MEDIUM: &str = "solace-medium";
pub const SEMIBOLD: &str = "solace-semibold";
pub const SERIF_FAMILY: &str = "solace-serif";
pub const SERIF_STRONG: &str = "solace-serif-semibold";

/// Install once during application creation, before the first UI pass.
pub fn install(ctx: &Context) {
	ctx.set_fonts(definitions(false));
	let installed = AtomicBool::new(false);
	let scanned = Mutex::new(0usize);
	ctx.on_end_pass(
		"solace-cjk-fallback",
		std::sync::Arc::new(move |ui| {
			if installed.load(Ordering::Relaxed) {
				return;
			}
			let ctx = ui.ctx();
			// Scanning every frame is wasteful; a CJK glyph only needs to be noticed once.
			{
				let mut frames = scanned.lock().expect("cjk scan");
				*frames = frames.wrapping_add(1);
				if !(*frames).is_multiple_of(15) {
					return;
				}
			}
			let layers: Vec<_> = ctx.memory(|memory| memory.layer_ids().collect());
			let needed = ctx.graphics(|graphics| {
				layers.iter().any(|layer| {
					graphics
						.get(*layer)
						.is_some_and(|list| list.all_entries().any(|entry| has_cjk(&entry.shape)))
				})
			});
			if needed {
				installed.store(true, Ordering::Relaxed);
				let worker = ctx.clone();
				let spawned =
					std::thread::Builder::new()
						.name("cjk-font".into())
						.spawn(move || {
							worker.set_fonts(definitions(true));
							worker.request_repaint();
						});
				if spawned.is_err() {
					ctx.set_fonts(definitions(true));
				}
			}
		}),
	);
}

fn has_cjk(shape: &egui::Shape) -> bool {
	match shape {
		egui::Shape::Text(text) => {
			let text = &text.galley.job.text;
			!text.is_ascii()
				&& text.chars().any(|c| {
					matches!(c as u32, 0x1100..=0x11ff | 0x2e80..=0xa4cf | 0xac00..=0xd7af | 0xf900..=0xfaff | 0xfe30..=0xffef | 0x20000..=0x323af)
				})
		}
		egui::Shape::Vec(shapes) => shapes.iter().any(has_cjk),
		_ => false,
	}
}

fn face(data: &'static [u8]) -> FontData {
	let mut font = FontData::from_static(data);
	font.tweak.hinting = Some(false);
	font.tweak.subpixel_binning = Some(true);
	font
}

fn cjk() -> Vec<u8> {
	let mut font = Vec::with_capacity(CJK_BYTES);
	ruzstd::decoding::FrameDecoder::new()
		.decode_all_to_vec(CJK_ZSTD, &mut font)
		.expect("bundled CJK font archive");
	font
}

fn definitions(with_cjk: bool) -> FontDefinitions {
	let mut definitions = FontDefinitions::default();
	let defaults = definitions.families[&FontFamily::Proportional].clone();
	let primaries = [
		(FontFamily::Proportional, "Inter", INTER),
		(
			FontFamily::Name(MEDIUM.into()),
			"Inter Medium",
			INTER_MEDIUM,
		),
		(
			FontFamily::Name(SEMIBOLD.into()),
			"Inter SemiBold",
			INTER_SEMIBOLD,
		),
		(
			FontFamily::Name(SERIF_FAMILY.into()),
			"Source Serif 4 Display",
			SERIF,
		),
		(
			FontFamily::Name(SERIF_STRONG.into()),
			"Source Serif 4 Display Semibold",
			SERIF_SEMIBOLD,
		),
	];
	for (family, name, data) in primaries {
		definitions.font_data.insert(name.into(), face(data).into());
		let list = definitions.families.entry(family).or_default();
		list.retain(|existing| !defaults.contains(existing));
		list.insert(0, name.into());
		// Serif titles fall back to Inter before egui's defaults so symbols stay consistent.
		if list.len() == 1 && name.starts_with("Source") {
			list.push("Inter".into());
		}
		list.extend(defaults.iter().cloned());
	}
	let fallbacks = with_cjk
		.then(|| ("Noto Sans CJK JP", FontData::from_owned(cjk())))
		.into_iter()
		.chain([
			("Noto Sans Arabic", FontData::from_static(ARABIC)),
			("Noto Sans Math", FontData::from_static(MATH)),
		]);
	for (name, data) in fallbacks {
		definitions.font_data.insert(name.into(), data.into());
		for family in [
			FontFamily::Proportional,
			FontFamily::Monospace,
			FontFamily::Name(MEDIUM.into()),
			FontFamily::Name(SEMIBOLD.into()),
			FontFamily::Name(SERIF_FAMILY.into()),
			FontFamily::Name(SERIF_STRONG.into()),
		] {
			definitions
				.families
				.entry(family)
				.or_default()
				.push(name.into());
		}
	}
	definitions
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_named_family_resolves_latin_text() {
		let ctx = Context::default();
		ctx.set_fonts(definitions(false));
		let mut output = ctx.run_ui(Default::default(), |ui| {
			for family in [
				FontFamily::Proportional,
				FontFamily::Name(MEDIUM.into()),
				FontFamily::Name(SEMIBOLD.into()),
				FontFamily::Name(SERIF_FAMILY.into()),
				FontFamily::Name(SERIF_STRONG.into()),
			] {
				let galley = ui.painter().layout_no_wrap(
					"The Horizon".into(),
					egui::FontId::new(16.0, family),
					egui::Color32::WHITE,
				);
				assert!(galley.size().x > 40.0);
			}
		});
		output.textures_delta.clear();
	}
}
