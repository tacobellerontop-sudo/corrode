use crate::{Channel, Id};

/// Which device-local shortcut list a channel belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Shortcut {
	Pinned,
	Favorite,
}

/// Outcome of one shortcut write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreferenceEdit {
	Unchanged,
	Changed,
	CapacityReached,
}

/// Device-local channel navigation preferences, isolated by account; never synchronized to Discord.
/// At most 256 IDs (2 KiB of ID payload) across all lists.
/// Vec order is display order: index 0 is shown first, and a new entry goes to the front.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ChannelPreferences {
	pub favorites: Vec<Id>,
	pub pinned: Vec<Id>,
	pub collapsed_categories: Vec<Id>,
	/// Custom label for the pinned-DM section; empty means the default.
	pub pinned_name: String,
	/// Custom sRGB colour for the pinned-DM section heading, packed 0xRRGGBB.
	pub pinned_color: Option<u32>,
}

impl ChannelPreferences {
	pub const MAX_ENTRIES: usize = 256;
	pub const MAX_JSON_BYTES: usize = 8192;
	/// Longest sidebar heading the preferences accept; display trims the rest.
	pub const MAX_PINNED_NAME_CHARS: usize = 24;
	/// Fallback heading while no custom label is set.
	pub const DEFAULT_PINNED_NAME: &'static str = "Pinned";

	pub fn is_valid(&self) -> bool {
		// ponytail: duplicate scans are capped at 256 IDs; use a set if this limit grows.
		self.favorites
			.len()
			.saturating_add(self.pinned.len())
			.saturating_add(self.collapsed_categories.len())
			<= Self::MAX_ENTRIES
			&& self
				.favorites
				.capacity()
				.saturating_add(self.pinned.capacity())
				.saturating_add(self.collapsed_categories.capacity())
				<= Self::MAX_ENTRIES * 3
		&& [&self.favorites, &self.pinned, &self.collapsed_categories]
			.into_iter()
			.all(|ids| {
				ids.iter()
					.enumerate()
					.all(|(index, id)| id.0 != 0 && !ids[..index].contains(id))
			})
		&& self.pinned_name.chars().count() <= Self::MAX_PINNED_NAME_CHARS
		&& self.pinned_color.is_none_or(|color| color <= 0x00FF_FFFF)
	}

	fn list(&self, kind: Shortcut) -> &Vec<Id> {
		match kind {
			Shortcut::Pinned => &self.pinned,
			Shortcut::Favorite => &self.favorites,
		}
	}

	/// Display-ordered ids of one list.
	pub fn ids(&self, kind: Shortcut) -> &[Id] {
		self.list(kind)
	}

	pub fn contains(&self, kind: Shortcut, channel: Id) -> bool {
		self.list(kind).contains(&channel)
	}

	/// Home pins are 1:1 and group DMs only.
	pub fn can_pin(channel: &Channel) -> bool {
		channel.id.0 != 0 && channel.guild.is_none() && matches!(channel.kind, 1 | 3)
	}

	/// Server favorites are guild channels that are not categories.
	pub fn can_favorite(channel: &Channel) -> bool {
		channel.id.0 != 0 && channel.guild.is_some() && channel.kind != 4
	}

	/// Idempotent. Turning a shortcut on inserts it at the front of its list.
	pub fn set(&mut self, kind: Shortcut, channel: Id, on: bool) -> PreferenceEdit {
		if channel.0 == 0 || !self.is_valid() || self.contains(kind, channel) == on {
			return PreferenceEdit::Unchanged;
		}
		let full = self.favorites.len() + self.pinned.len() + self.collapsed_categories.len()
			== Self::MAX_ENTRIES;
		let ids = match kind {
			Shortcut::Pinned => &mut self.pinned,
			Shortcut::Favorite => &mut self.favorites,
		};
		if on {
			if full {
				return PreferenceEdit::CapacityReached;
			}
			ids.insert(0, channel);
		} else {
			ids.retain(|id| *id != channel);
		}
		PreferenceEdit::Changed
	}

	pub fn category_collapsed(&self, category: Id) -> bool {
		self.collapsed_categories.contains(&category)
	}

	/// Sidebar label for the pinned-DM section; the default while blank.
	pub fn pinned_heading(&self) -> &str {
		let trimmed = self.pinned_name.trim();
		if trimmed.is_empty() {
			Self::DEFAULT_PINNED_NAME
		} else {
			trimmed
		}
	}

	/// Pinned-DM heading colour channels, if the owner overrode the theme default.
	pub fn pinned_color_rgb(&self) -> Option<[u8; 3]> {
		self.pinned_color
			.map(|color| [(color >> 16) as u8, (color >> 8) as u8, color as u8])
	}

	/// Idempotent. Control characters are dropped and the rest truncated; true on change.
	pub fn set_pinned_name(&mut self, name: &str) -> bool {
		if !self.is_valid() {
			return false;
		}
		let clean: String = name
			.chars()
			.filter(|c| !c.is_control())
			.take(Self::MAX_PINNED_NAME_CHARS)
			.collect();
		if clean == self.pinned_name {
			return false;
		}
		self.pinned_name = clean;
		true
	}

	/// Idempotent. Colours outside sRGB are masked down; true on change.
	pub fn set_pinned_color(&mut self, color: Option<[u8; 3]>) -> bool {
		if !self.is_valid() {
			return false;
		}
		let color =
			color.map(|[r, g, b]| (r as u32) << 16 | (g as u32) << 8 | b as u32);
		if color == self.pinned_color {
			return false;
		}
		self.pinned_color = color;
		true
	}

	pub fn set_category_collapsed(&mut self, category: Id, collapsed: bool) -> PreferenceEdit {
		if category.0 == 0 || !self.is_valid() || self.category_collapsed(category) == collapsed {
			return PreferenceEdit::Unchanged;
		}
		if collapsed {
			if self.favorites.len() + self.pinned.len() + self.collapsed_categories.len()
				== Self::MAX_ENTRIES
			{
				return PreferenceEdit::CapacityReached;
			}
			self.collapsed_categories.push(category);
		} else {
			self.collapsed_categories.retain(|id| *id != category);
		}
		PreferenceEdit::Changed
	}

	/// Drops a confirmed-deleted channel from every local navigation list.
	pub fn forget(&mut self, channel: Id) -> bool {
		let before = self.favorites.len() + self.pinned.len() + self.collapsed_categories.len();
		self.favorites.retain(|id| *id != channel);
		self.pinned.retain(|id| *id != channel);
		self.collapsed_categories.retain(|id| *id != channel);
		before != self.favorites.len() + self.pinned.len() + self.collapsed_categories.len()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pinned_heading_and_color_are_sanitized_and_idempotent() {
		let mut prefs = ChannelPreferences::default();
		assert_eq!(prefs.pinned_heading(), "Pinned");
		assert_eq!(prefs.pinned_color_rgb(), None);
		assert!(prefs.set_pinned_name("Starred"));
		assert!(!prefs.set_pinned_name("Starred"));
		assert_eq!(prefs.pinned_heading(), "Starred");
		assert!(prefs.set_pinned_name("  spaced  "));
		assert_eq!(prefs.pinned_heading(), "spaced");
		assert!(prefs.set_pinned_name("a\n\tb"));
		assert_eq!(prefs.pinned_name, "ab");
		let long = "x".repeat(ChannelPreferences::MAX_PINNED_NAME_CHARS + 5);
		assert!(prefs.set_pinned_name(&long));
		assert_eq!(
			prefs.pinned_name.chars().count(),
			ChannelPreferences::MAX_PINNED_NAME_CHARS
		);
		assert!(prefs.is_valid());
		assert!(prefs.set_pinned_color(Some([0x12, 0x34, 0x56])));
		assert_eq!(prefs.pinned_color, Some(0x12_34_56));
		assert_eq!(prefs.pinned_color_rgb(), Some([0x12, 0x34, 0x56]));
		assert!(!prefs.set_pinned_color(Some([0x12, 0x34, 0x56])));
		assert!(prefs.set_pinned_color(None));
		assert_eq!(prefs.pinned_color, None);
		assert!(prefs.is_valid());
	}
}
