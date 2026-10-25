//! Offline visual preview of Solace with the synthetic showcase; no account or network.
//! `--out path.png` saves a screenshot after the first frames and exits.
use eframe::egui;
use solace_ui::Solace;

#[path = "../../../apps/desktop/src/showcase.rs"]
mod showcase;

struct Preview {
	state: client_core::State,
	solace: Solace,
	out: Option<String>,
	signin: bool,
	frames: u32,
	token: String,
	authorized: bool,
}

impl eframe::App for Preview {
	fn persist_egui_memory(&self) -> bool {
		false
	}

	fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
		let ctx = ui.ctx().clone();
		if self.signin {
			let accounts = [model::SavedAccount {
				id: model::Id(101),
				name: "lina".into(),
				display: Some("Lina".into()),
				avatar: None,
				discriminator: 0,
				has_token: true,
			}];
			let _ = solace_ui::signin::show(
				&mut self.solace,
				ui,
				solace_ui::signin::Props {
					accounts: &accounts,
					status: "",
					busy: false,
					token: &mut self.token,
					authorized: &mut self.authorized,
				},
			);
		} else {
			let _ = self.solace.show(ui, &mut self.state);
			let _ = self.solace.take_requests();
		}
		self.frames += 1;
		if let Some(out) = &self.out
			&& self.frames == 12
		{
			let out = out.clone();
			let close = ctx.clone();
			ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
				egui::ScreenshotCallback::new(move |image| {
					let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
					image::save_buffer(
						&out,
						&rgba,
						image.size[0] as u32,
						image.size[1] as u32,
						image::ColorType::Rgba8,
					)
					.expect("save screenshot");
					close.send_viewport_cmd(egui::ViewportCommand::Close);
					close.request_repaint();
				}),
			));
		}
		ctx.request_repaint();
	}
}

fn main() -> eframe::Result {
	let args: Vec<String> = std::env::args().collect();
	let out = args
		.iter()
		.position(|a| a == "--out")
		.and_then(|i| args.get(i + 1).cloned());
	let dark = args.iter().any(|a| a == "--dark");
	let signin = args.iter().any(|a| a == "--signin");
	let size = args
		.iter()
		.position(|a| a == "--size")
		.and_then(|i| args.get(i + 1))
		.and_then(|s| s.split_once('x'))
		.and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?]))
		.unwrap_or([1536.0, 1024.0]);
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_inner_size(size)
			.with_title("Solace preview"),
		renderer: eframe::Renderer::Wgpu,
		persist_window: false,
		..Default::default()
	};
	eframe::run_native(
		"Solace preview",
		options,
		Box::new(move |cc| {
			let args: Vec<String> = std::env::args().collect();
			solace_ui::install(&cc.egui_ctx)?;
			cc.egui_ctx.set_theme(if dark {
				egui::ThemePreference::Dark
			} else {
				egui::ThemePreference::Light
			});
			let mut state = showcase::state();
			let mut solace = Solace::default();
			showcase::decorate(&mut solace, &cc.egui_ctx, &state);
			if dark {
				solace.theme = egui::ThemePreference::Dark;
			}
			if args.iter().any(|a| a == "--dm") {
				solace.open_thread(None);
				let dm = solace_ui::nav::direct(&state)
					.first()
					.map(|c| c.id)
					.unwrap();
				let _ = state.select(dm);
			}
			if args.iter().any(|a| a == "--home") {
				solace.open_thread(None);
				state.open_home();
			}
			if args.iter().any(|a| a == "--settings") {
				solace.open_settings(solace_ui::SettingsPage::Appearance);
			}
			Ok(Box::new(Preview {
				state,
				solace,
				out,
				signin,
				frames: 0,
				token: String::new(),
				authorized: false,
			}))
		}),
	)
}
