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

/// Tag names treated as markup rather than prose. Every dispatcher arm is
/// listed, plus the plausible-but-untranslated tags; anything else is escaped so
/// literal `<Maddened Enlightenment>` survives. A tag listed here without an arm
/// falls through to the dispatcher's catch-all, which keeps the content and drops
/// the tag - the intended handling where CommonMark has no equivalent.
const MARKUP_TAGS: &[&str] = &[
	"a",
	"abbr",
	"article",
	"aside",
	"b",
	"blockquote",
	"br",
	"caption",
	"center",
	"cite",
	"code",
	"col",
	"colgroup",
	"dd",
	"del",
	"details",
	"div",
	"dl",
	"dt",
	"em",
	"figcaption",
	"figure",
	"font",
	"footer",
	"h1",
	"h2",
	"h3",
	"h4",
	"h5",
	"h6",
	"header",
	"hgroup",
	"hr",
	"i",
	"img",
	"ins",
	"kbd",
	"li",
	"main",
	"mark",
	"menu",
	"ol",
	"p",
	"pre",
	"q",
	"rp",
	"rt",
	"ruby",
	"s",
	"samp",
	"section",
	"small",
	"span",
	"strike",
	"strong",
	"sub",
	"summary",
	"sup",
	"table",
	"tbody",
	"td",
	"tfoot",
	"th",
	"thead",
	"time",
	"tr",
	"tt",
	"u",
	"ul",
	"var",
	"wbr",
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

/// Wrap each blank-line-delimited block in `<p>`, returning a single fragment.
///
/// Chapter bodies arrive as plain text with blank-line paragraph breaks rather
/// than as HTML, so the blocks have to become elements before the converter can
/// walk the whole chapter in one pass. The container is required because the
/// fragment root cannot be traversed, a selected wrapper can.
/// A segment starting with real block markup must not be nested in an
/// injected `<p>`: the parser would close one block inside the other and the
/// emitted blank lines would double.
fn starts_with_block_markup(segment: &str) -> bool {
	let Some(tag) = segment.trim_start().strip_prefix('<').map(|rest| {
		let end = rest
			.find(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
			.unwrap_or(rest.len());
		rest[..end].to_ascii_lowercase()
	}) else {
		return false;
	};
	matches!(
		tag.as_str(),
		"blockquote"
			| "details"
			| "div" | "dl"
			| "dt" | "dd"
			| "figcaption"
			| "figure"
			| "h1" | "h2"
			| "h3" | "h4"
			| "h5" | "h6"
			| "hgroup"
			| "hr" | "menu"
			| "ol" | "p"
			| "pre" | "summary"
			| "ul"
	)
}

fn push_block_html(paragraphs: &mut String, block: &str) {
	let protected = protect_literal_angle_brackets(block);
	if starts_with_block_markup(&protected) {
		paragraphs.push_str(&protected);
	} else {
		paragraphs.push_str("<p>");
		paragraphs.push_str(&protected);
		paragraphs.push_str("</p>");
	}
}

fn assemble_paragraphs(input: &str) -> String {
	let mut paragraphs = String::default();
	let mut current = String::default();
	for line in input.lines() {
		if line.trim().is_empty() {
			if !current.is_empty() {
				push_block_html(&mut paragraphs, &current);
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
		push_block_html(&mut paragraphs, &current);
	}
	// Concatenated rather than formatted: chapter content may contain
	// braces, which format! would treat as placeholders.
	["<div id=\"chikari-root\">", &paragraphs, "</div>"].concat()
}

/// Convert chapter HTML to Aidoku Markdown.
///
/// Only text nodes are escaped; the Markdown this converter emits is left
/// alone. Watermark removal runs last so it sees the final block layout.
pub fn html_to_markdown(html: &str) -> String {
	let Ok(doc) = Html::parse_fragment(assemble_paragraphs(html)) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#chikari-root") {
		convert_children_to_markdown(&root, &mut output);
	}
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
	fn unmarked_tags_keep_their_content_only() {
		let out = html_to_markdown(
			"<p><u>underlined</u>, <del>gone</del>, <s>struck</s>, <sup>2</sup>, <ruby>kanji<rt>kana</rt></ruby></p>",
		);
		assert_eq!(out, "underlined\\, gone\\, struck\\, 2\\, kanjikana");
	}

	#[aidoku_test]
	fn routes_prose_block_variants_to_existing_layout() {
		let out = html_to_markdown(
			"<menu><li>One</li><li>Two</li></menu>\n\n<dl><dt>Term</dt><dd>Definition</dd></dl>\n\n<figure><figcaption>Caption</figcaption></figure>",
		);
		assert_eq!(out, "- One\n- Two\n\nTerm\n\nDefinition\n\nCaption");
	}

	#[aidoku_test]
	fn handles_inline_markup_split_by_a_paragraph_break() {
		// The chapter body is plain text, so a tag can straddle a blank line.
		// Formatting elements survive the `</p>` in the parser's active list, so
		// the orphaned close tag still applies to the paragraph after it. Parse
		// per block instead and the emphasis would be dropped.
		let out = html_to_markdown("First <em>starts\n\nand closes</em> here.");
		assert_eq!(out, "First *starts*\n\n*and closes* here\\.");
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
		let out = html_to_markdown("A <marquee>strange <em>x</em></marquee> tag");
		assert_eq!(out, "A \\<marquee\\>strange *x*\\<\\/marquee\\> tag");
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
