use crate::{
	models::{ChapterData, ComicData, NamedData, Node},
	settings::{get_languages, normalize_language},
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::net::Request,
	prelude::*,
};

pub const PORNOGRAPHIC_GENRES: &[&str] = &["adult", "hentai", "pornographic", "smut"];

/// `(id, title)` for the original-language filter; `_t` is the site's catch-all.
pub const LANGUAGES: &[(&str, &str)] = &[
	("en", "English"),
	("fr", "French"),
	("pt", "Portuguese"),
	("ko", "Korean"),
	("ja", "Japanese"),
	("id", "Indonesian"),
	("zh", "Chinese"),
	("ab", "Abkhazian"),
	("af", "Afrikaans"),
	("hy", "Armenian"),
	("ar", "Arabic"),
	("sq", "Albanian"),
	("az", "Azerbaijani"),
	("be", "Belarusian"),
	("bn", "Bengali"),
	("my", "Burmese"),
	("bg", "Bulgarian"),
	("bs", "Bosnian"),
	("km", "Cambodian"),
	("ca", "Catalan"),
	("ceb", "Cebuano"),
	("cs", "Czech"),
	("hr", "Croatian"),
	("cv", "Chuvash"),
	("da", "Danish"),
	("nl", "Dutch"),
	("et", "Estonian"),
	("eo", "Esperanto"),
	("eu", "Basque"),
	("fil", "Filipino"),
	("fi", "Finnish"),
	("de", "German"),
	("ka", "Georgian"),
	("el", "Greek"),
	("gn", "Guarani"),
	("gu", "Gujarati"),
	("hi", "Hindi"),
	("he", "Hebrew"),
	("ht", "Haitian Creole"),
	("hu", "Hungarian"),
	("is", "Icelandic"),
	("ig", "Igbo"),
	("gl", "Galician"),
	("ga", "Irish"),
	("it", "Italian"),
	("kk", "Kazakh"),
	("ky", "Kyrgyz"),
	("lt", "Lithuanian"),
	("la", "Latin"),
	("lo", "Laothian"),
	("ku", "Kurdish"),
	("jv", "Javanese"),
	("mg", "Malagasy"),
	("lv", "Latvian"),
	("ms", "Malay"),
	("ml", "Malayalam"),
	("mt", "Maltese"),
	("mo", "Moldavian"),
	("mr", "Marathi"),
	("mi", "Maori"),
	("mn", "Mongolian"),
	("ny", "Nyanja"),
	("ne", "Nepali"),
	("ps", "Pashto"),
	("no", "Norwegian"),
	("fa", "Persian"),
	("pt_br", "Portuguese (BR)"),
	("sr", "Serbian"),
	("st", "Sesotho"),
	("ru", "Russian"),
	("ro", "Romanian"),
	("pl", "Polish"),
	("sh", "Serbo-Croatian"),
	("si", "Sinhalese"),
	("so", "Somali"),
	("sv", "Swedish"),
	("th", "Thai"),
	("tr", "Turkish"),
	("ss", "Swati"),
	("sk", "Slovak"),
	("es", "Spanish"),
	("ti", "Tigrinya"),
	("ta", "Tamil"),
	("tk", "Turkmen"),
	("uk", "Ukrainian"),
	("to", "Tonga"),
	("te", "Telugu"),
	("es_419", "Spanish (LA)"),
	("sl", "Slovenian"),
	("vi", "Vietnamese"),
	("_t", "Other"),
	("uz", "Uzbek"),
	("zu", "Zulu"),
	("am", "Amharic"),
	("fo", "Faroese"),
	("ha", "Hausa"),
	("kn", "Kannada"),
	("lb", "Luxembourgish"),
	("mk", "Macedonian"),
	("rm", "Romansh"),
	("sd", "Sindhi"),
	("sm", "Samoan"),
	("sn", "Shona"),
	("sw", "Swahili"),
	("tg", "Tajik"),
	("ur", "Urdu"),
	("yo", "Yoruba"),
];

/// `(id, title)`. Fallback for the live list, and the source of the exclusion
/// setting, so the filter and the setting cannot drift apart.
pub const GENRES: &[(&str, &str)] = &[
	("fantasy", "Fantasy"),
	("supernatural", "Supernatural"),
	("action", "Action"),
	("drama", "Drama"),
	("psychological", "Psychological"),
	("romance", "Romance"),
	("slice_of_life", "Slice of Life"),
	("tragedy", "Tragedy"),
	("horror", "Horror"),
	("mystery", "Mystery"),
	("comedy", "Comedy"),
	("martial_arts", "Martial Arts"),
	("boys_love", "Boys Love"),
	("adventure", "Adventure"),
	("historical", "Historical"),
	("sci_fi", "Sci-Fi"),
	("girls_love", "Girls Love"),
	("adult", "Adult"),
	("smut", "Smut"),
	("thriller", "Thriller"),
	("hentai", "Hentai"),
	("longstrip", "Longstrip"),
	("full_color", "Full Color"),
	("web_comic", "Web Comic"),
	("doujinshi", "Doujinshi"),
	("web_novel", "Web Novel"),
	("original_doujinshi", "Original Doujinshi"),
	("4_koma", "4-Koma"),
	("fanbook", "Fanbook"),
	("light_novel", "Light Novel"),
	("japanese_novel", "Japanese Novel"),
	("novels", "Novels"),
	("illustration_book", "Illustration Book"),
	("guidebook", "Guidebook"),
	("illustbook", "Illustbook"),
	("artbook", "Artbook"),
	("partially_colored", "Partially Colored"),
	("1_koma", "1-Koma"),
	("fanwork", "Fanwork"),
	("partially_colored_webtoon", "Partially Colored Webtoon"),
	("3_koma", "3-koma"),
	("2_koma", "2-koma"),
];

pub fn absolute_url(base_url: &str, url: &str) -> String {
	let url = url.trim();
	if url.is_empty() {
		String::new()
	} else if url.starts_with("http://") || url.starts_with("https://") {
		url.into()
	} else if let Some(rest) = url.strip_prefix("//") {
		format!("https://{rest}")
	} else if url.starts_with('/') {
		format!("{base_url}{url}")
	} else {
		format!("{base_url}/{url}")
	}
}

fn clean(text: &str) -> String {
	text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn title_case(value: &str) -> String {
	let mut output = String::with_capacity(value.len());
	let mut capitalize = true;
	for character in value.replace('_', " ").chars() {
		if capitalize && character.is_alphabetic() {
			output.extend(character.to_uppercase());
			capitalize = false;
		} else {
			output.push(character);
			capitalize = character == ' ';
		}
	}
	output
}

fn node_names(nodes: Vec<Node<Option<NamedData>>>) -> Vec<String> {
	nodes
		.into_iter()
		.filter_map(|node| node.data)
		.filter_map(|data| data.name)
		.map(|name| clean(&name))
		.filter(|name| !name.is_empty())
		.collect()
}

pub fn is_pornographic(rating: Option<&str>, genres: Option<&[String]>) -> bool {
	// The API is inconsistent about genre casing.
	rating == Some("pornographic")
		|| genres.is_some_and(|genres| {
			genres.iter().any(|genre| {
				PORNOGRAPHIC_GENRES.contains(&genre.trim().to_ascii_lowercase().as_str())
			})
		})
}

fn content_rating(rating: Option<&str>, genres: &[String]) -> ContentRating {
	if is_pornographic(rating, Some(genres)) {
		ContentRating::NSFW
	} else if matches!(rating, Some("suggestive" | "erotica")) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

fn status(original_status: Option<&str>, upload_status: Option<&str>) -> MangaStatus {
	let status = original_status.or(upload_status).unwrap_or_default();
	if status.contains("completed") {
		MangaStatus::Completed
	} else if status.contains("releasing") || status.contains("ongoing") {
		MangaStatus::Ongoing
	} else if status.contains("hiatus") {
		MangaStatus::Hiatus
	} else if status.contains("cancelled") {
		MangaStatus::Cancelled
	} else {
		MangaStatus::Unknown
	}
}

fn viewer(read_direction: Option<&str>, kind: Option<&str>, genres: &[String]) -> Viewer {
	match read_direction {
		Some("ttb") => Viewer::Webtoon,
		Some("rtl") => Viewer::RightToLeft,
		Some("ltr") => Viewer::LeftToRight,
		_ if matches!(kind, Some("manhwa" | "manhua"))
			|| genres.iter().any(|genre| genre == "longstrip") =>
		{
			Viewer::Webtoon
		}
		_ if kind == Some("manga") => Viewer::RightToLeft,
		_ => Viewer::Unknown,
	}
}

/// Maps whatever the query returned; absent fields simply stay empty.
pub fn manga_from_data(mut comic: ComicData, base_url: &str) -> Manga {
	let mut raw_tags = comic.genres.take().unwrap_or_default();
	raw_tags.extend(comic.demographics.take().unwrap_or_default());
	raw_tags.extend(comic.tags.take().unwrap_or_default());
	if let Some(nodes) = comic.tag_nodes.take() {
		raw_tags.extend(node_names(nodes));
	}
	// Rating and viewer read the site's own lowercase ids, so derive them before
	// the tags are title cased for display.
	let rating = content_rating(comic.content_rating.as_deref(), &raw_tags);
	let preferred_viewer = viewer(
		comic.read_direction.as_deref(),
		comic.kind.as_deref(),
		&raw_tags,
	);

	let mut seen = Vec::new();
	let tags: Vec<String> = raw_tags
		.into_iter()
		.filter(|tag| !tag.trim().is_empty())
		.filter(|tag| {
			let normalized = tag.to_ascii_lowercase();
			let unseen = !seen.contains(&normalized);
			if unseen {
				seen.push(normalized);
			}
			unseen
		})
		.map(|tag| title_case(&tag))
		.collect();

	let cover = comic
		.url_cover
		.take()
		.map(|url| absolute_url(base_url, &url))
		.filter(|url| !url.is_empty());
	let url = comic
		.url_path
		.take()
		.map(|url| absolute_url(base_url, &url))
		.unwrap_or_else(|| format!("{base_url}/comic/{}", comic.id));
	let authors = comic
		.author_nodes
		.take()
		.map(node_names)
		.filter(|names| !names.is_empty());
	let artists = comic
		.artist_nodes
		.take()
		.map(node_names)
		.filter(|names| !names.is_empty());
	let description = comic
		.summary
		.take()
		.and_then(|summary| summary.text)
		.map(|summary| summary.trim().to_string())
		.filter(|description| !description.is_empty());
	let publish_status = status(
		comic.original_status.as_deref(),
		comic.upload_status.as_deref(),
	);
	let title = clean(&comic.name);

	Manga {
		key: comic.id,
		title,
		cover,
		url: Some(url),
		authors,
		artists,
		tags: (!tags.is_empty()).then_some(tags),
		description,
		status: publish_status,
		content_rating: rating,
		viewer: preferred_viewer,
		..Default::default()
	}
}

/// The two prefixes are different things, not aliases: `/title/{id}` is the
/// series, which owns one `/comic/` edition per language, and only edition ids
/// work as manga keys.
pub enum Target {
	Title(String),
	Comic(String, Option<String>),
}

pub fn parse_link(url: &str) -> Option<Target> {
	fn id(segment: &str) -> Option<String> {
		segment
			.split('-')
			.next()
			// The site rewrites its legacy chapter links to `/comic/_/{chapter}`,
			// where `_` stands in for a comic it does not name.
			.filter(|id| !id.is_empty() && *id != "_")
			.map(Into::into)
	}
	if let Some(path) = url.split("/title/").nth(1) {
		return id(path.split('/').next()?).map(Target::Title);
	}
	let mut segments = url.split("/comic/").nth(1)?.split('/');
	Some(Target::Comic(
		id(segments.next()?)?,
		segments.next().and_then(id),
	))
}

/// Target of a pasted url or an `id:<value>` query. A bare id is not accepted,
/// since it would swallow ordinary search terms.
pub fn target_from_query(query: &str) -> Option<Target> {
	let query = query.trim();
	if query.contains("/title/") || query.contains("/comic/") {
		return parse_link(query);
	}
	let rest = query
		.get(..3)
		.filter(|prefix| prefix.eq_ignore_ascii_case("id:"))
		.and_then(|_| query.get(3..))?
		.trim();
	let id = rest.split('-').next()?;
	(!id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
		.then(|| Target::Comic(id.into(), None))
}

/// A `/title/` page is not itself readable; its "Sources" list links one comic
/// per language, so prefer one the reader picked over the site's first choice.
pub fn resolve_title(base_url: &str, title_id: &str) -> Option<String> {
	let document = Request::get(format!("{base_url}/title/{title_id}"))
		.ok()?
		.html()
		.ok()?;
	let mut editions: Vec<(String, String)> = Vec::new();
	for anchor in document.select("a[href*='/comic/']")? {
		let Some(href) = anchor.attr("href") else {
			continue;
		};
		let Some(path) = href.split("/comic/").nth(1) else {
			continue;
		};
		// Chapter links live under an edition, so they carry a second segment.
		let mut segments = path.trim_end_matches('/').split('/');
		let Some(edition) = segments.next() else {
			continue;
		};
		if segments.next().is_some() {
			continue;
		}
		let mut fields = edition.split('-');
		let (Some(id), Some(language)) = (fields.next(), fields.next()) else {
			continue;
		};
		if !id.is_empty() && !editions.iter().any(|(seen, _)| seen == id) {
			editions.push((id.into(), language.into()));
		}
	}

	let languages = get_languages().unwrap_or_default();
	editions
		.iter()
		.find(|(_, language)| languages.iter().any(|wanted| wanted == language))
		.or_else(|| editions.first())
		.map(|(id, _)| id.clone())
}

pub fn comic_key(base_url: &str, target: Target) -> Result<String> {
	match target {
		Target::Comic(key, _) => Ok(key),
		Target::Title(id) => resolve_title(base_url, &id)
			.ok_or_else(|| error!("No comics are available for this series")),
	}
}

pub fn parse_year(value: &str) -> (Option<i64>, Option<i64>) {
	if let Some((minimum, maximum)) = value.split_once('-') {
		(minimum.trim().parse().ok(), maximum.trim().parse().ok())
	} else {
		let year = value.trim().parse().ok();
		(year, year)
	}
}

/// Language code from a chapter path, whose last segment is `{id}-{lang}-{name}`.
fn language_from_path(url_path: Option<&str>) -> Option<String> {
	let segment = url_path?.trim_end_matches('/').rsplit('/').next()?;
	segment.split('-').nth(1).and_then(normalize_language)
}

fn unix_seconds(timestamp: i64) -> Option<i64> {
	(timestamp > 0).then_some({
		if timestamp > 1_000_000_000_000 {
			timestamp / 1000
		} else {
			timestamp
		}
	})
}

/// `published_first` dates by publication instead of revision: the feed reports
/// an upload, while the chapter list should move edited chapters back up.
pub fn chapter_from_data(
	mut data: ChapterData,
	base_url: &str,
	language: Option<&str>,
	published_first: bool,
) -> Option<Chapter> {
	if data.db_status.as_deref().unwrap_or("normal") != "normal" {
		return None;
	}
	let display_name = data
		.dname
		.take()
		.map(|name| clean(&name))
		.filter(|name| !name.is_empty());
	let extra_title = data
		.title
		.take()
		.map(|title| clean(&title))
		.filter(|title| !title.is_empty());
	let title = match (display_name, extra_title) {
		(Some(display_name), Some(extra_title)) if display_name != extra_title => {
			Some(format!("{display_name}: {extra_title}"))
		}
		(Some(display_name), _) => Some(display_name),
		(None, Some(extra_title)) => Some(extra_title),
		_ => None,
	};
	let mut scanlators = data
		.src_name
		.take()
		.map(|name| title_case(&name))
		.filter(|name| !name.is_empty())
		.map(|name| vec![name])
		.unwrap_or_default();
	if scanlators.is_empty() {
		scanlators = data
			.profile_nodes
			.take()
			.map(node_names)
			.unwrap_or_default();
	}
	if scanlators.is_empty() {
		scanlators = data.group_nodes.take().map(node_names).unwrap_or_default();
	}
	// Every chapter path repeats its comic's language, so listing chapters needs no
	// lookup of the comic itself.
	let language = language
		.map(Into::into)
		.or_else(|| language_from_path(data.url_path.as_deref()));
	Some(Chapter {
		key: data.id,
		chapter_number: data.cha_num.or(data.serial).map(|number| number as f32),
		volume_number: data.vol_num.map(|number| number as f32),
		title,
		date_uploaded: if published_first {
			data.date_public.or(data.date_modify).or(data.date_create)
		} else {
			data.date_modify.or(data.date_create).or(data.date_public)
		}
		.and_then(unix_seconds),
		scanlators: (!scanlators.is_empty()).then_some(scanlators),
		url: data.url_path.map(|url| absolute_url(base_url, &url)),
		language,
		..Default::default()
	})
}
