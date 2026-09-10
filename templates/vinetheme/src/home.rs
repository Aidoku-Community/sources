use aidoku::{
	Chapter, Manga, MangaWithChapter,
	alloc::{string::String, vec::Vec},
	imports::html::Element,
	prelude::*,
};

pub fn home_section(html: &Element, marker: &str) -> Option<Element> {
	let selector = format!("a[href*='ref={marker}']");
	html.select("section")?
		.find(|section| section.select_first(&selector).is_some())
}

pub fn section_title(section: &Element) -> Option<String> {
	section.select_first("h2").and_then(|title| title.text())
}

pub fn home_entries_in(element: &Element, marker: &str, base_url: &str) -> Vec<Manga> {
	element
		.select(format!("a[href*='ref={marker}']"))
		.map(|links| {
			links
				.filter_map(|link| manga_from_link(&link, base_url))
				.collect()
		})
		.unwrap_or_default()
}

pub fn latest_entries(section: &Element, base_url: &str) -> Vec<MangaWithChapter> {
	section
		.select("a[href*='ref=homepage_latest']")
		.map(|links| {
			links
				.filter_map(|link| {
					let manga = manga_from_link(&link, base_url)?;
					let chapter = link.parent()?.select("a[href*='/chapter/']").and_then(
						|mut chapters| chapters.find_map(|link| chapter_from_link(&link)),
					)?;
					Some(MangaWithChapter { manga, chapter })
				})
				.collect()
		})
		.unwrap_or_default()
}

fn manga_from_link(link: &Element, base_url: &str) -> Option<Manga> {
	let key = series_key(&link.attr("href")?)?;
	let title = link
		.select_first("h1, h2, h3")
		.and_then(|title| title.text())
		.or_else(|| link.select_first("img").and_then(|image| image.attr("alt")))?;
	let cover = link
		.select_first("img")
		.or_else(|| link.parent()?.parent()?.parent()?.select_first("img"))
		.and_then(|image| image.attr("abs:src"));
	let url = format!("{base_url}/series/comic/{key}");
	Some(Manga {
		key,
		title,
		cover,
		url: Some(url),
		..Default::default()
	})
}

fn chapter_from_link(link: &Element) -> Option<Chapter> {
	let key = chapter_key(&link.attr("href")?)?;
	let title = link.text()?;
	Some(Chapter {
		key,
		chapter_number: first_number(&title),
		..Default::default()
	})
}

fn series_key(href: &str) -> Option<String> {
	href.split_once("/series/comic/")?
		.1
		.split(['?', '#', '/'])
		.next()
		.filter(|key| !key.is_empty())
		.map(Into::into)
}

fn chapter_key(href: &str) -> Option<String> {
	href.split_once("/chapter/")?
		.1
		.split(['?', '#', '/'])
		.next()
		.filter(|key| !key.is_empty())
		.map(Into::into)
}

fn first_number(text: &str) -> Option<f32> {
	let digits: String = text
		.chars()
		.skip_while(|character| !character.is_ascii_digit())
		.take_while(|character| character.is_ascii_digit() || *character == '.')
		.collect();
	digits.parse().ok()
}
