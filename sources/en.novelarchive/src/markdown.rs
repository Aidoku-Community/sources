use aidoku::{
	alloc::{String, Vec, string::ToString},
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{settings, watermark};

const VOID_TAGS: &[&str] = &["<br", "<img", "<hr"];

fn has_markup(input: &str) -> bool {
	input.contains("</") || VOID_TAGS.iter().any(|tag| input.contains(tag))
}

fn escape_markdown_text(text: &str) -> String {
	let mut output = String::default();
	for (index, line) in text.lines().enumerate() {
		if index > 0 {
			output.push_str("  \n");
		}
		output.push_str(&escape_line(line));
	}
	output
}

fn escape_line(line: &str) -> String {
	let trimmed = line.trim_start();
	let indent = &line[..line.len() - trimmed.len()];
	let marker = ordered_marker_escape_index(trimmed);
	let mut escaped = String::default();
	escaped.push_str(indent);
	for (index, character) in trimmed.char_indices() {
		if Some(index) == marker || character == '<' {
			escaped.push('\\');
		}
		escaped.push(character);
	}
	escaped
}

fn ordered_marker_escape_index(line: &str) -> Option<usize> {
	let digits = line.chars().take_while(char::is_ascii_digit).count();
	if digits > 0 && line[digits..].starts_with(['.', ')']) {
		return Some(digits);
	}
	None
}

fn escape_paragraph(block: &str) -> String {
	let mut escaped = String::default();
	for character in block.chars() {
		match character {
			'&' => escaped.push_str("&amp;"),
			'<' => escaped.push_str("&lt;"),
			_ => escaped.push(character),
		}
	}
	escaped
}

fn push_paragraph(paragraphs: &mut String, block: &str) {
	paragraphs.push_str("<p>");
	paragraphs.push_str(&escape_paragraph(block));
	paragraphs.push_str("</p>");
}

fn assemble_paragraphs(input: &str) -> String {
	let mut paragraphs = String::default();
	let mut current = String::default();
	for line in input.lines() {
		if line.trim().is_empty() {
			if !current.is_empty() {
				push_paragraph(&mut paragraphs, &current);
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
		push_paragraph(&mut paragraphs, &current);
	}
	paragraphs
}

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
					output.push_str(&escape_markdown_text(&text));
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

pub fn to_markdown(input: &str) -> String {
	let body = if has_markup(input) {
		input.to_string()
	} else {
		assemble_paragraphs(input)
	};
	// The fragment root cannot be traversed; a selected wrapper can.
	// Concatenated, not formatted: chapter text may contain braces.
	let wrapped = ["<div id=\"md-root\">", body.as_str(), "</div>"].concat();
	let Ok(doc) = Html::parse_fragment(wrapped) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#md-root") {
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
	fn converts_inline_markup() {
		let cases = [
			(
				"<p>A <strong>bold</strong>, <em>italic</em>, <u>underlined</u>, and <del>gone</del>.</p>",
				"A **bold**, *italic*, underlined, and gone.",
			),
			(
				"<p><u>underlined</u>, <del>gone</del>, <s>struck</s>, <sup>2</sup>, <ruby>kanji<rt>kana</rt></ruby></p>",
				"underlined, gone, struck, 2, kanjikana",
			),
			(
				"<p>Tom &amp; Jerry &mdash; use *literal*, _text_, and ~~this~~</p>",
				"Tom & Jerry — use *literal*, _text_, and ~~this~~",
			),
			(
				"<p><strong> bold </strong> and <em> italic </em></p>",
				"**bold** and *italic*",
			),
			("<p>Hello <span>world</span> end</p>", "Hello world end"),
			(
				"<p>Line one <b>bold</b> line two</p>",
				"Line one **bold** line two",
			),
		];
		for (html, expected) in cases {
			assert_eq!(to_markdown(html), expected);
		}
	}

	#[aidoku_test]
	fn lays_out_blocks() {
		let cases = [
			(
				"<menu><li>One</li><li>Two</li></menu><dl><dt>Term</dt><dd>Definition</dd></dl>",
				"- One\n- Two\n\nTerm\n\nDefinition",
			),
			(
				"<h2>Chapter 1</h2><p>First<br>second</p><p>Third</p>",
				"## Chapter 1\n\nFirst  \nsecond\n\nThird",
			),
			(
				"<div style=\"margin:0;padding:0;border:0;font-size:0;line-height:0\"></div><div> </div><p>Real content</p>",
				"Real content",
			),
			("<div>text in div</div>", "text in div"),
			(
				"<div> Chapter 218: Ather's Perspective\n<p> He woke up as he always did.</p></div>",
				"Chapter 218: Ather's Perspective\n\n He woke up as he always did.",
			),
			("<div>first</div><div>second</div>", "first\n\nsecond"),
			("<ul><li>a</li><li>b</li></ul>", "- a\n- b"),
			("<ol><li>a</li><p>note</p><li>b</li></ol>", "1. a\n2. b"),
			("<blockquote>cite</blockquote>", "> cite"),
			(
				"\n<blockquote><p>one</p><p>two</p></blockquote>",
				"> one\n> \n> two",
			),
		];
		for (html, expected) in cases {
			assert_eq!(to_markdown(html), expected);
		}
	}

	#[aidoku_test]
	fn renders_links_images_rules_and_code() {
		let html = "<p>Use <code>aidoku</code></p><hr><p><a href=\"https://example.com\">site</a></p><img src=\"https://example.com/i.png\" alt=\"pic\">";
		let out = to_markdown(html);
		assert!(out.contains("`aidoku`"), "inline code: {out}");
		assert!(out.contains("---"), "horizontal rule: {out}");
		assert!(out.contains("[site](https://example.com)"), "link: {out}");
		assert!(
			out.contains("![pic](https://example.com/i.png)"),
			"image: {out}"
		);

		let out = to_markdown("<p>use <code>a_b-c *x*</code></p><pre>let s = \"a_b\";</pre>");
		assert!(out.contains("`a_b-c *x*`"), "code span: {out}");
		assert!(out.contains("let s = \"a_b\";"), "fenced block: {out}");

		let out = to_markdown("<p><code>`left</code> and <code>right`</code></p>");
		assert!(out.contains("`` `left ``"), "leading backtick: {out}");
		assert!(out.contains("`` right` ``"), "trailing backtick: {out}");
	}

	#[aidoku_test]
	fn keeps_markdown_the_text_carries() {
		let out = to_markdown("First *bzz* line.\n\nSecond **bold** line.");
		assert_eq!(out, "First *bzz* line.\n\nSecond **bold** line.");
		let out = to_markdown("One.\n\n***\n\nTwo.");
		assert_eq!(out, "One.\n\n***\n\nTwo.");
		let out = to_markdown("Use `code`, ~~gone~~, [site](https://example.com) and |a|b|.");
		assert_eq!(
			out,
			"Use `code`, ~~gone~~, [site](https://example.com) and |a|b|."
		);
		let out = to_markdown("- item\n+ item\n# head\n| cell");
		assert_eq!(out, "- item  \n+ item  \n# head  \n| cell");
	}

	#[aidoku_test]
	fn escapes_what_would_change_structure() {
		let out = to_markdown("1. The Beginning\n2) Next");
		assert_eq!(out, "1\\. The Beginning  \n2\\) Next");
		let out = to_markdown("<Imperial Rieta Orphanage>\n\nHe shrugged.");
		assert_eq!(out, "\\<Imperial Rieta Orphanage>\n\nHe shrugged.");
		let out = to_markdown("Tom & Jerry, 5 < 10");
		assert_eq!(out, "Tom & Jerry, 5 \\< 10");
	}
}
