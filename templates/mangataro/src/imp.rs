use crate::{
	Params,
	api::SearchParams,
	helpers::{
		browse, excluded_slugs, html, link_keys, novel_title, options, parse, rating, request,
		status, tags, viewer,
	},
	home,
	models::*,
};
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, HomeLayout, Manga, MangaPageResult,
	MultiSelectFilter, MultiSelectSetting, Page, Result, SelectFilter, Setting,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{
		element::ElementHelpers,
		string::PlainText,
		uri::{QueryParameters, encode_uri_component},
	},
	imports::std::{current_date, parse_date_with_options, send_partial_result},
	prelude::*,
};
// parse_date only parses, so chrono formats the hour the chapter token is built from
use chrono::{DateTime, Utc};

const PAGE_SIZE: usize = 24;

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		mut filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base_url = params.base_url.as_ref();
		if let Some(url) = query.as_deref().filter(|url| {
			url.strip_prefix(base_url)
				.is_some_and(|path| path.starts_with('/'))
		}) && let Some((key, _)) = link_keys(url)
			&& let Ok(document) = html(
				base_url,
				&format!("{base_url}/manga/{}", encode_uri_component(&key)),
			) && let Some(title) = document
			.select_first("h1")
			.and_then(|el| el.text())
			.filter(|s| !s.is_empty())
		{
			return Ok(MangaPageResult {
				entries: vec![Manga {
					key,
					title,
					cover: document
						.select_first("[data-manga-cover]")
						.and_then(|el| el.attr("data-manga-cover")),
					..Default::default()
				}],
				has_next_page: false,
			});
		}
		// A tapped tag arrives by name, while browse takes its id
		for filter in &mut filters {
			if let FilterValue::Select { id, value } = filter
				&& id == "genre"
			{
				let (ids, names) = options(&browse(base_url)?, "genre-btn");
				let index = names
					.iter()
					.position(|name| name == value)
					.ok_or(error!("Tag not found"))?;
				*filter = FilterValue::MultiSelect {
					id: "genre".into(),
					included: vec![ids[index].to_string()],
					excluded: Vec::new(),
				};
			}
		}
		let search = SearchParams::new(query, page, filters)?;
		let titles: Vec<Title> = parse(search.request(base_url)?.send()?)?;
		if !titles.is_empty() && titles.iter().all(|title| title.key().is_none()) {
			bail!("The site returned no series links for this page. Try another sort.");
		}
		let has_next_page = titles.len() == PAGE_SIZE;
		let blocked = excluded_slugs(
			base_url,
			titles.iter().filter_map(Title::key),
			&search.excluded_tags,
		)?;
		Ok(MangaPageResult {
			entries: titles
				.into_iter()
				.filter(|title| {
					!search.excluded_types.contains(&title.manga_type)
						&& title.key().is_some_and(|key| !blocked.contains(key))
				})
				.filter_map(|title| Manga::try_from(title).ok())
				.collect(),
			has_next_page,
		})
	}

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		let document = browse(&params.base_url)?;
		let multi_select = |id: &'static str, title: &'static str, class| -> Filter {
			let (ids, options) = options(&document, class);
			MultiSelectFilter {
				id: id.into(),
				title: Some(title.into()),
				is_genre: id == "genre",
				uses_tag_style: id == "genre",
				options,
				ids: Some(ids),
				can_exclude: true,
				..Default::default()
			}
			.into()
		};
		// The site's status and year dropdowns take one value, after an "All" button
		let select = |id: &'static str, title: &'static str, class| -> Filter {
			let (mut ids, mut options) = options(&document, class);
			let all = document
				.select_first(format!(".{class}[data-value='']"))
				.and_then(|el| el.text())
				.unwrap_or_else(|| "All".into());
			ids.insert(0, "".into());
			options.insert(0, all.into());
			SelectFilter {
				id: id.into(),
				title: Some(title.into()),
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into()
		};
		Ok(vec![
			multi_select("type", "Type", "type-btn"),
			select("status", "Status", "status-btn"),
			select("year", "Year", "year-btn"),
			multi_select("genre", "Tags", "genre-btn"),
			SelectFilter {
				id: "genreMatchMode".into(),
				title: Some("Match".into()),
				hide_from_header: Some(true),
				options: vec!["Any".into(), "All".into()],
				ids: Some(vec!["any".into(), "all".into()]),
				..Default::default()
			}
			.into(),
		])
	}

	fn get_dynamic_settings(&self, params: &Params) -> Result<Vec<Setting>> {
		let (values, titles) = options(&browse(&params.base_url)?, "genre-btn");
		Ok(vec![
			MultiSelectSetting {
				key: "excludedTags".into(),
				title: "Excluded Tags".into(),
				values,
				titles: Some(titles),
				default: Some(Vec::new()),
				..Default::default()
			}
			.into(),
		])
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = params.base_url.as_ref();
		let url = format!("{base_url}/manga/{}", encode_uri_component(&manga.key));
		let document = html(base_url, &url)?;
		if needs_details {
			let title = document
				.select_first("h1")
				.and_then(|el| el.text())
				.filter(|s| !s.is_empty())
				.ok_or(error!("Missing series title"))?;
			let metadata = document
				.select("script[type='application/ld+json']")
				.into_iter()
				.flatten()
				.filter_map(|el| el.data().or_else(|| el.html()))
				.filter_map(|s| serde_json::from_str::<Metadata>(&s).ok())
				.find(|m| matches!(m.kind.as_str(), "ComicSeries" | "Book"))
				.ok_or(error!("Missing series metadata"))?;
			manga.cover = metadata.image;
			// The site writes names surname first ("Oda, Eiichiro"), so a credit stays whole
			manga.authors = metadata
				.author
				.map(|credit| String::from(credit.name.trim()))
				.filter(|name| !name.is_empty())
				.map(|name| vec![name]);
			manga.status = status(&metadata.status);
			let novel = metadata.kind == "Book";
			manga.viewer = viewer(if novel { "Novel" } else { &metadata.genre });
			manga.title = if novel { novel_title(title) } else { title };
			let tags = tags(&document);
			manga.content_rating = rating(&tags);
			manga.tags = (!tags.is_empty()).then_some(tags);
			manga.description = document
				.select_first("#description-content-tab")
				.and_then(|el| el.text_with_newlines().or_else(|| el.text()))
				.map(|text| String::from(text.trim()))
				.filter(|s| !s.is_empty())
				.map(|text| text.escape_markdown());
			manga.url = Some(url);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}
		if needs_chapters {
			let id = document
				.select_first("body")
				.and_then(|el| el.attr("data-manga-id"))
				.filter(|id| !id.is_empty())
				.ok_or(error!("Missing series id"))?;
			let mut chapters = Vec::new();
			let mut offset = 0;
			let mut timestamp = current_date();
			loop {
				let fetch = |timestamp| -> Result<_> {
					let date = DateTime::<Utc>::from_timestamp(timestamp, 0)
						.ok_or(error!("Invalid clock"))?;
					let token = format!(
						"{:x}",
						md5::compute(format!("{timestamp}mng_ch_{}", date.format("%Y%m%d%H")))
					);
					let mut query = QueryParameters::new();
					query.push_encoded("manga_id", Some(&id));
					query.push_encoded("offset", Some(&offset.to_string()));
					query.push_encoded("limit", Some("500"));
					query.push_encoded("order", Some("DESC"));
					query.push_encoded("_t", Some(&token[..16]));
					query.push_encoded("_ts", Some(&timestamp.to_string()));
					Ok(
						request(base_url, &format!("{base_url}/auth/manga-chapters?{query}"))?
							.send()?,
					)
				};
				let mut response = fetch(timestamp)?;
				if response.status_code() == 403
					&& let Some(server_time) = response.get_header("Date").and_then(|date| {
						parse_date_with_options(
							date,
							"EEE, dd MMM yyyy HH:mm:ss zzz",
							"en_US_POSIX",
							"UTC",
						)
					}) {
					// Tokens expire quickly when the device clock is skewed
					timestamp = server_time;
					response = fetch(timestamp)?;
				}
				let result: ChapterList = parse(response)?;
				if !result.success {
					bail!("Could not load chapters");
				}
				if result.has_more && result.chapters.is_empty() {
					bail!("Incomplete chapter list");
				}
				offset += result.chapters.len();
				chapters.extend(
					result
						.chapters
						.into_iter()
						.filter(|c| c.language.eq_ignore_ascii_case("en"))
						.map(Chapter::try_from)
						.collect::<Result<Vec<_>>>()?,
				);
				if !result.has_more {
					break;
				}
			}
			manga.chapters = Some(chapters);
		}
		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let id = chapter
			.key
			.rsplit_once('-')
			.map(|(_, id)| id)
			.filter(|id| id.parse::<u64>().is_ok())
			.ok_or(error!("Invalid chapter key"))?;
		let base_url = params.base_url.as_ref();
		let url = format!("{base_url}/auth/chapter-content?chapter_id={id}");
		parse::<ChapterContent>(request(base_url, &url)?.send()?)?.try_into()
	}

	fn handle_deep_link(&self, _params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(link_keys(&url).map(|(key, chapter)| match chapter {
			Some(chapter_key) => DeepLinkResult::Chapter {
				manga_key: key,
				key: chapter_key,
			},
			None => DeepLinkResult::Manga { key },
		}))
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		home::get_home(params)
	}
}
