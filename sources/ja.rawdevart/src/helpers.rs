use aidoku::{
	ContentRating,
	alloc::String,
	imports::html::{Element, Html},
};

/// Strips the letters the site puts in front of ids in page urls, keeping only what looks like
/// an id afterwards.
pub fn manga_key(id: &str) -> Option<&str> {
	let key = id.trim_start_matches(char::is_alphabetic);
	(!key.is_empty() && key.bytes().all(|byte| byte.is_ascii_digit())).then_some(key)
}

/// Some descriptions hold markup, which the app doesn't render.
pub fn strip_html(description: &str) -> String {
	Html::parse_fragment(description)
		.ok()
		.and_then(|document| Element::from(document).text())
		.unwrap_or_else(|| String::from(description))
		.trim()
		.into()
}

/// Derives a content rating from the genres of an entry, which the api doesn't provide directly.
pub fn content_rating(tags: &[String]) -> ContentRating {
	let mut rating = ContentRating::Safe;
	for tag in tags {
		match tag.to_lowercase().as_str() {
			"adult" | "hentai" | "loli" | "lolicon" | "mature" | "shotacon" | "smut" => {
				return ContentRating::NSFW;
			}
			"ecchi" => rating = ContentRating::Suggestive,
			_ => continue,
		}
	}
	rating
}
