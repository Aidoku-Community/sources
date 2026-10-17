//! Convert chapter HTML to Aidoku Markdown.
//!
//! Adapted from en.novelbuddy's markdown converter: Aidoku renders
//! `PageContent::Text` as Markdown, so chapter HTML must be converted
//! rather than stripped.

use aidoku::{
	alloc::{String, Vec, string::ToString},
	helpers::string::PlainText,
	imports::html::{Element, Kind},
};
use core::fmt::Write as _;

fn longest_backtick_run(text: &str) -> usize {
	let mut longest = 0;
	let mut current = 0;
	for ch in text.chars() {
		if ch == '`' {
			current += 1;
			longest = longest.max(current);
		} else {
			current = 0;
		}
	}
	longest
}

/// Append an element's full descendant text without Markdown escaping:
/// backslashes inside code spans and fenced blocks are literal output.
fn append_raw_text(element: &Element, output: &mut String) {
	if let Some(text) = element.text() {
		output.push_str(&text);
	}
}

/// Append an element's direct text and child elements in document order.
///
/// Text nodes are only reachable via `child_nodes`, tag names only via
/// `children`, so element-kind nodes pair with the next `children` entry.
fn convert_children_to_markdown(element: &Element, output: &mut String) {
	let mut elements = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					output.push_str(&text.escape_markdown());
				}
			}
			Kind::Element => {
				if let Some(child) = elements.next() {
					convert_element_to_markdown(&child, output);
				}
			}
			_ => {}
		}
	}
}

/// Terminate any open inline run before a block element, so a block
/// following bare text starts a new paragraph. No-op when already separated.
fn break_before_block(output: &mut String) {
	if output.is_empty() {
		return;
	}
	while !output.ends_with("\n\n") {
		output.push('\n');
	}
}

fn convert_heading(element: &Element, output: &mut String) {
	break_before_block(output);
	let tag = element.tag_name().unwrap_or_default();
	let level = tag.as_bytes()[1].saturating_sub(b'0');
	for _ in 0..level.min(6) {
		output.push('#');
	}
	output.push(' ');
	convert_children_to_markdown(element, output);
	output.push_str("\n\n");
}

fn convert_emphasis(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children_to_markdown(element, &mut inner);
	let trimmed = inner.trim();
	if trimmed.is_empty() {
		return;
	}
	// CommonMark has neither strikethrough nor underline: `~~` is inert text
	// and `__` is strong emphasis. Emit the content unmarked.
	let marker = match element.tag_name().as_deref() {
		Some("strong" | "b") => "**",
		Some("em" | "i") => "*",
		_ => "",
	};
	// Surrounding whitespace must stay outside the markers: `** bold **` is not
	// recognized as emphasis.
	output.push_str(marker);
	output.push_str(trimmed);
	output.push_str(marker);
}

fn convert_inline_code(element: &Element, output: &mut String) {
	let mut raw = String::default();
	append_raw_text(element, &mut raw);
	let ticks = longest_backtick_run(&raw) + 1;
	for _ in 0..ticks {
		output.push('`');
	}
	// Space-pad content touching a delimiter boundary: CommonMark
	// strips one space from both sides, restoring the text verbatim.
	if raw.starts_with('`') || raw.ends_with('`') {
		output.push(' ');
		output.push_str(&raw);
		output.push(' ');
	} else {
		output.push_str(&raw);
	}
	for _ in 0..ticks {
		output.push('`');
	}
}

fn convert_fenced_code(element: &Element, output: &mut String) {
	break_before_block(output);
	let mut raw = String::default();
	append_raw_text(element, &mut raw);
	let fence = "`".repeat(3.max(longest_backtick_run(&raw) + 1));
	output.push_str(&fence);
	output.push('\n');
	output.push_str(&raw);
	if !raw.ends_with('\n') {
		output.push('\n');
	}
	output.push_str(&fence);
	output.push_str("\n\n");
}

fn convert_image(element: &Element, output: &mut String) {
	break_before_block(output);
	if let Some(src) = element.attr("src").or_else(|| element.attr("data-src")) {
		let alt = element.attr("alt").unwrap_or_default();
		let _ = writeln!(output, "![{alt}]({src})");
		output.push('\n');
	}
}

fn convert_link(element: &Element, output: &mut String) {
	if let Some(href) = element.attr("href") {
		output.push('[');
		convert_children_to_markdown(element, output);
		let _ = write!(output, "]({href})");
	} else {
		convert_children_to_markdown(element, output);
	}
}

fn convert_block_container(element: &Element, output: &mut String) {
	break_before_block(output);
	convert_children_to_markdown(element, output);
	if !output.ends_with("\n\n") && !output.ends_with('\n') {
		output.push('\n');
	}
}

fn convert_element_to_markdown(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();
	match tag.as_str() {
		"p" | "figcaption" | "dt" | "dd" | "summary" => {
			break_before_block(output);
			convert_children_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"br" => output.push_str("  \n"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => convert_heading(element, output),
		"strong" | "b" | "em" | "i" | "u" | "s" | "strike" | "del" => {
			convert_emphasis(element, output)
		}
		"code" => convert_inline_code(element, output),
		"pre" => convert_fenced_code(element, output),
		"img" => convert_image(element, output),
		"a" => convert_link(element, output),
		"hr" => {
			break_before_block(output);
			output.push_str("---\n\n")
		}
		// menu is the unordered-list alternative.
		"ul" | "ol" | "menu" => convert_list_to_markdown(element, &tag, output),
		"blockquote" => convert_blockquote_to_markdown(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" | "figure"
		| "hgroup" | "details" | "dl" => convert_block_container(element, output),
		// Inline containers carry no block semantics. `rt` annotation text is
		// kept so `<ruby>深渊侧<rt>abyss side</rt></ruby>` reads naturally.
		"span" | "li" | "ruby" | "rp" | "rt" => convert_children_to_markdown(element, output),
		// Unknown tags: recurse so their prose is still emitted.
		_ => convert_children_to_markdown(element, output),
	}
}

/// Render list items as Markdown bullets or numbered entries.
///
/// Non-item children are filtered out before enumeration so stray markup
/// cannot shift the sequence.
fn convert_list_to_markdown(element: &Element, tag: &str, output: &mut String) {
	break_before_block(output);
	let items: Vec<_> = element
		.children()
		.filter(|child| child.tag_name().as_deref() == Some("li"))
		.collect();
	for (index, item) in items.iter().enumerate() {
		if tag == "ol" {
			let _ = write!(output, "{}. ", index + 1);
		} else {
			output.push_str("- ");
		}
		convert_children_to_markdown(item, output);
		output.push('\n');
	}
	output.push('\n');
}

/// Render a blockquote by prefixing every emitted line with `> `, keeping
/// multi-block quotes valid Markdown.
fn convert_blockquote_to_markdown(element: &Element, output: &mut String) {
	break_before_block(output);
	let mut quoted = String::default();
	convert_children_to_markdown(element, &mut quoted);
	for (index, line) in quoted.trim_end().lines().enumerate() {
		if index > 0 {
			output.push('\n');
		}
		output.push_str("> ");
		output.push_str(line);
	}
	output.push_str("\n\n");
}

/// Convert a single chapter paragraph element to a Markdown string.
pub fn paragraph_to_markdown(paragraph: &Element) -> String {
	let mut output = String::default();
	convert_children_to_markdown(paragraph, &mut output);
	output.trim().to_string()
}
