use aidoku::{
	alloc::{vec, String, Vec},
	Home, HomeComponent, HomeLayout, Link, Listing, ListingKind, Manga, Result,
	Source,
};

use crate::Nhentai;

impl Home for Nhentai {
	fn get_home(&self) -> Result<HomeLayout> {
		let nhentai = Nhentai;
		let mut components = Vec::new();

		// Fetch popular today - use the same logic as search
		let popular_today = match nhentai.get_search_manga_list(
			None,
			1,
			vec![aidoku::FilterValue::Sort {
				id: String::from("sort"),
				index: 1, // popular-today
				ascending: false,
			}],
		) {
			Ok(result) => result.entries.into_iter().take(25).collect::<Vec<Manga>>(),
			Err(_) => Vec::new(),
		};

		if !popular_today.is_empty() {
			components.push(HomeComponent {
				title: Some("Popular Today".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: popular_today,
					auto_scroll_interval: Some(8.0),
				},
			});
		}

		// Fetch popular this week
		let popular_week = match nhentai.get_search_manga_list(
			None,
			1,
			vec![aidoku::FilterValue::Sort {
				id: String::from("sort"),
				index: 2, // popular-week
				ascending: false,
			}],
		) {
			Ok(result) => result
				.entries
				.into_iter()
				.take(25)
				.map(|manga| manga.into())
				.collect::<Vec<Link>>(),
			Err(_) => Vec::new(),
		};

		if !popular_week.is_empty() {
			components.push(HomeComponent {
				title: Some("Popular This Week".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: popular_week,
					listing: Some(Listing {
						id: "popular-week".into(),
						name: "Popular This Week".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		// Fetch popular all time
		let popular_all = match nhentai.get_search_manga_list(
			None,
			1,
			vec![aidoku::FilterValue::Sort {
				id: String::from("sort"),
				index: 3, // popular all time
				ascending: false,
			}],
		) {
			Ok(result) => result
				.entries
				.into_iter()
				.take(25)
				.map(|manga| manga.into())
				.collect::<Vec<Link>>(),
			Err(_) => Vec::new(),
		};

		if !popular_all.is_empty() {
			components.push(HomeComponent {
				title: Some("Popular All Time".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(3),
					entries: popular_all,
					listing: Some(Listing {
						id: "popular".into(),
						name: "Popular All Time".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		// Fetch recently added
		let recent = match nhentai.get_search_manga_list(
			None,
			1,
			vec![aidoku::FilterValue::Sort {
				id: String::from("sort"),
				index: 0, // recent
				ascending: false,
			}],
		) {
			Ok(result) => result
				.entries
				.into_iter()
				.take(25)
				.map(|manga| manga.into())
				.collect::<Vec<Link>>(),
			Err(_) => Vec::new(),
		};

		if !recent.is_empty() {
			components.push(HomeComponent {
				title: Some("Latest".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::Scroller {
					entries: recent,
					listing: Some(Listing {
						id: "latest".into(),
						name: "Latest".into(),
						kind: ListingKind::Default,
					}),
				},
			});
		}

		Ok(HomeLayout { components })
	}
}
