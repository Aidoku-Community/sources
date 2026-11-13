use crate::{Params, helpers, home, markdown, models::*, settings};
use aidoku::{
	Chapter, DeepLinkResult, FilterValue, HomeLayout, Manga, MangaPageResult, Page, PageContent,
	PageContext, Result,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::{QueryParameters, decode_uri},
	imports::{html::Document, net::Request, std::send_partial_result},
	prelude::*,
};

pub trait Impl {
	fn new() -> Self;
	fn params(&self) -> Params;

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let label_novels = settings::label_novels();
		if let Some(key) = query
			.as_deref()
			.and_then(|q| q.strip_prefix(params.base_url.as_ref()))
			.filter(|p| p.starts_with("/series/"))
			&& let Some(post) = helpers::post(params, key)?
		{
			return Ok(MangaPageResult {
				entries: vec![post.to_manga(label_novels)?],
				has_next_page: false,
			});
		}
		let mut qs = QueryParameters::new();
		qs.push("page", Some(&page.to_string()));
		qs.push("perPage", Some(&params.page_size.to_string()));
		qs.push("view", Some("archive"));
		qs.push("orderBy", Some(params.sorts[0]));
		qs.push("orderDirection", Some("desc"));
		if let Some(query) = query {
			qs.push("searchTerm", Some(query.trim()));
		}
		for filter in filters {
			match filter {
				FilterValue::Sort {
					index, ascending, ..
				} => {
					qs.set(
						"orderBy",
						Some(params.sorts.get(index as usize).unwrap_or(&params.sorts[0])),
					);
					qs.set(
						"orderDirection",
						Some(if ascending { "asc" } else { "desc" }),
					);
				}
				FilterValue::Text { id, value }
					if matches!(
						id.as_str(),
						"minChapters" | "maxChapters" | "createdAfter" | "createdBefore"
					) =>
				{
					qs.push(&id, Some(&value))
				}
				FilterValue::Select { id, value }
					if matches!(id.as_str(), "seriesStatus" | "seriesType")
						&& !value.is_empty() =>
				{
					qs.push(&id, Some(&value))
				}
				FilterValue::MultiSelect { id, included, .. } if id == "seriesType" => {
					qs.push(&id, Some(&included.join(",")))
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genreIds" => {
					qs.push(&id, Some(&included.join(",")));
					if !excluded.is_empty() {
						qs.push("excludedGenreIds", Some(&excluded.join(",")));
					}
				}
				_ => {}
			}
		}
		let data: SearchResponse = helpers::get_json(params, &format!("/api/query?{qs}"))?;
		Ok(MangaPageResult {
			has_next_page: data.total_count > page * params.page_size
				&& data.posts.len() < data.total_count as usize,
			entries: data
				.posts
				.iter()
				.filter_map(|p| p.to_manga(label_novels).ok())
				.collect(),
		})
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let label_novels = settings::label_novels();
		let show_locked = settings::show_locked();
		let mut slug = None;
		if needs_details || manga.key.parse::<i32>().is_err() {
			let post = helpers::post(params, &manga.key)?.ok_or(error!("Series not found"))?;
			if needs_details {
				manga.copy_from(post.to_details(&params.base_url, label_novels)?);
				slug = needs_chapters.then_some(post.slug);
				if needs_chapters {
					send_partial_result(&manga);
				}
			} else {
				manga.key = post.key()?;
				slug = Some(post.slug);
			}
		}
		if needs_chapters {
			manga.chapters = Some(
				helpers::chapters(params, &manga.key)?
					.into_iter()
					.filter(|c| show_locked || !c.locked())
					.map(|c| c.into_chapter(&params.base_url, slug.as_deref()))
					.collect::<Result<_>>()?,
			);
		}
		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let key = if let Some(path) = chapter.key.strip_prefix("/series/") {
			let (slug, chapter_slug) = path.split_once('/').ok_or(error!("Invalid chapter key"))?;
			let chapter_slug = decode_uri(chapter_slug);
			let post = helpers::post(params, slug)?.ok_or(error!("Series not found"))?;
			helpers::chapters(params, &post.key()?)?
				.into_iter()
				.find(|chapter| chapter.slug == chapter_slug)
				.ok_or(error!("Chapter not found"))?
				.id
				.to_string()
		} else {
			chapter.key
		};
		let data: ChapterResponse =
			helpers::get_json(params, &format!("/api/chapter?chapterId={key}"))?;
		if data.chapter.locked() {
			bail!("Chapter is locked");
		}
		if let Some(content) = data.chapter.content.filter(|s| !s.trim().is_empty()) {
			return Ok(vec![Page {
				content: PageContent::text(markdown::parse(&content)?),
				..Default::default()
			}]);
		}
		let mut images = data.chapter.images.ok_or(error!("Chapter has no pages"))?;
		if images.is_empty() {
			bail!("Chapter has no pages");
		}
		if params.sort_pages_by_filename {
			images.sort_by_key(|image| {
				let name = image.url.rsplit('/').next().unwrap_or(&image.url);
				let digits = name.trim_start_matches(|c: char| !c.is_ascii_digit());
				let end = digits
					.find(|c: char| !c.is_ascii_digit())
					.unwrap_or(digits.len());
				digits[..end].parse::<u32>().unwrap_or(u32::MAX)
			});
		} else {
			images.sort_by_key(|image| image.order.unwrap_or(i32::MAX));
		}
		images
			.into_iter()
			.map(|image| {
				if image.url.trim().is_empty() {
					bail!("Empty page URL");
				}
				Ok(Page {
					content: PageContent::url(image.url.replace(' ', "%20")),
					..Default::default()
				})
			})
			.collect()
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		home::get_home(self, params)
	}

	fn parse_home(&self, params: &Params, html: &Document) -> Result<HomeLayout> {
		home::parse_home(html, params)
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url
			.splitn(4, '/')
			.nth(3)
			.and_then(|path| path.strip_prefix("series/"))
		else {
			return Ok(None);
		};
		let path = path
			.split(['?', '#'])
			.next()
			.unwrap_or(path)
			.trim_end_matches('/');
		let (slug, chapter_slug) = path
			.split_once('/')
			.map_or((path, None), |(s, c)| (s, Some(c)));
		let Some(post) = helpers::post(params, slug)? else {
			return Ok(None);
		};
		let key = post.key()?;
		if let Some(chapter_slug) = chapter_slug {
			let chapter_slug = decode_uri(chapter_slug);
			if let Some(chapter) = helpers::chapters(params, &key)?
				.into_iter()
				.find(|c| c.slug == chapter_slug)
			{
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: key,
					key: chapter.id.to_string(),
				}));
			}
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga { key }))
	}
}
