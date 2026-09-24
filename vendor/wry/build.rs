// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() {
	let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
	assert_eq!(
		target_os, "windows",
		"Corrode's scoped Wry fork supports Windows only; other platforms use their own backends"
	);
}
