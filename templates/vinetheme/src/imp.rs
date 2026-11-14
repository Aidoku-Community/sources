use crate::{
	Params,
	helpers::{absolute_url, link_keys, resolve_urls, response, series_path, title_case},
	home, markdown,
	models::*,
	rsc, settings,
};
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, HomeLayout, Listing, Manga, MangaPageResult,
	MultiSelectFilter, Page, PageContent, PageContext, Result,
	alloc::{String, Vec, collections::BTreeSet, string::ToString, vec},
	helpers::uri::{QueryParameters, decode_uri, encode_uri_component},
	imports::net::Request,
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
		if let Some(query) = query.as_deref()
			&& query.starts_with(&format!("{}/series/", params.base_url))
			&& let Some((key, _)) = link_keys(query)
			&& let Ok(manga) = self.get_manga_update(
				params,
				Manga {
					key,
					..Default::default()
				},
				true,
				false,
			) {
			return Ok(MangaPageResult {
				entries: vec![compact(manga)],
				has_next_page: false,
			});
		}
		self.api_page(params, page, "", query.as_deref(), &filters)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		self.api_page(params, page, &listing.id, None, &[])
	}

	fn get_dynamic_filters(&self, params: &Params) -> Result<Vec<Filter>> {
		let facets: Facets = response(Request::get(format!(
			"{}/api/series/facets",
			params.base_url
		))?)?
		.get_json_owned()?;
		Ok([
			("genre", "Genres", facets.genres),
			("tag", "Tags", facets.tags),
		]
		.into_iter()
		.filter(|(_, _, facets)| !facets.is_empty())
		.map(|(id, title, facets)| {
			let (options, ids) = facets
				.into_iter()
				.filter(|f| !f.name.trim().is_empty() && !f.slug.trim().is_empty())
				.map(|f| (title_case(f.name.trim()).into(), f.slug.into()))
				.unzip();
			MultiSelectFilter {
				id: id.into(),
				title: Some(title.into()),
				options,
				ids: Some(ids),
				is_genre: true,
				uses_tag_style: true,
				can_exclude: true,
				..Default::default()
			}
			.into()
		})
		.collect())
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let path = series_path(&manga.key);
		let slug = path.split_once('/').ok_or(error!("Invalid series key"))?.1;
		let url = format!(
			"{}/api/ai/series/{}?chapters={}",
			params.base_url,
			encode_uri_component(slug),
			if needs_chapters { "all" } else { "none" }
		);
		let mut series: Series = response(Request::get(url)?)?.get_json_owned()?;
		let mut chapters = core::mem::take(&mut series.chapters);
		let count = series.chapter_count;
		if needs_details {
			let key = core::mem::take(&mut manga.key);
			manga.copy_from(Manga::try_from(series)?);
			resolve_urls(&mut manga, &params.base_url);
			manga.key = key;
		}
		if needs_chapters {
			if count != Some(chapters.len()) {
				bail!("Incomplete chapter list");
			}
			let mut keys = BTreeSet::new();
			for chapter in &chapters {
				if !keys.insert(chapter.number.to_bits()) {
					bail!("Duplicate chapter in response");
				}
			}
			chapters.reverse();
			let show_locked = settings::show_locked();
			manga.chapters = Some(
				chapters
					.into_iter()
					.filter(|chapter| show_locked || !chapter.locked())
					.map(|chapter| chapter.into_chapter(params, &manga.key))
					.collect(),
			);
		}
		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!(
			"{}/series/{}/chapter/{}",
			params.base_url,
			series_path(&manga.key),
			chapter.key
		);
		let body = response(Request::get(url)?.header("rsc", "1"))?.get_string()?;
		let body = rsc::decode(&body)?;
		let reader: Reader = rsc::extract(&body, &["chapter", "isUnlocked", "isLocked"])?;
		if reader.is_locked && !reader.is_unlocked {
			bail!("Chapter is locked");
		}
		// Page materials can remain after protection is disabled.
		let protected = reader
			.protection_config
			.is_none_or(|config| config.is_protected);
		let detail = reader.chapter;
		if let Some(content) = detail.content.as_deref().filter(|c| !c.trim().is_empty()) {
			let text = markdown::parse(&rsc::text(&body, content)?)?;
			if text.is_empty() {
				bail!("Empty chapter content");
			}
			return Ok(vec![Page {
				content: PageContent::text(text),
				..Default::default()
			}]);
		}
		if detail.pages.is_empty() {
			bail!("Chapter is locked or has no pages");
		}
		detail
			.pages
			.into_iter()
			.map(|page| {
				if page.is_redacted
					|| protected && (page.is_encrypted || page.has_strips || page.has_fragments)
				{
					bail!("This chapter uses an unsupported image format");
				}
				let url = page
					.image_url
					.filter(|url| !url.trim().is_empty())
					.ok_or(error!("Missing reader page URL"))?;
				Ok(Page {
					content: PageContent::url(absolute_url(&params.base_url, &url)),
					..Default::default()
				})
			})
			.collect()
	}

	fn get_image_request(
		&self,
		params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{}/", params.base_url)))
	}

	fn get_home(&self, params: &Params) -> Result<HomeLayout> {
		let body = response(Request::get(&params.base_url)?)?.get_string()?;
		self.parse_home(params, &body)
	}

	fn parse_home(&self, params: &Params, body: &str) -> Result<HomeLayout> {
		home::parse(body, params)
	}

	fn handle_deep_link(&self, _params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		Ok(link_keys(&url).map(|(manga_key, chapter)| match chapter {
			Some(key) => DeepLinkResult::Chapter { manga_key, key },
			None => DeepLinkResult::Manga { key: manga_key },
		}))
	}

	fn api_page(
		&self,
		params: &Params,
		page: i32,
		listing: &str,
		query: Option<&str>,
		filters: &[FilterValue],
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		qs.push("limit", Some("24"));
		qs.push("page", Some(&page.to_string()));
		qs.push("contentMode", Some("all"));
		for pair in listing.split('&') {
			if let Some((key, value)) = pair.split_once('=') {
				qs.set(key, Some(&decode_uri(value)));
			}
		}
		if let Some(query) = query {
			qs.set("q", Some(query));
		}
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => qs.set(
					"sort",
					Some(
						[
							"updated", "popular", "trending", "views", "rating", "longest",
							"newest",
						]
						.get(*index as usize)
						.copied()
						.unwrap_or("updated"),
					),
				),
				FilterValue::Select { id, value } if id == "genre" => {
					let facets: Facets = response(Request::get(format!(
						"{}/api/series/facets",
						params.base_url
					))?)?
					.get_json_owned()?;
					if let Some((parameter, facet)) = facets
						.genres
						.into_iter()
						.map(|f| ("genre", f))
						.chain(facets.tags.into_iter().map(|f| ("tag", f)))
						.find(|(_, f)| f.name.eq_ignore_ascii_case(value) || f.slug == *value)
					{
						qs.set(parameter, Some(&facet.slug));
					} else {
						return Ok(MangaPageResult::default());
					}
				}
				FilterValue::Check { id, value } if id == "images" => {
					qs.set(id, Some(if *value == 1 { "true" } else { "false" }));
				}
				FilterValue::Select { id, value } | FilterValue::Text { id, value } => {
					if !value.is_empty() {
						qs.set(id, Some(value));
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
					..
				} => {
					qs.set(id, Some(&included.join(",")));
					if matches!(id.as_str(), "genre" | "tag") {
						qs.set(&format!("ex{id}"), Some(&excluded.join(",")));
					}
				}
				_ => {}
			}
		}
		let data: SeriesResponse = response(Request::get(format!(
			"{}/api/series?{qs}",
			params.base_url
		))?)?
		.get_json_owned()?;
		Ok(MangaPageResult {
			entries: data
				.data
				.into_iter()
				.filter_map(|series| Manga::try_from(series).ok())
				.map(|mut manga| {
					resolve_urls(&mut manga, &params.base_url);
					compact(manga)
				})
				.collect(),
			has_next_page: data.meta.has_more,
		})
	}
}
