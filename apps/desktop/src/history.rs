//! Session glue between Gateway events, the active timeline and the local cache: which
//! events change access or history, how cached pages hydrate, and draft recovery.
use crate::cache;
use client_core::{Event, State};
use model::Delivery;

/// Check only navigation whose effective access can change with this event.
pub fn access_candidates(state: &State, event: &Event) -> Vec<model::Id> {
	use client_core::permissions::Event as Permission;
	let (guilds, channel): (Option<Vec<model::Id>>, Option<model::Id>) = match event {
		Event::Startup(_) | Event::Ready { .. } | Event::Permissions(Permission::Snapshot(_)) => {
			(None, None)
		}
		Event::Permissions(permission) => match permission {
			Permission::Guild(guild) => (Some(vec![guild.id]), None),
			Permission::Role { guild, .. }
			| Permission::RoleRemoved { guild, .. }
			| Permission::Member { guild, .. }
			| Permission::Owner { guild, .. }
			| Permission::UnavailableGuild(guild) => (Some(vec![*guild]), None),
			Permission::Members(members) => (Some(members.iter().map(|m| m.0).collect()), None),
			Permission::Channel { channel, .. } => (None, Some(*channel)),
			Permission::Snapshot(_) => unreachable!(),
		},
		Event::ChannelCreated(channel) | Event::ChannelRestored(channel) => {
			// A new channel cannot revoke existing access. Duplicate IDs can replace metadata.
			if state.channel(channel.id).is_none() {
				return Vec::new();
			}
			(None, Some(channel.id))
		}
		Event::ChannelChanged(patch) | Event::ThreadChanged { patch, .. } => (None, Some(patch.id)),
		Event::ThreadRemoved { id, .. } => (None, Some(*id)),
		Event::ChannelAction(client_core::channel_actions::Event::Finished {
			channel,
			result:
				Ok(
					client_core::channel_actions::Outcome::Deleted
					| client_core::channel_actions::Outcome::Channel { .. },
				),
			..
		}) => (None, Some(*channel)),
		Event::UserAction(client_core::user_actions::Event::Written {
			action: client_core::user_actions::Action::CloseDm(channel),
			result: Ok(()),
			..
		}) => (None, Some(*channel)),
		Event::GroupAction(client_core::group_actions::Event::Written {
			channel,
			result: Ok(None),
			..
		}) => (None, Some(*channel)),
		Event::ServerAction(client_core::server_actions::Event::Written {
			action: client_core::server_actions::Action::Leave(guild),
			result: Ok(None),
			..
		}) => (Some(vec![*guild]), None),
		Event::ThreadsSync { guild, .. } => (Some(vec![*guild]), None),
		_ => return Vec::new(),
	};
	state
		.channels
		.iter()
		.filter(|c| {
			c.supports_text()
				&& channel.is_none_or(|id| c.id == id || c.parent_id == Some(id))
				&& guilds
					.as_ref()
					.is_none_or(|ids| c.guild.is_some_and(|id| ids.contains(&id)))
		})
		.map(|c| c.id)
		.collect()
}
pub fn user_action_notice(event: &Event) -> Option<(solace_ui::Level, &'static str)> {
	let Event::UserAction(client_core::user_actions::Event::Written { action, result, .. }) = event
	else {
		return None;
	};
	Some(match result {
		Ok(()) if matches!(action, client_core::user_actions::Action::OpenDm(_)) => {
			(solace_ui::Level::Error, action.completion_label())
		}
		Ok(()) => (solace_ui::Level::Success, action.completion_label()),
		Err(failure) => (solace_ui::Level::Error, failure.label()),
	})
}
pub fn wants_cached_history(state: &State, channel: model::Id, request: u64) -> bool {
	state.selected == Some(channel)
		&& state.request == request
		&& state.history_pending
		&& state.freshness == model::Freshness::Loading
		&& state.timeline.row_count() == 0
		&& state.can_read_history(channel)
		&& state
			.channels
			.iter()
			.any(|c| c.id == channel && c.supports_text())
}
pub fn hydrate_cached_history(
	state: &mut State,
	channel: model::Id,
	request: u64,
	messages: Vec<model::Message>,
) {
	if wants_cached_history(state, channel, request)
		&& messages.iter().all(|message| message.channel == channel)
		&& state.timeline.seed_cache(messages).is_ok()
	{
		state.revision += 1;
	}
	state.enforce_resident_budget();
}
pub fn hydrate_cache_result(
	state: &mut State,
	safety: &cache::HistorySafety,
	outcome: cache::Outcome,
) {
	if let cache::Outcome::Channel {
		channel,
		request,
		messages,
		epoch,
	} = outcome
		&& safety.allows(epoch)
	{
		hydrate_cached_history(state, channel, request, messages);
	}
}

pub fn recovery_draft(state: &State, channel: model::Id) -> String {
	state
		.drafts
		.get(&channel)
		.filter(|text| !text.is_empty())
		.cloned()
		.or_else(|| {
			state
				.pending
				.iter()
				.rev()
				.find(|pending| {
					pending.channel == channel && pending.delivery != Delivery::Confirmed
				})
				.map(|pending| pending.content.clone())
		})
		.unwrap_or_default()
}
pub fn confirmed_recovery_channel(state: &State, event: &Event) -> Option<model::Id> {
	let (message, nonce) = match event {
		Event::SendResult {
			result: Ok(message),
			nonce,
		} => (message, nonce.as_str()),
		Event::Message(message) => (message, message.nonce.as_deref()?),
		_ => return None,
	};
	if state.user.as_ref().map(|user| user.id) != Some(message.author.id) {
		return None;
	}
	state
		.pending
		.iter()
		.any(|pending| pending.channel == message.channel && pending.nonce == nonce)
		.then_some(message.channel)
}
pub fn changes_active_history(state: &State, event: &Event) -> bool {
	let channel = match event {
		Event::History {
			channel, request, ..
		} if *request == state.request && state.history_pending => channel,
		Event::Message(message)
		| Event::SendResult {
			result: Ok(message),
			..
		} => &message.channel,
		Event::ServerAction(client_core::server_actions::Event::InviteSent {
			result: Ok(sent),
			..
		}) => &sent.1.channel,
		Event::Patch(patch) => &patch.channel,
		Event::Edited { channel, .. }
		| Event::Delete { channel, .. }
		| Event::DeleteBulk { channel, .. } => channel,
		_ => return false,
	};
	state.selected == Some(*channel)
}
#[cfg(test)]
mod tests {
	use super::*;
	use client_core::{Command, Envelope};
	#[test]
	fn user_action_results_use_toasts() {
		let success = Event::UserAction(client_core::user_actions::Event::Written {
			action: client_core::user_actions::Action::Nickname {
				user: model::Id(2),
				text: "Synthetic".into(),
			},
			request: 1,
			result: Ok(()),
		});
		let (level, text) = user_action_notice(&success).unwrap();
		assert!(matches!(level, solace_ui::Level::Success));
		assert_eq!(text, "Nickname saved");
	}
	#[test]
	fn disk_cache_does_not_replace_resident_previews_or_deleted_positions() {
		for deleted in [false, true] {
			let mut state = test_support::demo_state();
			let alpha = state.selected.unwrap();
			let beta = model::Id(900);
			let mut other = state
				.channels
				.iter()
				.find(|c| c.id == alpha)
				.unwrap()
				.clone();
			other.id = beta;
			other.guild = None;
			other.kind = 1;
			state.channels.push(other);
			state.timeline.clear();
			state
				.timeline
				.insert(test_support::message(1001, alpha), false, false)
				.unwrap();
			if deleted {
				state.timeline.delete(model::Id(1001)).unwrap();
			}
			state.select(beta).unwrap();
			state.apply(Envelope {
				generation: state.generation,
				event: Event::History {
					channel: beta,
					request: state.request,
					older: false,
					messages: vec![test_support::message(1002, beta)],
				},
			});
			state.select(alpha).unwrap();
			let request = state.request;
			assert_eq!(state.timeline.row_count(), 1);
			assert!(!wants_cached_history(&state, alpha, request));
			hydrate_cached_history(
				&mut state,
				alpha,
				request,
				vec![test_support::message(1003, alpha)],
			);
			assert_eq!(
				state.timeline.row_ids().collect::<Vec<_>>(),
				[model::Id(1001)]
			);
			assert_eq!(state.timeline.is_empty(), deleted);
			assert_eq!(state.freshness, model::Freshness::Loading);
			state.clear_cached_history();
			assert_eq!(
				state.timeline.row_count(),
				1,
				"Clear keeps the displayed conversation"
			);
			state.select(beta).unwrap();
			assert_eq!(
				state.timeline.row_count(),
				0,
				"Cleared dormant windows cannot return"
			);
			assert!(wants_cached_history(&state, beta, state.request));
		}
	}
	#[test]
	fn delayed_cache_results_cannot_hydrate_after_a_known_deletion() {
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		state.timeline.clear();
		state.history(None);
		let request = state.request;
		let safety = cache::HistorySafety::default();
		let outcome = |epoch| cache::Outcome::Channel {
			channel,
			request,
			epoch,
			messages: vec![test_support::message(1000, channel)],
		};
		let old_epoch = safety.epoch();
		safety.invalidate();
		hydrate_cache_result(&mut state, &safety, outcome(old_epoch));
		assert!(state.timeline.is_empty());
		safety.block();
		hydrate_cache_result(&mut state, &safety, outcome(safety.epoch()));
		assert!(state.timeline.is_empty());
		safety.cleared();
		hydrate_cache_result(&mut state, &safety, outcome(safety.epoch()));
		assert_eq!(state.timeline.len(), 1);
	}

	#[test]
	fn cached_history_requires_current_readable_navigation_and_pending_request() {
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		state.timeline.clear();
		state.history(None);
		let request = state.request;
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert_eq!(state.timeline.len(), 1);
		assert_eq!(state.freshness, model::Freshness::Loading);
		state.timeline.clear();
		let permissions = state.permissions.clone();
		state.permissions.channels.remove(&channel);
		state.permissions.clear_cache();
		assert!(!state.can_read_history(channel));
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(
			state.timeline.is_empty(),
			"Loaded navigation alone does not authorize cached history"
		);
		state.permissions = permissions;
		for invalid in [
			vec![test_support::message(1001, model::Id(999))],
			vec![test_support::message(1002, channel)],
		] {
			hydrate_cached_history(&mut state, channel, request.wrapping_sub(1), invalid);
			assert!(state.timeline.is_empty());
		}
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1001, model::Id(999))],
		);
		assert!(state.timeline.is_empty());
		state.history_pending = false;
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
		state.history_pending = true;
		let mut channels = state.channels.clone();
		channels.retain(|c| c.id != channel);
		state.apply(Envelope {
			generation: state.generation,
			event: Event::Ready {
				user: state.user.clone().unwrap(),
				guilds: state.guilds.clone(),
				permissions: test_support::permission_snapshot(&state),
				channels,
			},
		});
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
		// Even an inconsistent queued-cache admission state cannot bypass current navigation.
		state.selected = Some(channel);
		state.request = request;
		state.freshness = model::Freshness::Loading;
		state.history_pending = true;
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
		let mut restored = test_support::demo_state()
			.channels
			.into_iter()
			.find(|c| c.id == channel)
			.unwrap();
		restored.kind = 4; // Categories do not support text; voice channels do.
		state.channels.push(restored);
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
	}
	#[test]
	fn correlated_confirmation_preserves_other_pending_text_and_ignores_unrelated_events() {
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		state.drafts.insert(channel, "first pending draft".into());
		let Command::Send { nonce, .. } = state.prepare_send().unwrap() else {
			panic!()
		};
		state.drafts.insert(channel, "second pending draft".into());
		state.prepare_send().unwrap();
		let mut message = test_support::message(1000, channel);
		message.author = state.user.clone().unwrap();
		message.nonce = Some(nonce.clone());
		let mut foreign = message.clone();
		foreign.author.id = model::Id(999);
		assert!(confirmed_recovery_channel(&state, &Event::Message(foreign)).is_none());
		let confirmation = Event::SendResult {
			nonce,
			result: Ok(message),
		};
		assert_eq!(
			confirmed_recovery_channel(&state, &confirmation),
			Some(channel)
		);
		state.apply(Envelope {
			generation: state.generation,
			event: confirmation,
		});
		assert_eq!(recovery_draft(&state, channel), "second pending draft");
		state.drafts.insert(channel, String::new());
		assert_eq!(recovery_draft(&state, channel), "second pending draft");
		state.drafts.insert(channel, "new unsent edit".into());
		assert_eq!(recovery_draft(&state, channel), "new unsent edit");
		assert!(!changes_active_history(
			&state,
			&Event::Message(test_support::message(1001, model::Id(999)))
		));
		assert!(!changes_active_history(
			&state,
			&Event::History {
				channel,
				request: state.request.wrapping_sub(1),
				older: false,
				messages: vec![]
			}
		));
		assert!(changes_active_history(
			&state,
			&Event::DeleteBulk {
				channel,
				ids: vec![model::Id(1000)]
			}
		));
	}
}
