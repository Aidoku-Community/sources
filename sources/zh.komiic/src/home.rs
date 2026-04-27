use super::*;

impl KomiicSource {
	fn links(entries: Vec<Manga>) -> Vec<Link> {
		entries.into_iter().map(Link::from).collect()
	}

	fn push_big_scroller(components: &mut Vec<HomeComponent>, title: &str, entries: Vec<Manga>) {
		if entries.is_empty() {
			return;
		}
		components.push(HomeComponent {
			title: Some(String::from(title)),
			subtitle: None,
			value: HomeComponentValue::BigScroller {
				entries,
				auto_scroll_interval: Some(8.0),
			},
		});
	}

	fn push_manga_list(
		components: &mut Vec<HomeComponent>,
		title: &str,
		entries: Vec<Manga>,
		listing: Listing,
	) {
		if entries.is_empty() {
			return;
		}
		components.push(HomeComponent {
			title: Some(String::from(title)),
			subtitle: None,
			value: HomeComponentValue::MangaList {
				ranking: true,
				page_size: Some(5),
				entries: Self::links(entries),
				listing: Some(listing),
			},
		});
	}

	fn push_scroller(
		components: &mut Vec<HomeComponent>,
		title: String,
		entries: Vec<Manga>,
		listing: Listing,
	) {
		if entries.is_empty() {
			return;
		}
		components.push(HomeComponent {
			title: Some(title),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: Self::links(entries),
				listing: Some(listing),
			},
		});
	}
}

impl HomeProvider for KomiicSource {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut components = Vec::new();

		let month_views = Self::get_home_comic_list("hotComics", "MONTH_VIEWS", 1)?;
		Self::push_big_scroller(&mut components, "本月最热", month_views.entries);

		let views = Self::get_home_comic_list("hotComics", "VIEWS", 1)?;
		Self::push_manga_list(
			&mut components,
			"总排行",
			views.entries,
			Self::listing("views", "总排行"),
		);

		let recent_update = Self::get_comic_list("recentUpdate", "DATE_UPDATED", 1)?;
		Self::push_scroller(
			&mut components,
			String::from("最近更新"),
			recent_update.entries,
			Self::listing("recent_update", "最近更新"),
		);

		Ok(HomeLayout { components })
	}
}
