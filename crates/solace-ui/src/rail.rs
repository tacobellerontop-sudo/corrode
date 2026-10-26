//! Far-left icon rail: brand mark, collection switches, account and settings.
use crate::{
	Pane, SettingsPage, Solace,
	icons::{self, Icon},
	paint::{self, AvatarShape},
	theme::{self, mix},
};
use client_core::{Command, State};
use egui::{Rect, Sense, Ui, pos2, vec2};

pub fn show(solace: &mut Solace, ui: &mut Ui, state: &mut State, _commands: &mut Vec<Command>) {
	let p = theme::palette(ui.ctx());
	let rect = ui.max_rect();
	ui.painter().rect_filled(rect, 0, p.rail);
	let x = rect.center().x;

	icons::paint_weighted(
		ui.painter(),
		Icon::Logo,
		Rect::from_center_size(pos2(x, rect.top() + 42.0), vec2(30.0, 30.0)),
		p.text,
		1.1,
	);

	let items = [
		(Pane::Home, Icon::Home, "Home"),
		(Pane::Direct, Icon::People, "Direct messages"),
		(Pane::Explore, Icon::Compass, "Communities"),
		(Pane::Activity, Icon::Sparkle, "Activity"),
	];
	let mut y = rect.top() + 124.0;
	// The selection pill glides between collections instead of jumping.
	let index = items
		.iter()
		.position(|(pane, ..)| *pane == solace.pane)
		.unwrap_or(0) as f32;
	let pill_y =
		ui.ctx()
			.animate_value_with_time(ui.scope_id().with("rail-pill"), y + index * 56.0, 0.22);
	ui.painter().rect_filled(
		Rect::from_center_size(pos2(x, pill_y), vec2(48.0, 48.0)),
		14,
		p.selected,
	);
	ui.painter().rect_filled(
		Rect::from_center_size(pos2(rect.left() + 3.0, pill_y), vec2(4.0, 22.0)),
		2,
		p.accent,
	);
	for (pane, icon, label) in items {
		let item = Rect::from_center_size(pos2(x, y), vec2(48.0, 48.0));
		let response = ui
			.interact(item, ui.scope_id().with(("rail", label)), Sense::click())
			.on_hover_text(label);
		let active = solace.pane == pane;
		let t = paint::hover_t(ui, &response);
		if !active && t > 0.0 {
			ui.painter()
				.rect_filled(item, 14, p.hover.gamma_multiply(t));
		}
		// Icons dip slightly while pressed.
		let press = ui.ctx().animate_bool_with_time(
			response.id.with("press"),
			response.is_pointer_button_down_on(),
			0.08,
		);
		let item = item.shrink(press * 2.0);
		let color = if active {
			p.text
		} else {
			mix(p.muted, p.text, t)
		};
		if icon == Icon::Home && !active {
			// The filled house reads as "selected"; draw an outline variant otherwise.
			icons::paint(
				ui.painter(),
				Icon::Home,
				item.shrink(14.0),
				color.gamma_multiply(0.85),
			);
		} else {
			icons::paint(ui.painter(), icon, item.shrink(13.0), color);
		}
		if pane == Pane::Activity
			&& state
				.channels
				.iter()
				.any(|c| crate::nav::channel_unread(state, c))
		{
			ui.painter()
				.circle_filled(item.right_top() + vec2(-11.0, 11.0), 4.0, p.accent);
		}
		if response.clicked() {
			solace.pane = pane;
			// Home always lands on the dashboard.
			if pane == Pane::Home {
				state.open_home();
			}
		}
		y += 56.0;
	}

	// Settings sits with the collections, like the reference layout.
	let gear = Rect::from_center_size(pos2(x, y), vec2(48.0, 48.0));
	let response = ui
		.interact(gear, ui.scope_id().with("rail-settings"), Sense::click())
		.on_hover_text("Settings");
	let t = paint::hover_t(ui, &response);
	ui.painter().rect_filled(gear, 14, mix(p.rail, p.hover, t));
	icons::paint(
		ui.painter(),
		Icon::Settings,
		gear.shrink(13.0),
		mix(p.muted, p.text, t),
	);
	if response.clicked() {
		solace.settings = Some(SettingsPage::Appearance);
	}

	// Account at the bottom.
	if let Some(user) = state.user.clone() {
		let center = pos2(x, rect.bottom() - 46.0);
		let avatar = Rect::from_center_size(center, vec2(38.0, 38.0));
		let response = ui
			.interact(
				avatar.expand(4.0),
				ui.scope_id().with("rail-account"),
				Sense::click(),
			)
			.on_hover_text(user.name.as_str());
		let t = paint::hover_t(ui, &response);
		if t > 0.0 {
			ui.painter()
				.circle_filled(center, 23.0, p.hover.gamma_multiply(t));
		}
		let texture = solace.images.get(&user.avatar_key());
		paint::avatar(
			ui.painter(),
			avatar,
			AvatarShape::Circle,
			texture,
			&user.name,
			user.id.0,
			&p,
		);
		let status = if state.gateway_connected {
			p.online
		} else {
			p.faint
		};
		paint::presence(
			ui.painter(),
			avatar.right_bottom() - vec2(5.0, 5.0),
			5.0,
			status,
			p.rail,
		);
		if response.clicked() {
			solace.account_menu = !solace.account_menu;
		}
	}
}
