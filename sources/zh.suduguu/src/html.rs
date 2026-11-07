//! Parsers for suduguu.com pages, as extension traits on the Aidoku HTML
//! types, plus the chapter-content text converter.

use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, collections::VecDeque, format, string::ToString, vec},
	imports::html::{Document, Element},
};
use regex::Regex;
use spin::Lazy;

const AUTHOR_PREFIX: &str = "作者：";

/// The trailing numeric id of a URL segment, e.g. `/wanjie/2.html` and
/// `/448/640388.html` yield 2 and 640388. Falls back to the book id when the
/// segment is a bare book URL like `/448/`.
fn extract_id(href: &str) -> String {
	href.trim_end_matches('/')
		.rsplit('/')
		.next()
		.unwrap_or("")
		.trim_end_matches(".html")
		.split('_')
		.next()
		.unwrap_or("")
		.to_string()
}

/// The last numeric page segment of a listing URL, e.g. `/dushi/2.html` → 2.
/// `None` for unnumbered links such as `/wanjie/` or `/`.
fn page_of_href(href: &str) -> Option<i32> {
	let last = href.trim_end_matches(".html").rsplit('/').next()?;
	last.rsplit('_').next()?.parse().ok()
}

fn clean_author(text: &str) -> String {
	text.trim()
		.trim_start_matches(AUTHOR_PREFIX)
		.trim()
		.to_string()
}

pub trait MangaPage {
	fn manga_page_result(&self, page: i32) -> Result<MangaPageResult>;
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
}

impl MangaPage for Document {
	/// Parse a listing of books: search results, ranking, completed and
	/// latest-update pages all use `.container .item` entries.
	fn manga_page_result(&self, page: i32) -> Result<MangaPageResult> {
		let Some(items) = self.select(".container .item") else {
			return Ok(MangaPageResult {
				entries: Vec::new(),
				has_next_page: false,
			});
		};

		let mut entries: Vec<Manga> = Vec::new();
		for item in items {
			if let Some(manga) = parse_book_item(&item) {
				entries.push(manga);
			}
		}

		let has_next_page = self
			.select_first(".page ul")
			.and_then(|p| p.select("a"))
			.map(|links| {
				links
					.filter_map(|a| a.attr("href"))
					.filter_map(|href| page_of_href(&href))
					.max()
					.is_some_and(|next| next > page)
			})
			.unwrap_or(false);

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		manga.url = Some(format!("{}/{}/", crate::BASE_URL, manga.key));
		if let Some(title) = self
			.select_first(".itemtxt h1 a")
			.and_then(|el| el.text())
			.map(|t| t.trim().to_string())
			.filter(|t| !t.is_empty())
		{
			manga.title = title;
		}
		manga.cover = self
			.select_first(".item a img")
			.and_then(|img| img.attr("src").or_else(|| img.attr("data-src")));

		manga.description = self
			.select(".des p")
			.map(|paras| {
				paras
					.filter_map(|p| p.text())
					.map(|t| t.trim().to_string())
					.filter(|t| !t.is_empty())
					.collect::<Vec<_>>()
					.join("\n")
			})
			.filter(|d| !d.is_empty());

		// `<p><a href="/zuozhe/?tag=…">作者：天蚕土豆</a></p>` — the author
		// link text carries the "作者：" prefix.
		let authors: Vec<String> = self
			.select(".itemtxt p a")
			.map(|links| {
				links
					.filter_map(|a| a.text())
					.map(|t| t.trim().to_string())
					.filter(|t| t.contains(AUTHOR_PREFIX))
					.map(|t| clean_author(&t))
					.filter(|t| !t.is_empty())
					.collect()
			})
			.unwrap_or_default();
		if !authors.is_empty() {
			manga.authors = Some(authors);
		}

		// `<p><span>已完结</span><span>玄幻小说</span></p>`.
		let spans: Vec<String> = self
			.select(".itemtxt p span")
			.map(|els| {
				els.filter_map(|el| el.text())
					.map(|t| t.trim().to_string())
					.collect()
			})
			.unwrap_or_default();
		if let Some(status) = spans.first() {
			manga.status =
				if status.contains("已完结") || status.contains("完结") || status.contains("全本")
				{
					MangaStatus::Completed
				} else if status.contains("连载") || status.contains("連載") {
					MangaStatus::Ongoing
				} else {
					MangaStatus::Unknown
				};
		}
		if let Some(category) = spans.into_iter().nth(1) {
			manga.tags = Some(vec![category]);
		}

		Ok(())
	}
}

/// Parse one `.container .item` entry into a series entry.
fn parse_book_item(item: &Element) -> Option<Manga> {
	let href = item.select_first("h3 a").and_then(|a| a.attr("href"))?;
	let key = extract_id(&href);
	if key.is_empty() || !key.chars().all(|c| c.is_ascii_digit()) {
		return None;
	}
	let title = item
		.select_first("h3 a")
		.and_then(|a| a.text())
		.map(|t| t.trim().to_string())
		.unwrap_or_default();
	if title.is_empty() {
		return None;
	}
	let cover = item
		.select_first("a img")
		.and_then(|img| img.attr("src").or_else(|| img.attr("data-src")));

	let book_url = Some(format!("{}/{}/", crate::BASE_URL, key));
	Some(Manga {
		key,
		url: book_url,
		cover,
		title,
		..Default::default()
	})
}

pub trait ChapterPage {
	/// Parse the full chapter list embedded in the book page (`#list ul li
	/// a`) into a newest-first `Vec<Chapter>`.
	fn chapters(&self) -> Result<Vec<Chapter>>;
}

impl ChapterPage for Document {
	fn chapters(&self) -> Result<Vec<Chapter>> {
		let Some(links) = self.select("#list ul li a") else {
			return Ok(Vec::new());
		};

		// The list is oldest-first; Aidoku expects the newest chapter
		// first, so push each entry onto the front of the deque.
		let mut chapters: VecDeque<Chapter> = VecDeque::new();
		for link in links {
			let Some(href) = link.attr("href") else {
				continue;
			};
			let key = extract_id(&href);
			if key.is_empty() || !key.chars().all(|c| c.is_ascii_digit()) {
				continue;
			}
			let title = link
				.text()
				.map(|t| t.trim().to_string())
				.unwrap_or_default();
			let number = chapter_number_of(&title);
			chapters.push_front(Chapter {
				key,
				title: (!title.is_empty()).then_some(title),
				chapter_number: number,
				url: Some(href),
				..Default::default()
			});
		}
		Ok(chapters.into_iter().collect())
	}
}

// Compile the regex once instead of per chapter to keep the chapter-list
// call fast on interpreted runtimes (see Aidoku-Community/sources#829).
static CHAPTER_NUM_RE: Lazy<Regex> = Lazy::new(|| {
	#[expect(clippy::unwrap_used)]
	Regex::new(r"\d+").unwrap()
});

/// Extract the first numeric marker from a chapter title such as
/// `第1章 陨落的天才` or `001．美女学委`.
fn chapter_number_of(title: &str) -> Option<f32> {
	CHAPTER_NUM_RE
		.find(title)
		.and_then(|m| m.as_str().parse::<f32>().ok())
}

/// The chapter title from `.submenu h1`, stripping the "书名 > " prefix.
pub fn chapter_title(document: &Document) -> Option<String> {
	document
		.select_first(".submenu h1")
		.and_then(|el| el.text())
		.map(|t| t.trim().to_string())
		.filter(|t| !t.is_empty())
		.map(|t| t.rsplit('>').next().unwrap_or(&t).trim().to_string())
}

/// Whether the chapter page has a paginated continuation. Long chapters are
/// split into `/{book}/{ch}-{n}.html` pages; the "下一页" link in `.prenext`
/// carries the next page number while the last page's next link points to the
/// following chapter (no `-N` suffix), which does not match.
pub fn has_next_page(document: &Document, book: &str, chapter: &str, current: i32) -> bool {
	let Some(prenext) = document.select_first(".prenext") else {
		return false;
	};
	let Some(mut links) = prenext.select("a") else {
		return false;
	};
	let prefix = format!("/{}/{}-", book, chapter);
	links.any(|a| {
		a.attr("href").is_some_and(|href| {
			href.strip_prefix(&prefix)
				.and_then(|rest| rest.strip_suffix(".html"))
				.and_then(|num| num.parse::<i32>().ok())
				.is_some_and(|n| n > current)
		})
	})
}

// Compile the regexes once instead of per page to keep the page-list call
// fast on interpreted runtimes (see Aidoku-Community/sources#829).
static BLOCK_RE: Lazy<Regex> = Lazy::new(|| {
	#[expect(clippy::unwrap_used)]
	Regex::new(
		r"(?is)<(?:script|ins|iframe)[^>]*>.*?</(?:script|ins|iframe)>|<(?:script|ins|iframe)[^>]*/>",
	)
	.unwrap()
});
static AD_RE: Lazy<Regex> = Lazy::new(|| {
	#[expect(clippy::unwrap_used)]
	Regex::new(
		r#"(?is)<(?:div|span)[^>]*(?:class|id)="[^"]*(?:\b|_)(?:ad|gg|ggw|slt|promo)(?:\b|_)[^"]*"[^>]*>.*?</(?:div|span)>"#,
	)
	.unwrap()
});
static BR_RE: Lazy<Regex> = Lazy::new(|| {
	#[expect(clippy::unwrap_used)]
	Regex::new(r"(?i)<br\s*/?>|</p>|</div>|</li>|</h\d>").unwrap()
});
static TAG_RE: Lazy<Regex> = Lazy::new(|| {
	#[expect(clippy::unwrap_used)]
	Regex::new(r"(?s)<[^>]+>").unwrap()
});
static NL_RE: Lazy<Regex> = Lazy::new(|| {
	#[expect(clippy::unwrap_used)]
	Regex::new(r"\n{3,}").unwrap()
});

/// Convert the raw `.con` HTML into Aidoku text (paragraph-separated).
/// Paragraph tags are closed on this site, so each `<p>…</p>` becomes a
/// paragraph; scripts and ad placeholders are dropped.
pub fn chapter_text(raw: &str) -> String {
	let mut html = BLOCK_RE.replace_all(raw, "").into_owned();

	// Drop advertisement divs/spans (class or id keywords).
	html = AD_RE.replace_all(&html, "").into_owned();

	html = BR_RE.replace_all(&html, "\n\n").into_owned();

	html = TAG_RE.replace_all(&html, "").into_owned();

	let mut text = html_unescape(&html);
	text = NL_RE.replace_all(&text, "\n\n").into_owned();
	text = text.replace("(本章完)", "").replace("（本章完）", "");
	text.trim().to_string()
}

/// Drop the leading duplicate chapter-title paragraph, if any (the site
/// repeats the title as the first paragraph of the body).
pub fn strip_leading_title(text: &str, title: &str) -> String {
	if title.is_empty() {
		return text.to_string();
	}
	if let Some(rest) = text.strip_prefix(title) {
		return rest.trim_start_matches(['\n', '\r', ' ']).to_string();
	}
	text.to_string()
}

fn html_unescape(html: &str) -> String {
	html.replace("&nbsp;", " ")
		.replace("&amp;", "&")
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&#39;", "'")
		.replace("&ldquo;", "“")
		.replace("&rdquo;", "”")
		.replace("&hellip;", "…")
}
