use crate::BASE_URL;
use aidoku::{
	Chapter, ContentRating, Manga, MangaPageResult, MangaStatus, Page, PageContent, Result, Viewer,
	alloc::{Vec, string::ToString as _},
	imports::{
		html::Document,
		net::Request,
		std::{current_date, parse_date},
	},
	prelude::*,
};

fn parse_chapter_date(s: &str) -> Option<i64> {
	if let Some(ts) = parse_date(s, "yyyy 年 M 月 d 日") {
		return Some(ts);
	}

	let number = s
		.split_whitespace()
		.find_map(|w| w.parse::<i64>().ok())
		.unwrap_or(0);

	const MINUTE: i64 = 60;
	const HOUR: i64 = 60 * MINUTE;
	const DAY: i64 = 24 * HOUR;
	const WEEK: i64 = 7 * DAY;
	const MONTH: i64 = 30 * DAY;
	const YEAR: i64 = 365 * DAY;

	let offset = if s.contains("年") {
		number * YEAR
	} else if s.contains("月") {
		number * MONTH
	} else if s.contains("周") || s.contains("週") {
		number * WEEK
	} else if s.contains("天") || s.contains("日") {
		number * DAY
	} else if s.contains("小时") || s.contains("小時") {
		number * HOUR
	} else if s.contains("分") {
		number * MINUTE
	} else {
		return None;
	};

	Some(current_date() - offset)
}

pub trait MangaPage {
	fn manga_page_result(&self, is_search: bool) -> Result<MangaPageResult>;
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
}

pub trait ChapterPage {
	fn chapters(&self, manga_key: &str) -> Result<Vec<Chapter>>;
}

impl MangaPage for Document {
	fn manga_page_result(&self, is_search: bool) -> Result<MangaPageResult> {
		let entries = if is_search {
			self.select(".c-tabs-item__content")
				.map(|items| {
					items
						.filter_map(|item| {
							let href = item
								.select_first(".col-4>.tab-thumb>a")
								.and_then(|a| a.attr("href"))?;
							let id = href.split('/').rfind(|s| !s.is_empty())?.to_string();
							let cover = item
								.select_first(".col-4>.tab-thumb>a>img")
								.and_then(|img| img.attr("src"))
								.map(|s| s.replace("-193x278", ""));
							let title = item
								.select_first(".col-8>.tab-summary>.post-title>h3>a")
								.and_then(|a| a.text())?;
							let url = format!("{}/manga/{}/", BASE_URL, id);
							Some(Manga {
								key: id,
								cover,
								title,
								url: Some(url),
								content_rating: ContentRating::NSFW,
								viewer: Viewer::Webtoon,
								..Default::default()
							})
						})
						.collect::<Vec<Manga>>()
				})
				.unwrap_or_default()
		} else {
			self.select(".page-item-detail")
				.map(|items| {
					items
						.filter_map(|item| {
							let href = item
								.select_first(".item-thumb>a")
								.and_then(|a| a.attr("href"))?;
							let id = href.split('/').rfind(|s| !s.is_empty())?.to_string();
							let cover = item
								.select_first(".item-thumb>a>img")
								.and_then(|img| img.attr("src"))
								.map(|s| s.replace("-175x238", ""));
							let title = item
								.select_first(".item-summary>.post-title>h3>a")
								.and_then(|a| a.text())?;
							let url = format!("{}/manga/{}/", BASE_URL, id);
							Some(Manga {
								key: id,
								cover,
								title,
								url: Some(url),
								content_rating: ContentRating::NSFW,
								viewer: Viewer::Webtoon,
								..Default::default()
							})
						})
						.collect::<Vec<Manga>>()
				})
				.unwrap_or_default()
		};

		let has_next_page = self.select_first("a.nextpostslink").is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		manga.cover = self
			.select_first("meta[property='og:image']")
			.and_then(|m| m.attr("content"));

		manga.title = self
			.select_first("meta[property='og:title']")
			.and_then(|m| m.attr("content"))
			.unwrap_or_default();

		manga.authors = self
			.select(".author-content>a")
			.map(|els| els.filter_map(|el| el.text()).collect());

		let len = self
			.select(".post-content>div")
			.map(|e| e.count())
			.unwrap_or(0);
		if len > 0 {
			manga.description = self
				.select_first(format!(".post-content>div:nth-child({})>div>p", len))
				.and_then(|e| e.text())
				.map(|s| s.trim().to_string());
		}

		manga.tags = self
			.select(".tags-content>a")
			.map(|els| els.filter_map(|el| el.text()).collect());

		if len >= 3 {
			manga.status = self
				.select_first(format!(
					".post-content>div:nth-child({})>.summary-content",
					len - 2
				))
				.and_then(|e| e.text())
				.map(|s| match s.trim() {
					"OnGoing" | "Ongoing" | "连载中" => MangaStatus::Ongoing,
					"Completed" | "End" | "完结" => MangaStatus::Completed,
					_ => MangaStatus::Unknown,
				})
				.unwrap_or(MangaStatus::Unknown);
		}

		manga.content_rating = ContentRating::NSFW;
		manga.viewer = Viewer::Webtoon;

		Ok(())
	}
}

impl ChapterPage for Document {
	fn chapters(&self, _manga_key: &str) -> Result<Vec<Chapter>> {
		let items: Vec<_> = match self.select(".listing-chapters_main li.chapter-loveYou") {
			Some(els) => els.collect(),
			None => self
				.select(".listing-chapters_main li")
				.map(|els| els.collect())
				.unwrap_or_default(),
		};

		let len = items.len();
		let chapters: Vec<Chapter> = items
			.into_iter()
			.enumerate()
			.filter_map(|(index, li)| {
				let a = li.select_first("a")?;
				let chapter_url = match a.attr("storage-chapter-url") {
					Some(u) => u,
					None => a.attr("href")?,
				};

				let key = chapter_url
					.trim_start_matches("https://")
					.trim_start_matches("http://")
					.split_once('/')
					.map(|x| x.1)
					.map(|s| format!("/{}", s))
					.unwrap_or_else(|| chapter_url.clone());

				let title = a.text().map(|t| t.trim().to_string());
				let date_uploaded = li
					.select_first(".chapter-release-date i")
					.and_then(|el| el.text())
					.and_then(|s| parse_chapter_date(s.trim()));
				let chapter_number = (len - index) as f32;
				// 🔒 badge: sign-in locked or early access chapters.
				let locked = li.select_first(".mkjp-ea-badge").is_some();

				Some(Chapter {
					key,
					title,
					chapter_number: Some(chapter_number),
					date_uploaded,
					url: Some(chapter_url),
					locked,
					..Default::default()
				})
			})
			.collect();

		Ok(chapters)
	}
}

/// Whether the reader page says the chapter needs a login (`本章节需要登录后阅读`).
fn chapter_requires_login(html: &Document) -> bool {
	html.select_first(".mkjp-ea-lock .mkjp-ea-lock__title")
		.and_then(|el| el.text())
		.is_some_and(|t| t.contains("需要登录"))
}

fn parse_pages(html: &Document) -> Vec<Page> {
	html.select("img[id]")
		.map(|imgs| {
			imgs.filter_map(|img| {
				let src = img
					.attr("data-manga-src")
					.or_else(|| img.attr("src"))
					.map(|s| s.trim().to_string())?;
				if src.is_empty() {
					return None;
				}
				Some(Page {
					content: PageContent::url(src),
					..Default::default()
				})
			})
			.collect::<Vec<Page>>()
		})
		.unwrap_or_default()
}

pub fn get_page_list(chapter_key: &str) -> Result<Vec<Page>> {
	let url = format!("{}{}", BASE_URL, chapter_key);
	let mut html = crate::net::fetch_html_retrying(|| Ok(Request::get(&url)?))?;
	let mut pages = parse_pages(&html);

	if pages.is_empty() && chapter_requires_login(&html) && crate::auth::retry_after_lock() {
		html = crate::net::fetch_html_retrying(|| Ok(Request::get(&url)?))?;
		pages = parse_pages(&html);
	}

	Ok(pages)
}
