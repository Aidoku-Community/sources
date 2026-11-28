use crate::{
	TempleScan,
	helpers::{
		BASE_URL, fetch_doc, image_url, parse_chapter_anchor, parse_comic_href, rating_of,
		strip_html,
	},
	rsc::{Flight, slug_like},
	settings,
};
use aidoku::{
	ContentRating, Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, ListingKind,
	Manga, MangaWithChapter, Result,
	alloc::{String, Vec, collections::BTreeSet, format},
	imports::html::{Document, Element},
	prelude::*,
};
use serde_json::{Map, Value};

const HOME_KNOWN: [&str; 11] = [
	"id",
	"description",
	"thumbnail",
	"badge",
	"protagonist",
	"banner",
	"total_views",
	"day_views",
	"week_views",
	"month_views",
	"slug",
];

fn extract_home_entry(flight: &Flight, record: &Map<String, Value>) -> Option<(String, String)> {
	let strings: Vec<&str> = record
		.iter()
		.filter(|(k, _)| !HOME_KNOWN.contains(&k.as_str()))
		.filter_map(|(_, v)| {
			v.as_str()
				.and_then(|s| flight.resolve_str(s))
				.map(str::trim)
				.filter(|s| !s.is_empty())
		})
		.collect();
	let slug = record
		.get("series_slug")
		.and_then(Value::as_str)
		.and_then(|s| flight.resolve_str(s))
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.or_else(|| strings.iter().copied().find(|s| slug_like(s)))?;
	let title = record
		.get("title")
		.and_then(Value::as_str)
		.and_then(|s| flight.resolve_str(s))
		.map(str::trim)
		.filter(|s| !s.is_empty())
		.or_else(|| {
			strings
				.iter()
				.copied()
				.find(|s| *s != slug && !s.starts_with("http://") && !s.starts_with("https://"))
		})?;
	Some((slug.into(), title.into()))
}

fn parse_home_featured(flight: &Flight) -> Vec<Manga> {
	let mut seen = BTreeSet::new();
	for array in flight.all_arrays() {
		let records: Vec<&Map<String, Value>> = array.iter().filter_map(Value::as_object).collect();
		if !records
			.first()
			.is_some_and(|r| r.contains_key("protagonist") || r.contains_key("banner"))
		{
			continue;
		}
		let entries: Vec<Manga> = records
			.into_iter()
			.filter_map(|record| {
				let (slug, title) = extract_home_entry(flight, record)?;
				if !seen.insert(slug.clone()) {
					return None;
				}
				Some(Manga {
					url: Some(format!("{BASE_URL}/comic/{slug}")),
					key: slug,
					title,
					cover: image_url(
						record
							.get("protagonist")
							.or_else(|| record.get("banner"))
							.or_else(|| record.get("thumbnail"))
							.and_then(Value::as_str)
							.and_then(|s| flight.resolve_str(s)),
					),
					description: record
						.get("description")
						.and_then(Value::as_str)
						.and_then(|s| flight.resolve_str(s))
						.map(strip_html)
						.filter(|d| !d.is_empty()),
					content_rating: record
						.get("badge")
						.and_then(Value::as_str)
						.and_then(|s| flight.resolve_str(s))
						.map(rating_of)
						.unwrap_or(ContentRating::Safe),
					..Default::default()
				})
			})
			.collect();
		if !entries.is_empty() {
			return entries;
		}
	}
	Vec::new()
}

fn parse_card_anchor(anchor: &Element, title_override: Option<String>) -> Option<Manga> {
	let (slug, chapter) = parse_comic_href(&anchor.attr("href")?)?;
	if chapter.is_some() {
		return None;
	}
	let title = title_override
		.or_else(|| {
			anchor
				.select_first("h3, span[class*='line-clamp']")
				.and_then(|el| el.text())
		})
		.filter(|t| !t.is_empty())?;
	let cover = anchor
		.select_first("img")
		.and_then(|img| img.attr("src").or_else(|| img.attr("data-src")))
		.and_then(|s| image_url(Some(&s)));
	let content_rating = if anchor
		.select("span")
		.into_iter()
		.flatten()
		.filter_map(|span| span.text())
		.any(|label| rating_of(&label) == ContentRating::NSFW)
	{
		ContentRating::NSFW
	} else {
		ContentRating::Safe
	};
	Some(Manga {
		url: Some(format!("{BASE_URL}/comic/{slug}")),
		key: slug,
		title,
		cover,
		content_rating,
		..Default::default()
	})
}

fn parse_home_trending_week(doc: &Document, flight: &Flight) -> Vec<Manga> {
	let mut seen = BTreeSet::new();
	if let Some(items) = flight
		.find(&["weekRes"])
		.and_then(|props| props.get("weekRes"))
		.and_then(Value::as_array)
	{
		let entries: Vec<Manga> = items
			.iter()
			.filter_map(Value::as_object)
			.filter_map(|record| {
				let (slug, title) = extract_home_entry(flight, record)?;
				if !seen.insert(slug.clone()) {
					return None;
				}
				Some(Manga {
					url: Some(format!("{BASE_URL}/comic/{slug}")),
					key: slug,
					title,
					cover: image_url(
						record
							.get("thumbnail")
							.and_then(Value::as_str)
							.and_then(|s| flight.resolve_str(s)),
					),
					content_rating: record
						.get("badge")
						.and_then(Value::as_str)
						.and_then(|s| flight.resolve_str(s))
						.map(rating_of)
						.unwrap_or(ContentRating::Safe),
					..Default::default()
				})
			})
			.collect();
		if !entries.is_empty() {
			return entries;
		}
	}
	doc.select("section[aria-labelledby='trending-title'] ol > li > a[href]")
		.into_iter()
		.flatten()
		.filter_map(|anchor| {
			let manga = parse_card_anchor(&anchor, None)?;
			seen.insert(manga.key.clone()).then_some(manga)
		})
		.collect()
}

fn parse_home_latest(doc: &Document, show_locked: bool) -> Vec<MangaWithChapter> {
	let Some(section) = doc.select("section").into_iter().flatten().find(|s| {
		s.select_first("#latest-updates-title").is_some()
			|| s.select_first("h2")
				.and_then(|h| h.text())
				.is_some_and(|t| t.eq_ignore_ascii_case("latest updates"))
	}) else {
		return Vec::new();
	};
	let mut seen = BTreeSet::new();
	section
		.select("article")
		.into_iter()
		.flatten()
		.filter_map(|article| {
			if article.attr("data-sponsored").is_some() {
				return None;
			}
			let cover_link = article.select_first("a[href]")?;
			let title = article
				.select_first("h3 a[href]")
				.and_then(|el| el.text())
				.filter(|t| !t.is_empty());
			let manga = parse_card_anchor(&cover_link, title)?;
			let mut fallback = None;
			let mut picked = None;
			for anchor in article.select("ul a[href]").into_iter().flatten() {
				let Some(href) = anchor.attr("href") else {
					continue;
				};
				if let Some((series, Some(ch_slug))) = parse_comic_href(&href)
					&& series == manga.key
				{
					let ch = parse_chapter_anchor(&anchor, &series, ch_slug, None, false);
					if !ch.locked {
						picked = Some(ch);
						break;
					} else if fallback.is_none() {
						fallback = Some(ch);
					}
				}
			}
			let chapter = picked.or_else(|| show_locked.then_some(fallback).flatten())?;
			seen.insert(manga.key.clone())
				.then_some(MangaWithChapter { manga, chapter })
		})
		.collect()
}

fn parse_home_new_series(doc: &Document) -> Vec<Manga> {
	let Some(section) = doc.select("section").into_iter().flatten().find(|s| {
		s.select_first("h2")
			.and_then(|h| h.text())
			.is_some_and(|t| t.eq_ignore_ascii_case("new series"))
	}) else {
		return Vec::new();
	};
	let mut seen = BTreeSet::new();
	section
		.select("a[href]")
		.into_iter()
		.flatten()
		.filter_map(|anchor| {
			anchor.select_first("img")?;
			let manga = parse_card_anchor(&anchor, None)?;
			seen.insert(manga.key.clone()).then_some(manga)
		})
		.collect()
}

fn listing(id: &str, name: &str) -> Listing {
	Listing {
		id: id.into(),
		name: name.into(),
		kind: ListingKind::Default,
	}
}

pub fn layout(doc: &Document, show_locked: bool) -> HomeLayout {
	let flight = Flight::from_document(doc);
	let mut components = Vec::new();

	let featured = parse_home_featured(&flight);
	if !featured.is_empty() {
		components.push(HomeComponent {
			title: Some("Featured".into()),
			subtitle: None,
			value: HomeComponentValue::BigScroller {
				entries: featured,
				auto_scroll_interval: Some(6.0),
			},
		});
	}

	let trending: Vec<Link> = parse_home_trending_week(doc, &flight)
		.into_iter()
		.map(Into::into)
		.collect();
	if !trending.is_empty() {
		components.push(HomeComponent {
			title: Some("Trending This Week".into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: trending,
				listing: Some(listing("views", "Trending")),
			},
		});
	}

	let latest = parse_home_latest(doc, show_locked);
	if !latest.is_empty() {
		components.push(HomeComponent {
			title: Some("Latest Updates".into()),
			subtitle: None,
			value: HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries: latest,
				listing: Some(listing("updated", "Latest Updates")),
			},
		});
	}

	let new_series: Vec<Link> = parse_home_new_series(doc)
		.into_iter()
		.map(Into::into)
		.collect();
	if !new_series.is_empty() {
		components.push(HomeComponent {
			title: Some("New Series".into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: new_series,
				listing: Some(listing("created", "New Series")),
			},
		});
	}

	HomeLayout { components }
}

impl Home for TempleScan {
	fn get_home(&self) -> Result<HomeLayout> {
		let layout = layout(
			&fetch_doc(&format!("{BASE_URL}/"))?,
			settings::show_locked_chapters(),
		);
		if layout.components.is_empty() {
			bail!("Temple Scan's home page had no sections");
		}
		Ok(layout)
	}
}
