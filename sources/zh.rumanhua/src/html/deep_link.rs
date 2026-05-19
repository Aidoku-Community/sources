use crate::net::{extract_chapter_key, extract_key, get_request};
use aidoku::{DeepLinkResult, Result, alloc::String};

pub fn handle_deep_link(url: String) -> Result<Option<DeepLinkResult>> {
	let clean = url.trim_end_matches('/');
	if clean.contains("/news/") {
		if let Some(key) = extract_key(clean) {
			return Ok(Some(DeepLinkResult::Manga { key }));
		}
	} else if clean.contains("/show/")
		&& let Some(chapter_key) = extract_chapter_key(clean)
	{
		let html = get_request(clean)?.html()?;
		if let Some(links) = html.select("a") {
			for a in links {
				let href = a.attr("href").unwrap_or_else(String::new);
				if href.contains("/news/")
					&& let Some(manga_key) = extract_key(&href)
				{
					return Ok(Some(DeepLinkResult::Chapter {
						manga_key,
						key: chapter_key,
					}));
				}
			}
		}
	}

	Ok(None)
}
