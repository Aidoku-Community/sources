#![no_std]
mod helpers;
mod home;
mod models;
mod settings;

use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	ImageRequestProvider, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus,
	MultiSelectFilter, Page, PageContent, PageContext, Result, Source, Viewer,
	alloc::{String, Vec, format, vec},
	helpers::{string::PlainText, uri::QueryParameters},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};
use helpers::{
	BASE_URL, SORTS, browse_page, browse_url, chapter_list, checked_html, decode_pages, get_html,
	info, key_of, parse_status, script, slugify, text,
};

struct RinkoComics;

impl Source for RinkoComics {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut kind = String::from("comic");
		let mut qs = QueryParameters::new();
		if query.is_some() {
			qs.push("q", query.as_deref());
		}
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					qs.set("sort", SORTS.get(index as usize).copied());
				}
				FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
					"type" => kind = value,
					"status" => qs.set("status", Some(&value)),
					"genre" => qs.push("genres[]", Some(&slugify(&value))),
					_ => {}
				},
				FilterValue::MultiSelect { id, included, .. } if id == "genres[]" => {
					for genre in included {
						qs.push("genres[]", Some(&genre));
					}
				}
				_ => {}
			}
		}
		Ok(browse_page(&get_html(&browse_url(&kind, page, &qs))?))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}/{}/", manga.key);
		let html = get_html(&url)?;

		if needs_details {
			let main = html
				.select_first("main")
				.ok_or_else(|| error!("Title not found"))?;
			// Comics link `comics_genres`, novels `novels_genres`.
			let mut tags = main
				.select("a[href*='_genres/']")
				.map(|links| links.filter_map(|link| link.text()).collect::<Vec<_>>())
				.unwrap_or_default();
			tags.sort_unstable();
			tags.dedup();
			manga.title = text(&main, "h1").ok_or_else(|| error!("Title not found"))?;
			manga.cover = html
				.select_first("meta[property='og:image']")
				.and_then(|meta| meta.attr("content"));
			manga.description = text(&main, "#cwSynopsis").map(|s| s.escape_markdown());
			manga.status = info(&html, "Status").map_or(MangaStatus::Unknown, |s| parse_status(&s));
			manga.tags = (!tags.is_empty()).then_some(tags);
			manga.content_rating = ContentRating::Safe;
			if !manga.key.starts_with("novel/") {
				manga.viewer = match info(&html, "Type").as_deref() {
					Some("Manga") => Viewer::RightToLeft,
					_ => Viewer::Webtoon,
				};
			}
			manga.url = Some(url.clone());
			if needs_chapters && html.select_first("#cwMoreChapters").is_some() {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(chapter_list(&html, &url, settings::show_locked_chapters())?);
		}
		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		if chapter.locked {
			bail!("This chapter is locked on Rinko Comics");
		}
		let url = format!("{BASE_URL}/{}/", chapter.key);
		let html = get_html(&url)?;

		if html.select_first("#cwNovelText").is_some() {
			let text = html
				.select("#cwNovelText p")
				.map(|paragraphs| {
					paragraphs
						.filter_map(|p| p.text())
						.map(|t| t.escape_markdown())
						.collect::<Vec<String>>()
						.join("\n\n")
				})
				.unwrap_or_default();
			if text.is_empty() {
				bail!("No text found for this episode");
			}
			return Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}]);
		}

		let Some(encoded) = script(&html, "#cwPages") else {
			bail!("No pages found for this chapter");
		};
		let mut context = PageContext::new();
		context.insert("Referer".into(), url);
		Ok(decode_pages(&encoded)?
			.into_iter()
			.map(|page| Page {
				content: PageContent::url_context(page, context.clone()),
				..Default::default()
			})
			.collect())
	}
}

impl DynamicFilters for RinkoComics {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		// Comics and novels each list their own genres; the filter offers both.
		let requests = ["comic", "novel"]
			.into_iter()
			.map(|kind| Request::get(format!("{BASE_URL}/{kind}/")))
			.collect::<core::result::Result<Vec<_>, _>>()?;
		let mut genres: Vec<(String, String)> = Vec::new();
		for response in Request::send_all(requests) {
			let html = checked_html(&response?)?;
			for label in html.select("#cwBrowse label").into_iter().flatten() {
				if let Some(input) = label.select_first("input")
					&& input.attr("name").as_deref() == Some("genres[]")
					&& let Some(id) = input.attr("value")
					&& let Some(name) = text(&label, "span")
					&& !genres.iter().any(|(_, known)| *known == id)
				{
					genres.push((name, id));
				}
			}
		}
		let (options, ids): (Vec<_>, Vec<_>) = genres
			.into_iter()
			.map(|(name, id)| (name.into(), id.into()))
			.unzip();
		Ok(vec![
			MultiSelectFilter {
				id: "genres[]".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: false,
				uses_tag_style: true,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}
}

impl ListingProvider for RinkoComics {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		// Indexes into SORTS: 1 latest update, 2 most viewed, 3 newest.
		let (kind, index) = match listing.id.as_str() {
			"updated" => ("comic", 1),
			"views" => ("comic", 2),
			"new" => ("comic", 3),
			"novels" => ("novel", 3),
			"novels-updated" => ("novel", 1),
			_ => bail!("Unknown listing"),
		};
		self.get_search_manga_list(
			None,
			page,
			vec![
				FilterValue::Sort {
					id: "sort".into(),
					index,
					ascending: false,
				},
				FilterValue::Select {
					id: "type".into(),
					value: kind.into(),
				},
			],
		)
	}
}

impl ImageRequestProvider for RinkoComics {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DeepLinkHandler for RinkoComics {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(key) = key_of(&url) else {
			return Ok(None);
		};
		let series = match key.split_once('/') {
			Some(("comic" | "novel", _)) => return Ok(Some(DeepLinkResult::Manga { key })),
			Some(("chapter", _)) => "comic",
			Some(("novel_chapter", _)) => "novel",
			_ => return Ok(None),
		};
		// A chapter page links back to its series inside `main`; the menu's index link is outside.
		let manga_key = get_html(&url)?
			.select_first(format!("main a[href*='/{series}/']"))
			.and_then(|link| key_of(&link.attr("abs:href")?));
		Ok(manga_key.map(|manga_key| DeepLinkResult::Chapter { manga_key, key }))
	}
}

register_source!(
	RinkoComics,
	Home,
	ListingProvider,
	DynamicFilters,
	ImageRequestProvider,
	DeepLinkHandler
);
