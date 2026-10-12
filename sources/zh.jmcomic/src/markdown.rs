use aidoku::{
	alloc::{String, Vec, format},
	helpers::string::PlainText,
	imports::html::{Element, Html, Kind},
};

pub fn from_html(html: &str, base_url: &str) -> String {
	let mut blocks = Vec::new();
	let mut line = String::new();
	if let Ok(document) = Html::parse_fragment_with_url(html, base_url)
		&& let Some(root) = document.select_first("html")
	{
		walk(&root, &mut blocks, &mut line);
	}
	end_line(&mut blocks, &mut line);
	blocks.join("\n\n")
}

fn walk(element: &Element, blocks: &mut Vec<String>, line: &mut String) {
	let mut children = element.children();
	for node in element.child_nodes() {
		match node.kind() {
			Kind::TextNode => {
				if let Some(text) = node.text() {
					line.push_str(&text);
				}
			}
			Kind::Element => {
				let Some(child) = children.next() else {
					continue;
				};
				match child.tag_name().as_deref() {
					Some("br") => end_line(blocks, line),
					Some("img") => {
						end_line(blocks, line);
						if let Some(src) = child.attr("abs:src") {
							blocks.push(format!("![]({src})"));
						}
					}
					Some("p") => {
						end_line(blocks, line);
						walk(&child, blocks, line);
						end_line(blocks, line);
					}
					_ => walk(&child, blocks, line),
				}
			}
			_ => {}
		}
	}
}

fn end_line(blocks: &mut Vec<String>, line: &mut String) {
	let text = line.trim();
	if !text.is_empty() {
		blocks.push(text.escape_markdown());
	}
	line.clear();
}
