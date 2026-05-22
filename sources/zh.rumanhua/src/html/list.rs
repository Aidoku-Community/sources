use crate::net::{extract_key, get_absolute_url};
use aidoku::{
	Manga, MangaPageResult, Result,
	imports::html::Document,
};

pub fn parse_manga_list(html: &Document) -> Result<MangaPageResult> {
	let manga = html.select("div.item, ul.rankList li")
		.map(|elements| {
			elements.filter_map(|node| {
				let href = node.select_first("a")?.attr("href")?;
				let key = extract_key(&href)?;
				let title = node
					.select_first(".title")
					.or_else(|| node.select_first("p"))
					.and_then(|t| t.text())
					.unwrap_or_default();
				let cover = node.select_first("img")
					.and_then(|img| img.attr("src"))
					.map(|src| get_absolute_url(&src));
				Some(Manga {
					key,
					title,
					cover,
					url: Some(get_absolute_url(&href)),
					..Default::default()
				})
			}).collect()
		})
		.unwrap_or_default();

	let has_more = html.select_first("a:contains(下一页), a:contains(下页)")
		.is_some();

	Ok(MangaPageResult {
		entries: manga,
		has_next_page: has_more,
	})
}
