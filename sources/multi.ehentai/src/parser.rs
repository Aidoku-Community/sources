use crate::models::{EHGallery, EHGalleryItem, EHTag};
use aidoku::{
	Manga, MangaPageResult,
	alloc::{
		Vec,
		string::{String, ToString},
	},
	imports::html::{Document, Element},
	prelude::*,
};

pub fn parse_gallery_list(
	html: &Document,
	base_url: &str,
) -> (Vec<EHGalleryItem>, bool, Option<String>) {
	let mut items = Vec::new();

	// Extended mode: table.itg.glte
	if let Some(rows) = html.select("table.itg.glte tr") {
		for row in rows {
			if row.select_first("th").is_some() {
				continue;
			}
			if let Some(item) = parse_gallery_extended(&row, base_url) {
				items.push(item);
			}
		}
	}

	// Compact / Minimal / Minimal+ mode: table.itg (but not glte)
	if items.is_empty()
		&& let Some(rows) = html.select("table.itg tr")
	{
		for row in rows {
			if row.select_first("th").is_some() {
				continue;
			}
			if let Some(item) = parse_gallery_row(&row, base_url) {
				items.push(item);
			}
		}
	}

	// Thumbnail mode: div.gl1t
	if items.is_empty()
		&& let Some(thumbs) = html.select("div.gl1t")
	{
		for thumb in thumbs {
			if let Some(item) = parse_gallery_thumb(&thumb, base_url) {
				items.push(item);
			}
		}
	}

	let has_next_page =
		html.select_first("a#dnext").is_some() || html.select_first("a#unext[href]").is_some();

	let last_gid = items
		.last()
		.map(|item| {
			let (gid, _) = parse_gallery_id_token(&item.url);
			gid
		})
		.filter(|s| !s.is_empty());

	(items, has_next_page, last_gid)
}

pub fn parse_toplist(
	html: &Document,
	base_url: &str,
	limit: Option<usize>,
) -> (Vec<EHGalleryItem>, bool) {
	let mut items = Vec::new();

	if let Some(rows) = html.select("table.itg tr") {
		for row in rows {
			if row.select_first("th").is_some() {
				continue;
			}
			if let Some(item) = parse_toplist_row(&row, base_url) {
				items.push(item);
				if let Some(limit) = limit
					&& items.len() >= limit
				{
					break;
				}
			}
		}
	}

	let has_next = has_toplist_next_page(html);

	(items, has_next)
}

fn parse_toplist_row(row: &Element, _base_url: &str) -> Option<EHGalleryItem> {
	let link_el = row
		.select_first("td.glname a")
		.or_else(|| row.select_first("td.gl3e a"))
		.or_else(|| row.select_first("td a"))?;

	let url = link_el.attr("href")?;
	let url = normalize_gallery_url(&url);

	let glink = link_el.select_first(".glink");
	let title_opt = glink
		.as_ref()
		.and_then(|e| e.text())
		.or_else(|| link_el.text())
		.and_then(|s| {
			let s = s.trim().to_string();
			if s.is_empty() { None } else { Some(s) }
		});

	let title = title_opt?;

	let alt_title = glink
		.as_ref()
		.and_then(|e| e.attr("title"))
		.unwrap_or_default();

	let cover = row
		.select_first(".glthumb img")
		.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")))
		.unwrap_or_default();

	let category = row
		.select_first(".cn")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.to_lowercase();

	let (tags, language) = parse_item_tags(row);

	Some(EHGalleryItem {
		url,
		title,
		alt_title,
		cover,
		category,
		tags,
		language,
	})
}

fn has_toplist_next_page(html: &Document) -> bool {
	if let Some(active) = html.select_first("td.ptds") {
		let page: i32 = active
			.text()
			.and_then(|t| t.trim().parse().ok())
			.unwrap_or(0);
		return page < 199;
	}
	false
}

#[allow(dead_code)]
pub fn parse_next_page_cursor(html: &Document) -> Option<String> {
	let href = html.select_first("a#dnext")?.attr("href")?;
	href.split("next=")
		.nth(1)
		.map(|s| s.split('&').next().unwrap_or(s).to_string())
}

fn parse_item_tags(el: &Element) -> (Vec<String>, Option<String>) {
	let mut tags: Vec<String> = Vec::new();
	let mut language: Option<String> = None;

	let tag_els = el.select("div.gt, div.gtl");
	if let Some(divs) = tag_els {
		for div in divs {
			if let Some(t) = div.attr("title") {
				if t.starts_with("language:") {
					let lang = t.trim_start_matches("language:").trim();
					if lang != "translated" && lang != "rewrite" {
						language = Some(lang.to_string());
					}
				} else {
					tags.push(t);
				}
			}
		}
	}

	(tags, language)
}

fn parse_gallery_row(row: &Element, _base_url: &str) -> Option<EHGalleryItem> {
	let link_el = row
		.select_first("td.glname a")
		.or_else(|| row.select_first("td.gl3e a"))?;
	let url = link_el.attr("href")?;

	let glink = link_el.select_first(".glink");
	let title_opt = glink
		.as_ref()
		.and_then(|e| e.text())
		.or_else(|| link_el.text())
		.and_then(|s| {
			let s = s.trim();
			if s.is_empty() { None } else { Some(s.into()) }
		});

	let title = title_opt?;

	let alt_title = glink
		.as_ref()
		.and_then(|e| e.attr("title"))
		.unwrap_or_default();

	let url = normalize_gallery_url(&url);

	let cover = row
		.select_first(".glthumb img")
		.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")))
		.unwrap_or_default();

	let category = row
		.select_first(".cn")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.to_lowercase();

	let (tags, language) = parse_item_tags(row);

	Some(EHGalleryItem {
		url,
		title,
		alt_title,
		cover,
		category,
		tags,
		language,
	})
}

fn parse_gallery_extended(row: &Element, _base_url: &str) -> Option<EHGalleryItem> {
	let glink_el = row
		.select_first("div.glname .glink")
		.or_else(|| row.select_first(".gl4e .glink"))
		.or_else(|| row.select_first(".glink"))?;

	let title = glink_el.text().and_then(|s| {
		let s = s.trim();
		if s.is_empty() { None } else { Some(s.into()) }
	})?;

	let alt_title = glink_el.attr("title").unwrap_or_default();

	let url_raw = row
		.select_first("td.gl1e a")
		.and_then(|a| a.attr("href"))
		.or_else(|| {
			row.select("td.gl2e a").and_then(|links| {
				links
					.filter_map(|a| a.attr("href"))
					.find(|h| h.contains("/g/"))
			})
		})?;
	let url = normalize_gallery_url(&url_raw);

	let cover = row
		.select_first("img")
		.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")))
		.unwrap_or_default();

	let category = row
		.select_first(".cn")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.trim()
		.to_lowercase();

	let (tags, language) = parse_item_tags(row);

	Some(EHGalleryItem {
		url,
		title,
		alt_title,
		cover,
		category,
		tags,
		language,
	})
}

fn parse_gallery_thumb(thumb: &Element, _base_url: &str) -> Option<EHGalleryItem> {
	let link_el = thumb.select_first("a")?;
	let url = link_el.attr("href")?;
	let url = normalize_gallery_url(&url);

	let glink = thumb.select_first(".glink");
	let title_opt = glink.as_ref().and_then(|e| e.text()).and_then(|s| {
		let s = s.trim();
		if s.is_empty() { None } else { Some(s.into()) }
	});

	let title = title_opt?;

	let alt_title = glink
		.as_ref()
		.and_then(|e| e.attr("title"))
		.unwrap_or_default();

	let cover = thumb
		.select_first("img")
		.and_then(|img| img.attr("data-src").or_else(|| img.attr("src")))
		.unwrap_or_default();

	let category = thumb
		.select_first(".cn")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.to_lowercase();

	let (tags, language) = parse_item_tags(thumb);

	Some(EHGalleryItem {
		url,
		title,
		alt_title,
		cover,
		category,
		tags,
		language,
	})
}

pub fn parse_gallery_detail(html: &Document, gallery_url: &str) -> Option<EHGallery> {
	let mut gallery = EHGallery::default();

	let parts = parse_gallery_id_token(gallery_url);
	gallery.gid = parts.0;
	gallery.token = parts.1;

	let parsed_title_opt = html
		.select_first("#gn")
		.and_then(|e| e.text())
		.and_then(|s| {
			let s = s.trim();
			if s.is_empty() { None } else { Some(s.into()) }
		});

	if let Some(parsed_title) = parsed_title_opt {
		gallery.title = parsed_title;
	} else {
		gallery.title = "Gallery may have been removed".to_string();
		gallery.alt_title = html
			.select_first("#gj")
			.and_then(|e| e.text())
			.unwrap_or_default()
			.trim()
			.to_string();
	}

	gallery.alt_title = html
		.select_first("#gj")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.trim()
		.to_string();

	gallery.cover = html
		.select_first("#gd1 div")
		.and_then(|e| e.attr("style"))
		.and_then(|style| extract_url_from_style(&style))
		.unwrap_or_default();

	gallery.category = html
		.select_first("#gdc div")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.trim()
		.to_string()
		.to_lowercase();

	gallery.uploader = html
		.select_first("#gdn")
		.and_then(|e| e.text())
		.unwrap_or_default()
		.trim()
		.to_string();

	if let Some(rows) = html.select("#gdd tr") {
		for row in rows {
			let label = row
				.select_first(".gdt1")
				.and_then(|e| e.text())
				.unwrap_or_default();
			let value = row
				.select_first(".gdt2")
				.and_then(|e| e.text())
				.unwrap_or_default();

			match label.trim().trim_end_matches(':').to_lowercase().as_str() {
				"posted" => gallery.posted = value.trim().to_string(),
				"visible" => gallery.visible = value.trim().to_string(),
				"language" => {
					let v = value.trim();
					gallery.translated = v.ends_with("TR");
					gallery.language = v.trim_end_matches("TR").trim().to_string();
				}
				"file size" => gallery.file_size = value.trim().to_string(),
				"length" => {
					gallery.length = value
						.trim()
						.trim_end_matches("pages")
						.trim()
						.parse::<i32>()
						.unwrap_or(0);
				}
				"favorited" => {
					gallery.favorites = value
						.trim()
						.trim_end_matches("times")
						.trim()
						.parse::<i32>()
						.unwrap_or(0);
				}
				_ => {}
			}
		}
	}

	gallery.avg_rating = html
		.select_first("#rating_label")
		.and_then(|e| e.text())
		.and_then(|t| {
			t.trim()
				.trim_start_matches("Average:")
				.trim()
				.parse::<f64>()
				.ok()
		})
		.unwrap_or(0.0);

	gallery.rating_count = html
		.select_first("#rating_count")
		.and_then(|e| e.text())
		.and_then(|t| t.trim().parse::<i32>().ok())
		.unwrap_or(0);

	if let Some(rows) = html.select("#taglist tr") {
		for row in rows {
			let namespace = row
				.select_first(".tc")
				.and_then(|e| e.text())
				.unwrap_or_default()
				.trim_end_matches(':')
				.trim()
				.to_string();

			if let Some(tag_divs) = row.select("div") {
				for div in tag_divs {
					let name = div.text().unwrap_or_default().trim().to_string();
					if name.is_empty() {
						continue;
					}
					let is_weak = div
						.attr("class")
						.map(|c| c.contains("gtl"))
						.unwrap_or(false);
					gallery.tags.push(EHTag {
						namespace: namespace.clone(),
						name,
						is_weak,
					});
				}
			}
		}
	}

	Some(gallery)
}

pub fn parse_gallery_pages(html: &Document) -> Vec<String> {
	html.select("#gdt a")
		.map(|links| links.filter_map(|a| a.attr("href")).collect())
		.unwrap_or_default()
}

pub fn parse_next_gallery_page(html: &Document) -> Option<String> {
	if let Some(els) = html.select("a[onclick='return false']") {
		for el in els {
			if el.text().as_deref() == Some(">") {
				return el.attr("href");
			}
		}
	}
	None
}

pub fn parse_image_page(html: &Document) -> Option<String> {
	html.select_first("#img").and_then(|img| img.attr("src"))
}

pub fn parse_nl_value(html: &Document) -> Option<String> {
	let onclick = html
		.select_first("#loadfail")
		.and_then(|e| e.attr("onclick"))?;

	extract_between(&onclick, "nl('", "')").map(|s| s.to_string())
}

pub fn parse_showkey(html: &Document) -> Option<String> {
	let scripts = html.select("script")?;
	for script in scripts {
		let text = script.data()?;
		if text.contains("showkey")
			&& let Some(key) = extract_between(&text, "showkey=\"", "\"")
		{
			return Some(key.to_string());
		}
	}
	None
}

pub fn parse_mpv_info(html: &Document) -> Option<(String, Vec<String>)> {
	let scripts = html.select("script")?;
	for script in scripts {
		let text = script.data()?;
		if !text.contains("mpvkey") {
			continue;
		}
		let mpvkey = extract_between(&text, "mpvkey = \"", "\"")
			.or_else(|| extract_between(&text, "mpvkey=\"", "\""))
			.map(|s| s.to_string())?;

		let list_start = text.find("imagelist")?;
		let bracket_start = text[list_start..].find('[')? + list_start;
		let json_bytes = &text.as_bytes()[bracket_start..];
		let mut depth = 0usize;
		let mut end_pos = None;
		for (i, &b) in json_bytes.iter().enumerate() {
			match b {
				b'[' => depth += 1,
				b']' => {
					depth -= 1;
					if depth == 0 {
						end_pos = Some(bracket_start + i + 1);
						break;
					}
				}
				_ => {}
			}
		}
		let list_json = &text[bracket_start..end_pos?];

		let parsed: serde_json::Value = serde_json::from_str(list_json.trim()).ok()?;
		let arr = parsed.as_array()?;
		let image_keys: Vec<String> = arr
			.iter()
			.filter_map(|v| v.get("k").and_then(|k| k.as_str()).map(|s| s.to_string()))
			.collect();

		if !mpvkey.is_empty() && !image_keys.is_empty() {
			return Some((mpvkey, image_keys));
		}
	}
	None
}

pub fn parse_imgkey_from_viewer_url(viewer_url: &str) -> Option<String> {
	let parts: Vec<&str> = viewer_url.splitn(7, '/').collect();
	if parts.len() >= 6 {
		let key = parts[4];
		if !key.is_empty() {
			return Some(key.to_string());
		}
	}
	None
}

pub fn parse_gid_page_from_viewer_url(viewer_url: &str) -> Option<(String, u32)> {
	let parts: Vec<&str> = viewer_url.splitn(7, '/').collect();
	if parts.len() >= 6 {
		let gid_page = parts[5].trim_end_matches('/');
		let mut split = gid_page.splitn(2, '-');
		let gid = split.next()?.to_string();
		let page: u32 = split.next()?.parse().ok()?;
		return Some((gid, page));
	}
	None
}

pub fn normalize_gallery_url(url: &str) -> String {
	let base = url.find('?').map(|i| &url[..i]).unwrap_or(url);
	// Ensure trailing slash
	if base.ends_with('/') {
		base.to_string()
	} else {
		format!("{}/", base)
	}
}

pub fn parse_gallery_id_token(url: &str) -> (String, String) {
	let url = url.trim_end_matches('/');
	let mut parts = url.rsplitn(3, '/');
	let token = parts.next().unwrap_or_default().to_string();
	let gid = parts.next().unwrap_or_default().to_string();
	(gid, token)
}

pub fn parse_gid_token(q: &str) -> Option<(String, String)> {
	let sep = if q.contains('/') { '/' } else { ' ' };
	let parts: Vec<&str> = q.splitn(2, sep).collect();
	if parts.len() == 2
		&& parts[0].chars().all(|c| c.is_ascii_digit())
		&& !parts[0].is_empty()
		&& !parts[1].trim().is_empty()
	{
		Some((parts[0].to_string(), parts[1].trim().to_string()))
	} else {
		None
	}
}

fn extract_url_from_style(style: &str) -> Option<String> {
	let start = style.find('(')?;
	let end = style.rfind(')')?;
	if start >= end {
		return None;
	}
	Some(style[start + 1..end].trim().to_string())
}

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
	let idx = s.find(start)?;
	let after = &s[idx + start.len()..];
	let end_idx = after.find(end)?;
	Some(&after[..end_idx])
}

pub fn items_to_manga_page(
	items: Vec<EHGalleryItem>,
	has_next_page: bool,
	blocklist: &[String],
) -> MangaPageResult {
	let entries: Vec<Manga> = items
		.into_iter()
		.filter(|item| {
			if blocklist.is_empty() {
				return true;
			}
			!item.tags.iter().any(|tag| {
				let tag_lc = tag.to_lowercase();
				blocklist.iter().any(|blocked| {
					if blocked.contains(':') {
						tag_lc == *blocked
					} else {
						tag_lc
							.split_once(':')
							.map(|(_, name)| name == blocked.as_str())
							.unwrap_or(false)
					}
				})
			})
		})
		.map(|item| item.into_basic_manga())
		.collect();
	MangaPageResult {
		entries,
		has_next_page,
	}
}
