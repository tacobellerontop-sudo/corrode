//! Device-level voice preferences: devices, push to talk, processing, and the overall and
//! per-person volumes. They load once at startup and save shortly after the reader changes
//! them; every other field of the stored preferences is carried through untouched.
use eframe::egui;
use local_store::AppPreferences;
use model::Id;
use solace_ui::{Media, VoiceGain};
use std::time::{Duration, Instant};

/// Saves wait until a slider has been still this long.
const SETTLE: Duration = Duration::from_millis(600);

#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
	input: Option<String>,
	output: Option<String>,
	gain: VoiceGain,
	push_to_talk: bool,
	processing: model::voice_settings::VoiceProcessing,
	user_volumes: Vec<(Id, u16)>,
	muted_users: Vec<Id>,
}

impl Voice {
	pub fn capture(media: &Media) -> Self {
		Self {
			input: media.voice_input.clone(),
			output: media.voice_output.clone(),
			gain: media.voice_gain,
			push_to_talk: media.voice_push_to_talk,
			processing: media.voice_processing,
			user_volumes: media.user_volumes.clone(),
			muted_users: media.muted_users.clone(),
		}
	}
}

/// Apply stored preferences to the call engine's view state.
pub fn apply(preferences: &AppPreferences, media: &mut Media) {
	media.voice_input = preferences.voice_input.clone();
	media.voice_output = preferences.voice_output.clone();
	media.voice_gain = VoiceGain {
		input_percent: preferences.input_percent,
		output_percent: preferences.output_percent,
	};
	media.voice_push_to_talk = preferences.voice_push_to_talk;
	if let Some(processing) = preferences.voice_processing {
		media.voice_processing = processing;
	}
	media.user_volumes.clear();
	media.muted_users.clear();
	for &(user, volume) in &preferences.user_volumes {
		media.set_user_volume(Id(user), volume);
	}
	for &user in &preferences.muted_users {
		media.set_user_muted(Id(user), true);
	}
}

fn store(voice: &Voice, preferences: &mut AppPreferences) {
	preferences.voice_input = voice.input.clone();
	preferences.voice_output = voice.output.clone();
	preferences.input_percent = voice.gain.input_percent.min(solace_ui::media::MAX_VOLUME);
	preferences.output_percent = voice.gain.output_percent.min(solace_ui::media::MAX_VOLUME);
	preferences.voice_push_to_talk = voice.push_to_talk;
	preferences.voice_processing = Some(voice.processing);
	preferences.user_volumes = voice
		.user_volumes
		.iter()
		.take(64)
		.map(|(user, volume)| (user.0, *volume))
		.collect();
	preferences.muted_users = voice
		.muted_users
		.iter()
		.take(64)
		.map(|user| user.0)
		.collect();
}

/// Tracks what was last saved and debounces the next save.
#[derive(Default)]
pub struct Preferences {
	/// The stored preferences once loaded; nothing is saved before then.
	loaded: Option<AppPreferences>,
	saved: Option<Voice>,
	changed: Option<(Voice, Instant)>,
}

impl Preferences {
	pub fn loaded(&mut self, preferences: AppPreferences, media: &mut Media) {
		apply(&preferences, media);
		self.saved = Some(Voice::capture(media));
		self.loaded = Some(preferences);
		self.changed = None;
	}

	/// The value to save once a change has settled, if any.
	pub fn poll(&mut self, media: &Media, ctx: &egui::Context) -> Option<AppPreferences> {
		let saved = self.saved.as_ref()?;
		let current = Voice::capture(media);
		if &current == saved {
			self.changed = None;
			return None;
		}
		let since = match &self.changed {
			Some((pending, at)) if *pending == current => at.elapsed(),
			_ => {
				self.changed = Some((current, Instant::now()));
				Duration::ZERO
			}
		};
		if since < SETTLE {
			ctx.request_repaint_after(SETTLE - since);
			return None;
		}
		let (current, _) = self.changed.take()?;
		let preferences = self.loaded.as_mut()?;
		store(&current, preferences);
		self.saved = Some(current);
		Some(preferences.clone())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn volumes_round_trip_through_preferences() {
		let mut stored = AppPreferences {
			user_volumes: vec![(5, 140)],
			muted_users: vec![6],
			output_percent: 80,
			..AppPreferences::default()
		};
		let mut media = Media::default();
		apply(&stored, &mut media);
		assert_eq!(media.user_volume(Id(5)), 140);
		assert!(media.is_user_muted(Id(6)));
		assert_eq!(media.voice_gain.output_percent, 80);
		media.set_user_volume(Id(7), 60);
		store(&Voice::capture(&media), &mut stored);
		assert!(stored.user_volumes.contains(&(7, 60)));
		assert!(stored.is_valid());
	}
}
