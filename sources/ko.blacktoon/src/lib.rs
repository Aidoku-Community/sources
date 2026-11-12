#![no_std]

use core::cell::RefCell;

use aidoku::{
	BaseUrlProvider, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue,
	ImageRequestProvider, Manga, MangaPageResult, NotificationHandler, Page, PageContent,
	PageContext, Source, Viewer,
	alloc::{String, Vec, format},
	bail,
	helpers::{element::ElementHelpers, uri::encode_uri},
	imports::{
		error::Result,
		net::{Request, TimeUnit, set_rate_limit},
		std::send_partial_result,
	},
	register_source,
};

mod helpers;
mod models;

use models::{ChapterItem, Hosts, SeriesItem};

const PAGE_SIZE: usize = 24;

pub struct BlackToon {
	hosts: RefCell<Option<Hosts>>,
	ongoing_series: RefCell<Option<Vec<SeriesItem>>>,
	completed_series: RefCell<Option<Vec<SeriesItem>>>,
}

impl Default for BlackToon {
	fn default() -> Self {
		Self {
			hosts: RefCell::new(None),
			ongoing_series: RefCell::new(None),
			completed_series: RefCell::new(None),
		}
	}
}

impl BlackToon {
	fn get_hosts(&self, base_url: &str) -> core::cell::Ref<'_, Hosts> {
		if self.hosts.borrow().is_none() {
			*self.hosts.borrow_mut() = Some(helpers::fetch_hosts(base_url));
		}
		core::cell::Ref::map(self.hosts.borrow(), |h| h.as_ref().unwrap())
	}

	fn ensure_series(&self, completed: bool, base_url: &str) -> Result<()> {
		let cache = if completed {
			&self.completed_series
		} else {
			&self.ongoing_series
		};
		if cache.borrow().is_some() {
			return Ok(());
		}
		let file = if completed {
			"webtoon_0.js"
		} else {
			"webtoon_1.js"
		};
		let hosts = self.get_hosts(base_url);
		let url = format!("{}/{file}", hosts.webtoon);
		drop(hosts);

		let items = helpers::fetch_series_list(&url, completed)?;
		*cache.borrow_mut() = Some(items);
		Ok(())
	}
}

impl Source for BlackToon {
	fn new() -> Self {
		set_rate_limit(5, 1, TimeUnit::Seconds);
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let base_url = helpers::get_base_url();

		let mut sort_index = 0; // 0: 최신순, 1: 인기순
		let mut weekday = String::new();
		let mut filter_genre: Option<String> = None;

		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => sort_index = index,
				FilterValue::Select { id, value } if id == "weekday" => {
					weekday = value;
				}
				FilterValue::Select { id, value } if id == "genre" && !value.is_empty() => {
					filter_genre = Some(value);
				}
				_ => {}
			}
		}

		let is_finish = weekday == "finish";
		let is_weekday = !weekday.is_empty() && !is_finish;

		if !is_finish {
			self.ensure_series(false, &base_url)?;
		}
		if !is_weekday {
			self.ensure_series(true, &base_url)?;
		}

		let ongoing_borrow = self.ongoing_series.borrow();
		let completed_borrow = self.completed_series.borrow();

		let ongoing_slice = ongoing_borrow.as_deref().unwrap_or(&[]);
		let completed_slice = completed_borrow.as_deref().unwrap_or(&[]);

		let query_clean = query
			.as_deref()
			.map(str::trim)
			.filter(|q| !q.is_empty())
			.map(|q| {
				let mut s = q.to_lowercase();
				s.retain(|c| !c.is_whitespace());
				s
			});

		let target_pday = match weekday.as_str() {
			"mon" => Some("1"),
			"tue" => Some("2"),
			"wed" => Some("3"),
			"thu" => Some("4"),
			"fri" => Some("5"),
			"sat" => Some("6"),
			"sun" => Some("7"),
			"ten" => Some("10"),
			_ => None,
		};

		let genre_pattern = filter_genre.as_deref().map(|g| format!(",{g},"));

		let matches = |item: &SeriesItem| -> bool {
			if let Some(ref q) = query_clean
				&& !item.title.contains(q)
				&& !item.author.contains(q)
			{
				let mut title_clean = item.title.to_lowercase();
				title_clean.retain(|c| !c.is_whitespace());
				if !title_clean.contains(q) {
					let mut author_clean = item.author.to_lowercase();
					author_clean.retain(|c| !c.is_whitespace());
					if !author_clean.contains(q) {
						return false;
					}
				}
			}

			if let Some(pday) = target_pday
				&& item.pday != pday
			{
				return false;
			}

			if let Some(ref pattern) = genre_pattern
				&& !item.tag.contains(pattern)
			{
				return false;
			}

			true
		};

		let mut filtered: Vec<&SeriesItem> = Vec::new();
		if !is_finish {
			filtered.extend(ongoing_slice.iter().filter(|item| matches(item)));
		}
		if !is_weekday {
			filtered.extend(completed_slice.iter().filter(|item| matches(item)));
		}

		match sort_index {
			0 => filtered.sort_by_key(|a| core::cmp::Reverse(a.updated_at)),
			1 => filtered.sort_by_key(|a| core::cmp::Reverse(a.hot)),
			_ => {}
		}

		let total = filtered.len();
		let start = ((page - 1).max(0) as usize) * PAGE_SIZE;
		if start >= total {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		}

		let end = (start + PAGE_SIZE).min(total);
		let hosts = self.get_hosts(&base_url);
		let entries: Vec<Manga> = filtered[start..end]
			.iter()
			.map(|item| item.to_manga(&hosts.image))
			.collect();

		Ok(MangaPageResult {
			entries,
			has_next_page: end < total,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let base_url = helpers::get_base_url();

		if needs_details {
			let url = format!("{base_url}/webtoon/{}.html", manga.key);
			let html = helpers::request(&url)?.html()?;

			if let Some(title_elem) = html
				.select_first(".col-12.col-sm-5.col-md-5 h3, h3 b, h3")
				.and_then(|h3| h3.text())
			{
				let trimmed = title_elem.trim();
				if !trimmed.is_empty() {
					manga.title = trimmed.into();
				}
			}

			if let Some(p_nodes) = html.select(".col-12.col-sm-5.col-md-5 p") {
				for p in p_nodes {
					if let Some(text) = p.text()
						&& let Some((_, author_part)) = text.split_once("작가 :")
					{
						let clean_author = author_part
							.split(['\n', '\r', '\t'])
							.next()
							.unwrap_or(author_part)
							.trim();
						if !clean_author.is_empty() {
							let authors: Vec<String> = clean_author
								.split(',')
								.map(|s| s.trim().into())
								.filter(|s: &String| !s.is_empty())
								.collect();
							if !authors.is_empty() {
								manga.authors = Some(authors);
							}
						}
						break;
					}
				}
			}

			if let Some(desc_meta) = html
				.select_first("meta[name=\"description\"]")
				.and_then(|m| m.attr("content"))
			{
				let trimmed = desc_meta.trim();
				if !trimmed.is_empty() && !trimmed.contains("가장 많은 무료웹툰을 보유하고 있으며")
				{
					manga.description = Some(trimmed.into());
				}
			}
			if manga.description.is_none()
				&& let Some(desc_elem) = html
					.select_first(".col-12.col-sm-5.col-md-5 p[style*=\"color:#6c6f7a\"]")
					.and_then(|p| p.text_with_newlines())
			{
				let trimmed = desc_elem.trim();
				if !trimmed.is_empty() {
					manga.description = Some(trimmed.into());
				}
			}

			let mut is_completed = false;
			let mut is_nsfw = false;
			let mut has_weekday = false;

			if let Some(badge_nodes) = html.select(".col-12.col-sm-5.col-md-5 .badge") {
				let mut tags = Vec::new();
				for node in badge_nodes {
					if node.has_class("badge-secondary") {
						has_weekday = true;
					}
					if let Some(text) = node.text() {
						let t = text.trim();
						if !t.is_empty() && !tags.iter().any(|existing| existing == t) {
							if t == "성인" || t.contains("19") {
								is_nsfw = true;
							}
							if t == "완결" {
								is_completed = true;
							}
							if t != "ALL"
								&& t != "N" && t != "성인"
								&& !t.contains("19") && !t.contains("15")
								&& !node.has_class("badge-secondary")
							{
								tags.push(t.into());
							}
						}
					}
				}
				if !tags.is_empty() {
					manga.tags = Some(tags);
				}
			}

			if !has_weekday {
				is_completed = true;
			}

			if let Some(ref completed) = *self.completed_series.borrow()
				&& completed.iter().any(|item| item.id == manga.key)
			{
				is_completed = true;
			}

			manga.content_rating = if is_nsfw {
				ContentRating::NSFW
			} else {
				ContentRating::Safe
			};

			manga.status = if is_completed {
				aidoku::MangaStatus::Completed
			} else {
				aidoku::MangaStatus::Ongoing
			};

			let hosts = self.get_hosts(&base_url);
			if let Some(o_src) = html
				.select_first("img.thumb2[o_src]")
				.and_then(|img| img.attr("o_src"))
			{
				manga.cover = Some(encode_uri(format!(
					"{}/{}",
					hosts.image,
					o_src.trim_start_matches('/')
				)));
			}

			manga.url = Some(url);
			manga.viewer = Viewer::Webtoon;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let hosts = self.get_hosts(&base_url);
			let url = format!("{}/data/toonlist/{}.js", hosts.toon_list, manga.key);
			drop(hosts);
			let content = helpers::request(&url)?.string()?;
			let json_str = helpers::extract_js_array(&content).unwrap_or("[]");
			let items: Vec<ChapterItem> = serde_json::from_str(json_str).unwrap_or_default();

			let chapters: Vec<Chapter> = items
				.into_iter()
				.rev()
				.map(|c| Chapter {
					chapter_number: helpers::extract_chapter_number(&c.title),
					date_uploaded: helpers::parse_date_to_epoch(&c.date),
					url: Some(format!("{base_url}/webtoons/{}/{}.html", manga.key, c.id)),
					title: Some(c.title),
					key: c.id,
					..Default::default()
				})
				.collect();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let base_url = helpers::get_base_url();
		let hosts = self.get_hosts(&base_url);
		let url = format!("{base_url}/webtoons/{}/{}.html", manga.key, chapter.key);
		let html = helpers::request(&url)?.html()?;

		let Some(imgs) = html.select("#toon_content_imgs img") else {
			bail!("회차 이미지를 불러올 수 없습니다.");
		};

		let pages: Vec<Page> = imgs
			.into_iter()
			.filter_map(|img| {
				let src = img.attr("o_src").or_else(|| img.attr("src"))?;
				let clean = src.trim();
				if clean.is_empty() {
					return None;
				}
				let full_url = if clean.starts_with("http") {
					clean.into()
				} else {
					format!("{}/{}", hosts.image, clean.trim_start_matches('/'))
				};
				Some(Page {
					content: PageContent::url(encode_uri(full_url)),
					..Default::default()
				})
			})
			.collect();

		if pages.is_empty() {
			bail!("회차 이미지를 불러올 수 없습니다.");
		}

		Ok(pages)
	}
}

impl BaseUrlProvider for BlackToon {
	fn get_base_url(&self) -> Result<String> {
		Ok(helpers::get_base_url())
	}
}

impl ImageRequestProvider for BlackToon {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		helpers::request(&url)
	}
}

impl DeepLinkHandler for BlackToon {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.split(['?', '#']).next().unwrap_or(&url);
		if let Some((_, rest)) = path.split_once("/webtoons/") {
			if let Some((manga_id, file)) = rest.split_once('/') {
				let chap_id = file.strip_suffix(".html").unwrap_or(file);
				return Ok(Some(DeepLinkResult::Chapter {
					manga_key: manga_id.into(),
					key: chap_id.into(),
				}));
			}
		} else if let Some((_, file)) = path.split_once("/webtoon/") {
			let manga_id = file.strip_suffix(".html").unwrap_or(file);
			return Ok(Some(DeepLinkResult::Manga {
				key: manga_id.into(),
			}));
		}
		Ok(None)
	}
}

impl NotificationHandler for BlackToon {
	fn handle_notification(&self, notification: String) {
		if notification == "fetchLatestUrl"
			&& let Some(final_url) = helpers::resolve_final_redirect(helpers::ENTRY_URL)
		{
			helpers::set_base_url(&final_url);
			*self.hosts.borrow_mut() = None;
			*self.ongoing_series.borrow_mut() = None;
			*self.completed_series.borrow_mut() = None;
		}
	}
}

register_source!(
	BlackToon,
	BaseUrlProvider,
	ImageRequestProvider,
	DeepLinkHandler,
	NotificationHandler
);
