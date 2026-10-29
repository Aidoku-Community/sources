use crate::{models::ViewpointData, urls};
use aidoku::{
	Page, PageContent,
	alloc::{String, Vec, format, vec},
	helpers::string::PlainText,
	imports::canvas::{
		Canvas, Color, Font, FontWeight, ImageRef, LineJoin, Path, Point, Rect, StrokeStyle,
	},
};
use core::f32::consts::SQRT_2;
use zaimanhua_template::net;

const EM: f32 = 44.0;
const WIDTH: f32 = 24.0 * EM;
const HEIGHT: f32 = 40.0 * EM;
const MARGIN: f32 = 1.25 * EM;
const LINE_WIDTH: f32 = WIDTH - 4.0 * MARGIN;
const LEADING: f32 = 1.45;
const GAP: f32 = 0.5 * EM;
const ROW_PADDING: f32 = 0.7 * EM;
const TITLE_SIZE: f32 = 1.3 * EM;
const NOTE_SIZE: f32 = 0.8 * EM;

pub fn page(chapter_key: &str) -> Option<Page> {
	let (comic_id, chapter_id) = chapter_key.split_once('/')?;
	let texts: Vec<String> =
		net::fetch::<ViewpointData>(&urls::viewpoints(comic_id, chapter_id), None)
			.ok()?
			.into_texts()
			.map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
			.filter(|text| !text.is_empty())
			.collect();
	if texts.is_empty() {
		return None;
	}
	Some(Page {
		content: PageContent::image(overview(&texts)),
		has_description: true,
		description: Some(description(&texts)),
		..Default::default()
	})
}

fn description(texts: &[String]) -> String {
	let comments: Vec<String> = texts.iter().map(PlainText::escape_markdown).collect();
	let count = texts.len();
	format!("### 章末吐槽（{count}）\n\n{}", comments.join("\n\n"))
}

fn overview(texts: &[String]) -> ImageRef {
	let footer = GAP + LEADING * NOTE_SIZE + MARGIN;
	let mut rows = Vec::new();
	let mut y = MARGIN + LEADING * TITLE_SIZE + GAP;
	for text in texts {
		let lines = preview(text);
		let bottom = y + 2.0 * ROW_PADDING + lines.len() as f32 * LEADING * EM;
		if bottom + footer + 2.0 * MARGIN > WIDTH * SQRT_2 {
			break;
		}
		rows.push((y, lines));
		y = bottom;
	}
	let panel_height = y + footer;
	let panel_top = (HEIGHT - panel_height) / 2.0;
	let x = 2.0 * MARGIN;
	let hairline = 0.05 * EM;

	let mut canvas = Canvas::new(WIDTH, HEIGHT);
	let core = Path::rect(&Rect::new(
		MARGIN + EM,
		panel_top + EM,
		WIDTH - 2.0 * (MARGIN + EM),
		panel_height - 2.0 * EM,
	));
	let edge = |width, color| StrokeStyle {
		color,
		width,
		join: LineJoin::Round,
		..Default::default()
	};
	canvas.stroke(&core, &edge(2.0 * EM + 2.0 * hairline, shade(0.18)));
	canvas.stroke(&core, &edge(2.0 * EM, Color::white()));
	canvas.fill(&core, &Color::white());
	for (top, _) in rows.iter().skip(1) {
		let separator = Rect::new(x, panel_top + top, LINE_WIDTH, hairline);
		canvas.fill(&Path::rect(&separator), &shade(0.08));
	}

	let bold = Font::system(FontWeight::Bold);
	let regular = Font::system(FontWeight::Regular);
	let mut draw = |text: &str, top: f32, size: f32, font: &Font, color: &Color| {
		let position = Point::new(x, panel_top + top + (LEADING - 1.0) * size / 2.0);
		canvas.draw_text(text, size, &position, font, color);
	};
	draw("章末吐槽", MARGIN, TITLE_SIZE, &bold, &Color::black());
	for (top, lines) in &rows {
		for (n, line) in lines.iter().enumerate() {
			let line_top = top + ROW_PADDING + n as f32 * LEADING * EM;
			draw(line, line_top, EM, &regular, &Color::black());
		}
	}
	let note = format!("共 {} 条 · 点击 ⓘ 查看全部", texts.len());
	draw(&note, y + GAP, NOTE_SIZE, &regular, &shade(0.5));
	canvas.get_image()
}

fn preview(text: &str) -> Vec<String> {
	let (first, rest) = split_at_width(text, LINE_WIDTH);
	if rest.is_empty() {
		return vec![first.into()];
	}
	let second = match split_at_width(rest, LINE_WIDTH) {
		(_, "") => rest.into(),
		_ => format!("{}…", split_at_width(rest, LINE_WIDTH - EM).0),
	};
	vec![first.into(), second]
}

fn split_at_width(text: &str, width: f32) -> (&str, &str) {
	let end = text
		.char_indices()
		.scan(0.0, |used, (i, c)| {
			*used += glyph_width(c);
			Some((i, *used))
		})
		.find(|&(_, used)| used > width)
		.map_or(text.len(), |(i, _)| i);
	text.split_at_checked(end).unwrap_or((text, ""))
}

fn glyph_width(c: char) -> f32 {
	let em = if c.is_ascii() {
		0.6
	} else if c >= '\u{1F000}' {
		1.25
	} else {
		1.0
	};
	em * EM
}

fn shade(alpha: f32) -> Color {
	Color::new(0.0, 0.0, 0.0, alpha)
}
