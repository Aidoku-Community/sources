//! Kept byte-identical with `sources/en.novelbuddy/src/markdown.rs`, and with
//! the `watermark.rs` and `mtl.rs` beside it: the two sources are separate
//! crates, so a change here belongs in both copies.

use aidoku::{
	alloc::{String, Vec},
	imports::html::{Element, Html, Kind},
};
use core::fmt::Write as _;

use crate::{BASE_URL, settings, watermark};

// Tags that close themselves, so they never keep a block open.
const VOID_TAGS: &[&str] = &[
	"br", "img", "hr", "input", "meta", "link", "source", "track", "wbr", "col", "area", "embed",
	"param", "base",
];

fn join_lines(text: &str) -> String {
	text.lines()
		.filter(|line| !line.is_empty())
		.collect::<Vec<&str>>()
		.join("  \n")
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

fn convert_children(element: &Element, output: &mut String) {
	let mut elements = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					output.push_str(&join_lines(&text));
				}
			}
			Kind::Element => {
				if let Some(child) = elements.next() {
					convert_element(&child, output);
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
	let mut inner = String::default();
	convert_children(element, &mut inner);
	if inner.trim().is_empty() {
		return;
	}
	break_before_block(output);
	let tag = element.tag_name().unwrap_or_default();
	let level = tag.as_bytes()[1] - b'0';
	for _ in 0..level {
		output.push('#');
	}
	output.push(' ');
	output.push_str(inner.trim());
	output.push_str("\n\n");
}

fn anchored(inner: &str, open: &str, close: &str) -> String {
	let start = inner.len() - inner.trim_start().len();
	let end = inner.trim_end().len();
	if start >= end {
		return inner.into();
	}
	let mut out = String::from(&inner[..start]);
	out.push_str(open);
	out.push_str(&inner[start..end]);
	out.push_str(close);
	out.push_str(&inner[end..]);
	out
}

fn convert_emphasis(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children(element, &mut inner);
	let marker = match element.tag_name().as_deref() {
		Some("strong" | "b") => "**",
		Some("s" | "del" | "strike") => "~~",
		_ => "*",
	};
	output.push_str(&anchored(&inner, marker, marker));
}

fn convert_inline_code(element: &Element, output: &mut String) {
	let mut raw = String::default();
	append_raw_text(element, &mut raw);
	if raw.is_empty() {
		return;
	}
	let ticks = longest_backtick_run(&raw) + 1;
	for _ in 0..ticks {
		output.push('`');
	}
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
	let mut raw = String::default();
	append_raw_text(element, &mut raw);
	if raw.trim().is_empty() {
		return;
	}
	break_before_block(output);
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
	let Some(src) = element
		.attr("abs:src")
		.filter(|src| src.starts_with("http"))
	else {
		return;
	};
	break_before_block(output);
	let alt = element.attr("alt").unwrap_or_default();
	let _ = writeln!(output, "![{alt}]({src})");
	output.push('\n');
}

fn convert_link(element: &Element, output: &mut String) {
	if let Some(href) = element.attr("abs:href") {
		output.push('[');
		convert_children(element, output);
		let _ = write!(output, "]({href})");
	} else {
		convert_children(element, output);
	}
}

fn convert_block_container(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children(element, &mut inner);
	if inner.trim().is_empty() {
		return;
	}
	break_before_block(output);
	output.push_str(&inner);
	if !output.ends_with('\n') {
		output.push('\n');
	}
}

fn convert_quoted(element: &Element, output: &mut String) {
	let mut inner = String::default();
	convert_children(element, &mut inner);
	output.push_str(&anchored(&inner, "\u{201C}", "\u{201D}"));
}

// Elements with a markdown form are converted below, listed against the WHATWG
// index at https://html.spec.whatwg.org/multipage/indices.html#elements-3 .
// A structural container keeps its line breaks, and any other tag keeps its
// text, so a chapter loses nothing it marked up.
fn convert_element(element: &Element, output: &mut String) {
	let tag = element.tag_name().unwrap_or_default();
	match tag.as_str() {
		"p" | "figcaption" | "dt" | "dd" | "summary" => {
			let mut inner = String::default();
			convert_children(element, &mut inner);
			if inner.trim().is_empty() {
				return;
			}
			break_before_block(output);
			output.push_str(&inner);
			output.push_str("\n\n");
		}
		"br" => output.push_str("  \n"),
		"h1" | "h2" | "h3" | "h4" | "h5" | "h6" => convert_heading(element, output),
		"strong" | "b" | "em" | "i" | "s" | "del" | "strike" => convert_emphasis(element, output),
		"code" => convert_inline_code(element, output),
		"q" => convert_quoted(element, output),
		"pre" => convert_fenced_code(element, output),
		"img" => convert_image(element, output),
		"a" => convert_link(element, output),
		"hr" => {
			break_before_block(output);
			output.push_str("---\n\n")
		}
		"ul" | "ol" | "menu" => convert_list(element, &tag, output),
		"table" => convert_table(element, output),
		"blockquote" => convert_blockquote(element, output),
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" | "figure"
		| "hgroup" | "details" | "dl" | "address" | "nav" | "dialog" | "search" | "body"
		| "html" | "form" | "fieldset" | "center" => convert_block_container(element, output),
		_ => convert_children(element, output),
	}
}

fn convert_list(element: &Element, tag: &str, output: &mut String) {
	let ordered = tag == "ol";
	let mut bodies: Vec<String> = Vec::new();
	for item in element.children() {
		if item.tag_name().as_deref() != Some("li") {
			continue;
		}
		let mut body = String::default();
		convert_children(&item, &mut body);
		if !body.trim().is_empty() {
			bodies.push(body);
		}
	}
	if bodies.is_empty() {
		return;
	}
	break_before_block(output);
	for (index, body) in bodies.iter().enumerate() {
		let mut marker = String::default();
		if ordered {
			let _ = write!(marker, "{}. ", index + 1);
		} else {
			marker.push_str("- ");
		}
		let indent = " ".repeat(marker.len());
		output.push_str(&marker);
		for (line_index, line) in body.trim().lines().enumerate() {
			if line_index > 0 {
				output.push('\n');
			}
			if line.is_empty() {
				continue;
			}
			if line_index > 0 {
				output.push_str(&indent);
			}
			output.push_str(line);
		}
		output.push('\n');
	}
	output.push('\n');
}

fn convert_table(element: &Element, output: &mut String) {
	let mut rows: Vec<Vec<String>> = Vec::new();
	table_rows(element, &mut rows);
	rows.retain(|row| row.iter().any(|cell| !cell.trim().is_empty()));
	let width = rows.iter().map(|row| row.len()).max().unwrap_or(0);
	if width == 0 {
		return;
	}
	break_before_block(output);
	for (index, row) in rows.iter().enumerate() {
		output.push('|');
		for column in 0..width {
			output.push(' ');
			output.push_str(row.get(column).map_or("", String::as_str));
			output.push_str(" |");
		}
		output.push('\n');
		if index == 0 {
			output.push('|');
			for _ in 0..width {
				output.push_str(" --- |");
			}
			output.push('\n');
		}
	}
	output.push('\n');
}

fn table_rows(element: &Element, rows: &mut Vec<Vec<String>>) {
	for child in element.children() {
		match child.tag_name().as_deref() {
			Some("thead" | "tbody" | "tfoot") => table_rows(&child, rows),
			Some("tr") => {
				let mut row: Vec<String> = Vec::new();
				for cell in child.children() {
					if matches!(cell.tag_name().as_deref(), Some("td" | "th")) {
						let mut text = String::default();
						convert_children(&cell, &mut text);
						row.push(text.replace('\n', " ").replace('|', "\\|").trim().into());
					}
				}
				rows.push(row);
			}
			_ => {}
		}
	}
}

fn convert_blockquote(element: &Element, output: &mut String) {
	let mut quoted = String::default();
	convert_children(element, &mut quoted);
	if quoted.trim().is_empty() {
		return;
	}
	break_before_block(output);
	for (index, line) in quoted.trim_end().lines().enumerate() {
		if index > 0 {
			output.push('\n');
		}
		output.push_str("> ");
		output.push_str(line);
	}
	output.push_str("\n\n");
}

fn html_to_markdown(input: &str) -> String {
	let wrapped = ["<div id=\"md-root\">", input, "</div>"].concat();
	let Ok(doc) = Html::parse_fragment_with_url(wrapped, BASE_URL) else {
		return String::default();
	};

	let mut output = String::default();
	if let Some(root) = doc.select_first("#md-root") {
		convert_children(&root, &mut output);
	}
	output
}

fn blocks(text: &str) -> Vec<String> {
	let mut blocks: Vec<String> = Vec::new();
	let mut block = String::default();
	for line in text.lines() {
		if line.trim().is_empty() {
			if !block.is_empty() {
				blocks.push(core::mem::take(&mut block));
			}
		} else {
			block.push_str(if block.is_empty() { "" } else { "\n" });
			block.push_str(line);
		}
	}
	if !block.is_empty() {
		blocks.push(block);
	}
	blocks
}

fn leaves_open(block: &str) -> bool {
	let mut depth = 0i32;
	let mut rest = block;
	while let Some(index) = rest.find('<') {
		let after = &rest[index + 1..];
		let end = after.find('>').unwrap_or(after.len());
		let body = &after[..end];
		if let Some(rest_of_tag) = body.strip_prefix('/') {
			if rest_of_tag
				.chars()
				.next()
				.is_some_and(|c| c.is_ascii_alphabetic())
			{
				depth = (depth - 1).max(0);
			}
		} else if !body.ends_with('/')
			&& body.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
		{
			let length = body.chars().take_while(|c| c.is_alphanumeric()).count();
			let name = &body[..length];
			if !VOID_TAGS.contains(&name) {
				depth += 1;
			}
		}
		rest = after;
	}
	depth > 0
}

pub fn to_markdown(input: &str) -> String {
	let mut pieces: Vec<String> = Vec::new();
	let mut pending = String::default();
	for block in blocks(input) {
		if !pending.is_empty() {
			pending.push('\n');
		}
		pending.push_str(&block);
		if leaves_open(&pending) {
			continue;
		}
		pieces.push(if pending.contains('<') {
			html_to_markdown(&pending).trim().into()
		} else {
			core::mem::take(&mut pending)
		});
		pending.clear();
	}
	if !pending.is_empty() {
		pieces.push(if pending.contains('<') {
			html_to_markdown(&pending).trim().into()
		} else {
			pending
		});
	}
	pieces.retain(|piece| !piece.is_empty());
	let text = pieces.join("\n\n");
	let text = if settings::hide_watermark() {
		blocks(&text)
			.iter()
			.map(|block| watermark::strip(block))
			.filter(|block| !block.is_empty())
			.collect::<Vec<String>>()
			.join("\n\n")
	} else {
		text
	};
	text.trim().into()
}
