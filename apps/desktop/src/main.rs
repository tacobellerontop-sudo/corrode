#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Solace desktop: wires the custom GPU-rendered interface (`solace-ui`) to the Discord
//! session, local cache, credential store, media engine and OS integrations.
// Backend glue is kept as it was; Solace's interface drives a subset of it (for example,
// the stored preference kinds and image-share paths have no UI yet), hence `dead_code`.
#[allow(dead_code)]
mod audio;
#[allow(dead_code)]
mod avatars;
mod background;
#[allow(dead_code)]
mod cache;
#[allow(dead_code)]
mod connection;
mod credentials;
#[allow(dead_code)]
mod downloads;
#[cfg_attr(feature = "demo", allow(dead_code))]
mod game_activity;
mod gpu;
mod history;
#[allow(dead_code)]
mod notification_runtime;
#[allow(dead_code)]
mod notification_sounds;
mod preferences;
mod screen;
#[cfg(feature = "demo")]
mod showcase;
#[cfg_attr(feature = "demo", allow(dead_code))]
mod spotify;
#[allow(dead_code)]
mod uploads;
mod video_gpu;
mod voice;
mod watch;

use client_core::{
	Command, Envelope, Event, State,
	auth::{AuthState, Failure, SessionSecret},
};
use eframe::egui;
use history::{
	access_candidates, changes_active_history, confirmed_recovery_channel, hydrate_cache_result,
	recovery_draft, user_action_notice, wants_cached_history,
};
use model::Delivery;
use solace_ui::{Level, Request, Solace};
use std::{sync::Arc, time::Duration};
use zeroize::Zeroizing;

const APP_ID: &str = "app.solace.desktop";

fn main() -> eframe::Result {
	let demo = std::env::args().any(|arg| arg == "--demo");
	if demo && !cfg!(feature = "demo") {
		eprintln!("The offline showcase is not included; rebuild with --features demo");
		std::process::exit(2);
	}
	// GPU preference is read before the first window exists.
	let gpu_preference = if demo {
		model::GpuPreference::default()
	} else {
		local_store::LocalStore::open_default()
			.and_then(|store| store.app_preferences())
			.map(|preferences| preferences.gpu_preference)
			.unwrap_or_default()
	};
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_title("Solace")
			.with_inner_size([1440.0, 920.0])
			.with_min_inner_size([860.0, 560.0])
			.with_app_id(APP_ID)
			.with_icon(app_icon())
			// Solace draws its own title bar and resize edges.
			.with_decorations(false),
		renderer: eframe::Renderer::Wgpu,
		wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
			wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
				eframe::egui_wgpu::WgpuSetupCreateNew {
					instance_descriptor: eframe::wgpu::InstanceDescriptor {
						// Windows defaults to DX12 to avoid Intel Vulkan driver startup crashes;
						// WGPU_BACKEND overrides it everywhere.
						backends: eframe::wgpu::Backends::from_env().unwrap_or(if cfg!(windows) {
							eframe::wgpu::Backends::DX12
						} else {
							eframe::wgpu::Backends::PRIMARY | eframe::wgpu::Backends::GL
						}),
						..eframe::wgpu::InstanceDescriptor::new_without_display_handle_from_env()
					},
					// Only adapters that can present to this window are eligible; the saved
					// preference orders them.
					native_adapter_selector: Some(Arc::new(
						move |adapters: &[eframe::wgpu::Adapter],
						      surface: Option<&eframe::wgpu::Surface<'_>>| {
							gpu::select(gpu_preference, adapters, surface)
						},
					)),
					..eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle()
				},
			),
			surface: eframe::egui_wgpu::SurfaceConfig {
				present_mode: eframe::wgpu::PresentMode::Fifo,
				..eframe::egui_wgpu::SurfaceConfig::LOW_LATENCY
			},
			..Default::default()
		},
		persist_window: true,
		persistence_path: None,
		..Default::default()
	};
	eframe::run_native(
		"Solace",
		options,
		Box::new(move |cc| Ok(Box::new(Desktop::new(cc, demo)?))),
	)
}

/// The window icon: Solace's four-pointed star on a sage tile, rasterized at startup.
fn app_icon() -> egui::IconData {
	const SIZE: usize = 128;
	let mut rgba = vec![0u8; SIZE * SIZE * 4];
	let tile = |x: f32, y: f32| {
		// Rounded square with radius 28.
		let r = 28.0;
		let qx = (x - 64.0).abs() - (64.0 - r);
		let qy = (y - 64.0).abs() - (64.0 - r);
		let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
		(0.5 - outside).clamp(0.0, 1.0)
	};
	let star = |x: f32, y: f32| {
		// |dx|^0.5 + |dy|^0.5 <= R^0.5 gives a star with concave sides.
		let (dx, dy) = ((x - 64.0).abs() / 44.0, (y - 64.0).abs() / 44.0);
		let v = dx.sqrt() + dy.sqrt();
		((1.0 - v) * 22.0).clamp(0.0, 1.0)
	};
	for y in 0..SIZE {
		for x in 0..SIZE {
			let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
			let a = tile(fx, fy);
			let s = star(fx, fy);
			let t = fy / SIZE as f32;
			let bg = [47.0 + 20.0 * t, 106.0 - 10.0 * t, 90.0 - 8.0 * t];
			let px = &mut rgba[(y * SIZE + x) * 4..][..4];
			for i in 0..3 {
				px[i] = (bg[i] * (1.0 - s) + 250.0 * s) as u8;
			}
			px[3] = (a * 255.0) as u8;
		}
	}
	egui::IconData {
		rgba,
		width: SIZE as u32,
		height: SIZE as u32,
	}
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SessionEnd {
	#[default]
	Logout,
	Switch(model::Id),
	Add,
}
impl SessionEnd {
	/// Switching and adding keep this account's cached data and saved login.
	fn forgets(self) -> bool {
		self == Self::Logout
	}
}

struct Desktop {
	runtime: tokio::runtime::Runtime,
	window: Arc<winit::window::Window>,
	state: State,
	solace: Solace,
	login: Option<platform::LoginView>,
	connection: Option<connection::Connection>,
	store: Option<credentials::Store>,
	cache: Option<cache::Cache>,
	cache_pending: usize,
	cache_clears: cache::HistoryClears,
	cache_error: bool,
	cache_status: &'static str,
	avatars: Option<avatars::AvatarWorker>,
	avatar_start_failed: bool,
	avatar_clear_account: Option<model::Id>,
	avatar_cleanup: Option<avatars::Cleanup>,
	voice: voice::Voice,
	uploads: uploads::Uploads,
	downloads: downloads::Downloads,
	audio: audio::Audio,
	notifications: platform::notifications::Notifications,
	notification_runtime: notification_runtime::Runtime,
	appearance: egui::ThemePreference,
	appearance_loaded: bool,
	pending_save: Option<Arc<SessionSecret>>,
	pending_account_save: Option<Arc<SessionSecret>>,
	account_presences: std::collections::BTreeMap<model::Id, model::OwnPresence>,
	presence_load_pending: bool,
	deferred_connect: Option<(SessionSecret, bool)>,
	credential_status: &'static str,
	forgetting: bool,
	switching: Option<model::Id>,
	roster_pending: bool,
	roster_failed: bool,
	roster_generation: Option<u64>,
	draft_restore_pending: bool,
	preferences: preferences::Preferences,
	background: background::Background,
	/// A close was refused once because of unsent work; the next close within this window wins.
	close_warned: Option<std::time::Instant>,
	token_input: Zeroizing<String>,
	authorized: bool,
}

impl Desktop {
	fn new(
		cc: &eframe::CreationContext<'_>,
		demo: bool,
	) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
		solace_ui::install(&cc.egui_ctx)?;
		cc.egui_ctx.set_theme(egui::ThemePreference::Light);
		let runtime = tokio::runtime::Builder::new_multi_thread()
			.worker_threads(2)
			.enable_all()
			.build()?;
		let window = cc
			.winit_window()
			.ok_or("Native window unavailable")?
			.clone();
		let mut store = (!demo).then(|| credentials::Store::start(cc.egui_ctx.clone()));
		let cache = (!demo).then(|| cache::Cache::start(cc.egui_ctx.clone()));
		#[cfg_attr(not(feature = "demo"), allow(unused_mut))]
		let mut state = State::default();
		#[cfg_attr(not(feature = "demo"), allow(unused_mut))]
		let mut solace = Solace::default();
		#[cfg(feature = "demo")]
		if demo {
			state = showcase::state();
			showcase::decorate(&mut solace, &cc.egui_ctx, &state);
			if std::env::args().any(|arg| arg == "--demo-call") {
				showcase::call_scene(&mut state, &mut solace);
			}
			if std::env::args().any(|arg| arg == "--demo-dark") {
				cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
				solace.theme = egui::ThemePreference::Dark;
			}
		}
		let _ = store
			.as_mut()
			.is_some_and(|store| store.load(state.generation, std::time::Instant::now()));
		let mut cache_pending = 0;
		let mut presence_load_pending = false;
		if let Some(cache) = &cache {
			for operation in [
				cache::Operation::LoadAppearance,
				cache::Operation::LoadAccounts,
				cache::Operation::LoadAppPreferences,
			] {
				cache_pending +=
					usize::from(cache.queue(state.generation, model::Id(0), operation));
			}
			presence_load_pending = cache.queue(
				state.generation,
				model::Id(0),
				cache::Operation::LoadAccountPresences,
			);
			cache_pending += usize::from(presence_load_pending);
		}
		let notifications = {
			let wake = cc.egui_ctx.clone();
			platform::notifications::Notifications::new(move || wake.request_repaint())
		};
		// Watched streams are colour-converted on the GPU that draws the window.
		let mut voice = voice::Voice::default();
		if let Some(render) = cc.wgpu_render_state.clone() {
			voice.use_gpu(render);
		}
		Ok(Self {
			runtime,
			window,
			state,
			solace,
			login: None,
			connection: None,
			store,
			cache,
			cache_pending,
			cache_clears: Default::default(),
			cache_error: false,
			cache_status: "",
			avatars: None,
			avatar_start_failed: false,
			avatar_clear_account: None,
			avatar_cleanup: None,
			voice,
			uploads: uploads::Uploads::default(),
			downloads: downloads::Downloads::default(),
			audio: audio::Audio::default(),
			notifications,
			notification_runtime: Default::default(),
			appearance: egui::ThemePreference::Light,
			appearance_loaded: demo,
			pending_save: None,
			pending_account_save: None,
			account_presences: Default::default(),
			presence_load_pending,
			deferred_connect: None,
			credential_status: "",
			forgetting: false,
			switching: None,
			roster_pending: false,
			roster_failed: false,
			roster_generation: None,
			draft_restore_pending: false,
			preferences: Default::default(),
			background: Default::default(),
			close_warned: None,
			token_input: Zeroizing::new(String::new()),
			authorized: false,
		})
	}

	fn connect(&mut self, secret: SessionSecret, save: bool, ctx: &egui::Context) {
		if self.presence_load_pending {
			self.deferred_connect = Some((secret, save));
			self.credential_status = "Connecting to Discord...";
			return;
		}
		if let Some(store) = &mut self.store {
			store.cancel_load();
		}
		self.credential_status = if save {
			"Your login will be saved once Discord connects"
		} else {
			"Connecting with the supplied session"
		};
		self.voice.stop();
		self.state
			.disconnect_voice("Discord login session changed; start a new call");
		self.uploads.cancel();
		self.login = None;
		self.connection = None;
		if let Some(worker) = self.avatars.take() {
			self.avatar_cleanup = Some(worker.shutdown());
		}
		self.avatar_start_failed = false;
		self.solace.images.clear();
		self.state.generation += 1;
		self.draft_restore_pending = false;
		self.state.auth = AuthState::Authenticating;
		self.state.status = "Connecting to Discord...";
		let secret = Arc::new(secret);
		self.pending_save = save.then(|| secret.clone());
		self.pending_account_save = Some(secret.clone());
		self.connection = Some(connection::Connection::start(
			self.runtime.handle(),
			secret,
			self.state.generation,
			self.state.user.as_ref().map(|u| u.id),
			self.account_presences.clone(),
			ctx.clone(),
		));
	}

	fn flush_deferred_connect(&mut self, ctx: &egui::Context) {
		if self.presence_load_pending {
			return;
		}
		if let Some((secret, save)) = self.deferred_connect.take() {
			self.connect(secret, save, ctx);
		}
	}

	/// Ends the current session. Only logging out removes this account's local data and saved
	/// login; switching and adding keep both so the account stays available.
	fn end_session(&mut self, ctx: &egui::Context, intent: SessionEnd) {
		self.notification_runtime.clear(&self.window);
		self.notifications.clear();
		self.uploads.cancel();
		self.downloads.cancel();
		self.audio.stop();
		if let Some(store) = &mut self.store {
			store.cancel_load();
		}
		self.voice.stop();
		let was_demo = self.state.demo;
		self.clear_avatars(ctx);
		self.login = None;
		self.connection = None;
		self.pending_save = None;
		self.pending_account_save = None;
		let old_account = self.state.user.as_ref().filter(|_| !was_demo).map(|u| u.id);
		self.switching = None;
		self.roster_pending = false;
		self.state.logout();
		if intent.forgets()
			&& let (Some(cache), Some(account)) = (&self.cache, old_account)
		{
			if cache.queue(self.state.generation, account, cache::Operation::Forget) {
				self.cache_pending += 1;
			} else {
				self.cache_error = true;
				self.cache_status = "Could not queue local account data removal";
			}
		}
		self.solace.clear_session();
		self.solace.media.reset();
		// Sign-in screens keep the window's own edges: full screen ends with the session.
		if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
			ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
		}
		if intent.forgets() {
			self.solace
				.accounts
				.retain(|saved| Some(saved.id) != old_account);
		}
		ctx.memory_mut(|m| *m = egui::Memory::default());
		ctx.set_theme(self.appearance);
		self.token_input = Zeroizing::new(String::new());
		if !was_demo
			&& intent.forgets()
			&& let Some(store) = &self.store
		{
			self.forgetting = store
				.send
				.try_send((
					self.state.generation,
					credentials::Request::NONE,
					credentials::Operation::Forget,
				))
				.is_ok();
			if let Some(account) = old_account {
				let _ = store.send.try_send((
					self.state.generation,
					credentials::Request::NONE,
					credentials::Operation::ForgetAccount(account),
				));
			}
			self.credential_status = if self.forgetting {
				"Removing saved login..."
			} else {
				"Credential queue unavailable; saved login may remain"
			};
		} else if !was_demo {
			self.credential_status = "Signed out of this account; its saved login is kept";
		}
		if let SessionEnd::Switch(account) = intent {
			self.begin_switch(account);
		}
	}

	/// Reads the saved token of an account already signed in on this device.
	fn begin_switch(&mut self, account: model::Id) {
		if self.state.demo {
			return;
		}
		let started = self.store.as_mut().is_some_and(|store| {
			store.load_account(self.state.generation, account, std::time::Instant::now())
		});
		self.switching = started.then_some(account);
		self.credential_status = if started {
			"Reading the saved login for that account..."
		} else {
			"Credential lookup unavailable; sign in with Discord again"
		};
	}

	/// Removes one saved account: its token, roster entry and cached data.
	fn forget_saved_account(&mut self, ctx: &egui::Context, account: model::Id) {
		if self
			.state
			.user
			.as_ref()
			.is_some_and(|user| user.id == account)
		{
			self.end_session(ctx, SessionEnd::Logout);
			return;
		}
		self.solace.accounts.retain(|saved| saved.id != account);
		if let Some(store) = &self.store {
			let _ = store.send.try_send((
				self.state.generation,
				credentials::Request::NONE,
				credentials::Operation::ForgetAccount(account),
			));
		}
		if self.queue_cache_for(account, cache::Operation::Forget) {
			self.credential_status = "Saved account removed from this device";
		} else {
			self.cache_error = true;
			self.cache_status = "Could not queue local account data removal";
		}
	}

	/// Keeps the saved-account entry for the signed-in account current.
	fn sync_account_roster(&mut self) {
		if self.state.demo || self.roster_pending || self.roster_failed {
			return;
		}
		if self.state.auth != AuthState::Authenticated {
			return;
		}
		let Some(user) = &self.state.user else { return };
		let display = self
			.state
			.own_profile
			.data
			.as_ref()
			.and_then(|profile| profile.global_name.as_deref());
		let recorded = self.roster_generation == Some(self.state.generation);
		if recorded
			&& self.solace.accounts.iter().any(|saved| {
				saved.id == user.id
					&& saved.name == user.name
					&& saved.avatar == user.avatar
					&& saved.discriminator == user.discriminator
					&& saved.display.as_deref() == display
			}) {
			return;
		}
		let account = model::SavedAccount {
			id: user.id,
			name: user.name.clone(),
			display: display.map(str::to_owned),
			avatar: user.avatar.clone(),
			discriminator: user.discriminator,
			has_token: false,
		};
		if !account.is_valid() {
			return;
		}
		self.roster_pending =
			self.queue_cache_for(model::Id(0), cache::Operation::SaveAccount(account));
		self.roster_failed = !self.roster_pending;
		if self.roster_pending {
			self.roster_generation = Some(self.state.generation);
		}
	}

	fn queue_cache(&mut self, operation: cache::Operation) -> bool {
		if let Some(user) = &self.state.user {
			if matches!(operation, cache::Operation::ClearHistory) {
				self.request_history_clear(user.id);
				return true;
			}
			self.queue_cache_for(user.id, operation)
		} else {
			false
		}
	}

	fn queue_cache_for(&mut self, account: model::Id, operation: cache::Operation) -> bool {
		if self.state.demo {
			return false;
		}
		if let Some(cache) = &self.cache {
			if matches!(
				operation,
				cache::Operation::LoadChannel { .. }
					| cache::Operation::SaveChannel { .. }
					| cache::Operation::SaveChanges { .. }
			) && !cache.history.allows(cache.history.epoch())
			{
				return false;
			}
			if cache.queue(self.state.generation, account, operation) {
				self.cache_pending += 1;
				return true;
			}
			self.cache_error = true;
			self.cache_status = "Local storage queue full; some changes are not saved";
		}
		false
	}

	fn request_history_clear(&mut self, account: model::Id) {
		if self.state.demo {
			return;
		}
		let Some(cache) = &self.cache else {
			return;
		};
		cache.history.invalidate();
		cache.history.block();
		if !self.cache_clears.request(account) {
			cache.history.fail();
			self.cache_error = true;
			self.cache_status = "Cache cleanup backlog exceeded; history cache disabled until restart; deleted messages may remain on disk";
			return;
		}
		self.retry_history_clears();
	}

	fn retry_history_clears(&mut self) {
		let Some(cache) = &self.cache else {
			return;
		};
		for _ in 0..16 {
			let Some(account) = self.cache_clears.next() else {
				break;
			};
			if !cache.queue(
				self.state.generation,
				account,
				cache::Operation::ClearHistory,
			) {
				break;
			}
			self.cache_clears.queued(account);
			self.cache_pending += 1;
		}
	}

	fn delete_cached_messages(&mut self, event: &Event) {
		if self.state.demo {
			return;
		}
		let Some(account) = self.state.user.as_ref().map(|user| user.id) else {
			return;
		};
		let (channel, ids) = match event {
			Event::Delete { channel, id } => (*channel, vec![*id]),
			Event::DeleteBulk { channel, ids } if !ids.is_empty() && ids.len() <= 100 => {
				(*channel, ids.clone())
			}
			Event::DeleteBulk { ids, .. } if ids.len() > 100 => {
				self.request_history_clear(account);
				return;
			}
			_ => return,
		};
		self.delete_cached_ids(channel, ids);
	}

	fn delete_cached_ids(&mut self, channel: model::Id, ids: Vec<model::Id>) {
		if self.state.demo || ids.is_empty() {
			return;
		}
		let Some(account) = self.state.user.as_ref().map(|user| user.id) else {
			return;
		};
		let Some(cache) = &self.cache else {
			return;
		};
		if cache.delete_messages(self.state.generation, account, channel, ids) {
			self.cache_pending += 1;
		} else {
			self.request_history_clear(account);
		}
	}

	fn save_appearance(&mut self, preference: egui::ThemePreference) {
		self.appearance = preference;
		let saved = match preference {
			egui::ThemePreference::System => local_store::Appearance::System,
			egui::ThemePreference::Light => local_store::Appearance::Light,
			egui::ThemePreference::Dark => local_store::Appearance::Dark,
		};
		self.queue_cache_for(model::Id(0), cache::Operation::SaveAppearance(saved));
	}

	/// Dispatch one command to the live transport (or the offline showcase).
	fn command(&mut self, command: Command) {
		if let Command::Send { channel, nonce, .. } = &command
			&& self
				.state
				.pending
				.iter()
				.any(|p| p.nonce == *nonce && !p.attachments.is_empty())
		{
			let (channel, nonce) = (*channel, nonce.clone());
			let available = !self.state.demo
				&& self.state.can_attach(channel)
				&& self.state.auth == AuthState::Authenticated
				&& self.state.gateway_connected
				&& self.state.freshness == model::Freshness::Fresh
				&& self.state.selected == Some(channel)
				&& self.connection.is_some();
			if available
				&& let Some(source) = self.uploads.take_source(self.state.generation, channel)
			{
				let (progress, receive) =
					tokio::sync::watch::channel(discord_api::upload::Status::Preparing);
				let (cancel, _) = tokio::sync::watch::channel(false);
				if self.uploads.begin_upload(receive, cancel.clone()).is_ok() {
					let request = uploads::UploadRequest {
						command,
						source,
						progress,
						cancel,
					};
					if let Err(error) = self.connection.as_ref().unwrap().uploads.try_send(request)
					{
						let request = error.into_inner();
						request
							.progress
							.send_replace(discord_api::upload::Status::Failed(
								"Upload queue full; reselect the file",
							));
						self.state.command_rejected(request.command);
					}
					return;
				}
			}
			self.state.apply(Envelope {
				generation: self.state.generation,
				event: Event::SendResult {
					nonce,
					result: Err(Failure::ProtocolAt(
						"File not sent; reconnect and reselect the attachment",
					)),
				},
			});
			return;
		}
		if let Command::History {
			channel,
			before: None,
			after: None,
			..
		} = &command
			&& self.state.selected == Some(*channel)
			&& self.state.can_call(*channel)
			&& self
				.state
				.channels
				.iter()
				.any(|c| c.id == *channel && c.guild.is_none())
		{
			self.command(Command::Voice(client_core::voice::Command::Sync {
				channel: *channel,
			}));
		}
		if let Command::Voice(control) = &command {
			if self.state.demo {
				self.solace
					.toasts
					.push(Level::Info, "Voice is unavailable in the offline showcase");
				return;
			}
			if let client_core::voice::Command::Join {
				channel,
				request,
				ring,
				..
			} = control && let Err(message) = self.voice.begin(&self.state, *ring)
			{
				self.state.apply_voice(client_core::voice::Event::Failed {
					channel: *channel,
					request: *request,
					message,
				});
				return;
			}
			if matches!(control, client_core::voice::Command::Leave { .. }) {
				self.voice.stop();
			}
		}
		if let Command::History {
			channel,
			before: None,
			after: None,
			request,
		} = &command
			&& wants_cached_history(&self.state, *channel, *request)
		{
			self.queue_cache(cache::Operation::LoadChannel {
				channel: *channel,
				request: *request,
			});
		}
		#[cfg(feature = "demo")]
		if self.state.demo {
			self.showcase_command(command);
			return;
		}
		if let Some(connection) = &self.connection {
			if let Err(error) = connection.commands.try_send(command) {
				self.state.command_rejected(error.into_inner());
			}
		} else {
			self.state.command_rejected(command);
		}
	}

	/// The offline showcase answers a few commands locally so it feels alive.
	#[cfg(feature = "demo")]
	fn showcase_command(&mut self, command: Command) {
		let event = match command {
			Command::Send {
				channel,
				content,
				nonce,
				reply,
				..
			} => {
				let id = self
					.state
					.timeline
					.row_ids()
					.next_back()
					.map_or(model::Id(1 << 40), |id| model::Id(id.0 + 1));
				let mut message = test_support::message(1, channel);
				message.id = id;
				message.author = self.state.user.clone().unwrap();
				message.content = content;
				message.mentions.clear();
				message.reactions = Some(vec![]);
				message.attachments.clear();
				message.nonce = Some(nonce.clone());
				message.reply_to = reply.map(client_core::Reply::target);
				Event::SendResult {
					nonce,
					result: Ok(message),
				}
			}
			Command::Reactions(client_core::reactions::Command::Set {
				channel,
				message,
				emoji,
				add,
				request,
			}) => {
				let mut reactions = self
					.state
					.timeline
					.get(message)
					.and_then(|m| m.reactions.clone())
					.unwrap_or_default();
				if let Some(r) = reactions.iter_mut().find(|r| r.emoji.same(&emoji)) {
					if r.me != add {
						r.count = if add {
							r.count + 1
						} else {
							r.count.saturating_sub(1)
						};
						r.me = add;
					}
				} else if add {
					reactions.push(model::Reaction {
						emoji,
						count: 1,
						me: true,
						me_burst: false,
					});
				}
				reactions.retain(|r| r.count > 0);
				self.state.reactions.reset();
				let _ = self.state.timeline.set_reactions(message, Some(reactions));
				Event::Reactions(client_core::reactions::Event::Written {
					channel,
					message,
					request,
					result: Ok(()),
				})
			}
			Command::Edit {
				request,
				channel,
				message,
				content,
			} => {
				let result = self
					.state
					.timeline
					.get(message)
					.cloned()
					.ok_or(Failure::Protocol)
					.map(|mut updated| {
						updated.content = content;
						updated.edited = true;
						updated
					});
				Event::Edited {
					request,
					channel,
					message,
					result,
				}
			}
			Command::Delete { channel, message } => Event::Delete {
				channel,
				id: message,
			},
			Command::MarkRead {
				channel,
				message,
				request,
				..
			} => Event::ReadState(client_core::read_state::Event::Result {
				channel,
				message,
				request,
				result: Ok(()),
			}),
			Command::History {
				channel, request, ..
			} if self.state.selected == Some(channel) => Event::History {
				channel,
				request,
				older: false,
				messages: vec![],
			},
			_ => return,
		};
		self.state.apply(Envelope {
			generation: self.state.generation,
			event,
		});
	}

	fn clear_avatars(&mut self, ctx: &egui::Context) {
		self.avatar_start_failed = false;
		if self.avatar_cleanup.is_some() && !self.state.demo {
			self.avatar_clear_account = self.state.user.as_ref().map(|user| user.id);
		}
		if self.avatars.is_none()
			&& self.avatar_cleanup.is_none()
			&& !self.state.demo
			&& let Some(user) = &self.state.user
		{
			match avatars::AvatarWorker::start(&self.runtime, user.id, ctx.clone()) {
				Ok(worker) => self.avatars = Some(worker),
				Err(error) => {
					self.cache_error = true;
					self.cache_status = error;
				}
			}
		}
		if let Some(worker) = self.avatars.take() {
			self.avatar_cleanup = Some(worker.shutdown_and_clear());
		}
		self.solace.images.clear();
	}

	fn poll_avatars(&mut self, ctx: &egui::Context) {
		if let Some(cleanup) = &self.avatar_cleanup {
			match cleanup.try_recv() {
				Ok(result) => {
					self.avatar_cleanup = None;
					if let Err(error) = result {
						self.cache_error = true;
						self.cache_status = error;
					}
				}
				Err(std::sync::mpsc::TryRecvError::Disconnected) => {
					self.avatar_cleanup = None;
					self.cache_error = true;
					self.cache_status =
						"Image cache cleanup failed; cached pictures may remain on disk";
				}
				Err(std::sync::mpsc::TryRecvError::Empty) => {}
			}
		}
		if self.avatar_cleanup.is_none()
			&& let Some(account) = self.avatar_clear_account.take()
		{
			match avatars::AvatarWorker::start(&self.runtime, account, ctx.clone()) {
				Ok(worker) => self.avatar_cleanup = Some(worker.shutdown_and_clear()),
				Err(error) => {
					self.cache_error = true;
					self.cache_status = error;
				}
			}
		}
		if self.state.demo || self.state.auth != AuthState::Authenticated {
			if let Some(worker) = self.avatars.take() {
				self.avatar_cleanup = Some(worker.shutdown());
			}
			// Requests made while signed out (saved-account avatars) stay queued.
			return;
		}
		if !self.avatar_start_failed
			&& self.avatars.is_none()
			&& self.avatar_cleanup.is_none()
			&& let Some(user) = &self.state.user
		{
			match avatars::AvatarWorker::start(&self.runtime, user.id, ctx.clone()) {
				Ok(worker) => self.avatars = Some(worker),
				Err(error) => {
					self.avatar_start_failed = true;
					self.cache_error = true;
					self.cache_status = error;
				}
			}
		}
		if let Some(worker) = &mut self.avatars {
			for _ in 0..32 {
				let Some(result) = worker.poll() else {
					break;
				};
				if let Some(error) = result.error {
					self.cache_error = true;
					self.cache_status = error;
				}
				self.solace.images.accept(ctx, result.key, result.image);
			}
		}
	}

	fn request_images(&mut self, ctx: &egui::Context) {
		let keys = self.solace.images.take_requests();
		if self.state.demo {
			return;
		}
		for key in keys {
			if !self
				.avatars
				.as_ref()
				.is_some_and(|worker| worker.request(key.clone()))
			{
				self.solace.images.accept(ctx, key, None);
			}
		}
	}

	/// Saved login restore in progress, so launch shows progress instead of sign-in.
	fn restoring(&self) -> Option<&'static str> {
		if self.state.demo
			|| self.login.is_some()
			|| self.forgetting
			|| matches!(
				self.state.auth,
				AuthState::Failed | AuthState::Expired | AuthState::Challenged
			) {
			return None;
		}
		if self
			.store
			.as_ref()
			.is_some_and(|store| store.remaining(std::time::Instant::now()).is_some())
		{
			return Some("Checking your saved login");
		}
		self.connection.is_some().then_some("Connecting to Discord")
	}

	fn open_login(&mut self, ctx: &egui::Context) {
		if let Some(store) = &mut self.store {
			store.cancel_load();
		}
		let wake = ctx.clone();
		match platform::LoginView::open(self.window.clone(), move || wake.request_repaint()) {
			Ok(login) => {
				self.login = Some(login);
				self.state.auth = AuthState::Authenticating;
				self.state.status = "Waiting for Discord login";
			}
			Err(_) => {
				self.state.auth = AuthState::Failed;
				self.state.status = "The Discord login window is unavailable on this system; sign in with a token instead";
			}
		}
	}

	fn handle_requests(&mut self, ctx: &egui::Context) {
		for request in self.solace.take_requests() {
			match request {
				Request::Logout => self.end_session(ctx, SessionEnd::Logout),
				Request::SwitchAccount(account) => {
					if self
						.state
						.user
						.as_ref()
						.is_none_or(|user| user.id != account)
					{
						self.end_session(ctx, SessionEnd::Switch(account));
					}
				}
				Request::AddAccount => self.end_session(ctx, SessionEnd::Add),
				Request::Attach => {
					if let Some(channel) = self.state.selected
						&& self.state.can_attach(channel)
						&& let Err(error) = self.uploads.start_choose(
							self.state.generation,
							channel,
							self.runtime.handle(),
							ctx,
							self.window.clone(),
						) {
						self.solace.toasts.push(Level::Error, error);
					}
				}
				Request::RemoveAttachment(index) => self.uploads.remove_at(index),
				Request::CancelUpload => self.uploads.cancel(),
				Request::OpenLink(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
				Request::SaveDraft(channel) => {
					let content = recovery_draft(&self.state, channel);
					self.queue_cache(cache::Operation::SaveDraft { channel, content });
				}
				Request::Theme(preference) => self.save_appearance(preference),
				Request::Download(attachment) => {
					if self.state.demo {
						self.solace.toasts.push(
							Level::Info,
							"Downloads are unavailable in the offline showcase",
						);
					} else if let Err(error) = self.downloads.start(
						attachment,
						self.runtime.handle(),
						ctx,
						self.window.clone(),
					) {
						self.solace.toasts.push(Level::Error, error);
					}
				}
				Request::PlayAudio(attachment) => {
					let id = attachment.id;
					self.solace.audio = solace_ui::AudioView {
						active: Some(id),
						loading: true,
						..Default::default()
					};
					if let Err(error) =
						self.audio
							.start(attachment, self.runtime.handle(), ctx, self.state.demo)
					{
						self.solace.audio.error = Some(error);
					}
				}
				Request::PauseAudio(pause) => self.audio.pause(pause),
				Request::SeekAudio(seconds) => {
					self.audio.seek(Duration::from_secs_f32(seconds.max(0.0)))
				}
				Request::ClearCache => {
					self.state.clear_cached_history();
					self.clear_avatars(ctx);
					self.queue_cache(cache::Operation::ClearHistory);
				}
			}
		}
	}

	fn poll(&mut self, ctx: &egui::Context) {
		self.poll_cache(ctx);
		self.retry_history_clears();
		self.poll_credentials(ctx);
		self.poll_connection();
		if let Some(login) = &self.login {
			login.pump();
			if let Some(secret) = login.token() {
				self.connect(secret, true, ctx);
			} else if login.expired() {
				let crashed = login.crashed();
				self.login = None;
				self.state.auth = AuthState::Challenged;
				self.state.status = if crashed {
					"The login window stopped unexpectedly; no session was accepted"
				} else {
					"Login timed out; no session was accepted"
				};
			}
		}
		if let Some(connection) = &self.connection {
			connection.set_typing_channel(self.state.typing_scope());
		}
		if !self.state.demo
			&& let Some(command) = self.state.next_reaction_read()
		{
			self.command(command);
		}
		if self.login.is_some() {
			ctx.request_repaint_after(Duration::from_secs(1));
		}
	}

	fn poll_cache(&mut self, ctx: &egui::Context) {
		let mut cached = Vec::new();
		let mut presence_cache_stopped = false;
		if let Some(cache) = &self.cache {
			for _ in 0..16 {
				match cache.receive.try_recv() {
					Ok(value) => cached.push(value),
					Err(std::sync::mpsc::TryRecvError::Disconnected) => {
						presence_cache_stopped = self.presence_load_pending;
						break;
					}
					Err(std::sync::mpsc::TryRecvError::Empty) => break,
				}
			}
		}
		if presence_cache_stopped {
			self.presence_load_pending = false;
			self.flush_deferred_connect(ctx);
		}
		for (generation, outcome, _reservation) in cached {
			self.cache_pending = self.cache_pending.saturating_sub(1);
			match outcome {
				cache::Outcome::Appearance(appearance, _) => {
					if !self.appearance_loaded {
						self.appearance_loaded = true;
						self.appearance = match appearance {
							local_store::Appearance::System => egui::ThemePreference::System,
							local_store::Appearance::Light => egui::ThemePreference::Light,
							local_store::Appearance::Dark => egui::ThemePreference::Dark,
						};
						self.solace.theme = self.appearance;
						ctx.set_theme(self.appearance);
					}
				}
				cache::Outcome::Failed {
					message,
					draft_restore,
					history_cleanup,
					..
				} => {
					if history_cleanup && let Some(cache) = &self.cache {
						self.cache_clears.acknowledge(&cache.history);
					}
					self.cache_error = true;
					self.cache_status = message;
					if draft_restore && generation == self.state.generation {
						self.draft_restore_pending = false;
						self.state.drafts.retain(|_, content| !content.is_empty());
					}
				}
				cache::Outcome::Accounts { roster, pruned } => {
					self.roster_pending = false;
					for account in &pruned {
						if let Some(store) = &self.store {
							let _ = store.send.try_send((
								self.state.generation,
								credentials::Request::NONE,
								credentials::Operation::ForgetAccount(*account),
							));
						}
						self.queue_cache_for(*account, cache::Operation::Forget);
					}
					match roster {
						Ok(accounts) => self.solace.accounts = accounts,
						Err(_) => {
							self.roster_failed = true;
							self.cache_error = true;
							self.cache_status = "Could not read or update the saved account list";
						}
					}
				}
				cache::Outcome::AccountPresences(result) => {
					self.presence_load_pending = false;
					if let Ok(rows) = result {
						self.account_presences = rows;
					}
					self.flush_deferred_connect(ctx);
				}
				cache::Outcome::HistoryCleared => {
					if let Some(cache) = &self.cache
						&& self.cache_clears.acknowledge(&cache.history)
						&& !self.cache_error
					{
						self.cache_status = "Cached history cleared; saved drafts preserved";
					}
				}
				cache::Outcome::Drafts(drafts) if generation == self.state.generation => {
					for (channel, content) in drafts {
						if !self.state.pending.iter().any(|p| p.channel == channel) {
							self.state.drafts.entry(channel).or_insert(content);
						}
					}
					self.draft_restore_pending = false;
					self.state.drafts.retain(|_, content| !content.is_empty());
				}
				outcome @ cache::Outcome::Channel { .. } if generation == self.state.generation => {
					if let Some(cache) = &self.cache {
						hydrate_cache_result(&mut self.state, &cache.history, outcome);
					}
				}
				cache::Outcome::GifFavorites(favorites) if generation == self.state.generation => {
					self.state.restore_gif_favorites(favorites);
				}
				cache::Outcome::AppPreferences(Ok(loaded)) => {
					self.preferences.loaded(*loaded, &mut self.solace.media);
				}
				// Unreadable preferences are left alone: other settings live in the same record.
				cache::Outcome::AppPreferences(Err(_)) => {
					self.cache_status = "Could not read voice settings; changes last this session";
				}
				cache::Outcome::AppPreferencesSaved(Err(_)) => {
					self.cache_error = true;
					self.cache_status = "Could not save voice settings; changes last this session";
				}
				// Preferences this interface does not expose, saves and stale results.
				_ => {}
			}
		}
		self.solace.storage_status = self.cache_status;
	}

	fn poll_credentials(&mut self, ctx: &egui::Context) {
		let mut results = Vec::new();
		if let Some(store) = &mut self.store {
			for _ in 0..4 {
				match store.poll(std::time::Instant::now()) {
					Some(result) => results.push(result),
					None => break,
				}
			}
			if let Some(remaining) = store.remaining(std::time::Instant::now()) {
				ctx.request_repaint_after(remaining);
			}
		}
		for (generation, outcome) in results {
			if generation != self.state.generation {
				continue;
			}
			match outcome {
				credentials::Outcome::Loaded(result) => {
					let switching = self.switching.take();
					if let Ok(Some(secret)) = result {
						self.credential_status = "";
						self.connect(secret, switching.is_some(), ctx);
					} else {
						self.credential_status = credentials::loaded_status(&result);
						if let Some(account) = switching {
							self.queue_cache_for(
								model::Id(0),
								cache::Operation::SetAccountToken {
									account,
									has_token: false,
								},
							);
							self.credential_status = "That account's saved login is missing on this device. Sign in again to refresh it.";
						}
					}
				}
				credentials::Outcome::AccountSaved(account, result) => {
					if result.is_ok() {
						self.queue_cache_for(
							model::Id(0),
							cache::Operation::SetAccountToken {
								account,
								has_token: true,
							},
						);
					}
				}
				credentials::Outcome::Saved(Ok(())) => {
					self.credential_status = "Login saved in the OS credential store"
				}
				credentials::Outcome::Saved(Err(_)) => {
					self.credential_status =
						"Could not save login; this session will not restore automatically"
				}
				credentials::Outcome::AccountForgotten(result) => {
					if result.is_err() {
						self.solace.toasts.push(
							Level::Error,
							"Could not remove that account's saved login from the OS credential store",
						);
					}
				}
				credentials::Outcome::Forgotten(result) => {
					self.forgetting = false;
					self.credential_status = if result.is_ok() {
						"Saved login removed"
					} else {
						"Could not remove the saved login; remove the Solace entry in your OS credential manager"
					};
				}
			}
		}
	}

	fn poll_connection(&mut self) {
		let mut events = Vec::new();
		let mut terminal = None;
		if let Some(connection) = &mut self.connection {
			for _ in 0..client_core::EVENT_SLOTS {
				match connection.events.try_recv() {
					Ok(event) => events.push(event),
					Err(_) => break,
				}
			}
			// Reliable events first, then typing; apply typing before them so queued
			// messages retire older indicators.
			let reliable_count = events.len();
			for _ in 0..8 {
				match connection.typing.try_recv() {
					Ok(event) => events.push(event),
					Err(_) => break,
				}
			}
			let typing_count = events.len() - reliable_count;
			events.rotate_right(typing_count);
			terminal = *connection.terminal.borrow();
		}
		let mut persist_timeline = false;
		let mut full_window = false;
		let mut changed_messages = std::collections::BTreeSet::new();
		for mut event in events {
			if event.generation != self.state.generation {
				continue;
			}
			let notice = user_action_notice(&event.event);
			let notice_was_pending = notice.is_some() && self.state.user_action_pending();
			self.delete_cached_messages(&event.event);
			let voice_failure = self.voice.observe(&self.state, &mut event.event);
			let ready = event.event.ready_navigation().is_some();
			let resumed = matches!(event.event, Event::Resumed);
			let confirmed_channel = confirmed_recovery_channel(&self.state, &event.event);
			let mut removed_channels = access_candidates(&self.state, &event.event);
			removed_channels.retain(|id| self.state.can_read_history(*id));
			let invalidate = matches!(
				event.event,
				Event::Resync | Event::PermissionsChanged | Event::Unavailable(_)
			) || matches!(&event.event, Event::RecipientRemoved { user, .. } if self.state.user.as_ref().is_some_and(|owner| owner.id == *user))
				|| matches!(&event.event, Event::HistoryFailed { channel, request, failure: Failure::Forbidden }
					if self.state.selected == Some(*channel) && self.state.request == *request && self.state.history_pending);
			let history_changed = changes_active_history(&self.state, &event.event);
			if history_changed {
				match &event.event {
					Event::Message(message)
					| Event::SendResult {
						result: Ok(message),
						..
					} => {
						changed_messages.insert(message.id);
					}
					Event::Edited { message, .. } => {
						changed_messages.insert(*message);
					}
					Event::Patch(patch) => {
						changed_messages.insert(patch.id);
					}
					_ => full_window = true,
				}
			}
			if invalidate
				|| event.event.changes_access()
				|| matches!(
					&event.event,
					Event::NotificationPreferences(_)
						| Event::Disconnected
						| Event::ReadState(client_core::read_state::Event::Ack { .. })
				) {
				self.notifications.dismiss();
			}
			self.state.apply(event);
			if notice_was_pending
				&& !self.state.user_action_pending()
				&& let Some((level, text)) = notice
			{
				self.solace.toasts.push(level, text);
			}
			let deleted_replies = self.state.take_reply_deletions();
			if let Some(&(channel, _)) = deleted_replies.first() {
				full_window = true;
				let ids: Vec<_> = deleted_replies.into_iter().map(|(_, id)| id).collect();
				self.delete_cached_ids(channel, ids);
			}
			removed_channels.retain(|id| !self.state.can_read_history(*id));
			if let Some(error) = voice_failure
				&& let Some(command) = self.voice.fail(&mut self.state, error)
			{
				self.command(command);
			}
			if invalidate || !removed_channels.is_empty() {
				self.queue_cache(cache::Operation::ClearHistory);
			}
			persist_timeline |= history_changed;
			if let Some(channel) = confirmed_channel {
				let content = recovery_draft(&self.state, channel);
				self.queue_cache(cache::Operation::SaveDraft { channel, content });
			}
			if ready && self.state.auth == AuthState::Authenticated {
				if !self.draft_restore_pending {
					self.draft_restore_pending = self.queue_cache(cache::Operation::LoadDrafts);
				}
				self.save_credentials();
			}
			if (ready || resumed)
				&& self.state.auth == AuthState::Authenticated
				&& self.state.selected.is_some_and(|selected| {
					self.state
						.channels
						.iter()
						.any(|channel| channel.id == selected && channel.supports_text())
				}) {
				let command = self.state.history(None);
				self.command(command);
			}
		}
		if persist_timeline
			&& self.state.freshness == model::Freshness::Fresh
			&& let Some(channel) = self.state.selected
			&& self.state.can_read_history(channel)
		{
			let messages = self
				.state
				.timeline
				.iter()
				.filter(|m| full_window || changed_messages.contains(&m.id))
				.cloned()
				.collect();
			let operation = if full_window {
				cache::Operation::SaveChannel { channel, messages }
			} else {
				cache::Operation::SaveChanges {
					channel,
					messages,
					retained: self.state.timeline.iter().map(|m| m.id).collect(),
				}
			};
			self.queue_cache(operation);
		}
		if let Some(failure) = terminal {
			self.connection = None;
			self.pending_save = None;
			self.pending_account_save = None;
			self.state.apply(Envelope {
				generation: self.state.generation,
				event: Event::Failure(failure),
			});
			if !matches!(self.state.auth, AuthState::Expired | AuthState::Challenged) {
				self.state.auth = AuthState::Failed;
			}
			for pending in &mut self.state.pending {
				if pending.delivery == Delivery::Sending {
					pending.delivery = Delivery::Ambiguous;
				}
			}
			if failure == Failure::Expired
				&& let Some(store) = &self.store
			{
				let _ = store.send.try_send((
					self.state.generation,
					credentials::Request::NONE,
					credentials::Operation::Forget,
				));
			}
		}
	}

	/// After READY: persist the launch-restore entry and this account's own entry.
	fn save_credentials(&mut self) {
		let Some(store) = &self.store else {
			return;
		};
		let mut queued = true;
		let fresh_token = self.pending_save.is_some();
		if let Some(secret) = self.pending_save.take() {
			queued &= store
				.send
				.try_send((
					self.state.generation,
					credentials::Request::NONE,
					credentials::Operation::Save(secret),
				))
				.is_ok();
		}
		if let Some(secret) = self.pending_account_save.take()
			&& let Some(account) = self.state.user.as_ref().map(|user| user.id)
			&& (fresh_token
				|| !self
					.solace
					.accounts
					.iter()
					.any(|saved| saved.id == account && saved.has_token))
		{
			queued &= store
				.send
				.try_send((
					self.state.generation,
					credentials::Request::NONE,
					credentials::Operation::SaveAccount(account, secret),
				))
				.is_ok();
		}
		if !queued {
			self.credential_status = "Could not queue saved login; session only";
		}
	}

	/// Reflect attachment playback and download progress in the interface.
	fn poll_media(&mut self, ctx: &egui::Context) {
		let hidden = ctx.input(|i| i.viewport().minimized == Some(true));
		if self.state.user.is_none() || hidden {
			self.audio.stop();
		}
		if self.solace.audio.active.is_some() {
			let status = self.audio.poll();
			let view = &mut self.solace.audio;
			view.position = status.position.as_secs_f32();
			view.duration = status.duration.as_secs_f32();
			view.loading = status.state == audio::State::Loading;
			view.playing = status.state == audio::State::Playing;
			view.ended = matches!(status.state, audio::State::Ended | audio::State::Idle);
			if let audio::State::Failed(error) = status.state {
				view.error = Some(error);
			}
		}
		match self.downloads.poll().clone() {
			downloads::Status::Downloading { received, total } if total > 0 => {
				self.solace.storage_status = "";
				ctx.request_repaint_after(Duration::from_millis(250));
				let _ = (received, total);
			}
			downloads::Status::Saved => {
				self.solace
					.toasts
					.push(Level::Success, "Saved to your computer");
				self.downloads.dismiss();
			}
			downloads::Status::Copied => {
				self.solace.toasts.push(Level::Success, "Copied");
				self.downloads.dismiss();
			}
			downloads::Status::Failed(error) => {
				self.solace.toasts.push(Level::Error, error);
				self.downloads.dismiss();
			}
			_ => {}
		}
	}

	fn sync_uploads(&mut self, ctx: &egui::Context, dropped: Vec<egui::DroppedFileHandle>) {
		let connected = self.state.user.is_some() && self.state.gateway_connected;
		let can_attach = self
			.state
			.selected
			.is_some_and(|channel| self.state.can_attach(channel));
		if !can_attach {
			self.uploads.cancel();
		}
		self.uploads
			.poll(self.state.generation, self.state.selected, connected, ctx);
		if !dropped.is_empty() {
			if connected
				&& can_attach
				&& self.state.freshness == model::Freshness::Fresh
				&& !self.solace.is_editing()
				&& let Some(channel) = self.state.selected
			{
				if let Err(error) = self.uploads.start_drop(
					self.state.generation,
					channel,
					self.runtime.handle(),
					ctx,
					dropped,
				) {
					self.solace.toasts.push(Level::Error, error);
				}
			} else {
				self.solace.toasts.push(
					Level::Warning,
					"Open a conversation where you can share files, then drop them again",
				);
			}
		}
		if !self.state.demo {
			self.solace.attachments = self.uploads.files();
		}
		self.solace.upload_busy = self.uploads.busy();
		self.solace.upload_progress = match self.uploads.transfer_progress() {
			(Some((sent, total)), _) if total > 0 => Some(sent as f32 / total as f32),
			(_, true) => Some(1.0),
			_ => None,
		};
		if let Some(notice) = self.uploads.take_notice() {
			self.solace.toasts.push(Level::Error, notice);
		}
	}
}

impl eframe::App for Desktop {
	fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
		visuals.panel_fill.to_opaque().to_normalized_gamma_f32()
	}

	fn persist_egui_memory(&self) -> bool {
		false
	}

	fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
		self.poll(ctx);
		self.poll_avatars(ctx);
		if self.state.auth != AuthState::Authenticated && !self.state.demo {
			self.notifications.clear();
		}
		if let Some(channel) = self.notifications.take_activation()
			&& self.state.auth == AuthState::Authenticated
			&& let Some(command) = self.state.select(channel)
		{
			self.command(command);
		}
		let viewing = self.state.selected.filter(|_| ctx.input(|i| i.focused));
		let demo = self.state.demo;
		if let Some(notification_runtime::Alert::Message {
			channel,
			title,
			body,
			avatar_key,
			image_path,
		}) = self.notification_runtime.poll(
			&mut self.state,
			&mut self.solace.media,
			true,
			viewing,
			&self.window,
			ctx,
			demo,
		) && self
			.notifications
			.notify_channel(channel, title, body, image_path)
			&& let Some(worker) = &self.avatars
		{
			worker.request(avatar_key);
		}
		self.poll_media(ctx);
		if let Some(value) = self.preferences.poll(&self.solace.media, ctx) {
			self.queue_cache_for(
				model::Id(0),
				cache::Operation::SaveAppPreferences(Box::new(value)),
			);
		}
		// Leave the CPU to the app in front (usually a game) unless something live is running.
		let (focused, minimized) = ctx.input(|i| {
			(
				i.viewport().focused.unwrap_or(true),
				i.viewport().minimized == Some(true),
			)
		});
		self.background.update(background::Activity {
			focused,
			minimized,
			realtime: self.state.voice.active.is_some()
				|| self.state.voice.incoming.is_some()
				|| self.solace.audio.playing
				|| self.solace.media.voice_preview_requested,
		});
		// Push to talk: hold V while Solace has focus and no text field is active.
		self.solace.media.voice_ptt_active = self.solace.media.voice_push_to_talk
			&& self.state.voice.active.is_some()
			&& ctx.input(|i| i.focused && i.key_down(egui::Key::V))
			&& ctx.memory(|m| m.focused().is_none());
	}

	fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
		let ctx = ui.ctx().clone();
		let (close_requested, dropped) = ctx.input_mut(|input| {
			(
				input.viewport().close_requested(),
				std::mem::take(&mut input.raw.dropped_files),
			)
		});
		if close_requested {
			let unsent = self.state.has_unsent() || self.uploads.has_unsent();
			let warned = self
				.close_warned
				.is_some_and(|at| at.elapsed() < Duration::from_secs(6));
			if unsent && !warned && !self.state.demo {
				ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
				self.close_warned = Some(std::time::Instant::now());
				self.solace.toasts.push(
					Level::Warning,
					"A message is still sending or unsaved. Close again to leave anyway.",
				);
			} else {
				self.uploads.cancel();
				self.downloads.cancel();
				self.audio.stop();
			}
		}
		self.sync_uploads(&ctx, dropped);

		// Discord's login page has its own header strip that doubles as the title bar.
		let window = ui.max_rect();
		let body = if self.login.is_some() {
			window
		} else {
			solace_ui::chrome::title_bar(ui, window, "Solace")
		};
		let mut body_ui =
			ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt("window-body"));
		body_ui.set_clip_rect(body);
		let ui = &mut body_ui;

		if self.login.is_some() {
			let full = ui.max_rect();
			let header = egui::Rect::from_min_size(
				full.min,
				egui::vec2(full.width(), platform::LOGIN_HEADER_HEIGHT),
			);
			if solace_ui::signin::login_header(ui, header) {
				self.login = None;
				self.state.auth = AuthState::Unauthenticated;
				self.state.status = "";
			} else {
				let body =
					egui::Rect::from_min_max(egui::pos2(full.left(), header.bottom()), full.max);
				let p = solace_ui::theme::palette(&ctx);
				ui.painter().rect_filled(body, 0, p.canvas);
				ui.painter().text(
					body.center(),
					egui::Align2::CENTER_CENTER,
					"Loading discord.com...",
					solace_ui::theme::sans(14.0),
					p.muted,
				);
			}
			if let Some(login) = &self.login {
				login.resize(&self.window);
			}
		} else if self.state.user.is_some() {
			let commands = self.solace.show(ui, &mut self.state);
			for command in commands {
				self.command(command);
			}
			if !self.state.demo
				&& let Some(command) =
					self.voice
						.poll(&self.runtime, &mut self.state, &mut self.solace.media, &ctx)
			{
				self.command(command);
			}
			self.handle_requests(&ctx);
		} else if let Some(stage) = self.restoring() {
			if solace_ui::signin::restoring(&mut self.solace, ui, stage) {
				if let Some(store) = &mut self.store {
					store.cancel_load();
				}
				self.connection = None;
				self.state.auth = AuthState::Unauthenticated;
				self.state.status = "";
			}
		} else {
			let status = [self.state.status, self.credential_status]
				.into_iter()
				.filter(|s| !s.is_empty() && *s != "Disconnected")
				.collect::<Vec<_>>()
				.join("\n");
			let busy = self.state.auth == AuthState::Authenticating || self.forgetting;
			let accounts = self.solace.accounts.clone();
			let action = solace_ui::signin::show(
				&mut self.solace,
				ui,
				solace_ui::signin::Props {
					accounts: &accounts,
					status: &status,
					busy,
					token: &mut self.token_input,
					authorized: &mut self.authorized,
				},
			);
			match action {
				Some(solace_ui::signin::Action::Browser) => self.open_login(&ctx),
				Some(solace_ui::signin::Action::Token) => {
					let input = std::mem::take(&mut *self.token_input);
					match SessionSecret::from_owner_input(input) {
						Ok(secret) => self.connect(secret, true, &ctx),
						Err(failure) => self.state.status = failure.label(),
					}
				}
				Some(solace_ui::signin::Action::Saved(account)) => self.begin_switch(account),
				Some(solace_ui::signin::Action::Forget(account)) => {
					self.forget_saved_account(&ctx, account)
				}
				None => {}
			}
			self.handle_requests(&ctx);
			self.solace.toasts.show(&ctx);
		}
		self.request_images(&ctx);
		self.sync_account_roster();
		solace_ui::chrome::resize_edges(&ctx);
	}
}
