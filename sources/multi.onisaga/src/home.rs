use crate::{
	OniSaga,
	api::{SearchParams, parse, rpc},
	models::{BrowsePage, HomeData, Work},
	settings::adult,
};
use aidoku::{
	AidokuError, Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link,
	Listing, ListingKind, Manga, MangaWithChapter, Result,
	alloc::{Vec, vec},
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
};
use serde::de::DeserializeOwned;
use serde_json::json;

pub const SECTIONS: [(&str, Option<&str>); 6] = [
	("Spotlight", None),
	("Trending This Week", None),
	("Start a New Series", Some("Recently added to OniSaga")),
	("Latest Updates", None),
	("Finished Series", None),
	("Newly Published", Some("By original publication date")),
];

fn shelf(
	items: Vec<Work>,
	filter_by: Option<&SearchParams>,
	id: &str,
	name: &str,
) -> Option<HomeComponentValue> {
	let entries = items
		.into_iter()
		.filter(|work| {
			filter_by.is_none_or(|s| work.allowed(&s.ratings, s.origin.as_deref(), &s.languages))
		})
		.filter_map(|work| Manga::try_from(work).ok().map(Link::from))
		.collect::<Vec<_>>();
	(!entries.is_empty()).then(|| HomeComponentValue::Scroller {
		entries,
		listing: Some(Listing {
			id: id.into(),
			name: name.into(),
			kind: ListingKind::Default,
		}),
	})
}

fn component((title, subtitle): &(&str, Option<&str>), value: HomeComponentValue) -> HomeComponent {
	HomeComponent {
		title: Some((*title).into()),
		subtitle: subtitle.map(Into::into),
		value,
	}
}

pub fn components(
	search: &SearchParams,
	home: Option<HomeData>,
	new: Option<BrowsePage>,
	finished: Option<BrowsePage>,
	published: Option<BrowsePage>,
) -> Vec<HomeComponent> {
	let origins = search.origin.as_deref();
	let (spotlight, trending, home_new, latest, home_finished, home_published) = match home {
		Some(d) => (
			d.spotlight.value,
			Some(d.trending),
			d.new_on_site.value,
			d.latest.value,
			d.finished.value,
			d.newly_published.value,
		),
		None => (None, None, None, None, None, None),
	};
	let pick = |page: Option<BrowsePage>, fallback: Option<Vec<Work>>, id, name| match page {
		Some(p) => shelf(p.items, None, id, name),
		None => fallback.and_then(|items| shelf(items, Some(search), id, name)),
	};

	let spotlight_val = spotlight.and_then(|slides| {
		let entries = slides
			.into_iter()
			.filter_map(|s| s.card)
			.filter(|w| w.allowed(&search.ratings, origins, &search.languages))
			.filter_map(|w| w.into_details(&search.languages).ok())
			.collect::<Vec<_>>();
		(!entries.is_empty()).then_some(HomeComponentValue::BigScroller {
			entries,
			auto_scroll_interval: Some(8.0),
		})
	});

	let trending_val = trending.and_then(|items| {
		let entries = items
			.into_iter()
			.filter(|w| w.allowed(&search.ratings, origins, &search.languages))
			.filter_map(|w| Manga::try_from(w).ok().map(Link::from))
			.collect::<Vec<_>>();
		(!entries.is_empty()).then_some(HomeComponentValue::MangaList {
			ranking: true,
			page_size: Some(3),
			entries,
			listing: None,
		})
	});

	let latest_val = latest.and_then(|entries| {
		let entries = entries
			.into_iter()
			.filter_map(|entry| {
				let card = entry.card?;
				if !card.allowed(&search.ratings, origins, &search.languages) {
					return None;
				}
				let chapter = entry
					.units
					.into_iter()
					.find_map(|u| u.into_chapter(&search.languages))?;
				Some(MangaWithChapter {
					manga: Manga::try_from(card).ok()?,
					chapter,
				})
			})
			.collect::<Vec<_>>();
		(!entries.is_empty()).then(|| HomeComponentValue::MangaChapterList {
			page_size: Some(5),
			entries,
			listing: Some(Listing {
				id: "updated".into(),
				name: "Latest Update".into(),
				kind: ListingKind::Default,
			}),
		})
	});

	SECTIONS
		.iter()
		.zip([
			spotlight_val,
			trending_val,
			pick(new, home_new, "new", "New on Site"),
			latest_val,
			pick(finished, home_finished, "finished", "Finished Series"),
			pick(published, home_published, "published", "First Published"),
		])
		.filter_map(|(section, value)| Some(component(section, value?)))
		.collect()
}

impl Home for OniSaga {
	fn get_home(&self) -> Result<HomeLayout> {
		let search = SearchParams::new(None);
		if search.is_empty() {
			return Ok(HomeLayout::default());
		}
		self.sync_age(adult())?;

		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: SECTIONS
				.iter()
				.zip([
					HomeComponentValue::empty_big_scroller(),
					HomeComponentValue::empty_manga_list(),
					HomeComponentValue::empty_scroller(),
					HomeComponentValue::empty_manga_chapter_list(),
					HomeComponentValue::empty_scroller(),
					HomeComponentValue::empty_scroller(),
				])
				.map(|(section, value)| component(section, value))
				.collect(),
		}));

		let mut requests = vec![rpc("catalog/home", json!({}))?];
		if search.shelves_narrowed() {
			requests.extend([
				SearchParams {
					limit: 18,
					..SearchParams::sorted("new")
				}
				.request()?,
				SearchParams {
					limit: 18,
					status: vec!["completed".into()],
					..SearchParams::sorted("updated")
				}
				.request()?,
				SearchParams {
					limit: 18,
					..SearchParams::sorted("published")
				}
				.request()?,
			]);
		}

		let mut failure = None;
		let mut responses = Request::send_all(requests).into_iter();
		fn next_parsed<T: DeserializeOwned>(
			responses: &mut impl Iterator<Item = core::result::Result<Response, RequestError>>,
			failure: &mut Option<AidokuError>,
		) -> Option<T> {
			responses
				.next()?
				.map_err(Into::into)
				.and_then(parse)
				.map_err(|e| *failure = Some(e))
				.ok()
		}
		let home = next_parsed(&mut responses, &mut failure);
		let new = next_parsed(&mut responses, &mut failure);
		let finished = next_parsed(&mut responses, &mut failure);
		let published = next_parsed(&mut responses, &mut failure);

		let components = components(&search, home, new, finished, published);
		if components.is_empty() {
			return Err(failure.unwrap_or_else(|| error!("OniSaga's home page was empty")));
		}
		Ok(HomeLayout { components })
	}
}
