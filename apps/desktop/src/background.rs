//! Stepping aside for the app in front. While Solace is minimized or unfocused and nothing
//! time-sensitive is running, it lowers its own CPU priority and asks Windows for Efficiency
//! mode (EcoQoS), which favours efficiency cores and low clocks. Whatever has focus, usually
//! a game, gets the CPU first. The moment Solace is focused again, or a call, a ring or audio
//! playback starts, it returns to normal so voice and sound are never throttled.

/// What decides whether Solace may yield the CPU this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activity {
	pub focused: bool,
	pub minimized: bool,
	/// A call, an incoming ring, a mic test or audio playback: keep full priority.
	pub realtime: bool,
}

impl Activity {
	pub fn may_yield(self) -> bool {
		(self.minimized || !self.focused) && !self.realtime
	}
}

#[derive(Default)]
pub struct Background {
	yielded: Option<bool>,
}

impl Background {
	/// Apply the priority for this frame; the system is only called when it changes.
	pub fn update(&mut self, activity: Activity) {
		let yielded = activity.may_yield();
		if self.yielded != Some(yielded) {
			self.yielded = Some(yielded);
			platform::priority::set_background(yielded);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn only_yields_in_the_background_without_realtime_work() {
		let base = Activity {
			focused: false,
			minimized: false,
			realtime: false,
		};
		assert!(base.may_yield());
		assert!(
			!Activity {
				focused: true,
				..base
			}
			.may_yield()
		);
		assert!(
			Activity {
				focused: true,
				minimized: true,
				..base
			}
			.may_yield()
		);
		assert!(
			!Activity {
				realtime: true,
				..base
			}
			.may_yield(),
			"a call keeps full priority"
		);
	}
}
