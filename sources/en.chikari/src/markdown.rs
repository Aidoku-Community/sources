use aidoku::{
	alloc::{String, Vec, string::ToString},
	helpers::string::PlainText,
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{settings, watermark};

// Anything not listed here is escaped, so literal "<NotATag>" prose survives.
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

fn append_raw_text(element: &Element, output: &mut String) {
	if let Some(text) = element.text() {
		output.push_str(&text);
	}
}

// Element-kind nodes pair with the next children entry.
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
	// CommonMark has no strikethrough or underline: emit the content unmarked.
	let marker = match element.tag_name().as_deref() {
		Some("strong" | "b") => "**",
		Some("em" | "i") => "*",
		_ => "",
	};
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
	// CommonMark strips one padding space: pad content touching the fence.
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
		"ul" | "ol" | "menu" => convert_list_to_markdown(element, &tag, output),
		"blockquote" => convert_blockquote_to_markdown(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" | "figure"
		| "hgroup" | "details" | "dl" => convert_block_container(element, output),
		"span" | "li" => convert_children_to_markdown(element, output),
		_ => convert_children_to_markdown(element, output),
	}
}

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

// Quote-aware so a '>' inside an attribute cannot end the tag early.
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

// Escape bare '<': the parser would eat literal prose like "<Maddened Enlightenment>".
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

// Real block markup must not be wrapped in <p>: the parser nests the blocks and doubles the blank lines.
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
	// concat!, not format!: chapter text may contain braces.
	["<div id=\"chikari-root\">", &paragraphs, "</div>"].concat()
}

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
	fn converts_inline_markup_and_escapes_prose() {
		let out = html_to_markdown("The <i>wen was</i> a coin");
		assert_eq!(out, "The *wen was* a coin");
		let out = html_to_markdown("Use *literal* and <strong>bold</strong>.");
		assert_eq!(out, "Use \\*literal\\* and **bold**\\.");
		let out = html_to_markdown("<p><strong> bold </strong></p>");
		assert_eq!(out, "**bold**");
	}

	#[aidoku_test]
	fn keeps_paragraphs_and_lists() {
		let out = html_to_markdown("First <strong>bold</strong>.\n\nSecond <em>italic</em>.");
		assert_eq!(out, "First **bold**\\.\n\nSecond *italic*\\.");
		let out = html_to_markdown(
			"<menu><li>One</li><li>Two</li></menu>\n\n<dl><dt>Term</dt><dd>Definition</dd></dl>\n\n<figure><figcaption>Caption</figcaption></figure>",
		);
		assert_eq!(out, "- One\n- Two\n\nTerm\n\nDefinition\n\nCaption");
		assert_eq!(html_to_markdown("first\r\n\r\nsecond"), "first\n\nsecond");
	}

	#[aidoku_test]
	fn escapes_unknown_tags_and_literal_brackets() {
		let out = html_to_markdown("A <marquee>strange <em>x</em></marquee> tag");
		assert_eq!(out, "A \\<marquee\\>strange *x*\\<\\/marquee\\> tag");
		let out = html_to_markdown("<Maddened Enlightenment> could replenish 30 Intelligence");
		assert_eq!(
			out,
			"\\<Maddened Enlightenment\\> could replenish 30 Intelligence"
		);
	}
}
