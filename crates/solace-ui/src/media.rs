//! Voice and media state shared with the desktop's call engine. The desktop polls devices,
//! speaking indicators and previews into this struct; views read it and record intents.
use client_core::screen::{Settings, Source, SourceId};
use model::Id;

/// A video picture the interface can draw: a texture and its size in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoView {
	pub texture: egui::TextureId,
	pub size: egui::Vec2,
}

/// Highest playback or capture volume, in percent.
pub const MAX_VOLUME: u16 = 200;
/// Per-person overrides (volumes plus local mutes) the mixer and preferences hold.
const MAX_OVERRIDES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceGain {
	pub input_percent: u16,
	pub output_percent: u16,
}

impl Default for VoiceGain {
	fn default() -> Self {
		Self {
			input_percent: 100,
			output_percent: 100,
		}
	}
}

pub mod screen {
	pub use super::ScreenUi;
	pub enum Request {
		Start(super::Settings),
		Stop,
	}
}

/// Screen-share selection. Solace does not offer screen sharing in its interface yet; the
/// engine still consults this so an active share from elsewhere is stopped cleanly.
#[derive(Default)]
pub struct ScreenUi {
	pub context: Option<(u64, Id, u64)>,
	pub open: bool,
	pub sources: Vec<Source>,
	pub selected: Option<SourceId>,
	pub refresh_requested: bool,
	pub request: Option<screen::Request>,
	pub busy: bool,
	pub status: &'static str,
	pub capture_status: Option<&'static str>,
	pub supported: bool,
	pub preview: Option<egui::TextureHandle>,
}

/// Field names mirror what the call engine reads and writes.
pub struct Media {
	pub voice_available: bool,
	pub voice_switch_ready: bool,
	pub screen: ScreenUi,
	pub voice_camera_available: bool,
	pub voice_camera_status: &'static str,
	pub voice_cameras: Vec<(String, String)>,
	pub voice_camera_device: Option<String>,
	pub voice_refresh_cameras: bool,
	pub voice_camera_device_status: &'static str,
	pub voice_camera_devices_loading: bool,
	pub voice_camera_preview: Option<egui::TextureHandle>,
	pub camera_test_requested: bool,
	pub camera_test_available: bool,
	pub camera_test_status: &'static str,
	pub camera_test_texture: Option<egui::TextureHandle>,
	pub voice_remote_video: Vec<(Id, egui::TextureHandle)>,
	/// The watched stream's latest picture, converted on the GPU by the desktop.
	pub voice_stream_view: Option<VideoView>,
	pub voice_stream_status: &'static str,
	pub voice_inputs: Vec<(String, String)>,
	pub voice_outputs: Vec<(String, String)>,
	pub voice_input: Option<String>,
	pub voice_output: Option<String>,
	pub voice_gain: VoiceGain,
	pub voice_refresh_devices: bool,
	pub voice_device_status: &'static str,
	pub voice_microphone_unavailable: bool,
	pub voice_push_to_talk: bool,
	pub voice_muted: bool,
	pub voice_deafened: bool,
	pub voice_processing: model::voice_settings::VoiceProcessing,
	pub voice_preview_requested: bool,
	pub voice_preview_level: Option<f32>,
	pub voice_preview_status: &'static str,
	pub voice_ptt_active: bool,
	pub voice_privacy_code: Option<String>,
	pub voice_speaking: Vec<Id>,
	pub notification_options: model::notification_preferences::Device,
	pub notification_preview: Option<model::notification_preferences::Sound>,
	/// Settings dialog is showing the voice page (the engine previews the mic only then).
	pub settings_voice_open: bool,
	/// Per-person playback volume overrides (percent, 0–200); people at 100% are absent.
	pub user_volumes: Vec<(Id, u16)>,
	/// People silenced on this device only.
	pub muted_users: Vec<Id>,
	/// Playback volume of a watched stream's audio (percent, 0–200).
	pub stream_volume: u16,
}

impl Default for Media {
	fn default() -> Self {
		Self {
			voice_available: true,
			voice_switch_ready: true,
			screen: ScreenUi::default(),
			voice_camera_available: false,
			voice_camera_status: "",
			voice_cameras: Vec::new(),
			voice_camera_device: None,
			voice_refresh_cameras: false,
			voice_camera_device_status: "",
			voice_camera_devices_loading: false,
			voice_camera_preview: None,
			camera_test_requested: false,
			camera_test_available: false,
			camera_test_status: "",
			camera_test_texture: None,
			voice_remote_video: Vec::new(),
			voice_stream_view: None,
			voice_stream_status: "",
			voice_inputs: Vec::new(),
			voice_outputs: Vec::new(),
			voice_input: None,
			voice_output: None,
			voice_gain: VoiceGain::default(),
			voice_refresh_devices: false,
			voice_device_status: "",
			voice_microphone_unavailable: false,
			voice_push_to_talk: false,
			voice_muted: false,
			voice_deafened: false,
			voice_processing: Default::default(),
			voice_preview_requested: false,
			voice_preview_level: None,
			voice_preview_status: "",
			voice_ptt_active: false,
			voice_privacy_code: None,
			voice_speaking: Vec::new(),
			notification_options: Default::default(),
			notification_preview: None,
			settings_voice_open: false,
			user_volumes: Vec::new(),
			muted_users: Vec::new(),
			stream_volume: 100,
		}
	}
}

impl Media {
	pub fn voice_settings_open(&self) -> bool {
		self.settings_voice_open
	}
	/// Per-user playback volumes (percent) for the call mixer; locally muted people play at 0.
	pub fn voice_user_volumes(&self) -> [(u64, u16); 64] {
		let mut volumes = [(0, 100); 64];
		let entries = self.muted_users.iter().map(|user| (user.0, 0)).chain(
			self.user_volumes
				.iter()
				.filter(|(user, _)| !self.muted_users.contains(user))
				.map(|(user, volume)| (user.0, (*volume).min(MAX_VOLUME))),
		);
		for (slot, entry) in volumes.iter_mut().zip(entries) {
			*slot = entry;
		}
		volumes
	}
	pub fn voice_stream_volume(&self) -> u16 {
		self.stream_volume.min(MAX_VOLUME)
	}
	pub fn user_volume(&self, user: Id) -> u16 {
		self.user_volumes
			.iter()
			.find(|(id, _)| *id == user)
			.map_or(100, |(_, volume)| *volume)
	}
	pub fn set_user_volume(&mut self, user: Id, volume: u16) {
		let volume = volume.min(MAX_VOLUME);
		self.user_volumes.retain(|(id, _)| *id != user);
		if volume != 100 {
			// The mixer and preferences hold at most 64 overrides; the oldest one gives way.
			if self.user_volumes.len() + self.muted_users.len() >= MAX_OVERRIDES {
				self.user_volumes.remove(0);
			}
			self.user_volumes.push((user, volume));
		}
	}
	pub fn is_user_muted(&self, user: Id) -> bool {
		self.muted_users.contains(&user)
	}
	pub fn set_user_muted(&mut self, user: Id, muted: bool) {
		self.muted_users.retain(|id| *id != user);
		if muted {
			if self.user_volumes.len() + self.muted_users.len() >= MAX_OVERRIDES
				&& !self.user_volumes.is_empty()
			{
				self.user_volumes.remove(0);
			}
			if self.muted_users.len() < MAX_OVERRIDES {
				self.muted_users.push(user);
			}
		}
	}
	/// Reset session-local call state (account change). Devices, volumes and other
	/// device-level preferences stay.
	pub fn reset(&mut self) {
		let kept = Self {
			voice_inputs: std::mem::take(&mut self.voice_inputs),
			voice_outputs: std::mem::take(&mut self.voice_outputs),
			voice_input: self.voice_input.take(),
			voice_output: self.voice_output.take(),
			voice_gain: self.voice_gain,
			voice_push_to_talk: self.voice_push_to_talk,
			voice_processing: self.voice_processing,
			user_volumes: std::mem::take(&mut self.user_volumes),
			muted_users: std::mem::take(&mut self.muted_users),
			stream_volume: self.stream_volume,
			..Self::default()
		};
		*self = kept;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn mixer_volumes_include_overrides_and_local_mutes() {
		let mut media = Media::default();
		media.set_user_volume(Id(7), 150);
		media.set_user_volume(Id(8), 100);
		media.set_user_muted(Id(9), true);
		let volumes = media.voice_user_volumes();
		assert!(volumes.contains(&(9, 0)));
		assert!(volumes.contains(&(7, 150)));
		assert!(!volumes.iter().any(|(user, _)| *user == 8));
		assert_eq!(media.user_volume(Id(7)), 150);
		media.set_user_volume(Id(7), 999);
		assert_eq!(media.user_volume(Id(7)), MAX_VOLUME);
		media.reset();
		assert!(
			media.is_user_muted(Id(9)),
			"volumes survive an account switch"
		);
	}
}
