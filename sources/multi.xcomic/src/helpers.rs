use crate::{
	models::{ChapterData, ComicData, NamedData, Node, TitleNode},
	settings::{self, languages, normalize_language},
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	imports::net::Request,
	prelude::*,
};
use core::cmp::Reverse;

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

/// `(id, title)` pairs shared by search and the global exclusion setting.
pub const GENRES: &[(&str, &str)] = &[
	("action", "Action"),
	("adult", "Adult"),
	("adventure", "Adventure"),
	("age_gap", "Age Gap"),
	("aliens", "Aliens"),
	("animals", "Animals"),
	("art_by_ai", "Art-by-AI"),
	("bara", "Bara"),
	("beasts", "Beasts"),
	("blackmail", "Blackmail"),
	("bloody", "Bloody"),
	("bodyswap", "Bodyswap"),
	("boys", "Boys"),
	("boys_love", "Boys Love"),
	("brocon_siscon", "Brocon Siscon"),
	("cars", "Cars"),
	("cheating_infidelity", "Cheating/Infidelity"),
	("childhood_friends", "Childhood Friends"),
	("college_life", "College life"),
	("comedy", "Comedy"),
	("comic", "Comic"),
	("contest_winning", "Contest winning"),
	("cooking", "Cooking"),
	("crime", "Crime"),
	("crossdressing", "Crossdressing"),
	("cultivation", "Cultivation"),
	("death_game", "Death Game"),
	("degeneratemc", "Degeneratemc"),
	("delinquents", "Delinquents"),
	("dementia", "Dementia"),
	("demons", "Demons"),
	("drama", "Drama"),
	("dungeons", "Dungeons"),
	("ecchi", "Ecchi"),
	("emperors_daughter", "Emperor's Daughter"),
	("fantasy", "Fantasy"),
	("female_protagonists", "Female-protagonists"),
	("fetish", "Fetish"),
	("futa", "Futa"),
	("game", "Game"),
	("genderswap", "Genderswap"),
	("ghosts", "Ghosts"),
	("girls", "Girls"),
	("girls_love", "Girls Love"),
	("gore", "Gore"),
	("gyaru", "Gyaru"),
	("harem", "Harem"),
	("harlequin", "Harlequin"),
	("hentai", "Hentai"),
	("historical", "Historical"),
	("horror", "Horror"),
	("incest", "Incest"),
	("isekai", "Isekai"),
	("kids", "Kids"),
	("loli", "Loli"),
	("mafia", "Mafia"),
	("magic", "Magic"),
	("magical_girls", "Magical Girls"),
	("mahjong", "Mahjong"),
	("male_protagonists", "Male-protagonists"),
	("martial_arts", "Martial Arts"),
	("master_servant", "Master-Servant"),
	("mature", "Mature"),
	("mecha", "Mecha"),
	("medical", "Medical"),
	("milf", "Milf"),
	("military", "Military"),
	("monster_girls", "Monster Girls"),
	("monsters", "Monsters"),
	("music", "Music"),
	("mystery", "Mystery"),
	("netorare_ntr", "Netorare/NTR"),
	("netori", "Netori"),
	("ninja", "Ninja"),
	("office_workers", "Office Workers"),
	("omegaverse", "Omegaverse"),
	("parody", "Parody"),
	("philosophical", "Philosophical"),
	("police", "Police"),
	("post_apocalyptic", "Post-Apocalyptic"),
	("psychological", "Psychological"),
	("regression", "Regression"),
	("reincarnation", "Reincarnation"),
	("revenge", "Revenge"),
	("reverse_harem", "Reverse Harem"),
	("reverse_isekai", "Reverse Isekai"),
	("romance", "Romance"),
	("royal_family", "Royal family"),
	("royalty", "Royalty"),
	("samurai", "Samurai"),
	("school_life", "School Life"),
	("sci_fi", "Sci-Fi"),
	("sexual_violence", "Sexual Violence"),
	("shota", "Shota"),
	("shoujo_ai", "Shoujo ai"),
	("shounen_ai", "Shounen ai"),
	("showbiz", "Showbiz"),
	("slice_of_life", "Slice of Life"),
	("sm_bdsm_sub_dom", "SM/BDSM/SUB-DOM"),
	("smut", "Smut"),
	("space", "Space"),
	("sports", "Sports"),
	("spy", "Spy"),
	("step_family", "Step-family"),
	("story_by_ai", "Story-by-AI"),
	("super_power", "Super Power"),
	("superhero", "Superhero"),
	("supernatural", "Supernatural"),
	("survival", "Survival"),
	("teacher_student", "Teacher-Student"),
	("thriller", "Thriller"),
	("time_travel", "Time Travel"),
	("tower_climbing", "Tower Climbing"),
	("traditional_games", "Traditional Games"),
	("tragedy", "Tragedy"),
	("transmigration", "Transmigration"),
	("vampires", "Vampires"),
	("video_games", "Video Games"),
	("villainess", "Villainess"),
	("violence", "Violence"),
	("virtual_reality", "Virtual Reality"),
	("wuxia", "Wuxia"),
	("xianxia", "Xianxia"),
	("xuanhuan", "Xuanhuan"),
	("yakuzas", "Yakuzas"),
	("youkai", "Youkai"),
	("zombies", "Zombies"),
	("1_koma", "1-Koma"),
	("2_koma", "2-koma"),
	("3_koma", "3-koma"),
	("4_koma", "4-Koma"),
	("adaptation", "Adaptation"),
	("anthology", "Anthology"),
	("artbook", "Artbook"),
	("award_winning", "Award Winning"),
	("doujinshi", "Doujinshi"),
	("fan_colored", "Fan Colored"),
	("fanbook", "Fanbook"),
	("fanwork", "Fanwork"),
	("full_color", "Full Color"),
	("guidebook", "Guidebook"),
	("illustbook", "Illustbook"),
	("illustration_book", "Illustration Book"),
	("japanese_novel", "Japanese Novel"),
	("light_novel", "Light Novel"),
	("long_strip", "Long Strip"),
	("longstrip", "Longstrip"),
	("novels", "Novels"),
	("official_colored", "Official Colored"),
	("oneshot", "Oneshot"),
	("original_doujinshi", "Original Doujinshi"),
	("partially_colored", "Partially Colored"),
	("partially_colored_webtoon", "Partially Colored Webtoon"),
	("web_comic", "Web Comic"),
	("web_novel", "Web Novel"),
	("webtoon", "Webtoon"),
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

/// A path the API gave, against the current mirror; blanks read as absent.
fn site_url(base_url: &str, path: Option<String>) -> Option<String> {
	path.map(|url| absolute_url(base_url, &url))
		.filter(|url| !url.is_empty())
}

/// The site spells a chapter-count range the way its presets do, as `min-max`.
pub fn chapter_count_range(value: &str) -> String {
	match parse_range(value) {
		(Some(minimum), Some(maximum)) if minimum != maximum => format!("{minimum}-{maximum}"),
		(Some(value), _) | (None, Some(value)) => format!("{value}"),
		_ => String::new(),
	}
}

/// A `min-max` range, or a single value standing for both.
pub fn parse_range(value: &str) -> (Option<i64>, Option<i64>) {
	match value.split_once('-') {
		Some((minimum, maximum)) => (minimum.trim().parse().ok(), maximum.trim().parse().ok()),
		None => {
			let value = value.trim().parse().ok();
			(value, value)
		}
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

fn machine_tag(value: &str) -> bool {
	value.split_once(':').is_some_and(|(head, tail)| {
		!head.is_empty()
			&& head
				.chars()
				.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
			&& !tail.contains(char::is_whitespace)
	})
}

fn bracketed_suffix(name: &str) -> Option<&str> {
	let suffix = name
		.trim_end()
		.strip_suffix(']')?
		.rsplit_once('[')?
		.1
		.trim();
	(!suffix.is_empty()).then_some(suffix)
}

/// The team behind an edition: its `subName`, or the bracketed suffix some records
/// carry in the name instead.
pub fn team_of(comic: &ComicData) -> Option<String> {
	if let Some(team) = comic
		.sub_name
		.as_deref()
		.map(str::trim)
		.filter(|team| !team.is_empty() && !machine_tag(team))
	{
		return Some(team.into());
	}
	bracketed_suffix(&comic.name)
		.filter(|team| !machine_tag(team))
		.map(Into::into)
}

pub fn editions_of(title: TitleNode, languages: &[String], every: bool) -> Vec<ComicData> {
	let TitleNode { data, comic_nodes } = title;
	let mut editions: Vec<ComicData> = comic_nodes
		.unwrap_or_default()
		.into_iter()
		.filter_map(|node| node.data)
		.filter(|edition| {
			// An edition that states no language is no reason to hide the work.
			!edition.id.is_empty()
				&& edition
					.translated_language
					.as_ref()
					.is_none_or(|language| languages.contains(language))
		})
		.collect();
	// The reader's own language order first, then whichever edition carries the most.
	editions.sort_by_key(|edition| {
		let language = edition
			.translated_language
			.as_deref()
			.and_then(|language| languages.iter().position(|value| value == language))
			.unwrap_or(usize::MAX);
		(language, Reverse(edition.chapter_count.unwrap_or_default()))
	});
	if !every {
		editions.truncate(1);
	}
	editions
		.into_iter()
		.map(|edition| ComicData {
			id: edition.id,
			name: if edition.name.trim().is_empty() {
				data.name.clone()
			} else {
				edition.name
			},
			sub_name: edition.sub_name,
			url_path: edition.url_path,
			translated_language: edition.translated_language,
			chapter_count: edition.chapter_count,
			url_cover: data.url_cover.clone(),
			content_rating: data.content_rating.clone(),
			kind: data.kind.clone(),
			genres: data.genres.clone(),
			original_status: data.original_status.clone(),
			description: data.description.clone(),
			..Default::default()
		})
		.collect()
}

pub fn manga_from_data(comic: ComicData, base_url: &str) -> Manga {
	let team = team_of(&comic);
	let ComicData {
		id,
		name,
		sub_name: _,
		kind,
		demographics,
		content_rating: rating_id,
		genres,
		tags,
		author_nodes,
		artist_nodes,
		tag_nodes,
		summary,
		description,
		url_path,
		url_cover,
		original_status,
		upload_status,
		read_direction,
		translated_language: _,
		chapter_count: _,
	} = comic;
	let mut raw_tags = genres.unwrap_or_default();
	raw_tags.extend(demographics.unwrap_or_default());
	raw_tags.extend(tags.unwrap_or_default());
	if let Some(nodes) = tag_nodes {
		raw_tags.extend(node_names(nodes));
	}
	// Rating and viewer read the site's own lowercase ids, so derive them before
	// the tags are title cased for display.
	let rating = content_rating(rating_id.as_deref(), &raw_tags);
	let preferred_viewer = viewer(read_direction.as_deref(), kind.as_deref(), &raw_tags);

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

	let cover = site_url(base_url, url_cover);
	let url = site_url(base_url, url_path).unwrap_or_else(|| format!("{base_url}/comic/{id}"));
	let authors = author_nodes
		.map(node_names)
		.filter(|names| !names.is_empty());
	let artists = artist_nodes
		.map(node_names)
		.filter(|names| !names.is_empty());
	let description = summary
		.and_then(|summary| summary.text)
		.or(description)
		.map(|summary| summary.trim().to_string())
		.filter(|description| !description.is_empty());
	let publish_status = status(original_status.as_deref(), upload_status.as_deref());
	let mut title = clean(&name);
	// An importer marker in the name is noise on a shelf, not part of the work's title.
	let marker = bracketed_suffix(&title)
		.filter(|suffix| machine_tag(suffix))
		.and_then(|_| title.rfind('['));
	if let Some(open) = marker {
		title.truncate(open);
		title = title.trim_end().into();
	}
	// Editions of one title are identical on a shelf without the team's name.
	if settings::show_source_in_title()
		&& let Some(team) = team
		&& !title.contains(team.as_str())
	{
		title = format!("{title} [{team}]");
	}

	Manga {
		key: id,
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
			// The site writes `/comic/_/{chapter}`, where `_` stands in for a
			// comic it does not name.
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
fn resolve_title(base_url: &str, title_id: &str) -> Option<String> {
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

	let languages = languages();
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
	data: ChapterData,
	base_url: &str,
	language: Option<&str>,
	team: Option<&str>,
	published_first: bool,
) -> Option<Chapter> {
	let ChapterData {
		id,
		db_status,
		serial,
		cha_num,
		vol_num,
		dname,
		title: chapter_title,
		url_path,
		date_create,
		date_modify,
		date_public,
		src_name,
		profile_nodes,
		group_nodes,
		comic_node: _,
	} = data;
	if db_status.as_deref().unwrap_or("normal") != "normal" {
		return None;
	}
	let display_name = dname
		.map(|name| clean(&name))
		.filter(|name| !name.is_empty());
	let extra_title = chapter_title
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
	// `srcName` is only the aggregator the upload came through.
	let mut scanlators = team
		.map(str::trim)
		.filter(|team| !team.is_empty())
		.map(|team| vec![team.into()])
		.unwrap_or_default();
	if scanlators.is_empty() {
		scanlators = src_name
			.map(|name| title_case(&name))
			.filter(|name| !name.is_empty())
			.map(|name| vec![name])
			.unwrap_or_default();
	}
	if scanlators.is_empty() {
		scanlators = profile_nodes.map(node_names).unwrap_or_default();
	}
	if scanlators.is_empty() {
		scanlators = group_nodes.map(node_names).unwrap_or_default();
	}
	// Every chapter path repeats its comic's language, so listing chapters needs no
	// lookup of the comic itself.
	let language = language
		.map(Into::into)
		.or_else(|| language_from_path(url_path.as_deref()));
	// An absent or blank path would otherwise leave the web view nothing to open.
	// `_` stands in for the comic, which is the shape the site's own url rewriter
	// produces when it only has a chapter id.
	let url = site_url(base_url, url_path).unwrap_or_else(|| format!("{base_url}/comic/_/{id}"));
	Some(Chapter {
		key: id,
		chapter_number: cha_num.or(serial).map(|number| number as f32),
		volume_number: vol_num.map(|number| number as f32),
		title,
		date_uploaded: if published_first {
			date_public.or(date_modify).or(date_create)
		} else {
			date_modify.or(date_create).or(date_public)
		}
		.and_then(unix_seconds),
		scanlators: (!scanlators.is_empty()).then_some(scanlators),
		url: Some(url),
		language,
		..Default::default()
	})
}
