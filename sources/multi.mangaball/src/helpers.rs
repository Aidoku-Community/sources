use crate::api::BASE_URL;
use aidoku::alloc::{String, Vec};

// Chapters also use legacy and regional codes.
const LANGUAGE_ALIASES: [(&str, &[&str]); 5] = [
	("es", &["es-la", "es-419"]),
	("ja", &["jp"]),
	("ko", &["kr"]),
	("pt", &["pt-pt"]),
	("zh", &["cn", "zh-hk", "zh-hans", "zh-hant"]),
];

// The site's types group original languages.
const TYPES: [(&str, &[&str]); 4] = [
	("manga", &["ja", "jp"]),
	("manhwa", &["ko", "kr"]),
	("manhua", &["cn", "zh", "zh-hk", "zh-hans", "zh-hant"]),
	("comics", &["en"]),
];

pub enum Link {
	Title(String),
	Chapter(String),
}

// Title URLs append a slug to the ID.
pub fn parse_link(url: &str) -> Option<Link> {
	let path = url.trim().strip_prefix(BASE_URL)?;
	let id = |prefix| {
		let id = path
			.strip_prefix(prefix)?
			.split(['-', '/', '?', '#'])
			.next()?;
		(id.len() == 24 && id.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| id.into())
	};
	id("/title-detail/")
		.map(Link::Title)
		.or_else(|| id("/chapter-detail/").map(Link::Chapter))
}

pub fn site_languages(languages: Vec<String>) -> Vec<(String, String)> {
	let mut codes = Vec::new();
	for language in languages {
		let aliases = LANGUAGE_ALIASES
			.iter()
			.find(|(code, _)| *code == language)
			.map(|(_, aliases)| *aliases)
			.unwrap_or_default();
		codes.extend(
			aliases
				.iter()
				.map(|alias| ((*alias).into(), language.clone())),
		);
		codes.push((language.to_lowercase(), language));
	}
	codes
}

pub fn title_type(original_language: &str) -> Option<&'static str> {
	TYPES
		.iter()
		.find(|(_, languages)| languages.contains(&original_language))
		.map(|(kind, _)| *kind)
}
