use aidoku::{
	ContentRating,
	alloc::String,
	imports::html::{Element, Html},
};

// page urls put letters in front of the id
pub fn manga_key(id: &str) -> Option<&str> {
	let key = id.trim_start_matches(char::is_alphabetic);
	(!key.is_empty() && key.bytes().all(|byte| byte.is_ascii_digit())).then_some(key)
}

// some descriptions hold markup, which the app doesn't render
pub fn strip_html(description: &str) -> String {
	Html::parse_fragment(description)
		.ok()
		.and_then(|document| Element::from(document).text())
		.unwrap_or_else(|| String::from(description))
		.trim()
		.into()
}

// the api provides no rating of its own
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
