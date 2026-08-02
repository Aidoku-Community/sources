use super::{PAGE_SIZE, Params, SERIES_TTL};
use crate::{AllSeriesItem, SeriesCache, SeriesDetail, strip_html};
use aidoku::{
	Chapter, ContentRating, DeepLinkResult, FilterValue, Listing, Manga, MangaPageResult, Page,
	PageContent, PageContext, Result,
	alloc::{collections::BTreeMap, format, string::String, vec, vec::Vec},
	imports::{net::Request, std::current_date},
	prelude::*,
};
use core::cmp::Reverse;

const USER_AGENT: &str = "Aidoku";

pub trait Impl {
	fn new() -> Self;

	fn params(&self) -> Params;

	fn content_rating_for(&self, _det: &SeriesDetail) -> ContentRating {
		ContentRating::Safe
	}

	fn api_get(&self, url: &str) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Accept", "application/json")
			.header("User-Agent", USER_AGENT))
	}

	fn html_get(&self, url: &str) -> Result<Request> {
		Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
	}

	fn fetch_all_series<'a>(
		&self,
		params: &Params,
		cache: &'a mut SeriesCache,
	) -> Result<&'a [(String, AllSeriesItem)]> {
		let now = current_date();
		let is_fresh = if let Some((_, ts)) = cache.as_ref() {
			now - *ts < SERIES_TTL
		} else {
			false
		};
		if !is_fresh {
			let map: BTreeMap<String, AllSeriesItem> = self
				.api_get(&format!("{}/api/get_all_series/", params.base_url))?
				.json_owned()?;
			let data: Vec<(String, AllSeriesItem)> = map
				.into_iter()
				.filter(|(_, v)| !v.slug.is_empty())
				.collect();
			*cache = Some((data, now));
		}
		Ok(match cache.as_ref() {
			Some((data, _)) => data.as_slice(),
			None => unreachable!(),
		})
	}

	fn fetch_filtered_series(
		&self,
		params: &Params,
		cache: &mut SeriesCache,
		query: Option<String>,
		sort_latest: bool,
		page: i32,
	) -> Result<MangaPageResult> {
		let mut series = self.fetch_all_series(params, cache)?.to_vec();

		if let Some(q) = &query {
			let q_lower = q.to_lowercase();
			series.retain(|(title, _)| title.to_lowercase().contains(&q_lower));
		}

		if sort_latest {
			series.sort_by_key(|(_, item)| Reverse(item.last_updated));
		} else {
			series.sort_by(|a, b| a.0.cmp(&b.0));
		}

		let offset = ((page - 1) as usize) * PAGE_SIZE;
		let has_next_page = offset + PAGE_SIZE < series.len();
		let entries = series
			.into_iter()
			.skip(offset)
			.take(PAGE_SIZE)
			.map(|(title, item)| item.into_manga(title, params.base_url))
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	// The series/oneshots/nsfw listing pages inject cards via a JS `series_data` array into
	// an empty <div>. Static HTML parsing finds nothing; raw text splitting extracts the slugs.
	fn fetch_html_series_list(
		&self,
		params: &Params,
		path: &str,
		page: i32,
		cache: &mut SeriesCache,
	) -> Result<MangaPageResult> {
		let base = params.base_url;
		let text = self.html_get(&format!("{base}{path}"))?.string()?;

		let mut series_map: BTreeMap<String, (String, Option<String>)> = self
			.fetch_all_series(params, cache)?
			.iter()
			.map(|(title, item)| {
				let cover = if item.cover.is_empty() {
					None
				} else {
					Some(format!("{base}{}", item.cover))
				};
				(item.slug.clone(), (title.clone(), cover))
			})
			.collect();

		let mut slugs: Vec<String> = Vec::new();
		for part in text.split("href=\"/read/manga/").skip(1) {
			let slug = part.split('/').next().unwrap_or("");
			if !slug.is_empty() && !slugs.iter().any(|s| s == slug) {
				slugs.push(String::from(slug));
			}
		}

		let all_entries: Vec<Manga> = slugs
			.into_iter()
			.map(|slug| {
				let url = format!("{base}/read/manga/{slug}/");
				let (title, cover) = if let Some(entry) = series_map.remove(&slug) {
					entry
				} else {
					(slug.clone(), None)
				};
				Manga {
					key: slug,
					title,
					url: Some(url),
					cover,
					..Default::default()
				}
			})
			.collect();

		let start = ((page - 1) as usize) * PAGE_SIZE;
		let has_next_page = start + PAGE_SIZE < all_entries.len();
		let entries = all_entries
			.into_iter()
			.skip(start)
			.take(PAGE_SIZE)
			.collect();
		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
		cache: &mut SeriesCache,
	) -> Result<MangaPageResult> {
		let sort_latest = filters
			.iter()
			.any(|f| matches!(f, FilterValue::Sort { index, .. } if *index == 1));
		self.fetch_filtered_series(params, cache, query, sort_latest, page)
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = params.base_url;
		let det: SeriesDetail = self
			.api_get(&format!("{base_url}/api/series/{}/", manga.key))?
			.json_owned()?;

		if needs_details {
			manga.content_rating = self.content_rating_for(&det);
			manga.title = det.title;
			manga.cover = if det.cover.is_empty() {
				None
			} else {
				Some(format!("{base_url}{}", det.cover))
			};
			manga.url = Some(format!("{base_url}/read/manga/{}/", det.slug));
			manga.viewer = params.viewer;

			let desc = strip_html(&det.description);
			if !desc.is_empty() {
				manga.description = Some(desc);
			}
			let has_artist = !det.artist.is_empty() && det.artist != det.author;
			if !det.author.is_empty() {
				manga.authors = Some(vec![det.author]);
			}
			if has_artist {
				manga.artists = Some(vec![det.artist]);
			}
		}

		if needs_chapters {
			let slug = det.slug.clone();
			let groups = &det.groups;
			let mut chapters: Vec<Chapter> = det
				.chapters
				.0
				.into_iter()
				.filter(|(_, ch)| ch.is_public)
				.flat_map(|(num_str, ch)| {
					let chapter_number = num_str.parse().ok();
					let volume_number = ch.volume.and_then(|s| s.parse().ok());
					let url = format!("{base_url}/read/manga/{slug}/{num_str}/1/");
					let title = ch.title.filter(|t| !t.is_empty());
					let folder = ch.folder.clone();
					let release_date = ch.release_date;
					ch.groups.0.into_iter().map(move |(group_id, _)| Chapter {
						key: format!("{folder}|{group_id}"),
						chapter_number,
						volume_number,
						title: title.clone(),
						date_uploaded: release_date.get(&group_id),
						scanlators: groups.get(&group_id).map(|name| vec![String::from(name)]),
						url: Some(url.clone()),
						..Default::default()
					})
				})
				.collect();
			chapters.sort_by(|a, b| {
				b.chapter_number
					.unwrap_or(0.0)
					.partial_cmp(&a.chapter_number.unwrap_or(0.0))
					.unwrap_or(core::cmp::Ordering::Equal)
			});
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, params: &Params, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let Some((folder, group_id)) = chapter.key.split_once('|') else {
			bail!("Invalid chapter key");
		};

		let base_url = params.base_url;
		let det: SeriesDetail = self
			.api_get(&format!("{base_url}/api/series/{}/", manga.key))?
			.json_owned()?;

		let Some(ch) = det.chapters.find_by_folder(folder) else {
			bail!("Chapter not found");
		};
		let Some(filenames) = ch.groups.get(group_id) else {
			bail!("Chapter group not found");
		};

		Ok(filenames
			.iter()
			.map(|filename| Page {
				content: PageContent::url(format!(
					"{base_url}/media/manga/{}/chapters/{folder}/{group_id}/{filename}",
					manga.key,
				)),
				..Default::default()
			})
			.collect())
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
		cache: &mut SeriesCache,
	) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"Series" => self.fetch_html_series_list(params, "/series/", page, cache),
			"Oneshots" => self.fetch_html_series_list(params, "/oneshots/", page, cache),
			"NSFW" => self.fetch_html_series_list(params, "/nsfw/", page, cache),
			_ => self.fetch_filtered_series(params, cache, None, true, page),
		}
	}

	fn handle_deep_link(&self, params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(params.base_url) else {
			return Ok(None);
		};
		let Some(rest) = path
			.strip_prefix("/read/manga/")
			.or_else(|| path.strip_prefix("/reader/manga/"))
			.or_else(|| path.strip_prefix("/reader/series/"))
			.or_else(|| path.strip_prefix("/read/series/"))
		else {
			return Ok(None);
		};
		let slug = rest.split('/').next().unwrap_or(rest);
		let slug = slug.split('?').next().unwrap_or(slug);
		let slug = slug.split('#').next().unwrap_or(slug);
		if slug.is_empty() {
			return Ok(None);
		}
		Ok(Some(DeepLinkResult::Manga {
			key: String::from(slug),
		}))
	}

	fn get_image_request(
		&self,
		_params: &Params,
		url: String,
		_context: Option<PageContext>,
	) -> Result<Request> {
		Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
	}
}
