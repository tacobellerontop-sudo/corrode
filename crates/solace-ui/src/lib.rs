//! Solace: a fully custom-painted desktop interface rendered through egui's wgpu backend.
//! Views read `client_core::State` and emit typed commands; no networking or credentials
//! live here.
pub mod clock;
pub mod emoji;
pub mod fonts;
pub mod icons;
pub mod images;
pub mod media;
pub mod nav;
pub mod paint;
pub mod scenery;
pub mod text;
pub mod theme;

mod aside;
mod composer;
mod conversation;
mod dashboard;
mod overlays;
mod rail;
mod sidebar;
pub mod signin;
mod thread;
mod toasts;

pub use media::{Media, VoiceGain};
pub use toasts::{Level, Toasts};

use client_core::{Command, State};
use egui::{Rect, Ui, UiBuilder, pos2};
use model::Id;
use std::collections::{HashMap, HashSet};

/// Install fonts, the emoji atlas and the Solace style. Call once at startup.
pub fn install(ctx: &egui::Context) -> Result<(), image::ImageError> {
	fonts::install(ctx);
	emoji::install(ctx)?;
	theme::apply(ctx);
	Ok(())
}

/// Things the interface asks the desktop shell to do; they need OS or worker access.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
	Logout,
	/// Choose files to attach to the open conversation.
	Attach,
	RemoveAttachment(usize),
	CancelUpload,
	/// The reader confirmed opening an external link.
	OpenLink(String),
	/// Persist the draft of this channel.
	SaveDraft(Id),
	Theme(egui::ThemePreference),
	SwitchAccount(Id),
	AddAccount,
	ClearCache,
	/// Save an attachment's original file (the desktop asks where).
	Download(model::Attachment),
	PlayAudio(model::Attachment),
	/// Pause (true) or resume (false) the active audio attachment.
	PauseAudio(bool),
	SeekAudio(f32),
}

/// Playback of one audio attachment, reported by the desktop's audio engine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioView {
	pub active: Option<Id>,
	pub playing: bool,
	pub loading: bool,
	pub ended: bool,
	pub position: f32,
	pub duration: f32,
	pub error: Option<&'static str>,
}

/// Which collection the rail has focused in the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Pane {
	#[default]
	Home,
	Direct,
	Explore,
	Activity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPage {
	Appearance,
	Account,
	Voice,
	About,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PickerTarget {
	Composer,
	Thread,
	Reaction(Id),
}

#[derive(Default)]
pub(crate) struct ScrollMemo {
	/// Channel whose timeline the memo describes.
	pub channel: Option<Id>,
	/// Oldest message before an older page was requested and the content height then.
	pub anchor: Option<(Id, f32)>,
	pub content_height: f32,
	pub offset: f32,
	pub at_bottom: bool,
	pub jump_to_bottom: bool,
}

pub struct Solace {
	pub images: images::Images,
	pub media: Media,
	pub toasts: Toasts,
	pub accounts: Vec<model::SavedAccount>,
	/// Files staged for the next message (name, bytes).
	pub attachments: Vec<(String, u64)>,
	/// Upload progress 0..=1 while a message with files is sending.
	pub upload_progress: Option<f32>,
	pub upload_busy: bool,
	pub storage_status: &'static str,
	pub audio: AudioView,
	pub theme: egui::ThemePreference,
	/// Offline preview: actions affect synthetic state only.
	pub demo: bool,
	requests: Vec<Request>,
	pub(crate) scenery: scenery::Scenery,
	pub(crate) pane: Pane,
	pub(crate) search: String,
	pub(crate) aside_open: bool,
	pub(crate) members_expanded: bool,
	pub(crate) settings: Option<SettingsPage>,
	pub(crate) thread: Option<Id>,
	pub(crate) thread_draft: String,
	pub(crate) link: Option<String>,
	pub(crate) picker: Option<(PickerTarget, egui::Pos2)>,
	pub(crate) join: Option<String>,
	pub(crate) editing: Option<(Id, String)>,
	pub(crate) revealed: HashSet<Id>,
	pub(crate) heights: HashMap<Id, (u64, f32)>,
	pub(crate) scroll: ScrollMemo,
	pub(crate) collapsed: HashSet<Id>,
	pub(crate) account_menu: bool,
	pub(crate) more_menu: bool,
	pub(crate) last_read_request: Option<Id>,
	pub(crate) last_draft: Option<(Id, String)>,
	pub(crate) draft_dirty: Option<(Id, std::time::Instant)>,
}

impl Default for Solace {
	fn default() -> Self {
		Self {
			images: Default::default(),
			media: Default::default(),
			toasts: Default::default(),
			accounts: Vec::new(),
			attachments: Vec::new(),
			upload_progress: None,
			upload_busy: false,
			storage_status: "",
			audio: AudioView::default(),
			theme: egui::ThemePreference::Light,
			demo: false,
			requests: Vec::new(),
			scenery: Default::default(),
			pane: Pane::Home,
			search: String::new(),
			aside_open: true,
			members_expanded: false,
			settings: None,
			thread: None,
			thread_draft: String::new(),
			link: None,
			picker: None,
			join: None,
			editing: None,
			revealed: HashSet::new(),
			heights: HashMap::new(),
			scroll: ScrollMemo::default(),
			collapsed: HashSet::new(),
			account_menu: false,
			more_menu: false,
			last_read_request: None,
			last_draft: None,
			draft_dirty: None,
		}
	}
}

impl Solace {
	pub(crate) fn request(&mut self, request: Request) {
		if !self.requests.contains(&request) {
			self.requests.push(request);
		}
	}

	pub fn take_requests(&mut self) -> Vec<Request> {
		std::mem::take(&mut self.requests)
	}

	/// Forget per-session view state (logout or account switch); keeps preferences.
	pub fn clear_session(&mut self) {
		let keep = (
			std::mem::take(&mut self.accounts),
			self.theme,
			self.demo,
			std::mem::take(&mut self.toasts),
		);
		*self = Self::default();
		(self.accounts, self.theme, self.demo, self.toasts) = keep;
	}

	/// Open the settings dialog on a page.
	pub fn open_settings(&mut self, page: SettingsPage) {
		self.settings = Some(page);
	}

	/// Show the thread panel for a message (or close it with `None`).
	pub fn open_thread(&mut self, message: Option<Id>) {
		self.thread = message;
	}

	pub fn is_editing(&self) -> bool {
		self.editing.is_some()
	}

	/// The whole signed-in interface. Returns commands for the desktop to execute.
	pub fn show(&mut self, ui: &mut Ui, state: &mut State) -> Vec<Command> {
		let mut commands = Vec::new();
		self.images.begin_frame();
		let p = theme::palette(ui.ctx());
		let full = ui.max_rect();
		ui.painter().rect_filled(full, 0, p.canvas);

		if self.scroll.channel != state.selected {
			self.on_navigate(state);
		}

		let width = full.width();
		// The dashboard uses the full width; the context column belongs to a conversation.
		let aside = self.aside_open && width >= 1180.0 && state.selected.is_some();
		// Home is the dashboard alone; the sidebar belongs to the other panes.
		let sidebar_w = if self.pane == Pane::Home {
			0.0
		} else if width < 900.0 {
			232.0
		} else {
			theme::size::SIDEBAR
		};
		let rail = Rect::from_min_max(
			full.min,
			pos2(full.left() + theme::size::RAIL, full.bottom()),
		);
		let sidebar = Rect::from_min_max(
			pos2(rail.right(), full.top()),
			pos2(rail.right() + sidebar_w, full.bottom()),
		);
		let aside_rect = aside.then(|| {
			Rect::from_min_max(
				pos2(full.right() - theme::size::ASIDE, full.top()),
				full.max,
			)
		});
		let main = Rect::from_min_max(
			pos2(sidebar.right(), full.top()),
			pos2(aside_rect.map_or(full.right(), |r| r.left()), full.bottom()),
		);

		self.region(ui, rail, "rail", |this, ui| {
			rail::show(this, ui, state, &mut commands)
		});
		if sidebar_w > 0.0 {
			self.region(ui, sidebar, "sidebar", |this, ui| {
				sidebar::show(this, ui, state, &mut commands)
			});
		}
		self.region(ui, main, "main", |this, ui| {
			conversation::show(this, ui, state, &mut commands)
		});
		if let Some(rect) = aside_rect {
			self.region(ui, rect, "aside", |this, ui| {
				aside::show(this, ui, state, &mut commands)
			});
		}
		if self.thread.is_some() {
			thread::show(self, ui, main, state, &mut commands);
		}
		overlays::show(self, ui, state, &mut commands);
		self.toasts.show(ui.ctx());
		self.sync_draft(state);
		if self.draft_dirty.is_some() {
			ui.ctx()
				.request_repaint_after(std::time::Duration::from_millis(300));
		}
		if let Some(status) = state.take_user_action_status() {
			self.toasts.push(Level::Info, status);
		}
		commands
	}

	fn region(&mut self, ui: &mut Ui, rect: Rect, id: &str, add: impl FnOnce(&mut Self, &mut Ui)) {
		let mut child = ui.new_child(
			UiBuilder::new()
				.max_rect(rect)
				.id_salt(id)
				.layout(egui::Layout::top_down(egui::Align::Min)),
		);
		child.set_clip_rect(rect);
		add(self, &mut child);
	}

	fn on_navigate(&mut self, state: &State) {
		self.scroll = ScrollMemo {
			channel: state.selected,
			jump_to_bottom: true,
			..Default::default()
		};
		// A thread opened for a message in the newly shown conversation stays open.
		if self
			.thread
			.is_some_and(|id| state.timeline.get_display(id).is_none())
		{
			self.thread = None;
			self.thread_draft.clear();
		}
		self.editing = None;
		self.picker = None;
		self.more_menu = false;
		self.heights.clear();
		self.revealed.clear();
		// Home has no sidebar, so opening a conversation moves to the pane that lists it.
		if let Some(channel) = state.selected.and_then(|id| state.channel(id)) {
			match (channel.guild.is_some(), self.pane) {
				(false, Pane::Home | Pane::Explore) => self.pane = Pane::Direct,
				(true, Pane::Home) => self.pane = Pane::Explore,
				_ => {}
			}
		}
	}

	/// Drafts are owned by `State`; ask the desktop to persist them once typing settles
	/// (or immediately when the reader leaves the conversation).
	fn sync_draft(&mut self, state: &State) {
		const SETTLE: std::time::Duration = std::time::Duration::from_millis(1200);
		if let Some((channel, since)) = self.draft_dirty
			&& (state.selected != Some(channel) || since.elapsed() >= SETTLE)
		{
			self.draft_dirty = None;
			self.request(Request::SaveDraft(channel));
		}
		let Some(channel) = state.selected else {
			return;
		};
		let draft = state.drafts.get(&channel).cloned().unwrap_or_default();
		match &self.last_draft {
			Some((last, text)) if *last == channel && *text == draft => {}
			Some((last, _)) if *last == channel => {
				self.last_draft = Some((channel, draft));
				self.draft_dirty = Some((channel, std::time::Instant::now()));
			}
			_ => self.last_draft = Some((channel, draft)),
		}
	}
}

/// Display name for a user in the open conversation's context.
pub(crate) fn display_name(state: &State, user: &model::User) -> String {
	if let Some(member) = nav::people(state)
		.into_iter()
		.find(|m| m.user.id == user.id)
		&& let Some(nick) = member.nick.as_deref().filter(|n| !n.is_empty())
	{
		return nick.to_owned();
	}
	state.user_display_name(user).to_owned()
}

#[cfg(test)]
mod tests;
