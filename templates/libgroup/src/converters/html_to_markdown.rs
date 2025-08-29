use aidoku::{
	alloc::{String, string::ToString},
	imports::html::{Element, Html},
	prelude::*,
};

pub fn convert_html_to_markdown(html: &str) -> String {
	match Html::parse(html) {
		Ok(doc) => {
			let mut markdown = String::new();

			if let Some(body) = doc.select_first("body") {
				convert_element_to_markdown(&body, &mut markdown, 0);
			} else if let Some(html_elem) = doc.select_first("html") {
				for child in html_elem.children() {
					convert_element_to_markdown(&child, &mut markdown, 0);
				}
			} else if let Some(elements) = doc.select("body > *, html > *, > *") {
				for element in elements {
					convert_element_to_markdown(&element, &mut markdown, 0);
				}
			} else {
				return convert_html_fragment_to_markdown(html);
			}

			markdown.trim().to_string()
		}
		Err(_) => convert_html_fragment_to_markdown(html),
	}
}

fn convert_html_fragment_to_markdown(html: &str) -> String {
	match Html::parse_fragment(html) {
		Ok(doc) => {
			let mut markdown = String::new();

			if let Some(body) = doc.select_first("body") {
				for child in body.children() {
					convert_element_to_markdown(&child, &mut markdown, 0);
				}
			} else if let Some(elements) = doc.select("*") {
				for element in elements {
					let tag = element.tag_name().unwrap_or_default();
					if tag != "html" && tag != "body" {
						convert_element_to_markdown(&element, &mut markdown, 0);
					}
				}
			}

			markdown.trim().to_string()
		}
		Err(_) => Html::unescape(html).unwrap_or_else(|| html.to_string()),
	}
}

fn convert_element_to_markdown(element: &Element, output: &mut String, depth: usize) {
	let tag = element.tag_name().unwrap_or_default();

	match tag.as_str() {
		"p" => {
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"br" => {
			output.push_str("  \n");
		}
		"h1" => {
			output.push_str("# ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"h2" => {
			output.push_str("## ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"h3" => {
			output.push_str("### ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"h4" => {
			output.push_str("#### ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"h5" => {
			output.push_str("##### ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"h6" => {
			output.push_str("###### ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"strong" | "b" => {
			output.push_str("**");
			convert_children_to_markdown(element, output, depth);
			output.push_str("**");
		}
		"em" | "i" => {
			output.push('*');
			convert_children_to_markdown(element, output, depth);
			output.push('*');
		}
		"u" => {
			output.push_str("__");
			convert_children_to_markdown(element, output, depth);
			output.push_str("__");
		}
		"s" | "strike" | "del" => {
			output.push_str("~~");
			convert_children_to_markdown(element, output, depth);
			output.push_str("~~");
		}
		"code" => {
			output.push('`');
			convert_children_to_markdown(element, output, depth);
			output.push('`');
		}
		"pre" => {
			output.push_str("```\n");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n```\n\n");
		}
		"img" => {
			if let Some(src) = element.attr("src") {
				let alt = element.attr("alt").unwrap_or_default();
				output.push_str(&format!("![{alt}]({src})\n\n"));
			}
		}
		"a" => {
			if let Some(href) = element.attr("href") {
				output.push('[');
				convert_children_to_markdown(element, output, depth);
				output.push_str(&format!("]({href})"));
			} else {
				convert_children_to_markdown(element, output, depth);
			}
		}
		"ul" | "ol" => {
			for (i, child) in element.children().enumerate() {
				if child.tag_name().as_deref() == Some("li") {
					if tag == "ol" {
						output.push_str(&format!("{}. ", i + 1));
					} else {
						output.push_str("- ");
					}
					convert_children_to_markdown(&child, output, depth + 1);
					output.push('\n');
				}
			}
			output.push('\n');
		}
		"blockquote" => {
			output.push_str("> ");
			convert_children_to_markdown(element, output, depth);
			output.push_str("\n\n");
		}
		"hr" => {
			output.push_str("---\n\n");
		}
		"div" | "section" | "article" | "header" | "footer" | "main" | "aside" => {
			convert_children_to_markdown(element, output, depth);
			if !output.ends_with("\n\n") && !output.ends_with("\n") {
				output.push('\n');
			}
		}
		"span" => {
			convert_children_to_markdown(element, output, depth);
		}
		_ => {
			if let Some(text) = element.own_text()
				&& !text.trim().is_empty()
			{
				output.push_str(&text);
			}
			convert_children_to_markdown(element, output, depth);
		}
	}
}

fn convert_children_to_markdown(element: &Element, output: &mut String, depth: usize) {
	if let Some(text) = element.own_text()
		&& !text.trim().is_empty()
	{
		output.push_str(&text);
	}

	for child in element.children() {
		convert_element_to_markdown(&child, output, depth);
	}
}
