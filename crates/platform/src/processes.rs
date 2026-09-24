//! Read-only enumeration of this user's running executable paths for local game detection.
//! Never inspects another user's processes, memory, arguments, environment or open files.

/// Bounds both the syscall/parse work and the memory a hostile process table can force.
pub const MAX_PROCESSES: usize = 4096;
const MAX_PATH: usize = 512;

/// Executable paths of the processes visible to this user, in no particular order.
/// A partial list is normal: processes exit while the table is read.
pub fn running() -> std::io::Result<Vec<String>> {
	native::running()
}

fn accept(path: &str, into: &mut Vec<String>) {
	let path = path.trim();
	if path.is_empty()
		|| path.len() > MAX_PATH
		|| path.chars().any(char::is_control)
		|| into.len() >= MAX_PROCESSES
	{
		return;
	}
	into.push(path.to_owned());
}

mod native {
	use super::{MAX_PROCESSES, accept};
	use std::os::windows::process::CommandExt;
	use std::process::Command;

	const CREATE_NO_WINDOW: u32 = 0x0800_0000;

	/// `tasklist` lists image names without opening another process' handle. Paths are
	/// unavailable this way, which is fine: detectable entries are image names on Windows.
	pub fn running() -> std::io::Result<Vec<String>> {
		let output = Command::new("tasklist.exe")
			.args(["/nh", "/fo", "csv"])
			.creation_flags(CREATE_NO_WINDOW)
			.output()?;
		if !output.status.success() {
			return Err(std::io::Error::other("process list is unavailable"));
		}
		let mut paths = Vec::new();
		for line in String::from_utf8_lossy(&output.stdout).lines() {
			if paths.len() >= MAX_PROCESSES {
				break;
			}
			// `"image.exe","1234","Console","1","12,345 K"`; only the quoted image name is used.
			let Some(name) = line.strip_prefix('"').and_then(|l| l.split('"').next()) else {
				continue;
			};
			accept(name, &mut paths);
		}
		Ok(paths)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn own_process_is_listed_within_bounds() {
		let paths = running().expect("the current user's process list must be readable");
		assert!(paths.len() <= MAX_PROCESSES);
		assert!(paths.iter().all(|path| path.len() <= MAX_PATH));
		let current = std::env::current_exe().unwrap();
		let name = current
			.file_name()
			.unwrap()
			.to_string_lossy()
			.to_lowercase();
		assert!(
			paths
				.iter()
				.any(|path| path.to_lowercase().contains(name.trim_end_matches(".exe"))),
			"the test binary must appear in {paths:?}"
		);
	}

	#[test]
	fn unbounded_and_control_character_paths_are_dropped() {
		let mut paths = Vec::new();
		accept("", &mut paths);
		accept("  ", &mut paths);
		accept("/usr/bin/game\u{7}", &mut paths);
		accept(&"x".repeat(MAX_PATH + 1), &mut paths);
		assert!(paths.is_empty());
		accept("  /usr/bin/game  ", &mut paths);
		assert_eq!(paths, ["/usr/bin/game"]);
		let mut full = vec![String::new(); MAX_PROCESSES];
		accept("/usr/bin/game", &mut full);
		assert_eq!(full.len(), MAX_PROCESSES);
	}
}
