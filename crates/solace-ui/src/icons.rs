//! Hand-drawn vector icons on a 24-unit grid. They are tessellated by epaint every frame, so
//! they stay crisp at any display scale and take their color from the current palette.
use egui::{
	Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2,
	epaint::{CubicBezierShape, PathShape, PathStroke},
};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
	Logo,
	Home,
	People,
	Compass,
	Sparkle,
	Settings,
	Search,
	Plus,
	Video,
	More,
	Smile,
	Image,
	Send,
	Close,
	Waveform,
	ChevronRight,
	ChevronDown,
	ChevronLeft,
	Hash,
	Speaker,
	Mic,
	MicOff,
	Headphones,
	HeadphonesOff,
	PhoneOff,
	Paperclip,
	Reply,
	Thread,
	Logout,
	Sun,
	Moon,
	Bell,
	Check,
	Pencil,
	Trash,
	At,
	Megaphone,
	File,
	Lock,
	Globe,
	Download,
	Play,
	Pause,
	Screen,
	Expand,
	Collapse,
	SpeakerOff,
	Sliders,
	WindowMinimize,
	WindowMaximize,
	WindowRestore,
}

struct Pen<'a> {
	painter: &'a Painter,
	origin: Pos2,
	scale: f32,
	stroke: Stroke,
}

impl Pen<'_> {
	fn p(&self, x: f32, y: f32) -> Pos2 {
		self.origin + Vec2::new(x, y) * self.scale
	}
	fn line(&self, points: &[(f32, f32)]) {
		let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
		self.painter.add(Shape::line(points, self.stroke));
	}
	fn closed(&self, points: &[(f32, f32)]) {
		let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
		self.painter.add(Shape::closed_line(points, self.stroke));
	}
	fn fill(&self, points: &[(f32, f32)]) {
		let points: Vec<Pos2> = points.iter().map(|(x, y)| self.p(*x, *y)).collect();
		self.painter.add(PathShape::convex_polygon(
			points,
			self.stroke.color,
			Stroke::NONE,
		));
	}
	fn circle(&self, x: f32, y: f32, r: f32) {
		self.painter
			.circle_stroke(self.p(x, y), r * self.scale, self.stroke);
	}
	fn dot(&self, x: f32, y: f32, r: f32) {
		self.painter
			.circle_filled(self.p(x, y), r * self.scale, self.stroke.color);
	}
	fn arc(&self, x: f32, y: f32, r: f32, from: f32, to: f32) {
		let steps = (((to - from).abs() * r * self.scale) / 3.0).ceil().max(6.0) as usize;
		let points: Vec<Pos2> = (0..=steps)
			.map(|i| {
				let a = from + (to - from) * i as f32 / steps as f32;
				self.p(x + r * a.cos(), y + r * a.sin())
			})
			.collect();
		self.painter.add(Shape::line(points, self.stroke));
	}
	fn curve(&self, points: [(f32, f32); 4]) {
		self.painter.add(CubicBezierShape::from_points_stroke(
			points.map(|(x, y)| self.p(x, y)),
			false,
			Color32::TRANSPARENT,
			PathStroke::from(self.stroke),
		));
	}
	fn rounded(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
		self.painter.rect_stroke(
			Rect::from_min_size(self.p(x, y), Vec2::new(w, h) * self.scale),
			r * self.scale,
			self.stroke,
			egui::StrokeKind::Middle,
		);
	}
}

/// Paint `icon` centered in `rect` using `color`. `weight` scales the stroke (1.0 = regular).
pub fn paint(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
	paint_weighted(painter, icon, rect, color, 1.0);
}

pub fn paint_weighted(painter: &Painter, icon: Icon, rect: Rect, color: Color32, weight: f32) {
	let side = rect.width().min(rect.height());
	let scale = side / 24.0;
	let origin = rect.center() - Vec2::splat(side / 2.0);
	let pen = Pen {
		painter,
		origin,
		scale,
		stroke: Stroke::new((1.6 * scale).max(1.1) * weight, color),
	};
	draw(&pen, icon);
}

fn star(cx: f32, cy: f32, r: f32, waist: f32) -> Vec<(f32, f32)> {
	// A four-pointed star whose sides bow inward, like light catching a horizon.
	let mut points = Vec::with_capacity(4 * 8);
	for i in 0..4 {
		let a = i as f32 * PI / 2.0 - PI / 2.0;
		let b = a + PI / 2.0;
		let tip = (cx + r * a.cos(), cy + r * a.sin());
		let next = (cx + r * b.cos(), cy + r * b.sin());
		let mid = a + PI / 4.0;
		let control = (cx + waist * mid.cos(), cy + waist * mid.sin());
		for step in 0..8 {
			let t = step as f32 / 8.0;
			let u = 1.0 - t;
			points.push((
				u * u * tip.0 + 2.0 * u * t * control.0 + t * t * next.0,
				u * u * tip.1 + 2.0 * u * t * control.1 + t * t * next.1,
			));
		}
	}
	points
}

fn draw(pen: &Pen, icon: Icon) {
	match icon {
		Icon::Logo => {
			let points = star(12.0, 12.0, 10.5, 2.2);
			pen.closed(&points);
		}
		Icon::Sparkle => {
			let points = star(12.0, 12.0, 9.0, 2.0);
			pen.closed(&points);
		}
		Icon::Home => {
			pen.fill(&[
				(4.0, 10.5),
				(12.0, 3.5),
				(20.0, 10.5),
				(20.0, 20.0),
				(4.0, 20.0),
			]);
			let painter = pen.painter;
			let door = Rect::from_min_max(pen.p(10.0, 14.0), pen.p(14.0, 20.0));
			painter.rect_filled(
				door,
				1.0 * pen.scale,
				if pen.stroke.color.r() as u16 + pen.stroke.color.g() as u16 > 300 {
					Color32::from_black_alpha(160)
				} else {
					Color32::from_white_alpha(230)
				},
			);
		}
		Icon::People => {
			pen.circle(9.0, 8.0, 3.4);
			pen.arc(9.0, 20.5, 6.0, PI + 0.25, TAU - 0.25);
			pen.arc(16.5, 8.5, 2.8, -PI / 2.0 - 0.6, PI / 2.0 + 0.2);
			pen.arc(16.5, 20.0, 5.0, PI + 0.95, TAU - 0.15);
		}
		Icon::Compass => {
			pen.circle(12.0, 12.0, 9.0);
			pen.closed(&[(15.5, 8.5), (13.4, 13.4), (8.5, 15.5), (10.6, 10.6)]);
		}
		Icon::Settings => {
			pen.circle(12.0, 12.0, 3.0);
			let mut points = Vec::new();
			for i in 0..=96 {
				let a = i as f32 / 96.0 * TAU;
				let tooth = ((a * 8.0).cos() * 3.0).clamp(-1.0, 1.0);
				let r = 8.0 + tooth * 1.1;
				points.push((12.0 + r * a.cos(), 12.0 + r * a.sin()));
			}
			pen.closed(&points);
		}
		Icon::Search => {
			pen.circle(10.5, 10.5, 6.5);
			pen.line(&[(15.3, 15.3), (20.0, 20.0)]);
		}
		Icon::Plus => {
			pen.line(&[(12.0, 5.0), (12.0, 19.0)]);
			pen.line(&[(5.0, 12.0), (19.0, 12.0)]);
		}
		Icon::Video => {
			pen.rounded(3.0, 6.5, 12.5, 11.0, 2.6);
			pen.closed(&[(15.5, 10.5), (21.0, 7.5), (21.0, 16.5), (15.5, 13.5)]);
		}
		Icon::More => {
			for x in [6.0, 12.0, 18.0] {
				pen.dot(x, 12.0, 1.5);
			}
		}
		Icon::Smile => {
			pen.circle(12.0, 12.0, 9.0);
			pen.dot(9.0, 10.0, 1.1);
			pen.dot(15.0, 10.0, 1.1);
			pen.arc(12.0, 12.5, 4.2, 0.35, PI - 0.35);
		}
		Icon::Image => {
			pen.rounded(3.5, 4.5, 17.0, 15.0, 2.8);
			pen.circle(9.0, 9.5, 1.6);
			pen.line(&[
				(4.0, 17.5),
				(9.5, 12.5),
				(13.0, 15.5),
				(16.0, 13.0),
				(20.0, 16.5),
			]);
		}
		Icon::Send => {
			pen.closed(&[(3.5, 11.5), (20.5, 4.0), (14.0, 20.5), (11.0, 13.5)]);
			pen.line(&[(11.0, 13.5), (20.5, 4.0)]);
		}
		Icon::Close => {
			pen.line(&[(6.0, 6.0), (18.0, 18.0)]);
			pen.line(&[(18.0, 6.0), (6.0, 18.0)]);
		}
		Icon::Waveform => {
			for (x, h) in [
				(5.0, 3.0),
				(8.5, 7.0),
				(12.0, 11.0),
				(15.5, 6.0),
				(19.0, 3.0),
			] {
				pen.line(&[(x, 12.0 - h / 2.0), (x, 12.0 + h / 2.0)]);
			}
		}
		Icon::ChevronRight => pen.line(&[(9.5, 6.0), (15.5, 12.0), (9.5, 18.0)]),
		Icon::ChevronLeft => pen.line(&[(14.5, 6.0), (8.5, 12.0), (14.5, 18.0)]),
		Icon::ChevronDown => pen.line(&[(6.0, 9.5), (12.0, 15.5), (18.0, 9.5)]),
		Icon::Hash => {
			pen.line(&[(9.5, 4.5), (7.5, 19.5)]);
			pen.line(&[(16.5, 4.5), (14.5, 19.5)]);
			pen.line(&[(5.0, 9.0), (19.5, 9.0)]);
			pen.line(&[(4.5, 15.0), (19.0, 15.0)]);
		}
		Icon::Speaker => {
			pen.closed(&[
				(4.0, 9.5),
				(8.0, 9.5),
				(12.5, 5.5),
				(12.5, 18.5),
				(8.0, 14.5),
				(4.0, 14.5),
			]);
			pen.arc(13.0, 12.0, 3.5, -0.9, 0.9);
			pen.arc(13.0, 12.0, 7.0, -0.85, 0.85);
		}
		Icon::Mic | Icon::MicOff => {
			pen.rounded(9.0, 3.5, 6.0, 11.0, 3.0);
			pen.arc(12.0, 11.0, 6.0, 0.15, PI - 0.15);
			pen.line(&[(12.0, 17.0), (12.0, 20.5)]);
			if icon == Icon::MicOff {
				pen.line(&[(4.5, 4.5), (19.5, 19.5)]);
			}
		}
		Icon::Headphones | Icon::HeadphonesOff => {
			pen.arc(12.0, 13.0, 8.0, PI, TAU);
			pen.rounded(4.0, 13.0, 4.0, 6.5, 1.6);
			pen.rounded(16.0, 13.0, 4.0, 6.5, 1.6);
			if icon == Icon::HeadphonesOff {
				pen.line(&[(4.5, 4.5), (19.5, 19.5)]);
			}
		}
		Icon::PhoneOff => {
			pen.curve([(3.5, 14.0), (8.0, 8.5), (16.0, 8.5), (20.5, 14.0)]);
			pen.line(&[(3.5, 14.0), (6.5, 16.5), (8.5, 13.5)]);
			pen.line(&[(20.5, 14.0), (17.5, 16.5), (15.5, 13.5)]);
		}
		Icon::Paperclip => {
			pen.line(&[(15.5, 7.0), (8.5, 14.0)]);
			pen.arc(10.5, 16.0, 2.83, PI * 0.75, PI * 1.75 + PI);
			pen.line(&[(12.5, 18.0), (18.5, 12.0)]);
			pen.arc(15.0, 8.5, 4.95, -PI * 0.25, PI * 0.75);
		}
		Icon::Reply => {
			pen.line(&[(9.5, 6.0), (4.5, 11.0), (9.5, 16.0)]);
			pen.curve([(4.5, 11.0), (12.0, 11.0), (19.5, 11.0), (19.5, 19.0)]);
		}
		Icon::Thread => {
			pen.rounded(3.5, 4.0, 13.0, 9.5, 3.0);
			pen.line(&[(7.0, 13.5), (6.0, 16.5), (10.0, 13.5)]);
			pen.line(&[
				(18.5, 9.0),
				(20.5, 9.0),
				(20.5, 17.5),
				(18.0, 17.5),
				(18.5, 20.0),
				(15.0, 17.5),
				(11.5, 17.5),
				(11.5, 15.5),
			]);
		}
		Icon::Logout => {
			pen.line(&[(13.0, 4.0), (5.0, 4.0), (5.0, 20.0), (13.0, 20.0)]);
			pen.line(&[(10.0, 12.0), (20.0, 12.0)]);
			pen.line(&[(16.5, 8.5), (20.0, 12.0), (16.5, 15.5)]);
		}
		Icon::Sun => {
			pen.circle(12.0, 12.0, 4.0);
			for i in 0..8 {
				let a = i as f32 * TAU / 8.0;
				pen.line(&[
					(12.0 + 6.5 * a.cos(), 12.0 + 6.5 * a.sin()),
					(12.0 + 9.0 * a.cos(), 12.0 + 9.0 * a.sin()),
				]);
			}
		}
		Icon::Moon => {
			pen.arc(12.0, 12.0, 8.0, -PI * 0.15, PI * 1.6);
			pen.curve([(19.8, 9.0), (14.0, 3.5), (8.5, 11.0), (14.6, 4.4)]);
		}
		Icon::Bell => {
			pen.line(&[(5.0, 17.0), (19.0, 17.0)]);
			pen.curve([(6.5, 17.0), (6.5, 6.0), (17.5, 6.0), (17.5, 17.0)]);
			pen.arc(12.0, 18.5, 2.2, 0.0, PI);
		}
		Icon::Check => pen.line(&[(5.0, 12.5), (10.0, 17.5), (19.0, 7.0)]),
		Icon::Pencil => {
			pen.closed(&[
				(5.0, 19.0),
				(6.0, 15.0),
				(16.0, 5.0),
				(19.0, 8.0),
				(9.0, 18.0),
			]);
			pen.line(&[(14.0, 7.0), (17.0, 10.0)]);
		}
		Icon::Trash => {
			pen.line(&[(4.5, 7.0), (19.5, 7.0)]);
			pen.line(&[(9.5, 7.0), (9.5, 4.5), (14.5, 4.5), (14.5, 7.0)]);
			pen.line(&[(6.5, 7.0), (7.5, 20.0), (16.5, 20.0), (17.5, 7.0)]);
		}
		Icon::At => {
			pen.circle(12.0, 12.0, 3.5);
			pen.arc(12.0, 12.0, 8.5, 0.0, TAU * 0.86);
			pen.line(&[(15.5, 9.0), (15.5, 13.5)]);
			pen.arc(18.0, 13.5, 2.5, PI, PI * 1.9);
		}
		Icon::Megaphone => {
			pen.closed(&[
				(4.0, 10.0),
				(14.0, 6.0),
				(19.5, 4.5),
				(19.5, 19.5),
				(14.0, 18.0),
				(4.0, 14.0),
			]);
			pen.line(&[(7.0, 15.0), (8.5, 20.0)]);
		}
		Icon::File => {
			pen.closed(&[
				(6.0, 3.5),
				(14.0, 3.5),
				(18.5, 8.0),
				(18.5, 20.5),
				(6.0, 20.5),
			]);
			pen.line(&[(14.0, 3.5), (14.0, 8.0), (18.5, 8.0)]);
		}
		Icon::Lock => {
			pen.rounded(5.5, 10.5, 13.0, 9.5, 2.4);
			pen.arc(12.0, 10.5, 4.0, PI, TAU);
		}
		Icon::Download => {
			pen.line(&[(12.0, 4.0), (12.0, 15.0)]);
			pen.line(&[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
			pen.line(&[(4.5, 15.5), (4.5, 19.5), (19.5, 19.5), (19.5, 15.5)]);
		}
		Icon::Play => pen.fill(&[(8.0, 5.5), (19.0, 12.0), (8.0, 18.5)]),
		Icon::Pause => {
			pen.fill(&[(7.0, 5.5), (10.5, 5.5), (10.5, 18.5), (7.0, 18.5)]);
			pen.fill(&[(13.5, 5.5), (17.0, 5.5), (17.0, 18.5), (13.5, 18.5)]);
		}
		Icon::Screen => {
			pen.rounded(3.0, 4.5, 18.0, 12.0, 2.2);
			pen.line(&[(9.0, 20.0), (15.0, 20.0)]);
			pen.line(&[(12.0, 16.5), (12.0, 20.0)]);
		}
		Icon::Expand => {
			pen.line(&[(14.0, 4.5), (19.5, 4.5), (19.5, 10.0)]);
			pen.line(&[(19.5, 4.5), (13.5, 10.5)]);
			pen.line(&[(10.0, 19.5), (4.5, 19.5), (4.5, 14.0)]);
			pen.line(&[(4.5, 19.5), (10.5, 13.5)]);
		}
		Icon::Collapse => {
			pen.line(&[(13.5, 5.0), (13.5, 10.5), (19.0, 10.5)]);
			pen.line(&[(13.5, 10.5), (19.5, 4.5)]);
			pen.line(&[(10.5, 19.0), (10.5, 13.5), (5.0, 13.5)]);
			pen.line(&[(10.5, 13.5), (4.5, 19.5)]);
		}
		Icon::SpeakerOff => {
			pen.closed(&[
				(4.0, 9.5),
				(8.0, 9.5),
				(12.5, 5.5),
				(12.5, 18.5),
				(8.0, 14.5),
				(4.0, 14.5),
			]);
			pen.line(&[(15.5, 9.5), (20.5, 14.5)]);
			pen.line(&[(20.5, 9.5), (15.5, 14.5)]);
		}
		Icon::Sliders => {
			for (y, knob) in [(6.5, 15.0), (12.0, 8.5), (17.5, 13.0)] {
				pen.line(&[(4.0, y), (20.0, y)]);
				pen.dot(knob, y, 2.4);
			}
		}
		Icon::WindowMinimize => pen.line(&[(6.5, 12.5), (17.5, 12.5)]),
		Icon::WindowMaximize => pen.rounded(6.5, 6.5, 11.0, 11.0, 1.2),
		Icon::WindowRestore => {
			pen.rounded(6.0, 8.5, 9.5, 9.5, 1.2);
			pen.line(&[
				(8.5, 8.5),
				(8.5, 6.0),
				(18.0, 6.0),
				(18.0, 15.5),
				(15.5, 15.5),
			]);
		}
		Icon::Globe => {
			pen.circle(12.0, 12.0, 9.0);
			pen.line(&[(3.0, 12.0), (21.0, 12.0)]);
			pen.curve([(12.0, 3.0), (7.0, 8.0), (7.0, 16.0), (12.0, 21.0)]);
			pen.curve([(12.0, 3.0), (17.0, 8.0), (17.0, 16.0), (12.0, 21.0)]);
		}
	}
}

/// Convenience for callers that only have a center point and a pixel size.
pub fn paint_at(painter: &Painter, icon: Icon, center: Pos2, size: f32, color: Color32) {
	paint(
		painter,
		icon,
		Rect::from_center_size(center, Vec2::splat(size)),
		color,
	);
}
