use aidoku::{
	ContentRating,
	alloc::{String, vec::Vec},
};

pub fn parse_chapter_number(title: &str) -> Option<f32> {
	let words: Vec<&str> = title.split_whitespace().collect();
	if let Some(last) = words.last() {
		return last.parse::<f32>().ok();
	}
	None
}

pub fn extract_meta_value(text: &str, label: &str) -> String {
	if let Some(pos) = text.find(label) {
		let after = &text[pos + label.len()..];
		let end = after.find(['\n', '.', '•', ':']).unwrap_or(after.len());
		String::from(after[..end].trim())
	} else {
		String::new()
	}
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
		paragraphs.push(String::from("* * *"));
	} else {
		paragraphs.push(text);
	}
}
