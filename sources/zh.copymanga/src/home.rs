use crate::favorites;
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, Link, LinkValue, Listing, Manga, Result,
	alloc::{String, Vec},
};

use crate::Copymanga;

impl Home for Copymanga {
	fn get_home(&self) -> Result<HomeLayout> {
		Ok(HomeLayout {
			components: home_components(),
		})
	}
}

fn home_components() -> Vec<HomeComponent> {
	if crate::auth::is_logged_in() {
		let mut components = Vec::new();
		if let Ok(result) = favorites::collect_page(1)
			&& !result.entries.is_empty()
		{
			components.push(favorites_component(&result.entries, "我的收藏"));
		}
		if let Some(first) = components.first_mut() {
			first.subtitle = crate::auth::nickname();
		}
		components
	} else {
		Vec::new()
	}
}

fn favorites_component(entries: &[Manga], title: &str) -> HomeComponent {
	let listing = Listing {
		id: String::from("f:fav"),
		name: String::from(title),
		..Default::default()
	};
	let links: Vec<Link> = entries
		.iter()
		.map(|manga| Link {
			title: manga.title.clone(),
			subtitle: None,
			image_url: manga.cover.clone(),
			value: Some(LinkValue::Manga(manga.clone())),
		})
		.collect();
	HomeComponent {
		title: Some(String::from(title)),
		subtitle: None,
		value: HomeComponentValue::Scroller {
			entries: links,
			listing: Some(listing),
		},
	}
}
