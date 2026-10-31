#![no_std]
use aidoku::{
	FilterValue, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, Listing,
	Manga, MangaPageResult, Result, Source,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};
use mangak::{Impl, Params};
use models::{ApiResponse, Item, ItemList};

mod models;

const BASE_URL: &str = "https://mangak.io";
const API_URL: &str = "https://api.mangak.io";

// Best Match sends no sort, as on the site
const SORTS: [Option<&str>; 10] = [
	None,
	Some("latest"),
	Some("newest"),
	Some("popular"),
	Some("rating"),
	Some("views_today"),
	Some("views_7days"),
	Some("views_30days"),
	Some("views"),
	Some("chapters"),
];

struct MangaK;

impl Impl for MangaK {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			api_url: API_URL.into(),
		}
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		if let Some(query) = query {
			qs.push("q", Some(&query));
		}
		for filter in filters {
			match filter {
				FilterValue::Sort { index, .. } => {
					if let Some(sort) = SORTS.get(index as usize).copied().flatten() {
						qs.push("sort", Some(sort));
					}
				}
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genres" => {
					for (name, slugs) in [("genres", included), ("exclude", excluded)] {
						if !slugs.is_empty() {
							qs.push(name, Some(&slugs.join(",")));
						}
					}
				}
				FilterValue::MultiSelect { id, included, .. } => {
					qs.push(&id, Some(&included.join(",")));
				}
				// The API answers an empty status or author with a 400
				FilterValue::Select { id, value } | FilterValue::Text { id, value }
					if !value.is_empty() =>
				{
					qs.push(&id, Some(&value));
				}
				FilterValue::Range { from, to, .. } => {
					for (name, bound) in [("min_ch", from), ("max_ch", to)] {
						if let Some(bound) = bound {
							qs.push(name, Some(&(bound as i32).to_string()));
						}
					}
				}
				_ => {}
			}
		}
		self.search(params, qs, page)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();
		match listing.id.as_str() {
			"trending" => {
				// The trending feed is one fixed list
				let response: ApiResponse<ItemList> =
					Request::get(format!("{API_URL}/trending/titles?window=week"))?.json_owned()?;
				return Ok(MangaPageResult {
					entries: response.data.items.into_iter().map(Into::into).collect(),
					has_next_page: false,
				});
			}
			"popular_updates" => {
				qs.push("sort", Some("bookmarks"));
				qs.push("window", Some("today"));
			}
			"new" => qs.push("sort", Some("newest")),
			"latest" => qs.push("sort", Some("latest")),
			_ => bail!("Unknown listing"),
		}
		self.search(params, qs, page)
	}

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		let sections = [
			(
				"Most Views",
				"titles/search?source=ranking&sort=views&limit=10",
				None,
			),
			(
				"Trending This Week",
				"trending/titles?window=week",
				Some("trending"),
			),
			(
				"Popular Updates",
				"titles/search?sort=bookmarks&window=today&limit=12",
				Some("popular_updates"),
			),
			(
				"New Manga",
				"titles/search?sort=newest&limit=18",
				Some("new"),
			),
			(
				"Recently Updated",
				"titles/search?sort=latest",
				Some("latest"),
			),
		];
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: sections
				.iter()
				.map(|(title, _, listing)| component(title, *listing, Vec::new()))
				.collect(),
		}));

		let requests = sections
			.iter()
			.map(|(_, path, _)| Ok(Request::get(format!("{API_URL}/{path}"))?))
			.collect::<Result<Vec<_>>>()?;
		// Returning the final layout drops the placeholder of a section that failed
		let components = sections
			.iter()
			.zip(Request::send_all(requests))
			.filter_map(|((title, _, listing), response)| {
				let response = response
					.ok()?
					.get_json_owned::<ApiResponse<ItemList>>()
					.ok()?;
				Some(component(title, *listing, response.data.items))
			})
			.collect::<Vec<_>>();
		if components.is_empty() {
			bail!("MangaK didn't answer");
		}
		Ok(HomeLayout { components })
	}
}

fn component(title: &str, listing: Option<&str>, items: Vec<Item>) -> HomeComponent {
	let link = listing.map(|id| Listing {
		id: id.into(),
		name: title.into(),
		..Default::default()
	});
	let value = match listing {
		None => HomeComponentValue::BigScroller {
			entries: items.into_iter().map(Item::into_featured).collect(),
			auto_scroll_interval: None,
		},
		// The site shows each title's newest chapter in these rows
		Some("popular_updates" | "latest") => HomeComponentValue::MangaChapterList {
			page_size: Some(5),
			entries: items
				.into_iter()
				.filter_map(Item::into_manga_with_chapter)
				.collect(),
			listing: link,
		},
		Some(_) => HomeComponentValue::Scroller {
			entries: items
				.into_iter()
				.map(|item| Manga::from(item).into())
				.collect::<Vec<Link>>(),
			listing: link,
		},
	};
	HomeComponent {
		title: Some(title.into()),
		subtitle: None,
		value,
	}
}

register_source!(
	mangak::MangaK<MangaK>,
	ListingProvider,
	Home,
	ImageRequestProvider,
	DeepLinkHandler
);
