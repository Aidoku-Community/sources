use crate::net::{extract_key, get_absolute_url};
use aidoku::{
	Manga, MangaPageResult, Result,
	alloc::{String, Vec},
	imports::html::Document,
};

pub fn parse_manga_list(html: &Document) -> Result<MangaPageResult> {
	let mut manga = Vec::new();
	if let Some(elements) = html.select("div.item, ul.rankList li") {
		for node in elements {
			let mut href = String::new();
			let mut title = String::new();
			let mut cover_src = String::new();

			if let Some(a) = node.select_first("a") {
				href = a.attr("href").unwrap_or_else(String::new);
			}

			if href.is_empty() {
				continue;
			}
			let key = match extract_key(&href) {
				Some(k) => k,
				None => continue,
			};

			if let Some(t) = node
				.select_first(".title")
				.or_else(|| node.select_first("p"))
			{
				title = t.text().unwrap_or_else(String::new);
			}

			if let Some(img) = node.select_first("img") {
				if title.is_empty() {
					title = img
						.attr("title")
						.unwrap_or_else(|| img.attr("alt").unwrap_or_else(String::new));
				}
				cover_src = img.attr("src").unwrap_or_else(String::new);
			}

			let cover = if !cover_src.is_empty() {
				Some(get_absolute_url(&cover_src))
			} else {
				None
			};

			manga.push(Manga {
				key,
				title,
				cover,
				url: Some(get_absolute_url(&href)),
				..Default::default()
			});
		}
	}

	let mut has_more = false;
	if let Some(elements) = html.select("a") {
		for a in elements {
			let text = a.text().unwrap_or_else(String::new);
			if text.contains("下一页") || text.contains("下页") {
				has_more = true;
				break;
			}
		}
	}

	Ok(MangaPageResult {
		entries: manga,
		has_next_page: has_more,
	})
}
