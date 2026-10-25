//! Snowflake and wall-clock formatting in the viewer's local zone.
use time::{OffsetDateTime, UtcOffset};

const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;

fn local(instant: OffsetDateTime) -> OffsetDateTime {
	let offset = if cfg!(test) {
		UtcOffset::UTC
	} else {
		UtcOffset::local_offset_at(instant).unwrap_or(UtcOffset::UTC)
	};
	instant.to_offset(offset)
}

pub fn now() -> OffsetDateTime {
	local(OffsetDateTime::now_utc())
}

/// Creation instant encoded in a snowflake.
pub fn snowflake(id: model::Id) -> OffsetDateTime {
	let ms = (id.0 >> 22) as i64 + DISCORD_EPOCH_MS;
	local(
		OffsetDateTime::from_unix_timestamp_nanos(ms as i128 * 1_000_000)
			.unwrap_or(OffsetDateTime::UNIX_EPOCH),
	)
}

/// A snowflake for a wall-clock time `days_ago` days back, in the viewer's zone. Used by the
/// offline showcase to build a believable timeline.
pub fn snowflake_at(days_ago: i64, hour: u8, minute: u8, sequence: u64) -> model::Id {
	let today = now();
	let at = today.replace_time(
		time::Time::from_hms(hour.min(23), minute.min(59), 0).unwrap_or(time::Time::MIDNIGHT),
	) - time::Duration::days(days_ago);
	let ms = (at.unix_timestamp_nanos() / 1_000_000) as i64 - DISCORD_EPOCH_MS;
	model::Id(((ms.max(0) as u64) << 22) | (sequence & 0x3f_ffff))
}

/// "10:24 AM"
pub fn clock(at: OffsetDateTime) -> String {
	let hour = at.hour();
	let (h, suffix) = match hour {
		0 => (12, "AM"),
		1..=11 => (hour, "AM"),
		12 => (12, "PM"),
		_ => (hour - 12, "PM"),
	};
	format!("{h}:{:02} {suffix}", at.minute())
}

/// "Today", "Yesterday" or "March 3, 2026".
pub fn day(at: OffsetDateTime) -> String {
	let today = now().date();
	let date = at.date();
	if date == today {
		"Today".into()
	} else if date.next_day() == Some(today) {
		"Yesterday".into()
	} else {
		format!("{} {}, {}", date.month(), date.day(), date.year())
	}
}

/// Compact age for list subtitles: "now", "12m", "3h", "2d", "Mar 3".
pub fn short_age(at: OffsetDateTime) -> String {
	let delta = now() - at;
	let minutes = delta.whole_minutes();
	if minutes < 1 {
		"now".into()
	} else if minutes < 60 {
		format!("{minutes}m")
	} else if minutes < 60 * 24 {
		format!("{}h", minutes / 60)
	} else if minutes < 60 * 24 * 7 {
		format!("{}d", minutes / (60 * 24))
	} else {
		let month = at.month().to_string();
		format!("{} {}", &month[..3], at.day())
	}
}

/// Render a `<t:seconds:style>` reference.
pub fn discord_timestamp(seconds: i64, style: u8) -> Option<String> {
	let instant = OffsetDateTime::from_unix_timestamp(seconds).ok()?;
	let at = local(instant);
	Some(match style {
		b't' => clock(at),
		b'T' => format!("{}:{:02}", clock(at), at.second()),
		b'd' => format!("{:02}/{:02}/{}", u8::from(at.month()), at.day(), at.year()),
		b'D' => format!("{} {}, {}", at.month(), at.day(), at.year()),
		b'R' => {
			let delta = instant - OffsetDateTime::now_utc();
			let minutes = delta.whole_minutes().abs();
			let amount = if minutes < 1 {
				"a few seconds".to_owned()
			} else if minutes < 60 {
				format!("{minutes} minutes")
			} else if minutes < 60 * 24 {
				format!("{} hours", minutes / 60)
			} else {
				format!("{} days", minutes / (60 * 24))
			};
			if delta.is_positive() {
				format!("in {amount}")
			} else {
				format!("{amount} ago")
			}
		}
		b'F' => format!(
			"{}, {} {}, {} {}",
			at.weekday(),
			at.month(),
			at.day(),
			at.year(),
			clock(at)
		),
		_ => format!("{} {}, {} {}", at.month(), at.day(), at.year(), clock(at)),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn formats_twelve_hour_clock_and_snowflakes() {
		let at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
		assert_eq!(clock(at), "10:13 PM");
		let id = model::Id(((1_700_000_000_000i64 - DISCORD_EPOCH_MS) as u64) << 22);
		assert_eq!(snowflake(id).unix_timestamp(), 1_700_000_000);
		assert_eq!(
			discord_timestamp(1_700_000_000, b'D').unwrap(),
			"November 14, 2023"
		);
	}
}
