//! Convert chapter HTML to Aidoku Markdown.

use aidoku::{
	alloc::{String, Vec, string::ToString},
	helpers::string::PlainText,
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{settings, watermark};

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

fn convert_element_to_markdown(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();
	match tag.as_str() {
		"p" => {
			break_before_block(output);
			convert_children_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"br" => output.push_str("  \n"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
			break_before_block(output);
			let level = tag.as_bytes()[1] - b'0';
			for _ in 0..level {
				output.push('#');
			}
			output.push(' ');
			convert_children_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"strong" | "b" | "em" | "i" => {
			// Trim so surrounding whitespace stays outside the markers;
			// `** bold **` is not recognized as emphasis by Markdown.
			let mut inner = String::default();
			convert_children_to_markdown(element, &mut inner);
			let trimmed = inner.trim();
			if !trimmed.is_empty() {
				let marker = if matches!(tag.as_str(), "strong" | "b") {
					"**"
				} else {
					"*"
				};
				output.push_str(marker);
				output.push_str(trimmed);
				output.push_str(marker);
			}
		}
		// CommonMark has neither strikethrough nor underline, and Rakuyomi
		// compiles with GFM off: `~~` would print literally and `__` would read
		// as strong. Emit the content unmarked.
		"u" | "s" | "strike" | "del" => convert_children_to_markdown(element, output),
		"code" => {
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
		"pre" => {
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
		"img" => {
			break_before_block(output);
			if let Some(src) = element.attr("src") {
				let alt = element.attr("alt").unwrap_or_default();
				let _ = write!(output, "![{alt}]({src})\n\n");
			}
		}
		"a" => {
			if let Some(href) = element.attr("href") {
				output.push('[');
				convert_children_to_markdown(element, output);
				let _ = write!(output, "]({href})");
			} else {
				convert_children_to_markdown(element, output);
			}
		}
		"hr" => {
			break_before_block(output);
			output.push_str("---\n\n")
		}
		"ul" | "ol" => convert_list_to_markdown(element, &tag, output),
		"blockquote" => convert_blockquote_to_markdown(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" => {
			break_before_block(output);
			convert_children_to_markdown(element, output);
			if !output.ends_with("\n\n") && !output.ends_with('\n') {
				output.push('\n');
			}
		}
		// Inline containers carry no block semantics.
		"span" | "li" => convert_children_to_markdown(element, output),
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

/// Convert chapter HTML to Aidoku Markdown.
///
/// The fragment is wrapped in a container before parsing: the fragment
/// root itself cannot be traversed, while a selected wrapper supports
/// the full traversal API. The API content carries no ad markup, only
/// a promotional paragraph that [`watermark::strip`] removes unless the
/// reader turned that off.
pub fn html_to_markdown(html: &str) -> String {
	// Concatenated rather than formatted: chapter content may contain
	// braces, which format! would treat as placeholders.
	let wrapped = ["<div id=\"nb-root\">", html, "</div>"].concat();
	let Ok(doc) = Html::parse_fragment(wrapped) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#nb-root") {
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
	fn preserves_inline_markdown_without_tags() {
		let html = "<p>A <strong>bold</strong>, <em>italic</em>, <u>underlined</u>, and <del>gone</del>.</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "A **bold**\\, *italic*\\, underlined\\, and gone\\.");
	}

	#[aidoku_test]
	fn preserves_breaks_and_headings() {
		let html = "<h2>Chapter 1</h2><p>First<br>second</p>\
			<div style=\"margin:0;padding:0;border:0;font-size:0;line-height:0\"></div>\
			<p>Third</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "## Chapter 1\n\nFirst  \nsecond\n\nThird");
	}

	#[aidoku_test]
	fn escapes_literal_markdown_and_decodes_entities() {
		let html = "<p>Tom &amp; Jerry &mdash; use *literal*, _text_, and ~~this~~</p>";
		let out = html_to_markdown(html);
		assert_eq!(
			out,
			"Tom \\& Jerry — use \\*literal\\*\\, \\_text\\_\\, and \\~\\~this\\~\\~"
		);
	}

	#[aidoku_test]
	fn ignores_empty_placement_divs() {
		// Real API content (verified on a live chapter): ad spacers are
		// empty, style-only divs without any class or id.
		let html = "<div style=\"margin:0;padding:0;border:0;font-size:0;line-height:0\"></div>\
			<div> </div><p>Real content</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "Real content");
	}

	#[aidoku_test]
	fn renders_text_in_div() {
		let html = "<div>text in div</div>";
		let out = html_to_markdown(html);
		assert_eq!(out, "text in div");
	}

	#[aidoku_test]
	fn separates_bare_title_from_following_paragraph() {
		// Real API shape (Chaotic Craftsman ch. 218): the chapter title is
		// a bare text node inside the container, directly followed by the
		// first paragraph. Without a break the two render glued together.
		// (`\:`/`\.` escaping and the trailing spaces come from the
		// source-whitespace handling, unchanged by the break. The leading
		// space before "He" is the `<p>` content's own, kept as-is.)
		let html =
			"<div> Chapter 218: Ather’s Perspective\n<p> He woke up as he always did.</p></div>";
		let out = html_to_markdown(html);
		assert_eq!(
			out,
			"Chapter 218\\: Ather’s Perspective  \n\n He woke up as he always did\\."
		);
	}

	#[aidoku_test]
	fn separates_consecutive_divs() {
		let html = "<div>first</div><div>second</div>";
		let out = html_to_markdown(html);
		assert_eq!(out, "first\n\nsecond");
	}

	#[aidoku_test]
	fn keeps_root_level_text_and_inline_elements() {
		let html = "Line one <b>bold</b> line two";
		let out = html_to_markdown(html);
		assert_eq!(out, "Line one **bold** line two");
	}

	#[aidoku_test]
	fn keeps_inline_span_in_paragraph() {
		let html = "<p>Hello <span>world</span> end</p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "Hello world end");
	}

	#[aidoku_test]
	fn renders_unordered_list() {
		let html = "<ul><li>a</li><li>b</li></ul>";
		let out = html_to_markdown(html);
		assert_eq!(out, "- a\n- b");
	}

	#[aidoku_test]
	fn renders_blockquote() {
		let html = "<blockquote>cite</blockquote>";
		let out = html_to_markdown(html);
		assert_eq!(out, "> cite");
	}

	#[aidoku_test]
	fn numbers_ordered_list_items_by_li_position() {
		let html = "<ol><li>a</li><p>note</p><li>b</li></ol>";
		let out = html_to_markdown(html);
		assert_eq!(out, "1. a\n2. b");
	}

	#[aidoku_test]
	fn prefixes_every_blockquote_line() {
		let html = "<blockquote><p>one</p><p>two</p></blockquote>";
		let out = html_to_markdown(html);
		assert_eq!(out, "> one\n> \n> two");
	}

	#[aidoku_test]
	fn trims_inline_whitespace() {
		let html = "<p><strong> bold </strong> and <em> italic </em></p>";
		let out = html_to_markdown(html);
		assert_eq!(out, "**bold** and *italic*");
	}

	#[aidoku_test]
	fn keeps_code_content_unescaped() {
		let html = "<p>use <code>a_b-c *x*</code></p><pre>let s = \"a_b\";</pre>";
		let out = html_to_markdown(html);
		assert!(out.contains("`a_b-c *x*`"), "code span: {out}");
		assert!(out.contains("let s = \"a_b\";"), "fenced block: {out}");
	}

	#[aidoku_test]
	fn widens_code_delimiters_past_embedded_backticks() {
		let html = "<p><code>a`b</code> and <code>`padded`</code></p>";
		let out = html_to_markdown(html);
		assert!(out.contains("``a`b``"), "inline code: {out}");
		assert!(out.contains("`` `padded` ``"), "padded span: {out}");
	}

	#[aidoku_test]
	fn pads_code_spans_touching_delimiter_boundaries() {
		let html = "<p><code>`left</code> and <code>right`</code></p>";
		let out = html_to_markdown(html);
		assert!(out.contains("`` `left ``"), "leading backtick: {out}");
		assert!(out.contains("`` right` ``"), "trailing backtick: {out}");
	}

	#[aidoku_test]
	fn widens_fences_past_embedded_backtick_runs() {
		let html = "<pre>```rust\nfn main() {}\n```</pre>";
		let out = html_to_markdown(html);
		assert!(
			out.contains("````\n```rust\nfn main() {}\n```\n````"),
			"fenced block: {out}"
		);
	}

	#[aidoku_test]
	fn renders_code_pre_links_images_and_rules() {
		let html = "<p>Use <code>aidoku</code></p><hr>\
			<pre>let x = 1;</pre>\
			<p><a href=\"https://example.com\">site</a></p>\
			<img src=\"https://example.com/i.png\" alt=\"pic\">";
		let out = html_to_markdown(html);
		assert!(out.contains("`aidoku`"), "inline code: {out}");
		assert!(out.contains("---"), "horizontal rule: {out}");
		assert!(out.contains("```\nlet x = 1;\n```"), "pre block: {out}");
		assert!(out.contains("[site](https://example.com)"), "link: {out}");
		assert!(
			out.contains("![pic](https://example.com/i.png)"),
			"image: {out}"
		);
	}
}
