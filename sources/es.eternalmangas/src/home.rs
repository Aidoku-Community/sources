use crate::{helpers, settings};
use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, Link, Manga, MangaWithChapter, Result,
	alloc::Vec, imports::html::Document, prelude::*,
};

pub fn parse_home(html: &Document) -> Result<HomeLayout> {
	let label_novels = settings::label_novels();
	let mut components = Vec::new();
	let featured: Vec<Manga> = html
		.select(".em-hero .carousel-item")
		.into_iter()
		.flatten()
		.filter_map(|card| {
			let mut manga = helpers::manga_from_card(&card, label_novels)?;
			manga.description = card
				.select_first(".em-hero-copy p.d-none")
				.and_then(|description| description.text())
				.map(|description| description.trim().into());
			let mut tags: Vec<_> = card
				.select(".em-hero-badges .em-badge")
				.into_iter()
				.flatten()
				.filter_map(|tag| tag.text())
				.map(|tag| tag.trim().into())
				.filter(|tag: &aidoku::alloc::String| !tag.is_empty())
				.collect();
			tags.sort_unstable();
			tags.dedup();
			manga.content_rating = helpers::content_rating(&tags, false);
			manga.tags = (!tags.is_empty()).then_some(tags);
			Some(manga)
		})
		.collect();
	if !featured.is_empty() {
		components.push(HomeComponent {
			value: HomeComponentValue::BigScroller {
				entries: featured,
				auto_scroll_interval: None,
			},
			..Default::default()
		});
	}
	for section in html.select("main section").into_iter().flatten() {
		let Some(title) = section
			.select_first("h2")
			.and_then(|heading| heading.text())
		else {
			continue;
		};
		let title = title.trim();
		let value = if title == "Últimas Actualizaciones" {
			let entries: Vec<MangaWithChapter> = section
				.select("a.em-update-card")
				.into_iter()
				.flatten()
				.filter_map(|card| {
					let manga = helpers::manga_from_card(&card, label_novels)?;
					let parent = card.parent()?;
					let link = parent.select_first("a.em-chapter-pill")?;
					let mut chapter = helpers::chapter(&link).ok()?;
					chapter.date_uploaded = link.parent().and_then(|parent| helpers::date(&parent));
					Some(MangaWithChapter { manga, chapter })
				})
				.collect();
			if entries.is_empty() {
				continue;
			}
			HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries,
				listing: None,
			}
		} else if matches!(title, "Popular Hoy!" | "Tendencias" | "Novelas") {
			let entries: Vec<Link> = section
				.select("a[href*='/series/']")
				.into_iter()
				.flatten()
				.filter_map(|card| helpers::manga_from_card(&card, label_novels))
				.map(|mut manga| {
					if title == "Novelas" {
						manga.title = helpers::display_title(&manga.title, true, label_novels);
					}
					Link::from(manga)
				})
				.collect();
			if entries.is_empty() {
				continue;
			}
			if title == "Tendencias" {
				HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries,
					listing: None,
				}
			} else {
				HomeComponentValue::Scroller {
					entries,
					listing: None,
				}
			}
		} else {
			continue;
		};
		components.push(HomeComponent {
			title: Some(title.into()),
			value,
			..Default::default()
		});
	}
	if components.is_empty() {
		bail!("Home sections not found");
	}
	Ok(HomeLayout { components })
}
