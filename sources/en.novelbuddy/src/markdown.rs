//! Convert chapter HTML to Aidoku Markdown.
//!
//! Approach adapted from en.freewebnovel's chapter converter and the shared
//! libgroup template converter.

use aidoku::{
	alloc::{String, Vec, string::ToString},
	helpers::string::PlainText,
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

/// Append an element's full descendant text without Markdown escaping:
/// backslashes inside code spans and fenced blocks are literal output.
fn append_raw_text(element: &Element, output: &mut String) {
	if let Some(text) = element.text() {
		output.push_str(&text);
	}
}

/// Append an element's direct text and child elements in document order.
///
/// `child_nodes` yields text nodes (whose text is only reachable there),
/// while `children` yields elements with reliable tag names; element-kind
/// nodes are therefore paired with the next entry from `children`.
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

fn convert_element_to_markdown(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();
	match tag.as_str() {
		"p" => {
			convert_children_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"br" => output.push_str("  \n"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
			let level = tag.as_bytes()[1] - b'0';
			for _ in 0..level {
				output.push('#');
			}
			output.push(' ');
			convert_children_to_markdown(element, output);
			output.push_str("\n\n");
		}
		"strong" | "b" | "em" | "i" | "u" | "s" | "strike" | "del" => {
			// Trim so surrounding whitespace stays outside the markers;
			// `** bold **` is not recognized as emphasis by Markdown.
			let mut inner = String::default();
			convert_children_to_markdown(element, &mut inner);
			let trimmed = inner.trim();
			if !trimmed.is_empty() {
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
		"code" => {
			output.push('`');
			append_raw_text(element, output);
			output.push('`');
		}
		"pre" => {
			output.push_str("```\n");
			append_raw_text(element, output);
			output.push_str("\n```\n\n");
		}
		"img" => {
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
		"hr" => output.push_str("---\n\n"),
		"ul" | "ol" => convert_list_to_markdown(element, &tag, output),
		"blockquote" => convert_blockquote_to_markdown(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" => {
			convert_children_to_markdown(element, output);
			if !output.ends_with("\n\n") && !output.ends_with('\n') {
				output.push('\n');
			}
		}
		// Inline containers carry no block semantics: pass their content
		// through without injecting newlines mid-paragraph.
		"span" | "li" => convert_children_to_markdown(element, output),
		// Unknown tags: recurse so their prose is still emitted.
		_ => convert_children_to_markdown(element, output),
	}
}

/// Render list items as Markdown bullets or numbered entries.
///
/// Numbering follows `li` position: non-item children are filtered out
/// before enumeration so stray markup cannot shift the sequence.
fn convert_list_to_markdown(element: &Element, tag: &str, output: &mut String) {
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
/// The API's chapter content carries no ad markup (verified on live
/// chapters): its placement spacers are empty, style-only divs that
/// naturally emit nothing during conversion.
///
/// The fragment is wrapped in a container element before parsing: the
/// fragment root itself cannot be traversed (its child lists come back
/// empty), while a selected wrapper element supports the full traversal
/// API, including root-level text and inline elements.
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
	output.trim().to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn preserves_inline_markdown_without_tags() {
		let html = "<p>A <strong>bold</strong>, <em>italic</em>, <u>underlined</u>, and <del>gone</del>.</p>";
		let out = html_to_markdown(html);
		assert_eq!(
			out,
			"A **bold**\\, *italic*\\, __underlined__\\, and ~~gone~~\\."
		);
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
