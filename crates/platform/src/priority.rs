//! Process scheduling hints. In the background Solace lowers its CPU priority and asks for
//! Windows' Efficiency mode (EcoQoS: efficiency cores, low clocks) so the app in front,
//! usually a game, gets the CPU first; `false` restores normal scheduling.
#[cfg(not(target_os = "windows"))]
pub fn set_background(_background: bool) {}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
pub fn set_background(background: bool) {
	use windows::Win32::System::Threading::{
		BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, NORMAL_PRIORITY_CLASS,
		PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
		PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass,
		SetProcessInformation,
	};
	let throttling = PROCESS_POWER_THROTTLING_STATE {
		Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
		ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
		StateMask: if background {
			PROCESS_POWER_THROTTLING_EXECUTION_SPEED
		} else {
			0
		},
	};
	// SAFETY: the process pseudo-handle needs no closing, and `throttling` outlives the call
	// that receives its address and size. A refusal (Windows without EcoQoS) keeps the
	// previous scheduling, which is harmless.
	unsafe {
		let process = GetCurrentProcess();
		let _ = SetPriorityClass(
			process,
			if background {
				BELOW_NORMAL_PRIORITY_CLASS
			} else {
				NORMAL_PRIORITY_CLASS
			},
		);
		let _ = SetProcessInformation(
			process,
			ProcessPowerThrottling,
			(&raw const throttling).cast(),
			size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
		);
	}
}
