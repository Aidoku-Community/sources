use crate::auth::{AuthedRequest, require_login};
use crate::helpers::{apply_headers, get_base_url};
use crate::keys::{manga_key, ranobe_key, ranobe_slug};
use crate::settings::{eng_title, ranobe_cover_preview, rewrite_media_url};
use aidoku::{
	Chapter, Manga, MangaPageResult, MangaStatus, Result, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element},
		net::Request,
	},
	prelude::*,
};

const RANOBE_PAGE_SIZE: i32 = 24;

fn fetch_html(url: &str) -> Result<Document> {
	require_login()?;
	let response = apply_headers(Request::get(url)?.authed()).send()?;
	if response.status_code() == 401 {
		bail!("Требуется вход в аккаунт Desu");
	}
	if response.status_code() >= 400 {
		bail!("HTTP {}", response.status_code());
	}
	Ok(response.get_html()?)
}

fn cover_from_style(style: &str) -> Option<String> {
	let start = style.find("url(")?;
	let rest = &style[start + 4..];
	let rest = rest.trim_start_matches(['\'', '"']);
	let end = rest.find(['\'', '"', ')'])?;
	let url = rest[..end].trim();
	(!url.is_empty()).then(|| url.into())
}

fn parse_catalog_item(li: &Element) -> Option<Manga> {
	let link = li.select_first("h3 a.animeTitle, h3 a")?;
	let href = link.attr("href")?;
	let slug = ranobe_slug(&href)?;
	let eng = link.text()?.trim().to_string();
	let russian = li
		.select_first(".dimmed.oTitle span, .dimmed.oTitle")
		.and_then(|el| el.text())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());
	let title = if eng_title() {
		eng
	} else {
		russian.clone().unwrap_or(eng)
	};
	let cover = li
		.select_first("span.img")
		.and_then(|el| el.attr("style"))
		.and_then(|s| cover_from_style(&s))
		.map(|url| rewrite_media_url(&url))
		.or_else(|| {
			slug.rsplit_once('.')
				.map(|(_, id)| ranobe_cover_preview(id))
		});
	let url = Some(format!("{}/ranobe/{slug}/", get_base_url()));
	Some(Manga {
		key: ranobe_key(&slug),
		title,
		cover,
		url,
		viewer: Viewer::LeftToRight,
		..Default::default()
	})
}

pub fn search_ranobe(query: Option<String>, page: i32) -> Result<MangaPageResult> {
	let mut qs = QueryParameters::new();
	qs.push("order_by", Some("updated"));
	qs.push("page", Some(page.to_string().as_str()));
	if let Some(q) = query.filter(|s| !s.is_empty()) {
		qs.push("search", Some(q.as_str()));
	}
	let url = format!("{}/ranobe/?{qs}", get_base_url());
	let html = fetch_html(&url)?;
	let entries: Vec<Manga> = html
		.select("li.memberListItem")
		.map(|els| els.filter_map(|li| parse_catalog_item(&li)).collect())
		.unwrap_or_default();
	let last_page = html
		.select_first(".PageNav")
		.and_then(|nav| nav.attr("data-last"))
		.and_then(|s| s.parse::<i32>().ok())
		.unwrap_or(1);
	let has_next_page =
		page < last_page || (last_page == 1 && entries.len() as i32 >= RANOBE_PAGE_SIZE);
	Ok(MangaPageResult {
		entries,
		has_next_page,
	})
}

fn parse_status(html: &Document) -> MangaStatus {
	let text = html
		.select_first("span.b-anime_status_tag")
		.and_then(|el| el.text())
		.unwrap_or_default()
		.to_lowercase();
	if text.contains("выход") || text.contains("онгоинг") || text.contains("ongoing") {
		MangaStatus::Ongoing
	} else if text.contains("заверш") || text.contains("издан") || text.contains("complet")
	{
		MangaStatus::Completed
	} else {
		MangaStatus::Unknown
	}
}

fn parse_title_pair(html: &Document) -> (String, Option<String>) {
	if let Some(og) = html
		.select_first("meta[property='og:title']")
		.and_then(|el| el.attr("content"))
	{
		let og = og.trim().to_string();
		if let Some(h1) = html.select_first("h1").and_then(|el| el.text()) {
			let h1 = h1.trim().to_string();
			if let Some((eng, rus)) = h1.split_once(" / ") {
				return (eng.trim().into(), Some(rus.trim().into()));
			}
			return (h1, Some(og));
		}
		return (og, None);
	}
	let h1 = html
		.select_first("h1")
		.and_then(|el| el.text())
		.unwrap_or_default()
		.trim()
		.to_string();
	if let Some((eng, rus)) = h1.split_once(" / ") {
		(eng.trim().into(), Some(rus.trim().into()))
	} else {
		(h1, None)
	}
}

fn parse_vol_ch_from_href(href: &str) -> (Option<f32>, Option<f32>) {
	let mut volume = None;
	let mut chapter = None;
	for part in href.split('/') {
		if let Some(v) = part.strip_prefix("vol") {
			volume = v.parse().ok();
		} else if let Some(c) = part.strip_prefix("ch") {
			chapter = c.parse().ok();
		}
	}
	(volume, chapter)
}

fn parse_chapter_item(li: &Element, base: &str) -> Option<Chapter> {
	let id = li
		.select_first("[data-chapters_id]")
		.and_then(|el| el.attr("data-chapters_id"))?;
	let link = li.select_first("h4 a")?;
	let href = link.attr("href")?;
	let title = link
		.attr("title")
		.or_else(|| link.text())
		.map(|s| s.trim().to_string())
		.filter(|s| !s.is_empty());
	let (volume_number, chapter_number) = parse_vol_ch_from_href(&href);
	let url = if href.starts_with("http") {
		rewrite_media_url(&href)
	} else {
		format!("{base}/{}", href.trim_start_matches('/'))
	};
	Some(Chapter {
		key: id,
		title,
		volume_number,
		chapter_number,
		url: Some(url),
		..Default::default()
	})
}

pub fn fetch_ranobe(slug: &str, needs_details: bool, needs_chapters: bool) -> Result<Manga> {
	let base = get_base_url();
	let url = format!("{base}/ranobe/{slug}/");
	let html = fetch_html(&url)?;

	let mut manga = Manga {
		key: ranobe_key(slug),
		url: Some(url.clone()),
		viewer: Viewer::LeftToRight,
		..Default::default()
	};

	if needs_details {
		let (eng, russian) = parse_title_pair(&html);
		manga.title = if eng_title() {
			eng
		} else {
			russian.clone().unwrap_or(eng)
		};
		manga.cover = html
			.select_first("img[src*='ranobe/covers']")
			.and_then(|el| el.attr("abs:src"))
			.map(|url| rewrite_media_url(&url))
			.or_else(|| {
				slug.rsplit_once('.')
					.map(|(_, id)| ranobe_cover_preview(id))
			});
		manga.description = html
			.select_first("[itemprop=description]")
			.and_then(|el| el.text())
			.map(|s| s.trim().to_string())
			.filter(|s| !s.is_empty());
		manga.status = parse_status(&html);
		manga.tags = html.select("a[href*='genre']").map(|els| {
			els.filter_map(|el| {
				el.text()
					.map(|s| s.trim().to_string())
					.filter(|s| !s.is_empty())
			})
			.collect()
		});
	}

	if needs_chapters {
		manga.chapters = Some(
			html.select("ul.chlist > li")
				.map(|els| {
					els.filter_map(|li| parse_chapter_item(&li, &base))
						.collect()
				})
				.unwrap_or_default(),
		);
	}

	Ok(manga)
}

pub fn fetch_ranobe_chapter_text(chapter_url: &str) -> Result<String> {
	let html = fetch_html(chapter_url)?;
	let text = html
		.select("div.ranobe-reader-text p, div[data-reader-text] p")
		.map(|els| {
			els.filter_map(|el| {
				el.text()
					.map(|s| s.trim().to_string())
					.filter(|s| !s.is_empty())
			})
			.collect::<Vec<_>>()
			.join("\n\n")
		})
		.unwrap_or_default();
	if text.is_empty() {
		bail!("Текст главы не найден");
	}
	Ok(text)
}

pub fn listing_manga(page: i32) -> Result<MangaPageResult> {
	use crate::helpers::search;
	use aidoku::FilterValue;
	let result = search(
		None,
		page,
		vec![FilterValue::Sort {
			id: "order".into(),
			index: 3,
			ascending: false,
		}],
	)?;
	Ok(MangaPageResult {
		has_next_page: result.has_next_page,
		entries: result
			.entries
			.into_iter()
			.map(|m| {
				let mut manga = m.into_manga(None, true, false);
				if !manga.key.starts_with("m:") && !manga.key.starts_with("r:") {
					manga.key = manga_key(&manga.key);
				}
				manga
			})
			.collect(),
	})
}
