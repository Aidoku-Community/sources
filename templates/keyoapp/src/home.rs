use crate::{
	Params,
	helpers::{chapter_date, chapter_from_link, chapter_parts, manga_from_card, manga_from_link},
};
use aidoku::{
	Chapter, HomeComponent, HomeComponentValue, HomeLayout, Link, Manga, MangaWithChapter,
	alloc::{String, Vec, string::ToString},
	helpers::string::StripPrefixOrSelf,
	imports::html::{Document, Element},
	prelude::*,
};
use base64::{Engine, engine::general_purpose::STANDARD};

pub fn parse_home(params: &Params, html: &Document) -> HomeLayout {
	let mut components = Vec::new();

	let featured: Vec<Manga> = html
		.select("a.splide__slide[href*='/series/']")
		.into_iter()
		.flatten()
		.filter(|slide| !slide.has_class("splide__slide--clone"))
		.filter_map(|slide| manga_from_link(&slide, &slide, &params.base_url))
		.collect();
	if !featured.is_empty() {
		// The app misreads bracketed aspect ratios inside attribute selectors.
		let value = if html
			.select("a.splide__slide > div")
			.is_some_and(|mut slides| {
				slides.any(|slide| {
					slide
						.attr("class")
						.is_some_and(|class| class.contains("aspect-[2/1]"))
				})
			}) {
			HomeComponentValue::ImageScroller {
				links: featured.into_iter().map(Link::from).collect(),
				auto_scroll_interval: Some(5.0),
				width: Some(400),
				height: Some(200),
			}
		} else {
			HomeComponentValue::Scroller {
				entries: featured.into_iter().map(Link::from).collect(),
				listing: None,
			}
		};
		components.push(HomeComponent {
			title: None,
			subtitle: None,
			value,
		});
	}

	for heading in html.select("h2").into_iter().flatten() {
		let Some(title) = heading.text() else {
			continue;
		};
		// Mobile layouts repeat rows; bulk discounts only sell paid chapters.
		if components
			.iter()
			.any(|component| component.title.as_ref() == Some(&title))
			|| title == "Bulk Discounted Series"
			|| params.hidden_rows.contains(&title.as_str())
		{
			continue;
		}
		let Some(section) = section(&heading) else {
			continue;
		};
		let value = if section.select_first(".editor-pick-button").is_some() {
			let entries: Vec<Manga> = section
				.select(".editor-pick-button")
				.map(|picks| picks.filter_map(|pick| editor_pick(&pick)).collect())
				.unwrap_or_default();
			if entries.is_empty() {
				continue;
			}
			HomeComponentValue::BigScroller {
				entries,
				auto_scroll_interval: None,
			}
		} else if title == "Latest Updates" {
			let entries: Vec<MangaWithChapter> = section
				.select(".latest-poster")
				.into_iter()
				.flatten()
				.filter_map(|poster| newest_chapter(&poster, &params.base_url))
				.collect();
			if entries.is_empty() {
				continue;
			}
			HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries,
				listing: None,
			}
		} else {
			// Pinned Series shares the latest row's posters
			let entries: Vec<Link> = section
				.select("button[title], .latest-poster")
				.into_iter()
				.flatten()
				.filter(|card| {
					card.parent()
						.is_none_or(|parent| !parent.has_class("splide__slide--clone"))
				})
				.filter_map(|card| {
					manga_from_link(
						&card.select_first("a[href*='/series/']")?,
						&card,
						&params.base_url,
					)
				})
				.map(Link::from)
				.collect();
			if entries.is_empty() {
				continue;
			}
			HomeComponentValue::Scroller {
				entries,
				listing: None,
			}
		};
		components.push(HomeComponent {
			title: Some(title),
			subtitle: None,
			value,
		});
	}

	HomeLayout { components }
}

pub fn parse_home_v2(params: &Params, html: &Document) -> HomeLayout {
	let mut components: Vec<HomeComponent> = Vec::new();
	for section in html.select("section").into_iter().flatten() {
		let Some(title) = section
			.select_first("h2")
			.and_then(|heading| heading.text())
			.map(|title| title.trim().to_string())
		else {
			continue;
		};
		// Editor's Choice repeats for the mobile layout
		if components
			.iter()
			.any(|component| component.title.as_ref() == Some(&title))
			|| params.hidden_rows.contains(&title.as_str())
		{
			continue;
		}
		// Some layouts put the cover grid immediately after its heading section.
		let section = if section.id().as_deref() == Some("home-newly-added")
			&& section.select_first("a.group[href]").is_none()
		{
			let Some(grid) = html.select_first("#home-newly-added + #series-grid") else {
				continue;
			};
			grid
		} else {
			section
		};
		let value = if section.select_first("article").is_some() {
			let entries: Vec<MangaWithChapter> = section
				.select("article")
				.into_iter()
				.flatten()
				.filter_map(|card| recent_chapter(&card, &params.base_url))
				.collect();
			if entries.is_empty() {
				continue;
			}
			HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries,
				listing: None,
			}
		} else {
			let entries: Vec<Link> = section
				.select("a[data-slide-card], a[data-ec-card], a.group[href]")
				.into_iter()
				.flatten()
				.filter(|card| card.attr("data-ec-clone").is_none())
				.filter_map(|card| manga_from_card(&card, &params.base_url))
				.map(Link::from)
				.collect();
			if entries.is_empty() && title != "Most Reviewed Series" {
				continue;
			}
			HomeComponentValue::Scroller {
				entries,
				listing: None,
			}
		};
		components.push(HomeComponent {
			title: Some(title),
			subtitle: None,
			value,
		});
	}
	HomeLayout { components }
}

// Stop before another row so empty sections cannot absorb the whole page.
fn section(heading: &Element) -> Option<Element> {
	let title = heading.text();
	let mut node = heading.parent();
	for _ in 0..3 {
		let element = node?;
		if element
			.select("h2")
			.is_some_and(|mut headings| headings.any(|other| other.text() != title))
		{
			return None;
		}
		if element
			.select_first(".latest-poster, .editor-pick-button, button[title]")
			.is_some()
		{
			return Some(element);
		}
		node = element.parent();
	}
	None
}

// These cards use a scripted data-id link and a base64 description.
fn editor_pick(pick: &Element) -> Option<Manga> {
	let description = pick
		.attr("data-description")
		.and_then(|encoded| STANDARD.decode(encoded).ok())
		.and_then(|bytes| String::from_utf8(bytes).ok())
		.map(|text| text.trim().into())
		.filter(|text: &String| !text.is_empty());
	Some(Manga {
		key: format!("/series/{}/", pick.attr("data-id")?),
		title: pick
			.attr("title")
			.filter(|title| !title.trim().is_empty())?
			.trim()
			.into(),
		cover: pick
			.attr("data-img")
			.map(|image| format!("https://wsrv.nl/?w=600&url=cdn.meowing.org/uploads/{image}")),
		description,
		..Default::default()
	})
}

fn newest_chapter(poster: &Element, base_url: &str) -> Option<MangaWithChapter> {
	let chapter = poster
		.select("a[href*='/chapter/']")?
		.find_map(|link| chapter_from_link(&link, base_url))?;
	Some(MangaWithChapter {
		manga: manga_from_link(
			&poster.select_first("a[href*='/series/']")?,
			poster,
			base_url,
		)?,
		chapter,
	})
}

fn recent_chapter(card: &Element, base_url: &str) -> Option<MangaWithChapter> {
	let series = card.select_first("h2 a")?;
	let link = card.select("a")?.find(|link| {
		link.select_first("time").is_some() && link.select_first("img[alt$=coins]").is_none()
	})?;
	let key: String = link.attr("href")?.strip_prefix_or_self(base_url).into();
	let (chapter_number, title) = chapter_parts(&link.select_first("span")?.text()?);
	Some(MangaWithChapter {
		manga: Manga {
			key: series.attr("href")?.strip_prefix_or_self(base_url).into(),
			title: series.text()?.trim().into(),
			cover: card
				.select_first("img")
				.and_then(|image| image.attr("data-src")),
			..Default::default()
		},
		chapter: Chapter {
			url: Some(format!("{base_url}{key}")),
			key,
			title,
			chapter_number,
			date_uploaded: link
				.select_first("time")
				.and_then(|time| time.text())
				.and_then(|date| chapter_date(&date)),
			..Default::default()
		},
	})
}
