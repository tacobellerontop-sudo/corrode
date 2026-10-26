//! Headless CPU profile of the interface on the offline showcase: for each scene, how long a
//! frame takes to build and tessellate, how much geometry it makes, and whether the interface
//! stops asking for frames once nothing is happening.
//! `cargo run --release -p solace-ui --example profile`
use eframe::egui;
use solace_ui::Solace;
use std::time::{Duration, Instant};

#[path = "../../../apps/desktop/src/showcase.rs"]
mod showcase;

const SIZE: egui::Vec2 = egui::vec2(1536.0, 1000.0);

struct Frame {
	build: Duration,
	tessellate: Duration,
	vertices: usize,
	delay: Duration,
}

fn frame(
	ctx: &egui::Context,
	solace: &mut Solace,
	state: &mut client_core::State,
	time: f64,
) -> Frame {
	let input = egui::RawInput {
		screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
		time: Some(time),
		focused: true,
		..Default::default()
	};
	let start = Instant::now();
	let mut output = ctx.run_ui(input, |ui| {
		let window = ui.max_rect();
		let body = solace_ui::chrome::title_bar(ui, window, "Solace");
		let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body));
		let _ = solace.show(&mut body_ui, state);
		let _ = solace.take_requests();
		solace_ui::chrome::resize_edges(ui.ctx());
	});
	let build = start.elapsed();
	output.textures_delta.clear();
	let start = Instant::now();
	let meshes = ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
	let tessellate = start.elapsed();
	let vertices = meshes
		.iter()
		.map(|m| match &m.primitive {
			egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
			egui::epaint::Primitive::Callback(_) => 0,
		})
		.sum();
	let delay = output
		.viewport_output
		.values()
		.map(|v| v.repaint_delay)
		.min()
		.unwrap_or(Duration::MAX);
	Frame {
		build,
		tessellate,
		vertices,
		delay,
	}
}

fn scene(name: &str, setup: impl FnOnce(&mut Solace, &mut client_core::State, &egui::Context)) {
	let ctx = egui::Context::default();
	solace_ui::install(&ctx).unwrap();
	let mut state = showcase::state();
	let mut solace = Solace::default();
	showcase::decorate(&mut solace, &ctx, &state);
	solace.open_thread(None);
	setup(&mut solace, &mut state, &ctx);
	// Let entrances finish: simulated time advances 1/60 s per frame for three seconds.
	let mut time = 0.0;
	for _ in 0..180 {
		frame(&ctx, &mut solace, &mut state, time);
		time += 1.0 / 60.0;
	}
	// Idle: does the interface still want frames?
	let idle = frame(&ctx, &mut solace, &mut state, time);
	// Cost of a frame when one is drawn (hover, typing, an incoming message).
	let mut build = Duration::ZERO;
	let mut tess = Duration::ZERO;
	let mut vertices = 0;
	let n = 60;
	for _ in 0..n {
		time += 1.0 / 60.0;
		let f = frame(&ctx, &mut solace, &mut state, time);
		build += f.build;
		tess += f.tessellate;
		vertices = f.vertices;
	}
	let idle_text = if idle.delay == Duration::ZERO {
		"CONTINUOUS (repaints every frame while idle)".to_owned()
	} else if idle.delay == Duration::MAX {
		"idle (no repaint requested)".to_owned()
	} else {
		format!("wakes every {:?}", idle.delay)
	};
	let note = if state.history_pending {
		" (loading dots: the offline showcase never answers history requests)"
	} else {
		""
	};
	println!(
		"{name:<22} build {:>6.2} ms  tessellate {:>6.2} ms  {:>7} vertices  {idle_text}{note}",
		build.as_secs_f64() * 1000.0 / n as f64,
		tess.as_secs_f64() * 1000.0 / n as f64,
		vertices,
	);
}

fn main() {
	scene("community chat", |_, _, _| {});
	scene("community + thread", |solace, state, _| {
		let id = state
			.timeline
			.iter()
			.find(|m| !m.attachments.is_empty())
			.map(|m| m.id);
		solace.open_thread(id);
	});
	scene("home dashboard", |_, state, _| state.open_home());
	scene("direct message", |_, state, _| {
		let dm = solace_ui::nav::direct(state)[1].id;
		let _ = state.select(dm);
	});
	scene("group call", |solace, state, _| {
		showcase::call_scene(state, solace)
	});
	scene("settings dialog", |solace, _, _| {
		solace.open_settings(solace_ui::SettingsPage::Voice)
	});
}
