use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::html::{Element, Html, Kind},
	prelude::*,
};

pub fn parse(content: &str) -> Result<String> {
	let html = Html::parse(format!("<div id=novel>{content}</div>"))?;
	let root = html
		.select_first("#novel")
		.ok_or(error!("Invalid chapter content"))?;
	let mut markdown = Markdown::default();
	markdown.element(&root);
	markdown.end_line();
	Ok(markdown.paragraphs.join("\n\n"))
}

#[derive(Default)]
struct Markdown {
	paragraphs: Vec<String>,
	line: String,
	open: Vec<&'static str>,
	written: usize,
	just_closed: Vec<&'static str>,
}

impl Markdown {
	fn text(&mut self, text: &str) {
		for (index, part) in text.split('\n').enumerate() {
			if index > 0 {
				self.end_line();
			}
			for c in part.chars() {
				// Whitespace stays outside emphasis markers.
				if c.is_whitespace() {
					if !self.line.is_empty() && !self.line.ends_with(char::is_whitespace) {
						self.line.push(c);
						self.just_closed.clear();
					}
					continue;
				}
				while let Some(&marker) = self.open.get(self.written) {
					if self.just_closed.last() == Some(&marker) {
						self.just_closed.pop();
						self.line.truncate(self.line.len() - marker.len());
					} else {
						self.just_closed.clear();
						self.line.push_str(marker);
					}
					self.written += 1;
				}
				self.just_closed.clear();
				let block = (self.line.is_empty() && matches!(c, '#' | '-' | '+' | '='))
					|| (matches!(c, '.' | ')')
						&& !self.line.is_empty()
						&& self.line.bytes().all(|b| b.is_ascii_digit()));
				// Escaping ordinary punctuation creates excessive inline nodes in long prose.
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
		if self.open.contains(&marker) {
			self.element(element);
			return;
		}
		self.open.push(marker);
		self.element(element);
		if self.written > 0 && self.written == self.open.len() {
			let trailing = self.line.split_off(self.line.trim_end().len());
			self.line.push_str(marker);
			self.line.push_str(&trailing);
			if trailing.is_empty() {
				self.just_closed.push(marker);
			} else {
				self.just_closed.clear();
			}
			self.written -= 1;
		}
		self.open.pop();
	}

	fn end_line(&mut self) {
		self.line.truncate(self.line.trim_end().len());
		// Emphasis cannot span Markdown paragraphs.
		for marker in self.open[..self.written].iter().rev() {
			self.line.push_str(marker);
		}
		self.written = 0;
		self.just_closed.clear();
		if !self.line.is_empty() {
			self.paragraphs.push(core::mem::take(&mut self.line));
		}
	}

	fn element(&mut self, element: &Element) {
		let mut children = element.children();
		for node in element.child_nodes() {
			match node.kind() {
				Kind::TextNode => self.text(&node.text().unwrap_or_default()),
				Kind::Element => {
					let Some(child) = children.next() else {
						continue;
					};
					match child.tag_name().as_deref() {
						Some("br") => self.end_line(),
						Some("strong" | "b") => self.emphasis("**", &child),
						Some("em" | "i") => self.emphasis("*", &child),
						Some("p" | "div" | "center") => {
							self.end_line();
							self.element(&child);
							self.end_line();
						}
						Some("script" | "style") => {}
						_ => self.element(&child),
					}
				}
				_ => {}
			}
		}
	}
}
