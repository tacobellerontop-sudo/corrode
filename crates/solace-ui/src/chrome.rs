//! Window chrome: Solace draws its own title bar (drag to move, double-click to maximize,
//! minimize / maximize / close) and resize edges, so the window has no system frame.
use crate::{
	icons::{self, Icon},
	paint,
	theme::{self, mix},
};
use egui::{
	Align2, Area, Color32, Context, CursorIcon, Order, Rect, Sense, Ui, ViewportCommand, vec2,
	viewport::ResizeDirection,
};

pub const TITLE_BAR: f32 = 36.0;
/// Width of each window button.
const BUTTON: f32 = 46.0;
/// Thickness of the invisible resize edges.
const EDGE: f32 = 5.0;
const CORNER: f32 = 12.0;

fn maximized(ctx: &Context) -> bool {
	ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// Make `rect` move the window when dragged and toggle maximize on double-click. Call it
/// before adding anything clickable on top.
pub fn drag_area(ui: &mut Ui, rect: Rect) {
	let response = ui.interact(
		rect,
		ui.scope_id().with("window-drag"),
		Sense::click_and_drag(),
	);
	if response.double_clicked() {
		let max = maximized(ui.ctx());
		ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!max));
	} else if response.drag_started_by(egui::PointerButton::Primary) {
		ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
	}
}

/// Minimize, maximize/restore and close, right-aligned in `rect`. Returns their total width.
pub fn window_buttons(ui: &mut Ui, rect: Rect) -> f32 {
	let p = theme::palette(ui.ctx());
	let max = maximized(ui.ctx());
	let buttons = [
		(
			Icon::WindowMinimize,
			"Minimize",
			ViewportCommand::Minimized(true),
		),
		(
			if max {
				Icon::WindowRestore
			} else {
				Icon::WindowMaximize
			},
			if max { "Restore" } else { "Maximize" },
			ViewportCommand::Maximized(!max),
		),
		(Icon::Close, "Close", ViewportCommand::Close),
	];
	let count = buttons.len() as f32;
	for (i, (icon, tip, command)) in buttons.into_iter().enumerate() {
		let button = Rect::from_min_size(
			rect.right_top() - vec2((count - i as f32) * BUTTON, 0.0),
			vec2(BUTTON, rect.height()),
		);
		let response = ui
			.interact(
				button,
				ui.scope_id().with(("window-button", i)),
				Sense::click(),
			)
			.on_hover_text(tip);
		let t = paint::hover_t(ui, &response);
		let close = icon == Icon::Close;
		let fill = if close {
			Color32::from_rgb(0xd9, 0x4f, 0x45).gamma_multiply(t)
		} else {
			p.hover.gamma_multiply(t)
		};
		ui.painter().rect_filled(button, 0, fill);
		let ink = if close {
			mix(p.text_soft, Color32::WHITE, t)
		} else {
			mix(p.text_soft, p.text, t)
		};
		icons::paint_at(ui.painter(), icon, button.center(), 15.0, ink);
		if response.clicked() {
			ui.ctx().send_viewport_cmd(command);
		}
	}
	count * BUTTON
}

/// The title bar across the top of `full`; returns the area below it.
pub fn title_bar(ui: &mut Ui, full: Rect, title: &str) -> Rect {
	// F11 works on every screen with a title bar, not only once signed in, so full
	// screen can always be left the way it was entered.
	if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F11)) {
		let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
		ui.ctx()
			.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
	}
	// Full screen has no window to move or resize: give the content every pixel.
	if ui.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
		return full;
	}
	let p = theme::palette(ui.ctx());
	let bar = Rect::from_min_size(full.min, vec2(full.width(), TITLE_BAR));
	ui.painter().rect_filled(bar, 0, p.rail);
	paint::hairline(ui.painter(), bar.left_bottom(), bar.right_bottom(), &p);
	drag_area(ui, bar);
	let focused = ui.input(|i| i.viewport().focused.unwrap_or(true));
	icons::paint_weighted(
		ui.painter(),
		Icon::Logo,
		Rect::from_center_size(bar.left_center() + vec2(22.0, 0.0), vec2(15.0, 15.0)),
		if focused { p.text } else { p.faint },
		1.1,
	);
	paint::label(
		ui.painter(),
		bar.left_center() + vec2(38.0, 0.0),
		Align2::LEFT_CENTER,
		title,
		theme::medium(12.5),
		if focused { p.text_soft } else { p.faint },
	);
	window_buttons(ui, bar);
	egui::Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max)
}

/// Invisible edges and corners that resize the window. Call at the end of the frame so they
/// sit above everything else; they are inactive while maximized.
pub fn resize_edges(ctx: &Context) {
	if maximized(ctx) || ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
		return;
	}
	let screen = ctx.content_rect();
	let (l, r, t, b) = (screen.left(), screen.right(), screen.top(), screen.bottom());
	let zones = [
		(
			Rect::from_min_max(egui::pos2(l, t), egui::pos2(l + CORNER, t + CORNER)),
			ResizeDirection::NorthWest,
			CursorIcon::ResizeNorthWest,
		),
		(
			Rect::from_min_max(egui::pos2(r - CORNER, t), egui::pos2(r, t + CORNER)),
			ResizeDirection::NorthEast,
			CursorIcon::ResizeNorthEast,
		),
		(
			Rect::from_min_max(egui::pos2(l, b - CORNER), egui::pos2(l + CORNER, b)),
			ResizeDirection::SouthWest,
			CursorIcon::ResizeSouthWest,
		),
		(
			Rect::from_min_max(egui::pos2(r - CORNER, b - CORNER), egui::pos2(r, b)),
			ResizeDirection::SouthEast,
			CursorIcon::ResizeSouthEast,
		),
		(
			Rect::from_min_max(egui::pos2(l + CORNER, t), egui::pos2(r - CORNER, t + EDGE)),
			ResizeDirection::North,
			CursorIcon::ResizeNorth,
		),
		(
			Rect::from_min_max(egui::pos2(l + CORNER, b - EDGE), egui::pos2(r - CORNER, b)),
			ResizeDirection::South,
			CursorIcon::ResizeSouth,
		),
		(
			Rect::from_min_max(egui::pos2(l, t + CORNER), egui::pos2(l + EDGE, b - CORNER)),
			ResizeDirection::West,
			CursorIcon::ResizeWest,
		),
		(
			Rect::from_min_max(egui::pos2(r - EDGE, t + CORNER), egui::pos2(r, b - CORNER)),
			ResizeDirection::East,
			CursorIcon::ResizeEast,
		),
	];
	Area::new(egui::Id::unique("solace-resize-edges"))
		.order(Order::Foreground)
		.fixed_pos(screen.min)
		.interactable(true)
		.show(ctx, |ui| {
			for (i, (zone, direction, cursor)) in zones.into_iter().enumerate() {
				let response = ui.interact(zone, ui.scope_id().with(("edge", i)), Sense::drag());
				if response.hovered() || response.dragged() {
					ui.ctx().set_cursor_icon(cursor);
				}
				if response.drag_started() {
					ui.ctx()
						.send_viewport_cmd(ViewportCommand::BeginResize(direction));
				}
			}
		});
}
