use crate::IMAGE_URL;
use aidoku::{
	alloc::{String, Vec, borrow::Cow, format},
	helpers::string::PlainText,
};

pub enum Block<'a> {
	Text(&'a str),
	Image(Cow<'a, str>),
}

pub fn blocks(text: &str) -> Vec<Block<'_>> {
	let mut blocks = Vec::new();
	for mut line in text.lines() {
		while let Some((prose, src, rest)) = find_image(line) {
			blocks.extend(paragraph(prose));
			if !src.is_empty() {
				blocks.push(Block::Image(image_url(src)));
			}
			line = rest;
		}
		blocks.extend(paragraph(line));
	}
	blocks
}

fn paragraph(text: &str) -> Option<Block<'_>> {
	let text = text.trim();
	(!text.is_empty()).then_some(Block::Text(text))
}

fn find_image(line: &str) -> Option<(&str, &str, &str)> {
	line.match_indices("<img").find_map(|(start, _)| {
		let (tag, rest) = line.get(start..)?.split_once('>')?;
		let (_, value) = tag.rsplit_once("src=")?;
		let (src, _) = value.strip_prefix(['"', '\''])?.split_once(['"', '\''])?;
		Some((line.get(..start)?, src, rest))
	})
}

// a few books link illustrations as `../../img/…`, which the image host serves under `/xs/`
fn image_url(src: &str) -> Cow<'_, str> {
	match src.strip_prefix("../../") {
		Some(path) => format!("{IMAGE_URL}/xs/{path}").into(),
		None => src.into(),
	}
}

pub fn from_blocks(blocks: &[Block]) -> String {
	blocks
		.iter()
		.map(|block| match block {
			Block::Text(line) => line.escape_markdown(),
			Block::Image(src) => format!("![]({src})"),
		})
		.collect::<Vec<_>>()
		.join("\n\n")
}
