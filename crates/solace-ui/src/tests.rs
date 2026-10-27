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

#[test]
fn clicking_the_open_community_again_closes_it() {
	let ctx = egui::Context::default();
	install(&ctx).unwrap();
	let mut solace = Solace {
		pane: Pane::Explore,
		..Solace::default()
	};
	let mut state = test_support::demo_state();
	let guild = state.guilds[0].id;
	let _ = state.select(nav::landing(&state, guild).unwrap());
	let size = egui::vec2(1400.0, 900.0);
	// The first community row sits under the "Communities" header in the sidebar.
	let row = egui::pos2(theme::size::RAIL + 140.0, 128.0 + 34.0 + 28.0);
	let mut run = |events: Vec<egui::Event>, state: &mut client_core::State| {
		let input = egui::RawInput {
			screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
			events,
			..Default::default()
		};
		let mut output = ctx.run_ui(input, |ui| {
			let _ = solace.show(ui, state);
		});
		output.textures_delta.clear();
	};
	let click = |pressed| egui::Event::PointerButton {
		pos: row,
		button: egui::PointerButton::Primary,
		pressed,
		modifiers: Default::default(),
	};
	run(vec![egui::Event::PointerMoved(row)], &mut state);
	run(vec![click(true)], &mut state);
	run(vec![click(false)], &mut state);
	assert_eq!(state.selected, None, "the open community closes");
	assert!(nav::selected_guild(&state).is_none());
	// Clicking it again reopens the community.
	run(vec![click(true)], &mut state);
	run(vec![click(false)], &mut state);
	assert_eq!(nav::selected_guild(&state).map(|g| g.id), Some(guild));
}

#[test]
fn home_shows_the_dashboard_without_a_sidebar() {
	let ctx = egui::Context::default();
	install(&ctx).unwrap();
	let mut solace = Solace::default();
	let mut state = test_support::demo_state();
	state.open_home();
	assert!(!nav::direct(&state).is_empty());
	for size in [egui::vec2(1536.0, 1024.0), egui::vec2(900.0, 640.0)] {
		frame(&ctx, size, |ui| {
			let _ = solace.show(ui, &mut state);
		});
	}
	// Opening a direct message from the dashboard moves to the Direct pane and its sidebar.
	let dm = nav::direct(&state)[0].id;
	let _ = state.select(dm);
	frame(&ctx, egui::vec2(1400.0, 900.0), |ui| {
		let _ = solace.show(ui, &mut state);
	});
	assert_eq!(solace.pane, Pane::Direct);
}

/// An open conversation or the dashboard must stop asking for frames once its entrances have
/// finished: idle redraws burn CPU that games and other apps need.
#[test]
fn idle_interface_stops_requesting_frames() {
	let idle_delay = |setup: &dyn Fn(&mut client_core::State, &mut Solace)| {
		let ctx = egui::Context::default();
		install(&ctx).unwrap();
		let mut solace = Solace::default();
		let mut state = test_support::demo_state();
		setup(&mut state, &mut solace);
		let mut delay = std::time::Duration::ZERO;
		for i in 0..240 {
			let input = egui::RawInput {
				screen_rect: Some(Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(1400.0, 900.0),
				)),
				time: Some(f64::from(i) / 60.0),
				focused: true,
				..Default::default()
			};
			let mut output = ctx.run_ui(input, |ui| {
				let _ = solace.show(ui, &mut state);
			});
			output.textures_delta.clear();
			delay = output
				.viewport_output
				.values()
				.map(|v| v.repaint_delay)
				.min()
				.unwrap_or(std::time::Duration::MAX);
		}
		delay
	};
	let second = std::time::Duration::from_secs(1);
	assert!(
		idle_delay(&|state, _| {
			let channel = nav::landing(state, state.guilds[0].id).unwrap();
			let _ = state.select(channel);
			// The history page arrived: nothing is loading any more.
			state.history_pending = false;
		}) >= second,
		"an idle conversation keeps redrawing"
	);
	assert!(
		idle_delay(&|state, _| state.open_home()) >= second,
		"the idle dashboard keeps redrawing"
	);
	// A call where someone is talking redraws when talking starts or stops, not every frame.
	assert!(
		idle_delay(&|state, solace| {
			let dm = nav::direct(state)[0].clone();
			let _ = state.select(dm.id);
			state.history_pending = false;
			let person = |user| client_core::voice::Participant {
				user,
				muted: false,
				deafened: false,
				server_muted: false,
				server_deafened: false,
				video: false,
				streaming: false,
			};
			let me = state.user.as_ref().unwrap().id;
			let peer = dm.recipients[0].id;
			state.voice.active = Some(client_core::voice::Call {
				channel: dm.id,
				guild: None,
				connected_at: None,
				server_muted: false,
				server_deafened: false,
				request: 1,
				phase: client_core::voice::Phase::Connected,
				muted: false,
				deafened: false,
				participants: vec![person(me), person(peer)],
				camera: false,
				watching: None,
				error: None,
			});
			solace.media.voice_speaking = vec![peer];
		}) >= second,
		"a call with someone talking redraws every frame"
	);
}

#[test]
fn focus_mode_toggles_with_the_shortcut_and_leaves_on_escape() {
	let ctx = egui::Context::default();
	install(&ctx).unwrap();
	let mut solace = Solace::default();
	let mut state = test_support::demo_state();
	let _ = state.select(nav::landing(&state, state.guilds[0].id).unwrap());
	let run =
		|events: Vec<egui::Event>, solace: &mut Solace, state: &mut client_core::State| {
			let input = egui::RawInput {
				screen_rect: Some(Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(1400.0, 900.0),
				)),
				events,
				..Default::default()
			};
			let mut output = ctx.run_ui(input, |ui| {
				let _ = solace.show(ui, state);
			});
			output.textures_delta.clear();
		};
	let key = |key, modifiers| egui::Event::Key {
		key,
		physical_key: None,
		pressed: true,
		repeat: false,
		modifiers,
	};
	let shortcut = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
	run(vec![key(egui::Key::F, shortcut)], &mut solace, &mut state);
	assert!(solace.focus, "Ctrl+Shift+F enters focus mode");
	// The conversation fills the window without the rail, sidebar or details.
	run(vec![], &mut solace, &mut state);
	run(
		vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
		&mut solace,
		&mut state,
	);
	assert!(!solace.focus, "Esc leaves focus mode");
	// Esc that closes the thread panel does not also leave focus mode.
	solace.set_focus(true);
	solace.open_thread(state.timeline.display_iter().next().map(|m| m.id));
	run(
		vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
		&mut solace,
		&mut state,
	);
	assert!(solace.focus);
}
