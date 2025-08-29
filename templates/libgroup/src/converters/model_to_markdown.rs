use aidoku::{
	alloc::{String, string::ToString},
	prelude::*,
};

use crate::{
	cdn::get_selected_image_server_url,
	context::Context,
	models::chapter::{
		LibGroupAttachment, LibGroupContentModel, LibGroupContentNode, LibGroupTextNode,
	},
};

pub fn convert_model_to_markdown(
	model: &LibGroupContentModel,
	attachments: &[LibGroupAttachment],
	ctx: &Context,
) -> String {
	let mut markdown = String::new();

	if let Some(content) = &model.content {
		for node in content {
			convert_node_to_markdown(node, &mut markdown, attachments, ctx);
		}
	}

	markdown.trim().to_string()
}

fn convert_node_to_markdown(
	node: &LibGroupContentNode,
	output: &mut String,
	attachments: &[LibGroupAttachment],
	ctx: &Context,
) {
	match node.node_type.as_str() {
		"paragraph" => {
			if let Some(content) = &node.content {
				for text_node in content {
					convert_text_node_to_markdown(text_node, output);
				}
			}
			output.push_str("\n\n");
		}
		"image" | "images" => {
			if let Some(attrs) = &node.attrs
				&& let Some(images) = &attrs.images
			{
				for img in images {
					let image_url = attachments
						.iter()
						.find(|a| {
							a.name.as_ref() == Some(&img.image)
								|| a.filename
									.as_ref()
									.and_then(|f| f.split('.').next())
									.map(|name| name == img.image)
									.unwrap_or(false)
						})
						.map(|attachment| {
							let url = &attachment.url;
							if url.starts_with("http://") || url.starts_with("https://") {
								url.clone()
							} else if url.starts_with('/') {
								format!("{}{}", ctx.base_url.trim_end_matches('/'), url)
							} else {
								format!("{}/{}", ctx.base_url.trim_end_matches('/'), url)
							}
						})
						.unwrap_or_else(|| {
							if img.image.starts_with("http://") || img.image.starts_with("https://")
							{
								img.image.clone()
							} else {
								format!("{}{}", get_selected_image_server_url(ctx), img.image)
							}
						});

					output.push_str(&format!("![]({image_url})\n\n"));
				}
			}
		}
		"heading" => {
			output.push_str("## ");
			if let Some(content) = &node.content {
				for text_node in content {
					convert_text_node_to_markdown(text_node, output);
				}
			}
			output.push_str("\n\n");
		}
		"horizontalRule" | "hr" => {
			output.push_str("---\n\n");
		}
		"blockquote" => {
			output.push_str("> ");
			if let Some(content) = &node.content {
				for text_node in content {
					convert_text_node_to_markdown(text_node, output);
				}
			}
			output.push_str("\n\n");
		}
		"codeBlock" | "code_block" => {
			output.push_str("```\n");
			if let Some(content) = &node.content {
				for text_node in content {
					if let Some(text) = &text_node.text {
						output.push_str(text);
					}
				}
			}
			output.push_str("\n```\n\n");
		}
		"bulletList" | "bullet_list" => {
			if let Some(content) = &node.content {
				for item in content.iter() {
					output.push_str("- ");
					convert_text_node_to_markdown(item, output);
					output.push('\n');
				}
			}
			output.push('\n');
		}
		"orderedList" | "ordered_list" => {
			if let Some(content) = &node.content {
				for (i, item) in content.iter().enumerate() {
					output.push_str(&format!("{}. ", i + 1));
					convert_text_node_to_markdown(item, output);
					output.push('\n');
				}
			}
			output.push('\n');
		}
		_ => {
			if let Some(content) = &node.content {
				for text_node in content {
					convert_text_node_to_markdown(text_node, output);
				}
			}
		}
	}
}

fn convert_text_node_to_markdown(text_node: &LibGroupTextNode, output: &mut String) {
	if let Some(text) = &text_node.text {
		let mut formatted_text = text.clone();

		if let Some(marks) = &text_node.marks {
			for mark in marks.iter().rev() {
				match mark.mark_type.as_str() {
					"bold" | "strong" => {
						formatted_text = format!("**{formatted_text}**");
					}
					"italic" | "em" => {
						formatted_text = format!("*{formatted_text}*");
					}
					"underline" => {
						formatted_text = format!("__{formatted_text}__");
					}
					"strike" | "strikethrough" => {
						formatted_text = format!("~~{formatted_text}~~");
					}
					"code" => {
						formatted_text = format!("`{formatted_text}`");
					}
					"link" => {}
					_ => {}
				}
			}
		}

		output.push_str(&formatted_text);
	}
}
