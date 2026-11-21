use crate::{
	VyManga,
	helpers::{manga_key, parse_html, parse_manga, parse_page, sorted_url},
	settings,
};
use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, Listing, Manga,
	Result,
	alloc::{Vec, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

pub const ROWS: [(&str, &str); 2] = [
	("updated_at", "Latest Update"),
	("created_at", "New Release"),
];

impl Home for VyManga {
	fn get_home(&self) -> Result<HomeLayout> {
		let component = |title: &str, value| HomeComponent {
			title: Some(title.into()),
			subtitle: None,
			value,
		};
		let mut components = vec![component(
			"Popular Manga",
			HomeComponentValue::empty_big_scroller(),
		)];
		components.extend(
			ROWS.iter()
				.map(|(_, title)| component(title, HomeComponentValue::empty_scroller())),
		);
		send_partial_result(&HomePartialResult::Layout(HomeLayout { components }));

		let mut requests = vec![Request::get(settings::base_url())?];
		for (sort, _) in ROWS {
			requests.push(Request::get(sorted_url(sort, 1))?);
		}
		let mut failure = None;
		let pages: Vec<_> = Request::send_all(requests)
			.into_iter()
			.map(|response| {
				response
					.map_err(Into::into)
					.and_then(|response| parse_html(&response))
					.map_err(|error| failure = Some(error))
					.ok()
			})
			.collect();
		let mut pages = pages.into_iter();

		// The final layout removes failed-section placeholders.
		let mut components = Vec::new();
		if let Some(html) = pages.next().flatten() {
			// A genre id is "<name>-<number>-<slug>"; some names carry line breaks a slide's text drops.
			let excluded = settings::excluded_genres();
			let hidden = |manga: &Manga| {
				manga.tags.iter().flatten().any(|tag| {
					excluded.iter().any(|id| {
						id.rsplitn(3, '-')
							.last()
							.is_some_and(|name| name.split_whitespace().eq(tag.split_whitespace()))
					})
				})
			};
			components.push(component(
				"Popular Manga",
				HomeComponentValue::BigScroller {
					entries: html
						.select(".slick-detail > a")
						.map(|slides| {
							slides
								.filter_map(|slide| {
									parse_manga(manga_key(&slide.attr("href")?)?, &slide).ok()
								})
								.filter(|manga| !hidden(manga))
								.collect()
						})
						.unwrap_or_default(),
					auto_scroll_interval: Some(5.0),
				},
			));
		}
		for (sort, title) in ROWS {
			if let Some(html) = pages.next().flatten() {
				components.push(component(
					title,
					HomeComponentValue::Scroller {
						entries: parse_page(&html)
							.entries
							.into_iter()
							.map(Link::from)
							.collect(),
						listing: Some(Listing {
							id: sort.into(),
							name: title.into(),
							..Default::default()
						}),
					},
				));
			}
		}
		if components.is_empty() {
			return Err(failure.unwrap_or_else(|| error!("No home sections")));
		}
		Ok(HomeLayout { components })
	}
}
