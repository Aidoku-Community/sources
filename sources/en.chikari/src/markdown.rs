//! Convert chapter HTML to Aidoku Markdown.
//!
//! Chikari bodies are line-based with inline tags only (`em`, `strong`).
//! Converting one paragraph at a time keeps blank lines as paragraph breaks
//! whatever the parser does with whitespace between elements.

use aidoku::{
	alloc::{String, Vec, string::ToString},
	helpers::string::PlainText,
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{settings, watermark};

/// Tag names `convert_element_to_markdown` translates. The list is that
/// function's own vocabulary: every arm is listed and nothing else is, so an
/// angle-bracket sequence outside it is prose rather than a tag. Chapters carry
/// literal `<Maddened Enlightenment>`, which must survive.
const MARKUP_TAGS: &[&str] = &[
	"a",
	"article",
	"aside",
	"b",
	"blockquote",
	"br",
	"code",
	"del",
	"div",
	"em",
	"footer",
	"h1",
	"h2",
	"h3",
	"h4",
	"h5",
	"h6",
	"header",
	"hr",
	"i",
	"img",
	"li",
	"main",
	"ol",
	"p",
	"pre",
	"s",
	"section",
	"span",
	"strike",
	"strong",
	"u",
	"ul",
];

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

/// Append an element's descendant text unescaped: backslashes inside code
/// spans and fences are literal output.
fn append_raw_text(element: &Element, output: &mut String) {
	if let Some(text) = element.text() {
		output.push_str(&text);
	}
}

/// Append an element's text and children in document order.
///
/// Text nodes come from `child_nodes` and tag names from `children`, so
/// element-kind nodes pair with the next `children` entry.
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
/// following bare text starts a new paragraph.
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
	let level = tag.as_bytes()[1] - b'0';
	for _ in 0..level {
		output.push('#');
	}
	output.push(' ');
	convert_children_to_markdown(element, output);
	output.push_str("\n\n");
}

fn convert_emphasis(element: &Element, output: &mut String) {
	// Trim so surrounding whitespace stays outside the markers;
	// `** bold **` is not recognized as emphasis by Markdown.
	let mut inner = String::default();
	convert_children_to_markdown(element, &mut inner);
	let trimmed = inner.trim();
	if !trimmed.is_empty() {
		let tag = element.tag_name().unwrap_or_default();
		let marker = match tag.as_str() {
			"strong" | "b" => "**",
			"em" | "i" => "*",
			"u" => "__",
			_ => "~~",
		};
		output.push_str(marker);
		output.push_str(trimmed);
		output.push_str(marker);
	}
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
	if let Some(src) = element.attr("src") {
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
		"p" => {
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
		"ul" | "ol" => convert_list_to_markdown(element, &tag, output),
		"blockquote" => convert_blockquote_to_markdown(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" => {
			convert_block_container(element, output)
		}
		// Inline containers carry no block semantics.
		"span" | "li" => convert_children_to_markdown(element, output),
		// Unknown tags: recurse so their prose is still emitted.
		_ => convert_children_to_markdown(element, output),
	}
}

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

/// Prefixing every emitted line with `> ` keeps multi-block quotes valid
/// Markdown.
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

/// Byte index just past the `>` when `start` opens recognized markup, else
/// `None` for literal text.
///
/// Attribute values are skipped with quote awareness so a `>` inside one
/// cannot end the tag early.
fn markup_tag_end(input: &str, start: usize) -> Option<usize> {
	let bytes = input.as_bytes();
	let mut index = start + 1;
	if bytes.get(index) == Some(&b'/') {
		index += 1;
	}
	let name_start = index;
	while let Some(&byte) = bytes.get(index) {
		if byte.is_ascii_alphanumeric() || byte == b'-' {
			index += 1;
		} else {
			break;
		}
	}
	if index == name_start {
		return None;
	}
	let name = input[name_start..index].to_ascii_lowercase();
	if !MARKUP_TAGS.contains(&name.as_str()) {
		return None;
	}
	let mut quote = 0u8;
	while let Some(&byte) = bytes.get(index) {
		if quote != 0 {
			if byte == quote {
				quote = 0;
			}
		} else if byte == b'"' || byte == b'\'' {
			quote = byte;
		} else if byte == b'>' {
			return Some(index + 1);
		}
		index += 1;
	}
	None
}

/// Escape `<` on every angle bracket that does not open recognized markup.
/// Otherwise the parser reads literal prose like `<Maddened Enlightenment>`
/// as an unknown element and keeps only its attributes.
fn protect_literal_angle_brackets(input: &str) -> String {
	let mut output = String::default();
	let mut index = 0;
	while index < input.len() {
		if input.as_bytes()[index] == b'<' {
			if let Some(end) = markup_tag_end(input, index) {
				output.push_str(&input[index..end]);
				index = end;
			} else {
				output.push_str("&lt;");
				index += 1;
			}
		} else {
			let ch = input[index..].chars().next().unwrap_or('\u{FFFD}');
			output.push(ch);
			index += ch.len_utf8();
		}
	}
	output
}

/// Convert one blank-line-delimited paragraph of chapter HTML to Markdown.
///
/// The paragraph is wrapped in a container first: the fragment root cannot be
/// traversed, a selected wrapper can.
fn paragraph_to_markdown(paragraph: &str) -> String {
	// Concatenated rather than formatted: chapter content may contain
	// braces, which format! would treat as placeholders.
	let paragraph = protect_literal_angle_brackets(paragraph);
	let wrapped = ["<div id=\"chikari-root\">", &paragraph, "</div>"].concat();
	let Ok(doc) = Html::parse_fragment(wrapped) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#chikari-root") {
		convert_children_to_markdown(&root, &mut output);
	}
	output.trim().to_string()
}

/// Convert chapter HTML to Aidoku Markdown.
///
/// Only text nodes are escaped; the Markdown this converter emits is left
/// alone. Watermark removal runs last so it sees the final block layout.
pub fn html_to_markdown(html: &str) -> String {
	let mut blocks: Vec<String> = Vec::new();
	let mut current = String::default();
	for line in html.lines() {
		if line.trim().is_empty() {
			if !current.is_empty() {
				blocks.push(paragraph_to_markdown(&current));
				current.clear();
			}
		} else {
			if !current.is_empty() {
				current.push('\n');
			}
			current.push_str(line);
		}
	}
	if !current.is_empty() {
		blocks.push(paragraph_to_markdown(&current));
	}
	let output = blocks.join("\n\n");
	if settings::hide_watermark() {
		watermark::strip(output.trim())
	} else {
		output.trim().to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn converts_strong_and_em() {
		let out = html_to_markdown("<strong>bold</strong> and <em>italic</em>");
		assert_eq!(out, "**bold** and *italic*");
	}

	#[aidoku_test]
	fn converts_i_alias_without_spaces() {
		// Live shape: emphasis glued to surrounding prose, no spacing.
		let out = html_to_markdown("The <i>wen was</i> a coin");
		assert_eq!(out, "The *wen was* a coin");
	}

	#[aidoku_test]
	fn keeps_blank_line_paragraph_separation() {
		let out = html_to_markdown("First <strong>bold</strong>.\n\nSecond <em>italic</em>.");
		assert_eq!(out, "First **bold**\\.\n\nSecond *italic*\\.");
	}

	#[aidoku_test]
	fn escapes_literal_markdown_but_not_emitted_markers() {
		let out = html_to_markdown("Use *literal* and <strong>bold</strong>.");
		assert_eq!(out, "Use \\*literal\\* and **bold**\\.");
	}

	#[aidoku_test]
	fn unknown_tags_are_escaped_not_parsed() {
		// A tag outside MARKUP_TAGS is prose, so the parser must not read it as
		// an element: the brackets are escaped and the content between them
		// survives.
		let out = html_to_markdown("A <small>strange <em>x</em></small> tag");
		assert_eq!(out, "A \\<small\\>strange *x*\\<\\/small\\> tag");
	}

	#[aidoku_test]
	fn preserves_literal_angle_bracket_prose() {
		// Sampled live (genetic-ascension ch. 101/401): the brackets are
		// prose, not tags, and must not be consumed by the parser.
		let out = html_to_markdown("<Maddened Enlightenment> could replenish 30 Intelligence");
		assert_eq!(
			out,
			"\\<Maddened Enlightenment\\> could replenish 30 Intelligence"
		);
	}

	#[aidoku_test]
	fn keeps_literal_prose_alongside_real_markup() {
		let out = html_to_markdown(
			"It is a <Dark Exploration Records> chapter.\n\n<strong>Bold</strong>",
		);
		assert_eq!(
			out,
			"It is a \\<Dark Exploration Records\\> chapter\\.\n\n**Bold**"
		);
	}

	#[aidoku_test]
	fn handles_crlf_paragraph_breaks() {
		let out = html_to_markdown("first\r\n\r\nsecond");
		assert_eq!(out, "first\n\nsecond");
	}

	#[aidoku_test]
	fn converts_bare_text_without_tags() {
		let out = html_to_markdown("Just \"quoted\" prose — no tags.");
		assert_eq!(out, "Just \\\"quoted\\\" prose — no tags\\.");
	}

	#[aidoku_test]
	fn empty_body_converts_to_nothing() {
		assert_eq!(html_to_markdown(""), "");
		assert_eq!(html_to_markdown("  \n\n  "), "");
	}

	#[aidoku_test]
	fn trims_inline_whitespace() {
		let out = html_to_markdown("<p><strong> bold </strong></p>");
		assert_eq!(out, "**bold**");
	}

	#[aidoku_test]
	fn dispatcher_arms_are_known_markup_tags() {
		const SOURCE: &str = include_str!("markdown.rs");
		assert!(SOURCE.contains("fn convert_element_to_markdown"));
		let rest = &SOURCE[SOURCE.find("fn convert_element_to_markdown").unwrap_or(0)..];
		let end = rest.find("\nfn ").unwrap_or_else(|| rest.len());
		let body = &rest[..end];
		let mut missing = String::default();
		let bytes = body.as_bytes();
		let mut i = 0;
		while i < bytes.len() {
			if bytes[i] == b'"' {
				let mut k = i + 1;
				while k < bytes.len() && bytes[k] != b'"' {
					k += 1;
				}
				let lit = &body[i + 1..k];
				if !lit.is_empty()
					&& lit.bytes().all(|c| c.is_ascii_alphanumeric())
					&& !MARKUP_TAGS.contains(&lit)
				{
					if !missing.is_empty() {
						missing.push_str(", ");
					}
					missing.push_str(lit);
					missing.push_str(" missing from MARKUP_TAGS");
				}
				i = k + 1;
			} else {
				i += 1;
			}
		}
		assert_eq!(missing.as_str(), "");
	}
}
