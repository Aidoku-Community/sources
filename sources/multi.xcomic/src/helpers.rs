use crate::{
	graphql::{
		BrowseParams, browse_request, editions, fetch_chapters, parse_browse, title_edition,
	},
	models::{ChapterData, ComicData, NamedData, Node, TitleNode},
	settings::{self, normalize_language},
};
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec, collections::BTreeMap, format, string::ToString},
};
use core::cmp::{Ordering, Reverse};

pub const PORNOGRAPHIC_GENRES: &[&str] = &["adult", "hentai", "pornographic", "smut"];

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

fn clean_names(names: impl IntoIterator<Item = String>) -> Vec<String> {
	names
		.into_iter()
		.map(|name| clean(&name))
		.filter(|name| !name.is_empty())
		.collect()
}

fn node_names(nodes: Vec<Node<Option<NamedData>>>) -> Vec<String> {
	clean_names(
		nodes
			.into_iter()
			.filter_map(|node| node.data)
			.filter_map(|data| data.name),
	)
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
				&& edition.is_public != Some(false)
				&& edition.db_status.as_deref().unwrap_or("normal") == "normal"
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
		db_status: _,
		is_public: _,
		work,
	} = comic;
	let work = work.and_then(|node| node.data).unwrap_or_default();
	let mut raw_tags = genres.unwrap_or_default();
	raw_tags.extend(work.format_ids.unwrap_or_default());
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
	let url = site_url(base_url, url_path).unwrap_or_else(|| format!("{base_url}/source/{id}"));
	let authors = author_nodes
		.map(node_names)
		.filter(|names| !names.is_empty())
		.or_else(|| work.authors.map(clean_names))
		.filter(|names| !names.is_empty());
	let artists = artist_nodes
		.map(node_names)
		.filter(|names| !names.is_empty())
		.or_else(|| work.artists.map(clean_names))
		.filter(|names| !names.is_empty());
	let description = summary
		.and_then(|summary| summary.text)
		.filter(|summary| !summary.trim().is_empty())
		.or(description)
		.or(work.description)
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
	// Editions of one title are identical on a shelf without the team's name. Merged
	// editions name their teams on each chapter instead.
	if settings::show_source_in_title()
		&& !settings::merge_editions()
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

/// The site writes an edition as `/source/`; saved links still carry `/comic/`.
const EDITION_PREFIXES: [&str; 2] = ["/source/", "/comic/"];

/// The two prefixes are different things, not aliases: `/title/{id}` is the
/// series, which owns one edition per language, and only edition ids work as
/// manga keys.
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
	let mut segments = EDITION_PREFIXES
		.iter()
		.find_map(|prefix| url.split(prefix).nth(1))?
		.split('/');
	Some(Target::Comic(
		id(segments.next()?)?,
		segments.next().and_then(id),
	))
}

/// Target of a pasted url or an `id:<value>` query. A bare id is not accepted,
/// since it would swallow ordinary search terms.
pub fn target_from_query(query: &str) -> Option<Target> {
	let query = query.trim();
	if query.contains("/title/") || EDITION_PREFIXES.iter().any(|prefix| query.contains(prefix)) {
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

pub fn comic_key(base_url: &str, target: Target) -> Result<String> {
	match target {
		Target::Comic(key, _) => Ok(key),
		Target::Title(id) => title_edition(base_url, &id),
	}
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

pub fn browse_page(base_url: &str, params: &BrowseParams) -> Result<MangaPageResult> {
	let response = browse_request(base_url, params)?.send()?;
	let (comics, has_next_page) = parse_browse(response, params)?;
	let entries = comics
		.into_iter()
		.map(|comic| manga_from_data(comic, base_url))
		.collect();
	Ok(MangaPageResult {
		has_next_page,
		entries,
	})
}

/// One edition's chapters, or with Merge Editions every edition's, each naming its team.
pub fn chapters(base_url: &str, key: &str, team: Option<&str>) -> Result<Vec<Chapter>> {
	if !settings::merge_editions() {
		return Ok(fetch_chapters(base_url, &[key])?
			.into_iter()
			.flatten()
			.filter_map(|chapter| chapter_from_data(chapter, base_url, None, team, false))
			.collect());
	}
	let editions = editions(base_url, key)?;
	let ids: Vec<&str> = editions.iter().map(|edition| edition.id.as_str()).collect();
	// One team's editions read as one scanlator, spelled as its leading edition spells it,
	// and keep one copy of each numbered chapter per language and volume: the leading
	// edition's.
	let mut teams: Vec<String> = Vec::new();
	let mut owners: BTreeMap<(usize, Option<String>, Option<u32>, u32), usize> = BTreeMap::new();
	let mut chapters = Vec::new();
	for (index, (edition, list)) in editions
		.iter()
		.zip(fetch_chapters(base_url, &ids)?)
		.enumerate()
	{
		let team = team_of(edition);
		for chapter in list {
			let Some(mut chapter) =
				chapter_from_data(chapter, base_url, None, team.as_deref(), false)
			else {
				continue;
			};
			if let Some(name) = chapter
				.scanlators
				.as_mut()
				.and_then(|names| names.first_mut())
			{
				let team_index = match teams
					.iter()
					.position(|seen| seen.eq_ignore_ascii_case(name))
				{
					Some(position) => {
						name.clone_from(&teams[position]);
						position
					}
					None => {
						teams.push(name.clone());
						teams.len() - 1
					}
				};
				// An unnumbered chapter may be any extra, so only numbered ones collapse.
				if let Some(number) = chapter.chapter_number
					&& *owners
						.entry((
							team_index,
							chapter.language.clone(),
							chapter.volume_number.map(f32::to_bits),
							number.to_bits(),
						))
						.or_insert(index) != index
				{
					continue;
				}
			}
			chapters.push(chapter);
		}
	}
	// Each list arrives newest first, so interleave them by number, then by upload.
	chapters.sort_by(|a, b| {
		b.chapter_number
			.partial_cmp(&a.chapter_number)
			.unwrap_or(Ordering::Equal)
			.then(b.date_uploaded.cmp(&a.date_uploaded))
	});
	Ok(chapters)
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
		comic_node,
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
	let scanlators: Vec<String> = team
		.map(str::trim)
		.filter(|team| !team.is_empty())
		.map(Into::into)
		.or_else(|| src_name.map(|name| title_case(&name)))
		.filter(|name| !name.is_empty())
		.into_iter()
		.collect();
	// Chapter paths no longer name a language, but each chapter can carry its edition's.
	let language = language.map(Into::into).or_else(|| {
		comic_node
			.and_then(|node| node.data)
			.and_then(|comic| comic.translated_language)
			.and_then(|language| normalize_language(&language))
	});
	// The shape the site's own rewriter takes when it has only a chapter id.
	let url = site_url(base_url, url_path).unwrap_or_else(|| format!("{base_url}/chapter/{id}"));
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
