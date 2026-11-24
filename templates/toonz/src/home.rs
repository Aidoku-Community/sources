use crate::{
	Params, Row,
	helpers::{
		Content, age, card, cards, check_response, cover_url, parse_series_path, rating, status,
		tags,
	},
	models::FeaturedSeries,
	rsc::featured_series,
};
use aidoku::{
	AidokuError, Chapter, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link,
	Listing, ListingKind, Manga, MangaWithChapter, Result,
	alloc::{String, Vec, collections::BTreeSet, format},
	imports::{
		html::{Document, Element},
		net::Request,
		std::{current_date, send_partial_result},
	},
	prelude::*,
};

fn section(page: &Document, heading: &str) -> Option<Element> {
	page.select("section")?.find(|section| {
		section
			.select_first("h2")
			.and_then(|title| title.text())
			.is_some_and(|title| title == heading)
	})
}

fn featured(params: &Params, article: &Element, series: &[FeaturedSeries]) -> Option<(f32, Manga)> {
	let link = article.select_first("h2 a")?;
	let href = link.attr("href")?;
	let (key, _) = parse_series_path(params, &href)?;
	let title = link.text().filter(|title| !title.is_empty())?;
	let image = article.select_first("img[data-cover]");
	let series = series.iter().find(|series| series.slug == key);
	let nsfw = image
		.as_ref()
		.is_some_and(|image| image.has_attr("data-nsfw"))
		|| matches!(
			series.and_then(|series| series.content_rating.as_deref()),
			Some("erotica" | "pornographic")
		);
	let tags = tags(article.select("a[href^='/genre/']"));
	let score = article
		.select("dl > div")
		.into_iter()
		.flatten()
		.find(|row| {
			row.select_first("dt")
				.and_then(|label| label.text())
				.as_deref() == Some("Rating")
		})
		.and_then(|row| row.select_first("dd")?.text())
		.and_then(|text| text.split(' ').next()?.parse().ok())
		.unwrap_or_default();
	Some((
		score,
		Manga {
			key: key.into(),
			title,
			cover: image
				.and_then(|image| image.attr("abs:src"))
				.or_else(|| cover_url(params, series)),
			description: article
				.select_first("h2 + p")
				.and_then(|text| text.text())
				.filter(|text| !text.is_empty()),
			content_rating: rating(nsfw, &tags),
			tags: Some(tags).filter(|tags| !tags.is_empty()),
			..Default::default()
		},
	))
}

fn slide(
	params: &Params,
	heading: &Element,
	series: &[FeaturedSeries],
	cards: &[Manga],
) -> Option<Manga> {
	let link = heading.parent()?;
	let href = link.attr("href")?;
	let (key, _) = parse_series_path(params, &href)?;
	let info = link.parent()?;
	let cover = info
		.parent()?
		.select_first("picture img")
		.and_then(|cover| cover.attr("abs:src"))
		.or_else(|| cover_url(params, series.iter().find(|series| series.slug == key)))
		.or_else(|| cards.iter().find(|card| card.key == key)?.cover.clone());
	Some(Manga {
		key: key.into(),
		title: heading.text().filter(|title| !title.is_empty())?,
		cover,
		description: info
			.select_first("p")
			.and_then(|text| text.untrimmed_text())
			.and_then(|text| {
				// Some slide texts start with the alternate titles, whose non-Latin names the
				// site strips to empty " /  / " entries, and end with a cut "Original Webtoon"
				// credit label; the synopsis is what lies between.
				let synopsis = match text.split_once(" /  / ") {
					Some((_, synopsis)) => synopsis
						.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '/' | '!')),
					None => text.as_str(),
				};
				let synopsis = synopsis
					.find(".Original Webtoon")
					.map_or(synopsis, |end| &synopsis[..end + 1])
					.trim();
				(!synopsis.is_empty()).then(|| synopsis.into())
			}),
		status: info
			.select("[data-slot=badge]")
			.into_iter()
			.flatten()
			.filter_map(|badge| badge.text())
			.find_map(|text| status(&text))
			.unwrap_or_default(),
		..Default::default()
	})
}

fn chapter(
	manga: Manga,
	key: String,
	number: Option<f32>,
	date_uploaded: Option<i64>,
) -> MangaWithChapter {
	MangaWithChapter {
		chapter: Chapter {
			key,
			chapter_number: number,
			date_uploaded,
			..Default::default()
		},
		manga,
	}
}

fn update(params: &Params, article: &Element, now: i64) -> Option<MangaWithChapter> {
	let (_, manga) = card(params, &article.select_first("img")?)?;
	let (key, number) = article
		.select("span")?
		.filter_map(|span| span.text())
		.find_map(|text| {
			let key = text.strip_prefix("CH.")?.trim();
			key.parse::<f32>()
				.ok()
				.map(|number| (String::from(key), number))
		})?;
	let date_uploaded = article
		.select("p")
		.and_then(|mut paragraphs| paragraphs.next_back()?.text())
		.as_deref()
		.and_then(age)
		.and_then(|seconds| now.checked_sub(seconds));
	Some(chapter(manga, key, Some(number), date_uploaded))
}

fn aged_cards(params: &Params, scope: &Element) -> Vec<(i64, Manga)> {
	scope
		.select("img[data-cover]")
		.into_iter()
		.flatten()
		.filter_map(|image| {
			let (link, manga) = card(params, &image)?;
			let seconds = link
				.select("span")
				.into_iter()
				.flatten()
				.filter_map(|span| span.text())
				.find_map(|text| age(&text))
				.unwrap_or(i64::MAX);
			Some((seconds, manga))
		})
		.collect()
}

fn scroller(entries: impl Iterator<Item = Manga>) -> Option<HomeComponentValue> {
	let mut keys = BTreeSet::new();
	let entries: Vec<Link> = entries
		.filter(|manga| keys.insert(manga.key.clone()))
		.map(Link::from)
		.collect();
	(!entries.is_empty()).then_some(HomeComponentValue::Scroller {
		entries,
		listing: None,
	})
}

fn chapter_list(entries: impl Iterator<Item = MangaWithChapter>) -> Option<HomeComponentValue> {
	let mut seen = BTreeSet::new();
	let entries: Vec<_> = entries
		.filter(|entry| seen.insert(entry.manga.key.clone()))
		.collect();
	(!entries.is_empty()).then(|| HomeComponentValue::MangaChapterList {
		page_size: Some(5),
		entries,
		listing: Some(Listing {
			id: "latest".into(),
			name: "Latest Updates".into(),
			kind: ListingKind::Default,
		}),
	})
}

pub fn fill(params: &Params, row: Row, pages: &[Option<Document>]) -> Option<HomeComponentValue> {
	let loaded = |indices: &'static [usize]| {
		indices
			.iter()
			.filter_map(|&index| pages.get(index)?.as_ref())
	};
	match row {
		Row::Featured(indices) => {
			let mut entries: Vec<_> = loaded(indices)
				.flat_map(|page| {
					let series = featured_series(page, r#""slides":"#);
					page.select("section[aria-label=Featured] article")
						.into_iter()
						.flatten()
						.filter_map(move |article| featured(params, &article, &series))
				})
				.collect();
			// Toonz splits its top-rated carousel across the manhwa and manga pages, so the
			// merged row is ordered by the rating each slide shows. Stable, so ties keep the page's order.
			entries.sort_by(|(a, _), (b, _)| b.total_cmp(a));
			let mut seen = BTreeSet::new();
			let entries: Vec<_> = entries
				.into_iter()
				.map(|(_, manga)| manga)
				.filter(|manga| seen.insert(manga.key.clone()))
				.collect();
			(!entries.is_empty()).then_some(HomeComponentValue::BigScroller {
				entries,
				auto_scroll_interval: Some(5.0),
			})
		}
		Row::Hero(page) => {
			let page = pages.get(page)?.as_ref()?;
			let series = featured_series(page, r#""comics":"#);
			let cards = cards(params, page.select("img[data-cover]"));
			let entries: Vec<_> = page
				.select("section div[aria-hidden] a > h2")?
				.filter_map(|heading| slide(params, &heading, &series, &cards))
				.collect();
			(!entries.is_empty()).then_some(HomeComponentValue::BigScroller {
				entries,
				auto_scroll_interval: Some(5.0),
			})
		}
		Row::Newest(indices, heading) => {
			let mut entries: Vec<_> = loaded(indices)
				.filter_map(|page| section(page, heading))
				.flat_map(|section| aged_cards(params, &section))
				.collect();
			// Stable, so cards of the same age keep the site's order.
			entries.sort_by_key(|(seconds, _)| *seconds);
			scroller(entries.into_iter().map(|(_, manga)| manga))
		}
		Row::Tab(page, heading) => {
			let panel = section(pages.get(page)?.as_ref()?, heading)?
				.select_first("div[role=tabpanel]:not([hidden])")?;
			scroller(
				panel
					.select("img[data-cover]")
					.into_iter()
					.flatten()
					.filter_map(|image| card(params, &image).map(|(_, manga)| manga)),
			)
		}
		Row::Chapters(page, heading) => {
			let section = section(pages.get(page)?.as_ref()?, heading)?;
			let mut series = cards(params, section.select("img[data-cover]"));
			chapter_list(section.select("a[href*='/chapter/']")?.filter_map(|link| {
				let href = link.attr("href")?;
				let (key, chapter_key) = parse_series_path(params, &href)?;
				let chapter_key = chapter_key?;
				let index = series.iter().position(|manga| manga.key == key)?;
				let number = link
					.text()
					.as_deref()
					.and_then(|text| text.strip_prefix("Ch.")?.trim().parse().ok());
				Some(chapter(
					series.swap_remove(index),
					chapter_key.into(),
					number,
					None,
				))
			}))
		}
		Row::Updates(page) => {
			let now = current_date();
			chapter_list(
				pages
					.get(page)?
					.as_ref()?
					.select("article")?
					.filter_map(|article| update(params, &article, now)),
			)
		}
	}
}

pub fn get_home(params: &Params) -> Result<HomeLayout> {
	let content = Content::new(params);
	if content.types.as_slice() == ["none"] {
		return Ok(HomeLayout::default());
	}
	send_partial_result(&HomePartialResult::Layout(HomeLayout {
		components: params
			.home_rows
			.iter()
			.map(|(title, row)| HomeComponent {
				title: Some(String::from(*title)),
				subtitle: None,
				value: match row {
					Row::Featured(_) | Row::Hero(_) => HomeComponentValue::empty_big_scroller(),
					Row::Chapters(..) | Row::Updates(_) => {
						HomeComponentValue::empty_manga_chapter_list()
					}
					_ => HomeComponentValue::empty_scroller(),
				},
			})
			.collect(),
	}));

	let requests = params
		.home_pages
		.iter()
		.map(|path| content.request(&format!("{}{path}", params.base_url)))
		.collect::<Result<Vec<_>>>()?;
	let mut failure = None;
	let pages: Vec<Option<Document>> = Request::send_all(requests)
		.into_iter()
		.map(|response| {
			response
				.map_err(AidokuError::from)
				.and_then(check_response)
				.and_then(|response| Ok(response.get_html()?))
				.map_err(|error| failure = Some(error))
				.ok()
		})
		.collect();
	let components: Vec<_> = params
		.home_rows
		.iter()
		.filter_map(|(title, row)| {
			Some(HomeComponent {
				title: Some(String::from(*title)),
				subtitle: None,
				value: fill(params, *row, &pages)?,
			})
		})
		.collect();
	if components.is_empty() {
		return Err(failure.unwrap_or_else(|| error!("No home sections")));
	}
	Ok(HomeLayout { components })
}
