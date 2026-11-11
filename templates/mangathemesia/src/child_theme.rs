use crate::{
	Params,
	helpers::{ElementImageAttr, find_first_f32},
};
use aidoku::{
	Chapter, DeepLinkResult, HomeComponent, HomeComponentValue, HomeLayout, Link, Manga,
	MangaStatus, MangaWithChapter, Result,
	alloc::{String, Vec},
	helpers::string::StripPrefixOrSelf,
	imports::{
		html::{Document, Element},
		net::Request,
	},
	prelude::*,
};

pub fn get_home(params: &Params) -> Result<HomeLayout> {
	let html = Request::get(&params.base_url)?.html()?;
	Ok(parse_home(params, &html))
}

pub fn handle_deep_link(params: &Params, url: String) -> Result<Option<DeepLinkResult>> {
	let html = Request::get(&url)?.html()?;
	Ok(parse_deep_link(params, &html, &url))
}

pub fn parse_deep_link(params: &Params, html: &Document, url: &str) -> Option<DeepLinkResult> {
	let canonical = html
		.select_first("link[rel=canonical]")
		.and_then(|link| link.attr("href"));
	let path = canonical
		.as_deref()
		.unwrap_or(url)
		.strip_prefix_or_self(&params.base_url);
	let path = path.split_once(['?', '#']).map_or(path, |(path, _)| path);
	let key = format!("{}/", path.trim_end_matches('/'));
	if html.select_first("#readerarea").is_some() {
		let directory = format!("{}/", params.manga_url_directory);
		let manga_key = html.select(".ts-breadcrumb a")?.find_map(|link| {
			let href = link.attr("href")?;
			let path = href.strip_prefix_or_self(&params.base_url);
			let path = path.split_once(['?', '#']).map_or(path, |(path, _)| path);
			path.strip_prefix(&directory)
				.filter(|slug| !slug.trim_matches('/').is_empty())?;
			Some(format!("{}/", path.trim_end_matches('/')))
		})?;
		return Some(DeepLinkResult::Chapter { manga_key, key });
	}
	html.select_first("div.bigcontent, div.animefull, div.main-info, div.postbody")?
		.select_first(&params.series_title_selector)?
		.text()
		.filter(|title| !title.trim().is_empty())?;
	Some(DeepLinkResult::Manga { key })
}

pub fn parse_home(params: &Params, html: &Document) -> HomeLayout {
	let base_url = &params.base_url;
	let mut components: Vec<HomeComponent> = slider_banners(html, params).into_iter().collect();

	// `.bixbox` rather than `div.bixbox`: Editor Picks is a <section>.
	for section in html.select(".bixbox").into_iter().flatten() {
		let Some(title) = section
			.select_first(".releases h2")
			.and_then(|h2| h2.text())
		else {
			continue;
		};
		let picks = editor_picks(&section, base_url);
		if !picks.is_empty() {
			components.push(HomeComponent {
				title: Some(title),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: picks,
					auto_scroll_interval: None,
				},
			});
			continue;
		}
		// Popular Today's cards sit beside its heading, inside `.hotslid`.
		let rail = section
			.parent()
			.filter(|parent| parent.has_class("hotslid"))
			.unwrap_or(section);
		let cards = rail.select(".bsx").into_iter().flatten();
		// Latest Novels stays cards because its newest chapters are mostly locked.
		let value = if rail
			.select_first("#manga-posts .chapter-list > a")
			.is_some()
		{
			let entries: Vec<MangaWithChapter> = cards
				.filter_map(|card| newest_chapter(&card, base_url))
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
			let entries: Vec<Link> = cards
				.filter_map(|card| series(&card, base_url))
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

fn slider_banners(html: &Document, params: &Params) -> Option<HomeComponent> {
	let links: Vec<Link> = html
		.select(".slidernew .swiper-slide a")?
		.filter_map(|link| {
			let image = link.select_first("img")?;
			Some(Manga {
				key: link_key(&link, &params.base_url)?,
				title: image
					.attr("alt")
					.filter(|title| !title.trim().is_empty())?
					.trim()
					.into(),
				cover: image.img_attr(),
				..Default::default()
			})
		})
		.map(Link::from)
		.collect();
	if links.is_empty() {
		return None;
	}
	Some(HomeComponent {
		title: None,
		subtitle: None,
		value: HomeComponentValue::ImageScroller {
			links,
			auto_scroll_interval: Some(5.0),
			width: params.banner_width,
			height: params.banner_height,
		},
	})
}

fn series(card: &Element, base_url: &str) -> Option<Manga> {
	let link = card.select_first("a")?;
	Some(Manga {
		key: link_key(&link, base_url)?,
		title: link
			.attr("title")
			.filter(|title| !title.trim().is_empty())?
			.trim()
			.into(),
		cover: card.select_first("img").and_then(|image| image.img_attr()),
		..Default::default()
	})
}

// Skip locked chapters and the "all chapters" link, which has no `.epxs`.
fn newest_chapter(card: &Element, base_url: &str) -> Option<MangaWithChapter> {
	let (link, title) = card
		.select(".chapter-list > a")?
		.filter(|link| link.select_first(".fa-coins").is_none())
		.find_map(|link| {
			let title = link.select_first(".epxs")?.text()?;
			Some((link, title))
		})?;
	let chapter_number = find_first_f32(&title);
	Some(MangaWithChapter {
		manga: series(card, base_url)?,
		chapter: Chapter {
			key: link_key(&link, base_url)?,
			title: chapter_number.is_none().then_some(title),
			chapter_number,
			..Default::default()
		},
	})
}

fn editor_picks(section: &Element, base_url: &str) -> Vec<Manga> {
	section
		.select(".violet-ep-panel")
		.into_iter()
		.flatten()
		.filter_map(|panel| {
			let link = panel.select_first(".violet-ep-series-title a")?;
			let index = panel.attr("data-index")?;
			Some(Manga {
				key: link_key(&link, base_url)?,
				title: link
					.text()
					.filter(|title| !title.trim().is_empty())?
					.trim()
					.into(),
				cover: section
					.select_first(format!(".violet-ep-cover[data-index=\"{index}\"] img"))
					.and_then(|image| image.img_attr()),
				description: panel
					.select_first(".violet-ep-tagline")
					.and_then(|tagline| tagline.text())
					.map(|text| text.trim().into())
					.filter(|text: &String| !text.is_empty()),
				status: match panel
					.select_first(".violet-ep-badge--status")
					.and_then(|badge| badge.text())
					.map(|status| status.to_lowercase())
					.as_deref()
				{
					Some("ongoing") => MangaStatus::Ongoing,
					Some("completed") => MangaStatus::Completed,
					Some("hiatus") => MangaStatus::Hiatus,
					_ => MangaStatus::Unknown,
				},
				tags: panel
					.select(".violet-ep-genres li")
					.map(|genres| genres.filter_map(|genre| genre.text()).collect()),
				..Default::default()
			})
		})
		.collect()
}

fn link_key(link: &Element, base_url: &str) -> Option<String> {
	let href = link.attr("href")?;
	let key = href.trim().strip_prefix_or_self(base_url);
	(!key.is_empty()).then(|| key.into())
}
