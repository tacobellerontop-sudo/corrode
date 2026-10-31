//! Floating search and pinned-messages panel for the open conversation. Results come from
//! `State::search`; choosing one jumps the timeline to that message.
use crate::{
	Solace, clock,
	icons::Icon,
	paint::{self, AvatarShape, ButtonKind},
	text::{self, Names},
	theme,
};
use client_core::{Command, State};
use egui::{Align2, Key, Rect, Sense, TextEdit, Ui, pos2, vec2};
use model::Id;

const WIDTH: f32 = 340.0;
const ROW: f32 = 84.0;

pub(crate) struct Finder {
	pub pins: bool,
	pub query: String,
	/// The channel results were last asked for; a new conversation asks again.
	requested: Option<Id>,
	/// The last request could not be sent (offline, or no access to history here).
	unavailable: bool,
}

impl Finder {
	pub fn search() -> Self {
		Self {
			pins: false,
			query: String::new(),
			requested: None,
			unavailable: false,
		}
	}

	pub fn pins() -> Self {
		Self {
			pins: true,
			query: String::new(),
			requested: None,
			unavailable: false,
		}
	}
}

pub fn show(
	solace: &mut Solace,
	ui: &mut Ui,
	main: Rect,
	state: &mut State,
	commands: &mut Vec<Command>,
) {
	let Some(channel) = state.selected else {
		solace.finder = None;
		return;
	};
	// The panel takes the thread panel's place.
	solace.thread = None;
	let Some(finder) = solace.finder.as_mut() else {
		return;
	};
	let pins = finder.pins;
	if pins {
		let stale = state.pins_changed == Some(channel);
		let showing = state
			.search
			.as_ref()
			.is_some_and(|s| s.pins && s.channel == channel);
		if finder.requested != Some(channel) || stale || !showing && finder.requested.is_none() {
			finder.requested = Some(channel);
			if stale {
				state.pins_changed = None;
			}
			let command = state.request_pins();
			finder.unavailable = command.is_none();
			commands.extend(command);
		}
	}
	let p = theme::palette(ui.ctx());
	let a = crate::motion::appear(
		ui.ctx(),
		egui::Id::unique(("solace-finder-enter", pins)),
		0.26,
	);
	let rect = Rect::from_min_max(
		pos2(
			main.right() - 24.0 - WIDTH,
			main.top() + theme::size::HEADER + 8.0,
		),
		pos2(main.right() - 24.0, main.bottom() - 112.0),
	)
	.translate(vec2((1.0 - a) * 36.0, 0.0));
	let mut close = false;
	let mut submit = false;
	let mut open: Option<Id> = None;
	let mut more = false;
	egui::Area::new(egui::Id::unique("solace-finder"))
		.order(egui::Order::Middle)
		.fixed_pos(rect.min)
		.show(ui.ctx(), |ui| {
			ui.multiply_opacity(a);
			ui.set_clip_rect(rect.expand(60.0));
			let _ = ui.allocate_exact_size(rect.size(), Sense::click_and_drag());
			paint::floating(ui.painter(), rect, 16, &p);
			paint::label(
				ui.painter(),
				rect.left_top() + vec2(18.0, 28.0),
				Align2::LEFT_CENTER,
				if pins { "Pinned messages" } else { "Search" },
				theme::serif(17.0),
				p.text,
			);
			let x = Rect::from_center_size(rect.right_top() + vec2(-26.0, 28.0), vec2(28.0, 28.0));
			if paint::icon_button(
				ui,
				ui.scope_id().with("finder-close"),
				x,
				Icon::Close,
				ButtonKind::Ghost,
				false,
				"Close (Esc)",
			)
			.clicked()
			{
				close = true;
			}
			let mut top = rect.top() + 52.0;
			if !pins {
				let field = Rect::from_min_size(
					pos2(rect.left() + 14.0, top),
					vec2(rect.width() - 28.0, 38.0),
				);
				ui.painter().rect(
					field,
					11,
					p.canvas,
					egui::Stroke::new(1.0, p.border),
					egui::StrokeKind::Inside,
				);
				paint::icon_button(
					ui,
					ui.scope_id().with("finder-glass"),
					Rect::from_center_size(field.left_center() + vec2(18.0, 0.0), vec2(24.0, 24.0)),
					Icon::Search,
					ButtonKind::Ghost,
					false,
					"Search",
				);
				let edit =
					Rect::from_min_max(field.min + vec2(34.0, 0.0), field.max - vec2(10.0, 0.0));
				let id = egui::Id::unique("finder-input");
				let response = ui.put(
					edit,
					TextEdit::singleline(&mut finder.query)
						.id(id)
						.frame(egui::Frame::NONE)
						.font(theme::sans(13.5))
						.text_color(p.text)
						.align(Align2::LEFT_CENTER)
						.hint_text(
							egui::RichText::new("Search this conversation")
								.color(p.faint)
								.font(theme::sans(13.5)),
						)
						.desired_width(edit.width()),
				);
				if finder.requested.is_none() && !response.has_focus() {
					response.request_focus();
				}
				if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
					submit = true;
				}
				top = field.bottom() + 10.0;
			}
			let view = state
				.search
				.as_ref()
				.filter(|s| s.pins == pins && s.channel == channel);
			let status = match view {
				None if finder.unavailable => {
					Some("Not available right now. Check your connection.".to_owned())
				}
				None if pins => Some("Loading pinned messages...".to_owned()),
				None => Some("Press Enter to search messages here.".to_owned()),
				Some(v) if v.loading => Some(
					if pins {
						"Loading pinned messages..."
					} else {
						"Searching..."
					}
					.to_owned(),
				),
				Some(v) if v.error.is_some() => v.error.map(str::to_owned),
				Some(v) => match &v.page {
					Some(page) if page.hits.is_empty() => Some(
						if pins {
							"Nothing is pinned here yet."
						} else {
							"No messages match."
						}
						.to_owned(),
					),
					Some(page) if !pins => Some(format!(
						"{} result{}{}",
						page.total,
						if page.total == 1 { "" } else { "s" },
						if page.partial { " so far" } else { "" }
					)),
					_ => None,
				},
			};
			if let Some(status) = status {
				paint::text_left(
					ui.painter(),
					pos2(rect.left() + 18.0, top + 10.0),
					&status,
					theme::sans(12.0),
					p.muted,
					rect.width() - 36.0,
				);
				top += 26.0;
			}
			let Some(page) = view.and_then(|v| v.page.as_ref()) else {
				return;
			};
			// Search pages hold 25 hits; a full page may have older ones behind it.
			let can_more = if pins {
				page.pin_cursor.is_some()
			} else {
				page.hits.len() >= 25 && page.total > page.hits.len() as u64
			};
			let body = Rect::from_min_max(
				pos2(rect.left() + 8.0, top),
				pos2(rect.right() - 8.0, rect.bottom() - 10.0),
			);
			let mut child =
				ui.new_child(egui::UiBuilder::new().max_rect(body).id_salt("finder-body"));
			child.set_clip_rect(body);
			egui::ScrollArea::vertical()
				.id_salt(("finder-scroll", pins))
				.auto_shrink([false, false])
				.show(&mut child, |ui| {
					ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
					let guild = state.channel(channel).and_then(|c| c.guild);
					for hit in &page.hits {
						let (row, response) =
							ui.allocate_exact_size(vec2(body.width(), ROW), Sense::click());
						let t = paint::hover_t(ui, &response);
						if t > 0.0 {
							ui.painter().rect_filled(row, 11, p.hover.gamma_multiply(t));
						}
						let avatar = Rect::from_min_size(
							row.left_top() + vec2(10.0, 12.0),
							vec2(30.0, 30.0),
						);
						let texture = solace.images.get(&hit.author.avatar_key());
						paint::avatar(
							ui.painter(),
							avatar,
							AvatarShape::Circle,
							texture,
							&hit.author.name,
							hit.author.id.0,
							&p,
						);
						let left = avatar.right() + 10.0;
						let name = paint::text_left(
							ui.painter(),
							pos2(left, avatar.top() + 7.0),
							state.user_display_name(&hit.author),
							theme::semibold(13.0),
							p.text,
							row.right() - left - 90.0,
						);
						let at = clock::snowflake(hit.id);
						paint::label(
							ui.painter(),
							pos2(name.right() + 8.0, name.center().y),
							Align2::LEFT_CENTER,
							&format!("{} {}", clock::day(at), clock::clock(at)),
							theme::sans(11.0),
							p.faint,
						);
						let mut excerpt = text::preview(
							&hit.excerpt,
							&Names {
								state,
								guild,
								mentions: &[],
							},
							140,
						);
						if excerpt.trim().is_empty() {
							excerpt = match hit.attachments.len() {
								0 => "Embedded content".to_owned(),
								1 => format!("Attachment: {}", hit.attachments[0].filename),
								n => format!("{n} attachments"),
							};
						}
						let galley = ui.painter().layout(
							excerpt,
							theme::sans(12.5),
							p.text_soft,
							row.right() - left - 12.0,
						);
						let clip = Rect::from_min_max(
							pos2(left, avatar.top() + 18.0),
							pos2(row.right() - 8.0, row.bottom() - 6.0),
						);
						ui.painter()
							.with_clip_rect(clip)
							.galley(clip.min, galley, p.text_soft);
						if response
							.on_hover_cursor(egui::CursorIcon::PointingHand)
							.clicked()
						{
							open = Some(hit.id);
						}
					}
					if can_more {
						ui.add_space(6.0);
						let (row, _) =
							ui.allocate_exact_size(vec2(body.width(), 40.0), Sense::hover());
						if paint::pill_button(
							ui,
							ui.scope_id().with("finder-more"),
							Rect::from_center_size(row.center(), vec2(160.0, 34.0)),
							Some(Icon::ChevronDown),
							if pins { "Older pins" } else { "Older results" },
							ButtonKind::Ghost,
							true,
						)
						.clicked()
						{
							more = true;
						}
					}
				});
		});
	let Some(finder) = solace.finder.as_mut() else {
		return;
	};
	if submit {
		let query = finder.query.trim().to_owned();
		finder.requested = Some(channel);
		let valid = model::valid_search_query(&query);
		match state.request_search(query, None) {
			Some(command) => {
				finder.unavailable = false;
				commands.push(command);
			}
			None if valid => finder.unavailable = true,
			None => {}
		}
	}
	if more {
		let command = if pins {
			state.request_older_pins()
		} else {
			let before = state
				.search
				.as_ref()
				.and_then(|s| s.page.as_ref())
				.and_then(|p| p.hits.last())
				.map(|h| h.id);
			state.request_search(finder.query.trim().to_owned(), before)
		};
		commands.extend(command);
	}
	if let Some(message) = open {
		if state.timeline.get(message).is_some() {
			state.search_target = Some(message);
			state.restore_scroll = false;
		} else if let Some(command) = state.open_search_hit(message) {
			commands.push(command);
		}
		// The panel steps aside so the message it found can be read.
		solace.finder = None;
		commands.push(state.clear_search());
		return;
	}
	let escape = ui.input(|i| i.key_pressed(Key::Escape))
		&& solace.picker.is_none()
		&& solace.profile_at.is_none()
		&& solace.switcher.is_none();
	if close || escape {
		solace.finder = None;
		commands.push(state.clear_search());
	}
}
