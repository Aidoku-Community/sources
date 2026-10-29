//! Chapter body extraction: wenku8 chapter pages hold prose in `#content`,
//! using `<br>` for line breaks and embedding illustrations as
//! `<div class="divimage"><img>`; promotional `<ul>` watermark blocks are
//! interleaved and must be skipped.
//!
//! Illustrations are collected separately rather than emitted as Markdown
//! images: Aidoku's paged text reader cannot load remote image URLs, and a
//! chapter mixing text and image pages would push the reader into image
//! mode. `get_page_list` returns text for prose chapters and image pages
//! for illustration-only chapters.

use aidoku::{
	alloc::{String, vec::Vec},
	imports::html::{Element, Kind},
	prelude::*,
};

use crate::BASE_URL;

/// Append an element's text content to `out`, converting `<br>` to newlines.
/// Images and promotional `<ul>` watermark blocks are skipped. Non-breaking
/// spaces become plain spaces.
pub fn extract_text(element: &Element, out: &mut String) {
	let mut elements = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					out.push_str(&text.replace('\u{a0}', " "));
				}
			}
			Kind::Element => {
				let Some(child) = elements.next() else {
					continue;
				};
				match child.tag_name().as_deref() {
					// Promotional blocks injected into every chapter.
					Some("ul" | "script" | "style" | "img") => continue,
					Some("br") => out.push('\n'),
					// Block boundaries become line breaks so paragraphs do
					// not fuse into a single run of text.
					Some("p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4") => {
						out.push('\n');
						extract_text(&child, out);
						out.push('\n');
					}
					_ => extract_text(&child, out),
				}
			}
			_ => {}
		}
	}
}

/// Collect illustration URLs (`<img src>`) in document order, skipping
/// promotional `<ul>` watermark blocks.
pub fn extract_image_urls(element: &Element, out: &mut Vec<String>) {
	let mut elements = element.children();
	for node in element.child_nodes() {
		if node.kind() != Kind::Element {
			continue;
		}
		let Some(child) = elements.next() else {
			continue;
		};
		match child.tag_name().as_deref() {
			Some("ul" | "script" | "style") => continue,
			Some("img") => {
				if let Some(src) = child.attr("src") {
					out.push(absolute_url(&src));
				}
			}
			_ => extract_image_urls(&child, out),
		}
	}
}

fn absolute_url(href: &str) -> String {
	if href.starts_with("http") {
		href.into()
	} else if let Some(rest) = href.strip_prefix('/') {
		format!("{BASE_URL}/{rest}")
	} else {
		format!("{BASE_URL}/{href}")
	}
}

/// Turn extracted chapter text into reader Markdown: every non-empty line
/// becomes its own paragraph so the reader renders the line structure.
/// Printed-book page markers (`- 12 -`) injected throughout the text are
/// dropped: as Markdown they parse as list items / code blocks and break
/// the layout.
pub fn text_to_markdown(raw: &str) -> String {
	raw.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty() && !is_page_marker(line))
		.collect::<Vec<_>>()
		.join("\n\n")
}

/// Printed page markers look like `- 1 -` (dashes with a page number,
/// surrounded by non-breaking spaces in the source HTML).
fn is_page_marker(line: &str) -> bool {
	let Some(inner) = line.strip_prefix('-').and_then(|s| s.strip_suffix('-')) else {
		return false;
	};
	let middle = inner.trim();
	!middle.is_empty() && middle.chars().all(|c| c.is_ascii_digit())
}
