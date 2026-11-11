use crate::{
	Params,
	helpers::{
		chapter_date, chapter_parts, content_rating, fetch, fetch_json, labelled_block,
		manga_from_card, manga_from_page, names, novel_text, status, viewer,
	},
	home::parse_home_v2,
	models::{ChaptersResponse, SearchResponse},
};
use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeComponentValue, HomeLayout, Link, Manga,
	MangaPageResult, Page, PageContent, Result,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{
		string::{PlainText, StripPrefixOrSelf},
		uri::QueryParameters,
	},
	imports::{html::Html, std::send_partial_result},
	prelude::*,
};

const SORTS: [&str; 7] = [
	"latest", "trending", "popular", "views", "rating", "az", "za",
];

pub trait ImplV2 {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(path) = query
			.as_deref()
			.and_then(|query| query.strip_prefix(params.base_url.as_ref()))
			.filter(|path| path.starts_with('/'))
		{
			let html = fetch(&format!("{}{path}", params.base_url))?.get_html()?;
			if let Some(manga) = manga_from_page(&html) {
				return Ok(MangaPageResult {
					entries: vec![manga],
					has_next_page: false,
				});
			}
		}

		let mut qs = QueryParameters::new();
		qs.push("_ajax", Some("search"));
		if let Some(query) = query {
			qs.push("q", Some(&query));
		}
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(sort) = SORTS.get(index as usize).filter(|_| index > 0) {
						qs.push("sort", Some(sort));
					}
				}
				// A tapped tag sends its name, which the site matches lowercased
				FilterValue::Select { id, value } if id == "genre" => {
					qs.push("tag[]", Some(&value.to_lowercase()));
				}
				FilterValue::Select { id, value } if !value.is_empty() => {
					qs.push(&id, Some(&value));
				}
				FilterValue::MultiSelect { included, .. } => {
					for tag in included {
						qs.push("tag[]", Some(&tag));
					}
				}
				_ => {}
			}
		}
		qs.push("offset", Some(&((page - 1) * 20).to_string()));
		let response: SearchResponse = fetch_json(&format!("{}/?{qs}", params.base_url))?;
		let html = Html::parse_fragment_with_url(response.html, params.base_url.as_ref())?;
		Ok(MangaPageResult {
			entries: html
				.select("a.group[href]")
				.into_iter()
				.flatten()
				.filter_map(|card| manga_from_card(&card, &params.base_url))
				.collect(),
			has_next_page: response.has_more,
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let slug = manga.key.trim_start_matches('/');
		if needs_details {
			let url = format!("{}{}", params.base_url, manga.key);
			let html = fetch(&url)?.get_html()?;
			// An unknown slug still answers with a page, one without a series
			let Some(card) = manga_from_page(&html) else {
				bail!("Series not found");
			};
			manga.title = card.title;
			manga.cover = card.cover.or(manga.cover);
			manga.description = html
				.select_first("#series-desc")
				.and_then(|description| description.text())
				.map(|description| description.trim().escape_markdown())
				.filter(|description| !description.is_empty());
			manga.authors = html
				.select_first("p.l2-by")
				.and_then(|by| by.text())
				.map(|by| names(by.trim().trim_start_matches("By ")))
				.or_else(|| labelled_block(&html, "Author").map(|author| names(&author)));
			manga.artists = labelled_block(&html, "Artist").map(|artist| names(&artist));
			// "Mangatoon · Action · Ongoing": the type, a genre, then the status
			let meta = html
				.select_first(".l2-meta span")
				.and_then(|meta| meta.text());
			let mut parts = meta.iter().flat_map(|meta| meta.split(" · "));
			let kind = parts
				.next()
				.map(String::from)
				.or_else(|| labelled_block(&html, "Type"));
			let state = parts
				.last()
				.map(String::from)
				.or_else(|| labelled_block(&html, "Status"));
			manga.viewer = kind.map(|kind| viewer(&kind)).unwrap_or_default();
			manga.status = state.map(|state| status(&state)).unwrap_or_default();
			let tags: Vec<String> = html
				.select(".l2-tagline a")
				.filter(|links| !links.is_empty())
				.or_else(|| html.select("a[href*='tag%5B%5D']"))
				.into_iter()
				.flatten()
				.filter_map(|link| link.text())
				.map(|tag| tag.trim().into())
				.collect();
			manga.content_rating = content_rating(tags.iter().map(String::as_str));
			manga.tags = (!tags.is_empty()).then_some(tags);
			manga.url = Some(url);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let rows: ChaptersResponse = fetch_json(&format!(
				"{}/?_chapters_html=1&series_slug={slug}",
				params.base_url
			))?;
			let html = Html::parse_fragment_with_url(rows.rows_html, params.base_url.as_ref())?;
			manga.chapters = html.select("a.chapter-row[href]").map(|rows| {
				rows
					// Coin chapters need a login
					.filter(|row| row.attr("data-ch-locked").as_deref() != Some("1"))
					.filter_map(|row| {
						let key: String = row.attr("href")?.strip_prefix_or_self(&params.base_url).into();
						let (chapter_number, title) = chapter_parts(&row.attr("data-ch-label")?);
						Some(Chapter {
							url: Some(format!("{}{key}", params.base_url)),
							key,
							title,
							chapter_number,
							date_uploaded: row
								.select_first(".ch-date-row span")
								.and_then(|date| date.text())
								.and_then(|date| chapter_date(&date)),
							..Default::default()
						})
					})
					.collect()
			});
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = fetch(&format!("{}{}", params.base_url, chapter.key))?.get_html()?;
		// Novel chapters hold text pages among the images, or in place of them
		let pages = html
			.select("#strip-reader .reader-page")
			.into_iter()
			.flatten()
			.map(|page| {
				let content = match page.select_first(".prose-note") {
					Some(note) => PageContent::text(
						novel_text(&note).ok_or_else(|| error!("A page has no text"))?,
					),
					None => PageContent::url(
						page.attr("src")
							.ok_or_else(|| error!("A page has no image"))?,
					),
				};
				Ok(Page {
					content,
					..Default::default()
				})
			})
			.collect::<Result<Vec<Page>>>()?;
		if pages.is_empty() {
			bail!("No pages found");
		}
		Ok(pages)
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let html = fetch(&params.base_url)?.get_html()?;
		let mut home = parse_home_v2(params, &html);
		if let Some(component) = home
			.components
			.iter_mut()
			.find(|component| component.title.as_deref() == Some("Most Reviewed Series"))
		{
			// Today can be empty even when the month has results.
			if let Ok(fragment) = fetch(&format!(
				"{}/?_most_reviewed=1&range=monthly",
				params.base_url
			))
			.and_then(|response| Ok(response.get_html()?))
			{
				let entries: Vec<Link> = fragment
					.select("a[data-slide-card]")
					.into_iter()
					.flatten()
					.filter_map(|card| manga_from_card(&card, &params.base_url))
					.map(Link::from)
					.collect();
				if !entries.is_empty() {
					component.value = HomeComponentValue::Scroller {
						entries,
						listing: None,
					};
				}
			}
		}
		home.components.retain(|component| match &component.value {
			HomeComponentValue::Scroller { entries, .. } => !entries.is_empty(),
			_ => true,
		});
		Ok(home)
	}

	// Series and chapter pages share one path shape, so the page says which it is
	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.strip_prefix_or_self(&params.base_url);
		let path = path
			.split_once(['?', '#'])
			.map_or(path, |(path, _)| path)
			.trim_end_matches('/');
		if path.is_empty() {
			return Ok(None);
		}
		let html = fetch(&url)?.get_html()?;
		if let Some(manga) = manga_from_page(&html) {
			return Ok(Some(DeepLinkResult::Manga { key: manga.key }));
		}
		Ok(html
			.select_first("#sidebar-ch-list")
			.and_then(|list| list.attr("data-series-slug"))
			.map(|slug| DeepLinkResult::Chapter {
				manga_key: format!("/{slug}"),
				key: path.into(),
			}))
	}
}
