//! Read-only navigation queries over `State`, shaped for Solace's sidebar and header.
use client_core::State;
use model::{Channel, Guild, Id, MemberSlot};

pub fn is_text(kind: u8) -> bool {
	matches!(kind, 0 | 5 | 15 | 16)
}
pub fn is_voice(kind: u8) -> bool {
	matches!(kind, 2 | 13)
}
pub fn is_dm(channel: &Channel) -> bool {
	channel.guild.is_none() && matches!(channel.kind, 1 | 3)
}

/// Communities in the account's order.
pub fn communities(state: &State) -> impl Iterator<Item = &Guild> {
	state.guilds.iter()
}

pub struct Section<'a> {
	pub category: Option<&'a Channel>,
	pub channels: Vec<&'a Channel>,
}

/// Visible channels of a community grouped by category, in service order.
pub fn sections(state: &State, guild: Id) -> Vec<Section<'_>> {
	let visible = |c: &&Channel| {
		c.guild == Some(guild) && (is_text(c.kind) || is_voice(c.kind)) && state.can_view(c.id)
	};
	let mut categories: Vec<&Channel> = state
		.channels
		.iter()
		.filter(|c| c.guild == Some(guild) && c.kind == 4)
		.collect();
	categories.sort_by_key(|c| (c.position, c.id));
	let order = |c: &&Channel| (is_voice(c.kind), c.position, c.id);
	let mut loose: Vec<&Channel> = state
		.channels
		.iter()
		.filter(visible)
		.filter(|c| {
			c.parent_id
				.is_none_or(|parent| !categories.iter().any(|cat| cat.id == parent))
		})
		.collect();
	loose.sort_by_key(order);
	let mut sections = Vec::new();
	if !loose.is_empty() {
		sections.push(Section {
			category: None,
			channels: loose,
		});
	}
	for category in categories {
		let mut channels: Vec<&Channel> = state
			.channels
			.iter()
			.filter(visible)
			.filter(|c| c.parent_id == Some(category.id))
			.collect();
		if channels.is_empty() {
			continue;
		}
		channels.sort_by_key(order);
		sections.push(Section {
			category: Some(category),
			channels,
		});
	}
	sections
}

/// First readable text channel of a community, for landing when it is chosen.
pub fn landing(state: &State, guild: Id) -> Option<Id> {
	sections(state, guild)
		.into_iter()
		.flat_map(|s| s.channels)
		.find(|c| is_text(c.kind))
		.map(|c| c.id)
}

pub fn voice_rooms(state: &State, guild: Id) -> Vec<&Channel> {
	sections(state, guild)
		.into_iter()
		.flat_map(|s| s.channels)
		.filter(|c| is_voice(c.kind))
		.collect()
}

/// Direct and group conversations, most recently active first.
pub fn direct(state: &State) -> Vec<&Channel> {
	let mut channels: Vec<&Channel> = state
		.channels
		.iter()
		.filter(|c| is_dm(c) && state.can_view(c.id))
		.collect();
	channels.sort_by_key(|c| std::cmp::Reverse(c.last_message.unwrap_or(c.id)));
	channels
}

pub fn selected_channel(state: &State) -> Option<&Channel> {
	state.channel(state.selected?)
}

pub fn selected_guild(state: &State) -> Option<&Guild> {
	let guild = selected_channel(state)?.guild?;
	state.guild(guild)
}

/// Category name for the selected channel (thread parents are followed once).
pub fn category_of<'a>(state: &'a State, channel: &Channel) -> Option<&'a Channel> {
	let parent = state.channel(channel.parent_id?)?;
	if parent.kind == 4 {
		Some(parent)
	} else {
		state.channel(parent.parent_id?).filter(|c| c.kind == 4)
	}
}

pub fn guild_unread(state: &State, guild: Id) -> bool {
	state.channels.iter().any(|c| {
		c.guild == Some(guild)
			&& is_text(c.kind)
			&& state.channel_unread(c) == Some(true)
			&& state.can_view(c.id)
	})
}

pub fn channel_unread(state: &State, channel: &Channel) -> bool {
	state.channel_unread(channel) == Some(true)
}

/// Loaded people of the open member list (guild members or conversation participants).
pub fn people(state: &State) -> Vec<&model::Member> {
	state
		.members
		.as_ref()
		.map(|list| {
			list.slots
				.iter()
				.flatten()
				.filter_map(|slot| match slot {
					MemberSlot::Person(member) => Some(member),
					MemberSlot::Group(_) => None,
				})
				.collect()
		})
		.unwrap_or_default()
}

/// (total members, online) when the service reported them for the selected list.
pub fn member_counts(state: &State, guild: Id) -> Option<(u64, u64)> {
	let list = state.members.as_ref().filter(|l| l.guild == Some(guild))?;
	let offline = list
		.groups
		.iter()
		.find(|(group, _)| group == "offline")
		.map_or(0, |(_, n)| *n);
	let grouped: u64 = list.groups.iter().map(|(_, n)| *n).sum();
	let total = list.total.max(grouped);
	Some((total, grouped.saturating_sub(offline)))
}

/// "12.4k" style compact counts.
pub fn compact(n: u64) -> String {
	match n {
		0..=999 => n.to_string(),
		1_000..=999_999 => {
			let v = n as f64 / 1000.0;
			if v < 10.0 {
				format!("{v:.1}k")
			} else {
				format!("{}k", v.round())
			}
		}
		_ => format!("{:.1}M", n as f64 / 1_000_000.0),
	}
}

/// Presence status for a user from DM presences or the open member list.
pub fn status_of(state: &State, user: Id) -> Option<&str> {
	if let Some(presence) = state.presence_for(user) {
		return presence.status.as_deref();
	}
	people(state)
		.into_iter()
		.find(|m| m.user.id == user)
		.and_then(|m| m.status.as_deref())
}

pub fn custom_status_of(state: &State, user: Id) -> Option<&str> {
	state
		.presence_for(user)
		.and_then(|p| p.custom_status.as_deref())
		.filter(|s| !s.is_empty())
}

/// Recently active conversations across communities and direct messages, newest first:
/// every direct conversation, plus community channels you opened or that have unread news.
pub fn recent(state: &State, limit: usize) -> Vec<&Channel> {
	let mut channels: Vec<&Channel> = state
		.channels
		.iter()
		.filter(|c| state.can_view(c.id))
		.filter(|c| {
			is_dm(c)
				|| (is_text(c.kind)
					&& c.guild.is_some()
					&& (state.last_viewed_channels.iter().any(|(_, id)| *id == c.id)
						|| channel_unread(state, c)))
		})
		.collect();
	channels.sort_by_key(|c| std::cmp::Reverse(c.last_message.unwrap_or(Id(0))));
	channels.truncate(limit);
	channels
}

/// Voice rooms with people in them right now, busiest first.
pub fn live_rooms(state: &State) -> Vec<(&Channel, Vec<Id>)> {
	let mut rooms: Vec<(&Channel, Vec<Id>)> = Vec::new();
	for entry in &state.voice.roster {
		let Some(channel) = state.channel(entry.channel) else {
			continue;
		};
		match rooms.iter_mut().find(|(c, _)| c.id == channel.id) {
			Some((_, people)) => people.push(entry.participant.user),
			None => rooms.push((channel, vec![entry.participant.user])),
		}
	}
	rooms.sort_by_key(|(_, people)| std::cmp::Reverse(people.len()));
	rooms
}

/// Friends and direct-message contacts who are online, with their status.
pub fn online_people(state: &State) -> Vec<(model::User, String)> {
	let mut people: Vec<(model::User, String)> = Vec::new();
	let candidates = state.friends().cloned().chain(
		state
			.channels
			.iter()
			.filter(|c| c.guild.is_none() && c.kind == 1)
			.flat_map(|c| c.recipients.iter().cloned()),
	);
	for user in candidates {
		if people.iter().any(|(u, _)| u.id == user.id) {
			continue;
		}
		if let Some(status) = state
			.presence_for(user.id)
			.and_then(|p| p.status.clone())
			.filter(|s| s != "offline")
		{
			people.push((user, status));
		}
	}
	people.sort_by_key(|(_, status)| match status.as_str() {
		"online" => 0,
		"dnd" => 1,
		_ => 2,
	});
	people
}

/// The direct conversation with `user`, if one exists.
pub fn dm_with(state: &State, user: Id) -> Option<Id> {
	state
		.channels
		.iter()
		.find(|c| c.guild.is_none() && c.kind == 1 && c.recipients.iter().any(|r| r.id == user))
		.map(|c| c.id)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn compact_counts_read_naturally() {
		assert_eq!(compact(8), "8");
		assert_eq!(compact(12_400), "12k");
		assert_eq!(compact(8_200), "8.2k");
		assert_eq!(compact(2_500_000), "2.5M");
	}

	#[test]
	fn fixture_navigation_groups_channels_under_categories() {
		let state = test_support::demo_state();
		let guild = state.guilds[0].id;
		let sections = sections(&state, guild);
		assert!(!sections.is_empty());
		assert!(landing(&state, guild).is_some());
		assert!(direct(&state).iter().all(|c| c.guild.is_none()));
	}
}
