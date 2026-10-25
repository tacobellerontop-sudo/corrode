//! Voice and media state shared with the desktop's call engine. The desktop polls devices,
//! speaking indicators and previews into this struct; views read it and record intents.
use client_core::screen::{Settings, Source, SourceId};
use model::Id;

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
	pub voice_stream_view: Option<egui::TextureHandle>,
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
		}
	}
}

impl Media {
	pub fn voice_settings_open(&self) -> bool {
		self.settings_voice_open
	}
	/// Per-user playback volume (percent). Solace keeps everyone at 100% for now.
	pub fn voice_user_volumes(&self) -> [(u64, u16); 64] {
		[(0, 100); 64]
	}
	pub fn voice_stream_volume(&self) -> u16 {
		100
	}
	/// Reset session-local call state (account change).
	pub fn reset(&mut self) {
		let devices = (
			std::mem::take(&mut self.voice_inputs),
			std::mem::take(&mut self.voice_outputs),
			self.voice_input.take(),
			self.voice_output.take(),
		);
		*self = Self::default();
		(
			self.voice_inputs,
			self.voice_outputs,
			self.voice_input,
			self.voice_output,
		) = devices;
	}
}
