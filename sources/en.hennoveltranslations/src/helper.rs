use aidoku::{
	ContentRating,
	alloc::{String, vec::Vec},
};

pub fn parse_chapter_number(title: &str) -> Option<f32> {
	title.split_whitespace().last()?.parse().ok()
}

pub fn content_rating_from_tags(tags: &[String]) -> ContentRating {
	const NSFW_TAGS: &[&str] = &["Adult", "Mature", "Smut"];
	if tags.iter().any(|tag| NSFW_TAGS.contains(&tag.as_str())) {
		ContentRating::NSFW
	} else {
		ContentRating::Safe
	}
}

pub fn push_paragraph(paragraphs: &mut Vec<String>, text: String) {
	if text.is_empty() {
		return;
	}
	if text.trim().chars().all(|c| c == '*') {
		paragraphs.push(String::from("\\ \\ \\"));
	} else {
		paragraphs.push(text);
	}
}
