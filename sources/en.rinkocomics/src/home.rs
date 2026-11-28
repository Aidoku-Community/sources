use crate::{
	RinkoComics,
	helpers::{BASE_URL, browse_page, card, chapter, checked_html, cover, key_of, text},
};
use aidoku::{
	Chapter, ContentRating, Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult,
	Link, Listing, ListingKind, Manga, MangaWithChapter, Result,
	alloc::{String, Vec, format, vec},
	imports::{html::Document, net::Request, std::send_partial_result},
	prelude::*,
};

fn component(title: &str, value: HomeComponentValue) -> HomeComponent {
	HomeComponent {
		title: Some(title.into()),
		subtitle: None,
		value,
	}
}

fn listing(id: &str, name: &str) -> Listing {
	Listing {
		id: id.into(),
		name: name.into(),
		kind: ListingKind::Default,
	}
}

fn editors_choice(home: &Document) -> Vec<Manga> {
	home.select("[data-cw-slider] [data-slide]")
		.map(|slides| {
			slides
				.filter_map(|slide| {
					let key = key_of(&slide.select_first("a.btn-primary")?.attr("abs:href")?)?;
					let mut tags = slide
						.select("span.chip")
						.map(|chips| {
							chips
								.filter_map(|chip| chip.text())
								.collect::<Vec<String>>()
						})
						.unwrap_or_default();
					tags.sort_unstable();
					Some(Manga {
						url: Some(format!("{BASE_URL}/{key}/")),
						title: text(&slide, "h2")?,
						key,
						cover: cover(&slide),
						description: text(&slide, "p"),
						tags: (!tags.is_empty()).then_some(tags),
						content_rating: ContentRating::Safe,
						..Default::default()
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

fn cards(doc: &Document, selector: &str) -> Vec<Link> {
	doc.select(selector)
		.map(|links| {
			links
				.filter_map(|link| card(&link).map(Into::into))
				.collect()
		})
		.unwrap_or_default()
}

// Each title's newest chapter it can open: paid rows on home carry no address.
fn latest(doc: &Document) -> Vec<MangaWithChapter> {
	doc.select("#cw-latest article")
		.map(|articles| {
			articles
				.filter_map(|article| {
					Some(MangaWithChapter {
						manga: Manga {
							cover: cover(&article),
							..card(&article.select_first("a.line-clamp-2")?)?
						},
						chapter: Chapter {
							url: None,
							..article
								.select("a.chapter-item")?
								.find_map(|row| chapter(&row))?
						},
					})
				})
				.collect()
		})
		.unwrap_or_default()
}

pub fn layout(
	home: Option<&Document>,
	new: Option<&Document>,
	novels: Option<&Document>,
) -> HomeLayout {
	let mut components = Vec::new();
	if let Some(home) = home {
		let entries = editors_choice(home);
		if !entries.is_empty() {
			components.push(component(
				"Editor's Choice",
				HomeComponentValue::BigScroller {
					entries,
					auto_scroll_interval: Some(6.0),
				},
			));
		}
		let entries = cards(home, "section[aria-labelledby='cw-week-m'] a");
		if !entries.is_empty() {
			components.push(component(
				"Featured This Week",
				HomeComponentValue::Scroller {
					entries,
					listing: Some(listing("views", "Featured This Week")),
				},
			));
		}
		let entries = latest(home);
		if !entries.is_empty() {
			components.push(component(
				"Latest Releases (Comics)",
				HomeComponentValue::MangaChapterList {
					page_size: Some(5),
					entries,
					listing: Some(listing("updated", "Latest Releases (Comics)")),
				},
			));
		}
	}
	if let Some(new) = new {
		let entries: Vec<Link> = browse_page(new)
			.entries
			.into_iter()
			.map(Into::into)
			.collect();
		if !entries.is_empty() {
			components.push(component(
				"New Comics",
				HomeComponentValue::Scroller {
					entries,
					listing: Some(listing("new", "New Comics")),
				},
			));
		}
	}
	if let Some(home) = home {
		let entries = cards(
			home,
			"section[aria-labelledby='cw-novels'] a[href*='/novel/']",
		);
		if !entries.is_empty() {
			components.push(component(
				"New Novels",
				HomeComponentValue::Scroller {
					entries,
					listing: Some(listing("novels", "New Novels")),
				},
			));
		}
	}
	if let Some(novels) = novels {
		let entries = latest(novels);
		if !entries.is_empty() {
			components.push(component(
				"Latest Releases (Novels)",
				HomeComponentValue::MangaChapterList {
					page_size: Some(5),
					entries,
					listing: Some(listing("novels-updated", "Latest Releases (Novels)")),
				},
			));
		}
	}
	HomeLayout { components }
}

impl Home for RinkoComics {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				component("Editor's Choice", HomeComponentValue::empty_big_scroller()),
				component("Featured This Week", HomeComponentValue::empty_scroller()),
				component(
					"Latest Releases (Comics)",
					HomeComponentValue::empty_manga_chapter_list(),
				),
				component("New Comics", HomeComponentValue::empty_scroller()),
				component("New Novels", HomeComponentValue::empty_scroller()),
				component(
					"Latest Releases (Novels)",
					HomeComponentValue::empty_manga_chapter_list(),
				),
			],
		}));

		let requests = ["?show=today", "comic/?sort=new", "?rel=novels"]
			.into_iter()
			.map(|path| Request::get(format!("{BASE_URL}/{path}")))
			.collect::<core::result::Result<Vec<_>, _>>()?;
		let mut failure = None;
		let pages: Vec<_> = Request::send_all(requests)
			.into_iter()
			.map(|response| {
				response
					.map_err(Into::into)
					.and_then(|response| checked_html(&response))
					.map_err(|error| failure = Some(error))
					.ok()
			})
			.collect();
		let mut pages = pages.into_iter();
		let (home, new, novels) = (
			pages.next().flatten(),
			pages.next().flatten(),
			pages.next().flatten(),
		);
		let layout = layout(home.as_ref(), new.as_ref(), novels.as_ref());
		if layout.components.is_empty() {
			return Err(failure.unwrap_or_else(|| error!("Rinko Comics' home page was empty")));
		}
		Ok(layout)
	}
}
