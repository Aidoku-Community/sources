//! Parsers for bilinovel pages, as extension traits on the Aidoku HTML
//! types, plus the shuffled chapter content extractor.

use crate::markdown::paragraph_to_markdown;
use crate::net::Url;
use crate::restore::{ShuffleParams, restore_indices};
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Result,
	alloc::{String, Vec, collections::VecDeque, string::ToString},
	imports::html::{Document, Element, ElementList},
	prelude::*,
};
use regex::Regex;

/// Text the server appends to the tail decoy paragraph. Never part of the
/// real chapter; strip it wherever it survives the restore.
pub const FAILURE_MARKERS: &[&str] = &[
	"（內容加載失敗！請刷新或更換瀏覽器）",
	"（内容加载失败！请刷新或更换浏览器）",
];

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

pub trait MangaPage {
	fn update_details(&self, manga: &mut Manga) -> Result<()>;
	/// Parse a book list page (wenku filter, rankings and search results
	/// all use `li.book-li > a.book-layout` items).
	fn manga_page_result(&self) -> Result<MangaPageResult>;
}

impl MangaPage for Document {
	fn update_details(&self, manga: &mut Manga) -> Result<()> {
		// Removed/age-gated works render an error panel instead of book info.
		if let Some(err_el) = self.select_first(".aui-ver-form") {
			let msg = err_el.text().unwrap_or_default().trim().to_string();
			if !msg.is_empty() {
				manga.description = Some(msg);
				return Ok(());
			}
		}

		manga.title = self
			.try_select_first(".book-cell h1.book-title")?
			.text()
			.unwrap_or_default();
		manga.cover = self
			.select_first(".book-layout img")
			.and_then(|img| img.attr("src").or_else(|| img.attr("data-src")));
		manga.description = self
			.select_first("#bookSummary content")
			.and_then(|el| el.text())
			.map(|text| text.trim().to_string())
			.filter(|text| !text.is_empty());

		let authors: Vec<String> = self
			.select(".authorname a")
			.map(|els| els.filter_map(|a| a.text()).collect())
			.unwrap_or_default();
		manga.authors = (!authors.is_empty()).then_some(authors);
		let artists: Vec<String> = self
			.select(".illname a")
			.map(|els| els.filter_map(|a| a.text()).collect())
			.unwrap_or_default();
		manga.artists = (!artists.is_empty()).then_some(artists);

		let mut tags: Vec<String> = self
			.select(".tag-small-group .tag-small a")
			.map(|els| els.filter_map(|a| a.text()).collect())
			.unwrap_or_default();
		// The publisher (文库) is shown as a standalone orange tag.
		if let Some(publisher) = self
			.select_first(".tag-small.orange")
			.and_then(|el| el.text())
			&& !tags.iter().any(|tag| tag == &publisher)
		{
			tags.push(publisher);
		}
		manga.tags = (!tags.is_empty()).then_some(tags);

		// e.g. <meta property="og:novel:status" content="完结" />
		manga.status = self
			.select_first("meta[property=\"og:novel:status\"]")
			.and_then(|el| el.attr("content"))
			.map(|status| {
				if status.contains("完结") {
					MangaStatus::Completed
				} else if status.contains("连载") {
					MangaStatus::Ongoing
				} else {
					MangaStatus::Unknown
				}
			})
			.unwrap_or(MangaStatus::Unknown);

		manga.url = Some(Url::manga(&manga.key).to_string());
		Ok(())
	}

	fn manga_page_result(&self) -> Result<MangaPageResult> {
		let mut entries: Vec<Manga> = Vec::new();

		if let Some(items) = self.select("li.book-li a.book-layout") {
			for item in items {
				if let Some(manga) = parse_book_item(&item) {
					entries.push(manga);
				}
			}
		}

		let has_next_page = self
			.select_first("#pagelink")
			.and_then(|pagelink| {
				let current = pagelink.select_first("strong").and_then(|s| s.text());
				let last = pagelink.select_first(".last").and_then(|l| l.text());
				match (current, last) {
					(Some(current), Some(last)) => Some(current != last),
					_ => pagelink
						.select_first(".next")
						.and_then(|n| n.attr("href"))
						.map(|href| href != "#"),
				}
			})
			.unwrap_or(false);

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}
}

pub trait ChapterPage {
	/// Parse the catalog (`/novel/{id}/catalog`) into a newest-first chapter
	/// list, as Aidoku expects. Volumes map to `volume_number`; chapter
	/// numbers are sequential since titles like "Life.1" or "后记" do not
	/// reliably contain one.
	fn chapters(&self) -> Result<Vec<Chapter>>;
}

impl ChapterPage for Document {
	fn chapters(&self) -> Result<Vec<Chapter>> {
		let items = self.try_select(".volume-chapters li")?;

		// The book title prefixes every volume name; stripping it leaves a
		// short label ("1 旧校舍的恶魔") prefixed onto each chapter title.
		let book = self.select_first("h1").and_then(|el| el.text());

		let mut chapters: VecDeque<Chapter> = VecDeque::new();
		let mut volume_index = 0i32;
		let mut volume_id: Option<String> = None;
		let mut volume_label = String::new();
		let mut index_in_volume = 0usize;
		// Cover of the volume a chapter belongs to (from `li.volume-cover`).
		let mut volume_cover: Option<String> = None;

		for item in items {
			let class_name = item.class_name().unwrap_or_default();
			if class_name.contains("chapter-bar") {
				volume_index += 1;
				index_in_volume = 0;
				volume_cover = None;
				let bar = item.select_first("a");
				volume_id = bar
					.as_ref()
					.and_then(|a| a.attr("href"))
					.and_then(|href| extract_volume_id(&href));
				let name = bar.and_then(|a| a.text());
				volume_label = match name {
					Some(name) => {
						match book
							.as_deref()
							.and_then(|book| name.strip_prefix(book).map(str::trim))
						{
							Some(rest) if !rest.is_empty() => rest.to_string(),
							_ => name.trim().to_string(),
						}
					}
					None => String::new(),
				};
				continue;
			}
			if class_name.contains("volume-cover") {
				volume_cover = item
					.select_first("a img")
					.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")))
					.filter(|src| !src.contains("book-cover-no"));
				continue;
			}
			if !class_name.contains("jsChapter") {
				continue;
			}

			let link = match item.select_first("a[href]") {
				Some(link) => link,
				None => continue,
			};
			let Some(href) = link.attr("href") else {
				continue;
			};
			let title = link
				.select_first(".chapter-index")
				.and_then(|el| el.text())
				.or_else(|| link.text())
				.unwrap_or_default();

			let (key, url) = if href.starts_with("/novel/") {
				let key = extract_id(&href);
				if key.is_empty() {
					continue;
				}
				(key, Some(format!("https://www.bilinovel.com{}", href)))
			} else {
				// The catalogue has placeholder `javascript:cid(N)` entries
				// for some chapters; the volume page lists their real links.
				// The key encodes volume id and the chapter's index within
				// the volume, resolved when the chapter is opened.
				match volume_id.as_deref() {
					Some(volume) => (format!("v{volume}-{index_in_volume}"), None),
					None => continue,
				}
			};
			index_in_volume += 1;

			let title = if volume_label.is_empty() {
				title
			} else {
				format!("【{volume_label}】{title}")
			};

			// The catalogue is oldest-first; Aidoku expects newest first.
			chapters.push_front(Chapter {
				key,
				title: Some(title),
				volume_number: (volume_index > 0).then_some(volume_index as f32),
				url,
				thumbnail: volume_cover.clone(),
				..Default::default()
			});
		}

		// Chapter numbers run in catalogue (reading) order: the oldest
		// chapter is 1, so number from the end of the newest-first deque.
		let total = chapters.len();
		for (position, chapter) in chapters.iter_mut().enumerate() {
			chapter.chapter_number = Some((total - position) as f32);
		}
		Ok(chapters.into_iter().collect())
	}
}

/// The numeric id of a volume page href (`/novel/2/vol_418.html` → `418`).
fn extract_volume_id(href: &str) -> Option<String> {
	let name = href.rsplit('/').next()?;
	let name = name.strip_prefix("vol_")?.strip_suffix(".html")?;
	name.bytes()
		.all(|b| b.is_ascii_digit())
		.then(|| name.to_string())
}

/// A `v{volume}-{index}` placeholder key → (volume, index). `volume`
/// strictly slices the passed key.
pub fn parse_placeholder_key(key: &str) -> Option<(&str, usize)> {
	let (volume, index) = key.strip_prefix('v')?.split_once('-')?;
	Some((volume, index.parse().ok()?))
}

/// Markers the site writes where it withheld paragraphs from a request it
/// took for a bot. A normal bilinovel page also carries the marker on its
/// scrambled decoy tail, so the signal is the marker plus the page
/// lacking the chapterlog script: a withheld serving is plain, unshuffled
/// markup without it.
const WITHHELD_MARKERS: &[&str] = &["內容加載失敗", "内容加载失败"];

/// Whether the page lost paragraphs to the site's bot check.
pub fn is_withheld(page_html: &str) -> bool {
	WITHHELD_MARKERS
		.iter()
		.any(|marker| page_html.contains(marker))
		&& !page_html.contains("chapterlog")
}

/// The other site's own copy of an illustration, served without the
/// Referer check the image host enforces (the host 403s requests without
/// a Referer; this copy answers 200). Markdown image loads cannot carry
/// headers, so every illustration url is rewritten to this copy.
const ATTACHMENT_BASE: &str = "https://tw.linovelib.com/files/article/attachment/";

fn attachment_url(src: &str) -> String {
	match src.split_once(".readpai.com/") {
		Some((host, path)) if host.starts_with("http") => {
			format!("{ATTACHMENT_BASE}{path}")
		}
		_ => src.into(),
	}
}

/// One top-level `#acontent` child worth emitting. Only paragraphs take
/// part in the shuffle; images stay anchored at their block positions.
enum Block {
	/// A paragraph; restored paragraphs zip back in document order.
	Paragraph,
	Image(String),
}

/// True for elements the shuffle algorithm treats as paragraphs: direct
/// non-empty `<p>` children. Ads and decoys are never `<p>` tags.
fn is_paragraph(element: &Element) -> bool {
	if element.tag_name().as_deref() != Some("p") {
		return false;
	}
	let html = element.html().unwrap_or_default();
	let text: String = html.chars().filter(|c| !c.is_whitespace()).collect();
	!text.is_empty()
}

/// Push an image element as a block, taking the lazy-loaded `data-src`
/// over the spinner `src` and skipping placeholders and ads.
fn push_image(img: &Element, blocks: &mut Vec<Block>) {
	let src = ["data-src", "src"]
		.iter()
		.filter_map(|attr| img.attr(attr))
		.map(|src| src.trim().to_string())
		.find(|src| src.starts_with("http"));
	if let Some(src) = src
		&& !src.contains("sloading")
		&& !src.contains("book-cover-no")
	{
		blocks.push(Block::Image(attachment_url(&src)));
	}
}

/// Collect the images of an `img`/`div` block, including the ones an
/// illustration chapter keeps in a `display:none` `#hidden-images` box:
/// they are the volume's other colour plates.
fn collect_images(element: &Element, blocks: &mut Vec<Block>) {
	if element.tag_name().as_deref() == Some("img") {
		push_image(element, blocks);
	}
	for img in element.select("img").into_iter().flatten() {
		push_image(&img, blocks);
	}
}

/// Convert one fetched chapter fragment and append it (with its page
/// number) to the accumulator. Illustrations are inlined into the text
/// at their original positions; an illustration already seen (across
/// fragments) is not added again.
pub fn extract_fragment(
	page_html: &str,
	page: i32,
	chapter_id: Option<i64>,
	params: Option<&ShuffleParams>,
	fragments: &mut Vec<(i32, String)>,
	seen_images: &mut Vec<String>,
) {
	let Ok(document) = aidoku::imports::html::Html::parse(page_html.as_bytes()) else {
		return;
	};
	let Some(content) = document.select_first("#acontent") else {
		return;
	};

	// Collect blocks and paragraph elements in document order.
	let mut blocks: Vec<Block> = Vec::new();
	let mut paragraphs: Vec<Element> = Vec::new();
	for child in content.children() {
		match child.tag_name().as_deref() {
			Some("p") if is_paragraph(&child) => {
				blocks.push(Block::Paragraph);
				paragraphs.push(child);
			}
			// Ads (`div.cgo`, `div.csgo`) carry no images and drop out;
			// the hidden colour plates live in a `div#hidden-images`.
			Some("img" | "div") => collect_images(&child, &mut blocks),
			_ => {}
		}
	}

	// Restore the shuffled paragraph order; image blocks keep their slots.
	if let (Some(chapter_id), Some(params)) = (chapter_id, params)
		&& paragraphs.len() as i64 > params.fixed_length
	{
		let indices = restore_indices(paragraphs.len(), chapter_id, params);
		let mut slots: Vec<Option<Element>> = paragraphs.into_iter().map(Some).collect();
		let mut restored: Vec<Option<Element>> = Vec::new();
		restored.resize_with(slots.len(), || None);
		for (i, paragraph) in slots.drain(..).enumerate() {
			if let Some(slot) = restored.get_mut(indices[i]) {
				*slot = paragraph;
			}
		}
		paragraphs = restored.into_iter().flatten().collect();
	}

	// Emit in document order, re-inserting the restored paragraphs at
	// their (fixed) slots.
	let mut output = String::default();
	let mut paragraph_iter = paragraphs.into_iter();
	for block in blocks {
		match block {
			Block::Paragraph => {
				if let Some(paragraph) = paragraph_iter.next() {
					let text = paragraph_to_markdown(&paragraph);
					if text.is_empty() {
						continue;
					}
					if !output.is_empty() {
						output.push_str("\n\n");
					}
					output.push_str(&text);
				}
			}
			Block::Image(url) => {
				if seen_images.iter().any(|seen| seen == &url) {
					continue;
				}
				if !output.is_empty() {
					output.push_str("\n\n");
				}
				output.push_str("![](<");
				output.push_str(&url);
				output.push_str(">)");
				seen_images.push(url);
			}
		}
	}

	for marker in FAILURE_MARKERS {
		output = output.replace(marker, "");
	}
	if !output.trim().is_empty() {
		fragments.push((page, output.trim().to_string()));
	}
}

/// The id of the chapter at `index` on a volume page.
pub fn volume_chapter(document: &Document, index: usize) -> Option<String> {
	let href = document
		.select(".volume-chapters li.jsChapter a[href]")?
		.get(index)?
		.attr("href")?;
	if !href.starts_with("/novel/") {
		return None;
	}
	let key = extract_id(&href);
	(!key.is_empty()).then_some(key)
}

/// Parse the `（page/total）` suffix of a fragment title, e.g.
/// `Life.1 不当人类。（3/7）`. Every fragment from the second page on
/// carries it, which lets a single probe learn the total page count;
/// fragments beyond the last render an out-of-range marker like `（8/7）`.
pub fn fragment_position(page_html: &str) -> Option<(i32, i32)> {
	let re = Regex::new(r#"<h1 id="atitle">[^<]*（(\d+)/(\d+)）</h1>"#).ok()?;
	let caps = re.captures(page_html)?;
	let page = caps.get(1)?.as_str().parse().ok()?;
	let total = caps.get(2)?.as_str().parse().ok()?;
	Some((page, total))
}

/// Extract the `src` of the chapterlog script, e.g.
/// `/themes/zhmb/js/chapterlog.js?v1006c1.82`.
pub fn chapterlog_src(page_html: &str) -> Option<String> {
	let re = Regex::new(r#"src="([^"]*chapterlog[^"]*)""#).ok()?;
	let caps = re.captures(page_html)?;
	Some(caps.get(1)?.as_str().to_string())
}

/// Extract the novel id from a `/novel/{id}...` href, dropping any `.html`
/// suffix and intra-chapter `_page` fragment. Shared with the home page
/// module.
pub(crate) fn extract_id(href: &str) -> String {
	href.rsplit('/')
		.next()
		.unwrap_or("")
		.trim_end_matches(".html")
		.split('_')
		.next()
		.unwrap_or("")
		.to_string()
}

/// Parse one `li.book-li > a.book-layout` item into a series entry. Shared
/// with the home page module.
pub(crate) fn parse_book_item(item: &Element) -> Option<Manga> {
	let href = item.attr("href")?;
	if !href.starts_with("/novel/") {
		return None;
	}
	let key = extract_id(&href);
	let cover = item
		.select_first(".book-cover img")
		.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")));
	let title = item
		.select_first(".book-title")
		.and_then(|el| el.text())
		.unwrap_or_default();

	if key.is_empty() || title.is_empty() {
		return None;
	}
	Some(Manga {
		key,
		cover,
		title,
		..Default::default()
	})
}
