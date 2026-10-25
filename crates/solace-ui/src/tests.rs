use super::*;

fn frame(ctx: &egui::Context, size: egui::Vec2, mut add: impl FnMut(&mut Ui)) {
	let input = egui::RawInput {
		screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
		..Default::default()
	};
	let mut output = ctx.run_ui(input, |ui| add(ui));
	output.textures_delta.clear();
}

#[test]
fn shell_renders_fixture_without_panicking_at_several_sizes() {
	let ctx = egui::Context::default();
	install(&ctx).unwrap();
	let mut solace = Solace::default();
	let mut state = test_support::demo_state();
	let guild = state.guilds[0].id;
	let channel = nav::landing(&state, guild).unwrap();
	let _ = state.select(channel);
	for size in [
		egui::vec2(1536.0, 1024.0),
		egui::vec2(1100.0, 760.0),
		egui::vec2(760.0, 520.0),
	] {
		for _ in 0..3 {
			frame(&ctx, size, |ui| {
				let _ = solace.show(ui, &mut state);
			});
		}
	}
	// Open every overlay once.
	solace.settings = Some(SettingsPage::Voice);
	solace.account_menu = true;
	solace.link = Some("https://example.com".into());
	solace.join = Some("discord.gg/x".into());
	solace.picker = Some((PickerTarget::Composer, egui::pos2(400.0, 700.0)));
	frame(&ctx, egui::vec2(1400.0, 900.0), |ui| {
		let _ = solace.show(ui, &mut state);
	});
	// Direct messages too.
	let dm = nav::direct(&state).first().map(|c| c.id).unwrap();
	let _ = state.select(dm);
	solace.settings = None;
	frame(&ctx, egui::vec2(1400.0, 900.0), |ui| {
		let _ = solace.show(ui, &mut state);
	});
	ctx.set_theme(egui::Theme::Dark);
	frame(&ctx, egui::vec2(1400.0, 900.0), |ui| {
		let _ = solace.show(ui, &mut state);
	});
}

#[test]
fn signin_screens_render() {
	let ctx = egui::Context::default();
	install(&ctx).unwrap();
	let mut solace = Solace::default();
	let mut token = String::new();
	let mut authorized = false;
	frame(&ctx, egui::vec2(1200.0, 800.0), |ui| {
		let accounts = [model::SavedAccount {
			id: Id(5),
			name: "lina".into(),
			display: Some("Lina".into()),
			avatar: None,
			discriminator: 0,
			has_token: true,
		}];
		let _ = signin::show(
			&mut solace,
			ui,
			signin::Props {
				accounts: &accounts,
				status: "Checking",
				busy: false,
				token: &mut token,
				authorized: &mut authorized,
			},
		);
	});
	frame(&ctx, egui::vec2(1200.0, 800.0), |ui| {
		let _ = signin::restoring(&mut solace, ui, "Connecting");
	});
}

#[test]
fn drafts_request_a_save_after_they_change() {
	let mut solace = Solace::default();
	let mut state = test_support::demo_state();
	let channel = nav::landing(&state, state.guilds[0].id).unwrap();
	let _ = state.select(channel);
	solace.sync_draft(&state);
	assert!(solace.take_requests().is_empty());
	state.drafts.insert(channel, "hello".into());
	solace.sync_draft(&state);
	assert!(
		solace.take_requests().is_empty(),
		"saves wait for typing to settle"
	);
	let other = nav::direct(&state).first().map(|c| c.id).unwrap();
	let _ = state.select(other);
	solace.sync_draft(&state);
	assert_eq!(solace.take_requests(), vec![Request::SaveDraft(channel)]);
}
