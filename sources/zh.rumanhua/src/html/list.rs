use crate::net::{extract_key, get_absolute_url};
use aidoku::{
	Manga, MangaPageResult, Result,
	alloc::Vec,
	imports::html::Document,
};

pub fn parse_manga_list(html: &Document) -> Result<MangaPageResult> {
	let mut manga = Vec::new();
	if let Some(elements) = html.select("div.item, ul.rankList li") {
		for node in elements {
			let href = node.select_first("a").and_then(|a| a.attr("href"));
			let Some(href) = href else {
				continue;
			};

			let key = match extract_key(&href) {
				Some(k) => k,
				None => continue,
			};

			let title = node
				.select_first(".title")
				.or_else(|| node.select_first("p"))
				.and_then(|t| t.text())
				.unwrap_or_default();

			let cover = node.select_first("img")
				.and_then(|img| img.attr("src"))
				.map(|src| get_absolute_url(&src));

			manga.push(Manga {
				key,
				title,
				cover,
				url: Some(get_absolute_url(&href)),
				..Default::default()
			});
		}
	}

	let has_more = html.select("a")
		.map(|mut elements| {
			elements.any(|a| {
				a.text()
					.map(|text| text.contains("下一页") || text.contains("下页"))
					.unwrap_or(false)
			})
		})
		.unwrap_or(false);

	Ok(MangaPageResult {
		entries: manga,
		has_next_page: has_more,
	})
}
