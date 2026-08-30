use crate::{BASE_URL, net::Url};
use aidoku::{
	Manga, MangaPageResult, MangaStatus, Page, Result, Viewer,
	alloc::{String, Vec, string::ToString as _},
	error,
	imports::{
		html::{Document, Element, ElementList},
		net::Request,
	},
	prelude::*,
};
use regex::Regex;

fn extract_chapter_number(title: &str) -> Option<f32> {
	// Normalize fullwidth digits and dot to ASCII, and lowercase for matching
	let normalize = |s: &str| {
		s.chars()
			.map(|c| match c {
				'０' => '0',
				'１' => '1',
				'２' => '2',
				'３' => '3',
				'４' => '4',
				'５' => '5',
				'６' => '6',
				'７' => '7',
				'８' => '8',
				'９' => '9',
				'．' => '.',
				other => other,
			})
			.collect::<String>()
			.to_lowercase()
	};

	let s = normalize(title);

	if let Ok(re) =
		Regex::new(r"(?:第\s*)(\d+(?:\.\d+)?)|(\d+(?:\.\d+)?)\s*(?:话|話|章|回|卷|册|冊)")
		&& let Some(captures) = re.captures(&s)
		&& let Some(m) = captures.get(1).or_else(|| captures.get(2))
		&& let Ok(num) = m.as_str().parse::<f32>()
	{
		return Some(num);
	}

	// Fallback: find any standalone number in the string
	if let Ok(re2) = Regex::new(r"(?:\s|\.)+(\d+(?:\.\d+)?)\s*$")
		&& let Some(captures) = re2.captures(&s)
		&& let Some(m) = captures.get(1)
		&& let Ok(num) = m.as_str().parse::<f32>()
	{
		return Some(num);
	}
	None
}

fn extract_key(href: &str) -> String {
	href.split("/")
		.filter(|s| !s.is_empty())
		.last()
		.unwrap_or("")
		.replace(".html", "")
}

fn create_chapter(
	chapter_href: String,
	title: String,
	volume_num: f32,
	chapters_len: usize,
	volume_thumbnail: Option<String>,
) -> aidoku::Chapter {
	let chapter_key = extract_key(&chapter_href);
	let chapter_num = extract_chapter_number(&title).unwrap_or(chapters_len as f32 + 1.0);
	let url = format!("{}{}", BASE_URL, chapter_href);

	aidoku::Chapter {
		key: chapter_key,
		title: Some(title),
		volume_number: if volume_num >= 0.0 {
			Some(volume_num)
		} else {
			None
		},
		chapter_number: if chapter_num >= 0.0 {
			Some(chapter_num)
		} else {
			None
		},
		url: Some(url),
		thumbnail: volume_thumbnail,
		..Default::default()
	}
}

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		// If the page is an error/notice page (e.g. removed content),
		// Bail early and put the message into manga.description so caller
		// can surface the info instead of failing on missing selectors.
		if let Some(err_el) = self.select_first(".aui-ver-form") {
			let msg = err_el.text().unwrap_or_default().trim().to_string();
			if !msg.is_empty() {
				manga.description = Some(msg);
				return Ok(());
			}
		}
		manga.cover = self.try_select_first(".book-cover")?.attr("src");
		manga.title = self
			.try_select_first("h1.book-title")?
			.text()
			.unwrap_or_default();
		let authors = self
			.try_select(".authorname,.illname")?
			.filter_map(|a| a.text())
			.collect::<Vec<String>>();
		manga.authors = Some(authors);
		manga.description = Some(
			self.try_select_first(".book-summary>content")?
				.text()
				.unwrap_or_default(),
		);
		let tags = self
			.try_select(".tag-small-group>.tag-small>a")?
			.filter_map(|a| a.text())
			.collect::<Vec<String>>();
		manga.tags = Some(tags);
		manga.status = {
			// Status lives in `.book-meta em` (e.g. "連載中"/"已完結"), joined by
			// "·"; mirror mihon's partition by 收藏|推薦|連載中|已完結.
			let meta: Vec<String> = self
				.select(".book-meta em")
				.map(|els| els.filter_map(|e| e.text()).collect())
				.unwrap_or_default();
			let status_re = Regex::new("收藏|推薦|連載|已完結").unwrap();
			let main: Vec<&String> = meta.iter().filter(|t| status_re.is_match(t)).collect();
			match main.last() {
				Some(s) if s.contains("連載") => MangaStatus::Ongoing,
				Some(s) if s.contains("已完結") => MangaStatus::Completed,
				_ => MangaStatus::Unknown,
			}
		};
		let tags = manga.tags.as_deref().unwrap_or(&[]);
		manga.viewer = if tags
			.iter()
			.any(|tag| tag.contains("大陸") || tag.contains("韓國"))
		{
			Viewer::Webtoon
		} else if tags.iter().any(|tag| tag.contains("日本")) {
			Viewer::RightToLeft
		} else {
			Viewer::LeftToRight
		};
		manga.url = Some(Url::manga(manga.key.clone()).to_string());
		Ok(())
	}
	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		let alternate_url = self
			.select_first("link[rel='alternate']")
			.and_then(|link| link.attr("href"))
			.unwrap_or_default();

		if alternate_url.contains("detail") {
			let key = extract_key(&alternate_url);

			let cover = self.try_select_first(".book-cover")?.attr("src");
			let title = self
				.try_select_first("h1.book-title")?
				.text()
				.unwrap_or_default();

			entries.push(Manga {
				key,
				cover,
				title,
				..Default::default()
			});
		} else {
			let items = self.try_select(".book-li>a")?;
			for item in items {
				let href = item.attr("href").unwrap_or_default();
				let key = extract_key(&href);

				let cover = item
					.select_first(".book-cover>img")
					.and_then(|img| img.attr("data-src"));
				let title = item
					.select_first(".book-title")
					.and_then(|title| title.text())
					.unwrap_or_default();

				if !key.is_empty() && !title.is_empty() {
					entries.push(Manga {
						key,
						cover,
						title,
						..Default::default()
					});
				}
			}
		}

		let has_next_page = self
			.select_first("#pagelink")
			.and_then(|pagelink| {
				let strong_text = pagelink.select_first("strong").and_then(|s| s.text());
				let last_text = pagelink.select_first(".last").and_then(|l| l.text());
				if let (Some(current), Some(last)) = (strong_text, last_text) {
					Some(current != last)
				} else {
					pagelink
						.select_first(".next")
						.and_then(|n| n.attr("href"))
						.map(|href| href != "#")
				}
			})
			.unwrap_or(false);

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

/// Fetch a volume's detail page and return its chapter `href`s **in order**.
/// Uses `select` (returns `Option`) so a missing/blocked page yields `None`
/// instead of failing the whole chapter list. The list order matches the
/// catalog's `.chapter-li-a` order for the same volume, so a placeholder
/// chapter's position maps directly onto its real URL here.
fn fetch_volume_chapter_map(vol_href: &str) -> Option<Vec<String>> {
	let vol_url = format!("{}{}", BASE_URL, vol_href);
	let doc = Request::get(vol_url)
		.ok()?
		.header("Origin", BASE_URL)
		.html()
		.ok()?;
	let links = doc.select(".catalog-volume .chapter-li-a")?;
	let mut hrefs: Vec<String> = Vec::new();
	for link in links {
		let href = link.attr("href").unwrap_or_default();
		if !href.is_empty() {
			hrefs.push(href);
		}
	}
	Some(hrefs)
}

pub trait ChapterPage {
	fn chapters(&self, manga_id: &str) -> Result<Vec<aidoku::Chapter>>;
}

impl ChapterPage for Document {
	fn chapters(&self, _manga_id: &str) -> Result<Vec<aidoku::Chapter>> {
		let volumes = self.try_select(".catalog-volume")?;
		let mut chapters: Vec<aidoku::Chapter> = Vec::new();

		// Cache the resolved (title -> real href) map of a volume's detail page
		// so we only fetch it once per volume that actually needs it.
		let mut cached_vol_href: Option<String> = None;
		let mut cached_map: Vec<String> = Vec::new();

		for volume in volumes {
			let volume_title = volume
				.select("h3")
				.and_then(|h3| h3.text())
				.unwrap_or_default();
			let volume_num = extract_chapter_number(&volume_title).unwrap_or(-1.0);
			let volume_thumbnail = volume
				.select_first(".volume-cover-img img")
				.and_then(|img| img.attr("data-src"));

			let links = match volume.select(".chapter-li-a") {
				Some(l) if !l.is_empty() => l.collect::<Vec<Element>>(),
				_ => continue,
			};

			// Only the volumes that contain a `javascript:cid(...)` placeholder
			// need the detail page; fetch + cache it lazily per volume.
			let has_javascript = links
				.iter()
				.any(|l| l.attr("href").is_some_and(|h| h.starts_with("javascript:")));
			if has_javascript
				&& let Some(vol_href) = volume
					.select_first(".volume-cover-img")
					.and_then(|v| v.attr("href"))
				&& !vol_href.is_empty()
				&& cached_vol_href.as_deref() != Some(vol_href.as_str())
			{
				cached_vol_href = Some(vol_href.clone());
				cached_map = fetch_volume_chapter_map(&vol_href).unwrap_or_default();
			}

			// Resolve each chapter individually: keep the catalog's real href
			// for normal links, and only substitute the single placeholder
			// chapter's URL from the (cached) detail page. The detail page
			// lists the same volume's chapters in the same order, so the
			// placeholder's index within this volume maps directly onto its
			// real URL there.
			for (idx, link) in links.iter().enumerate() {
				let chapter_href = link.attr("href").unwrap_or_default();
				let title = link
					.select_first("span")
					.and_then(|span| span.text())
					.unwrap_or_default();

				let resolved_href = if chapter_href.starts_with("javascript:") {
					match cached_map.get(idx) {
						Some(h) => h.clone(),
						None => continue,
					}
				} else {
					chapter_href
				};

				let chapter = create_chapter(
					resolved_href,
					title,
					volume_num,
					chapters.len(),
					volume_thumbnail.clone(),
				);
				chapters.push(chapter);
			}
		}
		chapters.reverse();
		Ok(chapters)
	}
}

pub trait PageList {
	fn pages(&self) -> Result<Vec<Page>>;
}

impl PageList for Document {
	fn pages(&self) -> Result<Vec<Page>> {
		let mut pages: Vec<Page> = Vec::new();
		for item in self.try_select(".imagecontent")? {
			let url = item.attr("data-src").unwrap_or_default().trim().to_string();
			pages.push(Page {
				content: aidoku::PageContent::Url(url, None),
				..Default::default()
			});
		}
		Ok(pages)
	}
}

trait TrySelect {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList>;
	fn try_select_first<S: AsRef<str>>(&self, css_query: S) -> Result<Element>;
}

impl TrySelect for Document {
	fn try_select<S: AsRef<str>>(&self, css_query: S) -> Result<ElementList> {
		self.select(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}

	fn try_select_first<S: AsRef<str>>(&self, css_query: S) -> Result<Element> {
		self.select_first(&css_query)
			.ok_or_else(|| error!("No element found for selector: `{}`", css_query.as_ref()))
	}
}
