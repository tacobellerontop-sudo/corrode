//! Offline showcase (`--demo`): synthetic communities, people and artwork that exercise the
//! whole interface without an account, network, or disk cache. Nothing here is real data.
use client_core::{Envelope, Event, State, voice};
use eframe::egui::{self, Color32, ColorImage};
use model::{
	Attachment, Channel, EmbedMedia, Guild, Id, Member, MemberSlot, Message, Reaction,
	ReactionEmoji, User,
};
use solace_ui::{Solace, clock::snowflake_at, images, scenery};

const HORIZON: Id = Id(10);
const BETTER_TOGETHER: Id = Id(301);
const HANGOUT: Id = Id(306);
const STUDIO: Id = Id(507);

fn user(id: u64, name: &str) -> User {
	User {
		id: Id(id),
		name: name.into(),
		avatar: None,
		discriminator: 0,
		kind: Default::default(),
		webhook: false,
		primary_guild: None,
	}
}

fn hash(n: u64) -> String {
	format!("{n:032x}")
}

fn channel(
	id: u64,
	guild: Option<Id>,
	parent: Option<u64>,
	position: i32,
	name: &str,
	kind: u8,
) -> Channel {
	Channel {
		icon: None,
		last_message: None,
		id: Id(id),
		guild,
		parent_id: parent.map(Id),
		position,
		name: name.into(),
		kind,
		recipients: vec![],
		member_list_id: guild.map(|_| "everyone".into()),
		message_count: None,
	}
}

struct People {
	me: User,
	lina: User,
	kai: User,
	jun: User,
	sophie: User,
	marco: User,
	avery: User,
}

fn people() -> People {
	People {
		me: user(1, "River"),
		lina: user(101, "Lina"),
		kai: user(102, "Kai"),
		jun: user(103, "Jun"),
		sophie: user(104, "Sophie"),
		marco: user(105, "Marco"),
		avery: user(106, "Avery"),
	}
}

fn image_attachment(id: u64, name: &str, width: u32, height: u32) -> Attachment {
	Attachment {
		id: Id(id),
		filename: name.into(),
		description: None,
		content_type: Some("image/png".into()),
		size: 240_000,
		media: EmbedMedia {
			url: Some(format!(
				"https://cdn.discordapp.com/attachments/301/{id}/{name}"
			)),
			proxy_url: Some(format!(
				"https://media.discordapp.net/attachments/301/{id}/{name}"
			)),
			width,
			height,
			placeholder: vec![],
		},
		spoiler: false,
		duration_ms: None,
		waveform: vec![],
	}
}

fn reaction(emoji: &str, count: u32, me: bool) -> Reaction {
	Reaction {
		emoji: ReactionEmoji {
			id: None,
			name: Some(emoji.into()),
		},
		count,
		me,
		me_burst: false,
	}
}

fn message(id: Id, channel: Id, author: &User, content: &str) -> Message {
	let mut message = test_support::message(1, channel);
	message.id = id;
	message.author = author.clone();
	message.author_nick = None;
	message.author_roles = vec![];
	message.content = content.into();
	message.mentions = vec![];
	message.reactions = Some(vec![]);
	message.reply_to = None;
	message.attachments = vec![];
	message.embeds = vec![];
	message.edited = false;
	message.nonce = None;
	message
}

/// The offline showcase state, shaped like the Solace reference composition.
pub fn state() -> State {
	let people = people();
	let mut state = State {
		demo: true,
		..State::default()
	};
	let communities = [
		(10, "The Horizon"),
		(11, "Design & Creatives"),
		(12, "Tech & Innovation"),
		(13, "Books & Ideas"),
		(14, "Music & Sounds"),
		(15, "Wellness & Lifestyle"),
	];
	let guilds = communities
		.iter()
		.map(|(id, name)| Guild {
			stickers: None,
			emojis: Some(vec![]),
			id: Id(*id),
			name: (*name).into(),
			icon: Some(hash(*id)),
		})
		.collect();
	let mut channels = vec![
		channel(201, Some(HORIZON), None, 0, "CREATIVITY", 4),
		channel(202, Some(HORIZON), None, 1, "TECHNOLOGY", 4),
		channel(203, Some(HORIZON), None, 2, "LOUNGE", 4),
		channel(301, Some(HORIZON), Some(201), 0, "better-together", 0),
		channel(302, Some(HORIZON), Some(201), 1, "moodboards", 0),
		channel(303, Some(HORIZON), Some(201), 2, "showcase", 0),
		channel(304, Some(HORIZON), Some(202), 0, "engineering", 0),
		channel(305, Some(HORIZON), Some(202), 1, "tools-and-tips", 0),
		channel(307, Some(HORIZON), Some(203), 0, "general", 0),
		channel(306, Some(HORIZON), Some(203), 1, "The Horizon Hangout", 2),
		channel(308, Some(HORIZON), Some(203), 2, "Quiet Focus", 2),
	];
	for (id, _) in communities.iter().skip(1) {
		channels.push(channel(400 + id, Some(Id(*id)), None, 0, "general", 0));
		channels.push(channel(420 + id, Some(Id(*id)), None, 1, "Lounge", 2));
	}
	let dms = [
		(501, &people.avery, 40),
		(502, &people.kai, 120),
		(503, &people.jun, 300),
		(504, &people.lina, 900),
		(505, &people.marco, 720),
		(506, &people.sophie, 1800),
	];
	for (id, peer, minutes_ago) in dms {
		let mut dm = channel(id, None, None, 0, &peer.name, 1);
		dm.recipients = vec![peer.clone()];
		let age = clock_minutes_ago(minutes_ago);
		dm.last_message = Some(age);
		channels.push(dm);
	}
	let mut studio = channel(STUDIO.0, None, None, 0, "Studio crew", 3);
	studio.recipients = vec![
		people.lina.clone(),
		people.kai.clone(),
		people.jun.clone(),
		people.avery.clone(),
	];
	studio.last_message = Some(clock_minutes_ago(8));
	channels.push(studio);
	state.apply(Envelope {
		generation: state.generation,
		event: Event::Ready {
			permissions: model::permissions::Snapshot::default(),
			user: people.me.clone(),
			guilds,
			channels,
		},
	});
	let snapshot = test_support::permission_snapshot(&state);
	state
		.permissions
		.replace(snapshot)
		.expect("synthetic permissions");

	let _ = state.select(BETTER_TOGETHER);
	let mut history = Vec::new();
	let yesterday = [
		(
			9,
			12,
			&people.marco,
			"Morning all ☀️ Coffee and sketches before the week gets busy.",
		),
		(
			9,
			40,
			&people.sophie,
			"Sharing a calm palette I keep returning to: sand, sage and a little dusk blue.",
		),
		(
			16,
			5,
			&people.jun,
			"Quick reminder that the **Thursday critique** is open to everyone — bring something unfinished!",
		),
	];
	for (i, (h, m, author, text)) in yesterday.into_iter().enumerate() {
		history.push(message(
			snowflake_at(1, h, m, i as u64),
			BETTER_TOGETHER,
			author,
			text,
		));
	}
	let concept_id = snowflake_at(0, 10, 24, 1);
	let mut concept = message(
		concept_id,
		BETTER_TOGETHER,
		&people.lina,
		"Just finished the new concept for the community site. Would love your thoughts on the layout and overall vibe!",
	);
	concept.attachments = vec![image_attachment(9001, "concept.png", 1200, 600)];
	concept.reactions = Some(vec![reaction("❤️", 12, false), reaction("🙌", 4, true)]);
	history.push(concept);
	let reply = |id: Id, author: &User, text: &str| {
		let mut m = message(id, BETTER_TOGETHER, author, text);
		m.reply_to = Some(concept_id);
		m
	};
	history.push(reply(
		snowflake_at(0, 10, 32, 2),
		&people.kai,
		"Looks great! Love the clean layout.",
	));
	history.push(reply(
		snowflake_at(0, 10, 41, 3),
		&people.sophie,
		"The color palette feels so calm and intentional.",
	));
	let mut kai = message(
		snowflake_at(0, 10, 42, 4),
		BETTER_TOGETHER,
		&people.kai,
		"This looks amazing! The minimal layout really makes the content shine.\nMaybe we could try a slightly bolder accent color for the call-to-actions?",
	);
	kai.reactions = Some(vec![reaction("👍", 8, false)]);
	history.push(kai);
	history.push(message(
		snowflake_at(0, 10, 56, 5),
		BETTER_TOGETHER,
		&people.lina,
		"Good call! I'll try a few variations and share them in a thread.",
	));
	history.push(reply(
		snowflake_at(0, 10, 58, 6),
		&people.marco,
		"+1 on the bolder accent idea!",
	));
	let mut jun = message(
		snowflake_at(0, 11, 3, 7),
		BETTER_TOGETHER,
		&people.jun,
		"Also, here's a moodboard I put together for the visual style:",
	);
	jun.attachments = vec![
		image_attachment(9002, "mist.png", 640, 480),
		image_attachment(9003, "fern.png", 640, 480),
		image_attachment(9004, "dusk.png", 640, 480),
		image_attachment(9005, "arch.png", 640, 480),
	];
	jun.reactions = Some(vec![reaction("🔥", 5, false)]);
	history.push(jun);
	history.push(reply(
		snowflake_at(0, 11, 12, 8),
		&people.lina,
		"Thanks everyone! I'll update it and share the next version soon.",
	));
	state.apply(Envelope {
		generation: state.generation,
		event: Event::History {
			channel: BETTER_TOGETHER,
			request: state.request,
			older: false,
			messages: history,
		},
	});
	state.older_exhausted = true;

	// Presence for direct conversations.
	let statuses = [
		(&people.avery, "online"),
		(&people.kai, "online"),
		(&people.jun, "online"),
		(&people.lina, "online"),
		(&people.marco, "idle"),
		(&people.sophie, "online"),
	];
	state.direct_presences = statuses
		.iter()
		.map(|(user, status)| model::MemberPresence {
			user: user.id,
			status: Some((*status).into()),
			custom_status: None,
			activities: vec![],
		})
		.collect();

	// The community's member list, as the service would page it.
	let member = |user: &User, status: &str| {
		Some(MemberSlot::Person(Member {
			roles: vec![],
			user: user.clone(),
			nick: None,
			status: Some(status.into()),
			custom_status: None,
			activities: vec![],
		}))
	};
	state.members = Some(model::MemberList {
		guild: Some(HORIZON),
		channel: BETTER_TOGETHER,
		request: state.member_request,
		start: 0,
		slots: vec![
			Some(MemberSlot::Group("online".into())),
			member(&people.lina, "online"),
			member(&people.kai, "online"),
			member(&people.jun, "online"),
			member(&people.sophie, "online"),
			member(&people.marco, "online"),
			member(&people.me, "online"),
			Some(MemberSlot::Group("offline".into())),
			member(&people.avery, "idle"),
		],
		total: 12_400,
		lazy: true,
		freshness: model::Freshness::Fresh,
		groups: vec![("online".into(), 8), ("offline".into(), 12_392)],
		ranges: vec![[0, 99]],
	});

	// People already in the hangout.
	for person in [&people.sophie, &people.kai, &people.jun, &people.marco] {
		state.voice.roster.push(voice::RosterEntry {
			guild: HORIZON,
			channel: HANGOUT,
			participant: voice::Participant {
				user: person.id,
				muted: false,
				deafened: false,
				server_muted: false,
				server_deafened: false,
				video: false,
				streaming: false,
			},
			member: None,
		});
	}
	state.auth = client_core::auth::AuthState::Authenticated;
	state.gateway_connected = true;
	state.freshness = model::Freshness::Fresh;
	state.status = "Offline showcase · synthetic data only";
	state
}

fn clock_minutes_ago(minutes: i64) -> Id {
	const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64;
	let at = now.saturating_sub(minutes.max(0) as u64 * 60_000);
	Id((at.saturating_sub(DISCORD_EPOCH_MS)) << 22)
}

/// Give the showcase artwork: community icons, attachments, and a speaking indicator.
pub fn decorate(solace: &mut Solace, ctx: &egui::Context, state: &State) {
	solace.demo = true;
	for guild in &state.guilds {
		if let Some(key) = guild.icon_key() {
			let mood = if guild.id.0 % 2 == 0 {
				scenery::Mood::Dawn
			} else {
				scenery::Mood::Lake
			};
			let wide = scenery::render(192, 96, guild.id.0 * 7, mood);
			solace.images.insert(ctx, key, crop_square(&wide));
		}
	}
	for message in state.timeline.iter() {
		for attachment in &message.attachments {
			let Some(key) = images::media_key(&attachment.media) else {
				continue;
			};
			let image = match attachment.filename.as_str() {
				"concept.png" => arches(720, 360),
				"mist.png" => scenery::render(320, 240, 77, scenery::Mood::Lake),
				"fern.png" => fern(320, 240),
				"dusk.png" => dusk(320, 240),
				_ => arches(320, 240),
			};
			solace.images.insert(ctx, key, image);
		}
	}
	solace.media.voice_speaking = vec![Id(104), Id(102), Id(103)];
	solace.open_thread(
		state
			.timeline
			.iter()
			.find(|m| !m.attachments.is_empty() && m.author.id == Id(101))
			.map(|m| m.id),
	);
}

/// Open the "Studio crew" group call: four people, one sharing a screen and one whose stream
/// just ended. The offline showcase cannot place real calls, so the call is staged locally.
pub fn call_scene(state: &mut State, solace: &mut Solace) {
	let _ = state.select(STUDIO);
	let me = state.user.as_ref().map_or(Id(100), |u| u.id);
	let person = |user: u64, muted: bool, streaming: bool| voice::Participant {
		user: Id(user),
		muted,
		deafened: false,
		server_muted: false,
		server_deafened: false,
		video: false,
		streaming,
	};
	let lina = state
		.channel(STUDIO)
		.and_then(|c| c.recipients.first())
		.map_or(101, |u| u.id.0);
	let others: Vec<u64> = state
		.channel(STUDIO)
		.map(|c| c.recipients.iter().map(|u| u.id.0).collect())
		.unwrap_or_default();
	let mut participants = vec![person(me.0, false, false)];
	for (i, user) in others.iter().enumerate() {
		participants.push(person(*user, i == 2, *user == lina));
	}
	state.voice.active = Some(voice::Call {
		channel: STUDIO,
		guild: None,
		connected_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(754)),
		server_muted: false,
		server_deafened: false,
		request: 1,
		phase: voice::Phase::Connected,
		muted: false,
		deafened: false,
		participants,
		camera: false,
		watching: None,
		error: None,
	});
	solace.media.voice_speaking = others.iter().take(1).map(|u| Id(*u)).collect();
	solace
		.media
		.set_user_volume(Id(*others.get(1).unwrap_or(&0)), 140);
}

/// Stage the last caller's stream: the first call starts watching it, the second ends it.
/// The preview example uses this to show a stream that has ended.
#[allow(dead_code)]
pub fn end_stream(state: &mut State) {
	let Some(call) = state.voice.active.as_mut() else {
		return;
	};
	if let Some(user) = call.participants.last().map(|p| p.user) {
		if call.watching.is_none() {
			// First pass: they are live and being watched.
			if let Some(p) = call.participants.iter_mut().find(|p| p.user == user) {
				p.streaming = true;
			}
			call.watching = Some(user);
		} else {
			if let Some(p) = call.participants.iter_mut().find(|p| p.user == user) {
				p.streaming = false;
			}
			call.watching = None;
		}
	}
}

fn crop_square(image: &ColorImage) -> ColorImage {
	let side = image.size[0].min(image.size[1]);
	let x0 = (image.size[0] - side) / 2;
	let mut out = ColorImage::filled([side, side], Color32::BLACK);
	for y in 0..side {
		for x in 0..side {
			out.pixels[y * side + x] = image.pixels[y * image.size[0] + x0 + x];
		}
	}
	out
}

fn shade(base: [f32; 3], light: f32) -> Color32 {
	let c = |v: f32| (v * light).clamp(0.0, 255.0) as u8;
	Color32::from_rgb(c(base[0]), c(base[1]), c(base[2]))
}

/// Soft architectural render: cream arches and a stair, lit from the left.
fn arches(width: usize, height: usize) -> ColorImage {
	let mut image = ColorImage::filled([width, height], Color32::WHITE);
	let cream = [236.0, 229.0, 219.0];
	for y in 0..height {
		for x in 0..width {
			let u = x as f32 / width as f32;
			let v = y as f32 / height as f32;
			let mut light = 0.92 + 0.08 * (1.0 - u);
			// Two arched openings.
			for (cx, w) in [(0.58, 0.16), (0.82, 0.12)] {
				let dx = (u - cx) / w;
				let top = 0.28 + 0.12 * dx * dx;
				if dx.abs() < 1.0 && v > top {
					light *= 0.78 + 0.1 * v;
				}
			}
			// A curved wall sweeping in from the left.
			let wall = 0.18 + 0.25 * (v * 3.0).sin().abs() * 0.3;
			if u < wall {
				light *= 0.86 + u;
			}
			// Stairs at the lower right.
			if u > 0.62 && v > 0.62 {
				let step = ((u - 0.62) * 18.0 + (v - 0.62) * 22.0).floor();
				light *= if step as i32 % 2 == 0 { 0.95 } else { 0.88 };
			}
			image.pixels[y * width + x] = shade(cream, light.min(1.08));
		}
	}
	image
}

fn fern(width: usize, height: usize) -> ColorImage {
	let mut image = ColorImage::filled([width, height], Color32::WHITE);
	for y in 0..height {
		for x in 0..width {
			let u = x as f32 / width as f32;
			let v = y as f32 / height as f32;
			let base = [226.0 - 20.0 * v, 220.0 - 18.0 * v, 206.0 - 14.0 * v];
			let mut color = shade(base, 1.0);
			// A stem and paired fronds.
			let stem = 0.5 + 0.06 * (v * 3.0).sin();
			if (u - stem).abs() < 0.008 && v > 0.15 {
				color = Color32::from_rgb(92, 104, 80);
			}
			for k in 0..8 {
				let fy = 0.2 + k as f32 * 0.09;
				let len = 0.26 - k as f32 * 0.018;
				for side in [-1.0f32, 1.0] {
					let t = (u - stem) * side;
					if t > 0.0 && t < len {
						let curve = fy - t * 0.55 + (t * 9.0).sin() * 0.01;
						if (v - curve).abs() < 0.012 * (1.0 - t / len) + 0.004 {
							color = Color32::from_rgb(108 - (k * 3) as u8, 124 - (k * 2) as u8, 92);
						}
					}
				}
			}
			image.pixels[y * width + x] = color;
		}
	}
	image
}

fn dusk(width: usize, height: usize) -> ColorImage {
	let mut image = ColorImage::filled([width, height], Color32::WHITE);
	for y in 0..height {
		let v = y as f32 / height as f32;
		let top = [92.0, 128.0, 176.0];
		let bottom = [236.0, 184.0, 150.0];
		let c = |i: usize| top[i] + (bottom[i] - top[i]) * v.powf(1.4);
		for x in 0..width {
			image.pixels[y * width + x] = Color32::from_rgb(c(0) as u8, c(1) as u8, c(2) as u8);
		}
	}
	image
}

/// Offline answer to a search (`Some(query)`) or a pinned-messages request (`None`), drawn
/// from the loaded window of the open conversation. Messages with pictures stand in for pins.
#[allow(dead_code)]
pub fn search_page(state: &State, query: Option<&str>) -> model::SearchPage {
	let needle = query.map(|q| q.trim().to_lowercase());
	let mut hits: Vec<model::SearchHit> = state
		.timeline
		.display_iter()
		.filter(|m| !m.is_system())
		.filter(|m| match &needle {
			Some(needle) => m.content.to_lowercase().contains(needle.as_str()),
			None => !m.attachments.is_empty() || m.reply_to.is_none() && m.content.len() > 60,
		})
		.map(|m| model::SearchHit {
			id: m.id,
			channel: m.channel,
			author: m.author.clone(),
			excerpt: m.content.clone(),
			attachments: m.attachments.clone(),
			embeds: Vec::new(),
		})
		.collect();
	hits.sort_by_key(|hit| std::cmp::Reverse(hit.id));
	hits.truncate(model::SEARCH_PAGE_SIZE);
	model::SearchPage {
		total: hits.len() as u64,
		hits,
		partial: false,
		pin_cursor: None,
	}
}
