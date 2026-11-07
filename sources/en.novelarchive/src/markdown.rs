use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::{String, Vec, format},
	imports::html::{Element, Html, Kind},
};
use core::mem::take;

// One escaped paragraph per line. The app's Markdown renderer overflows its stack past a few
// hundred inline nodes in one paragraph, and every escape is a node, so only syntax is escaped.
#[derive(Default)]
struct Markdown {
	paragraphs: Vec<String>,
	line: String,
	// Bold and italic markers open here; a line writes them before its first character.
	open: Vec<&'static str>,
	written: usize,
	// A marker that closed with nothing after it, which reopening continues ("**A****B**" isn't bold).
	just_closed: Option<&'static str>,
}

impl Markdown {
	fn text(&mut self, text: &str) {
		for (index, part) in text.split('\n').enumerate() {
			if index > 0 {
				self.end_line();
			}
			for c in part.chars() {
				// Spaces stay outside markers, since "** word**" isn't bold.
				if c.is_whitespace() {
					if !self.line.is_empty() {
						self.line.push(c);
						self.just_closed = None;
					}
					continue;
				}
				while let Some(&marker) = self.open.get(self.written) {
					if self.just_closed.take() == Some(marker) {
						self.line.truncate(self.line.len() - marker.len());
					} else {
						self.line.push_str(marker);
					}
					self.written += 1;
				}
				self.just_closed = None;
				// "-", "#" or "1." at the start would begin a list or heading.
				let block = (self.line.is_empty() && matches!(c, '#' | '-' | '+' | '='))
					|| (matches!(c, '.' | ')')
						&& !self.line.is_empty()
						&& self.line.bytes().all(|b| b.is_ascii_digit()));
				if block
					|| matches!(
						c,
						'\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '&' | '~' | '|'
					) {
					self.line.push('\\');
				}
				self.line.push(c);
			}
		}
	}

	fn emphasis(&mut self, marker: &'static str, element: &Element) {
		self.open.push(marker);
		self.element(element);
		if self.written > 0 && self.written == self.open.len() {
			let trailing = self.line.split_off(self.line.trim_end().len());
			self.line.push_str(marker);
			self.line.push_str(&trailing);
			self.just_closed = trailing.is_empty().then_some(marker);
			self.written -= 1;
		}
		self.open.pop();
	}

	fn end_line(&mut self) {
		self.line.truncate(self.line.trim_end().len());
		for marker in self.open[..self.written].iter().rev() {
			self.line.push_str(marker);
		}
		self.written = 0;
		self.just_closed = None;
		if !self.line.is_empty() {
			self.paragraphs.push(take(&mut self.line));
		}
	}

	fn block(&mut self, markdown: String) {
		self.end_line();
		self.paragraphs.push(markdown);
	}

	// Text comes from child_nodes and tags from children, whose elements the test runner can name.
	fn element(&mut self, element: &Element) {
		let mut children = element.children();
		for node in element.child_nodes() {
			match node.kind() {
				Kind::TextNode => self.text(&node.text().unwrap_or_default()),
				Kind::Element => {
					if let Some(child) = children.next() {
						self.child(&child);
					}
				}
				_ => {}
			}
		}
	}

	fn child(&mut self, child: &Element) {
		let tag = child.tag_name().unwrap_or_default();
		match tag.as_str() {
			"br" => self.end_line(),
			"hr" => self.block("---".into()),
			// Mirror images are served by the site under /api/novels/{id}/sources/{mirror}/img; an
			// inline data: image would be its whole file in the text.
			"img" => {
				if let Some(src) = child.attr("abs:src").filter(|src| src.starts_with("http")) {
					self.block(format!("![]({src})"));
				}
			}
			"strong" | "b" => self.emphasis("**", child),
			"em" | "i" => self.emphasis("*", child),
			"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
				self.end_line();
				let hashes = &"######"[..usize::from(tag.as_bytes()[1] - b'0')];
				self.line.push_str(hashes);
				self.line.push(' ');
				self.element(child);
				if self.line.trim_end() == hashes {
					self.line.clear();
				}
				self.end_line();
			}
			"p" | "div" | "center" => {
				self.end_line();
				self.element(child);
				self.end_line();
			}
			_ => self.element(child),
		}
	}

	fn finish(mut self) -> String {
		self.end_line();
		self.paragraphs.join("\n\n")
	}
}

pub fn paragraphs(text: &str) -> String {
	let mut markdown = Markdown::default();
	markdown.text(text);
	markdown.finish()
}

// The site inserts a mirror's copy as HTML. Its text nodes don't keep line breaks, so a copy
// without markup stays text and keeps its lines as paragraphs.
pub fn html_to_markdown(content: &str) -> Result<String> {
	if ![
		"<p>", "<p ", "<br", "<em>", "<i>", "<strong>", "<b>", "<div",
	]
	.iter()
	.any(|tag| content.contains(tag))
	{
		return Ok(paragraphs(content));
	}
	let mut markdown = Markdown::default();
	if let Some(root) = Html::parse_fragment_with_url(content, BASE_URL)?.select_first("html") {
		markdown.element(&root);
	}
	Ok(markdown.finish())
}
