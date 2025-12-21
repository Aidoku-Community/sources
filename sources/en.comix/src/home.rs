use aidoku::helpers::uri::QueryParameters;
use aidoku::{AidokuError, Listing, ListingProvider, MangaPageResult};
use aidoku::{
	Home, HomeComponent, HomeLayout, Manga, Result,
	alloc::{Vec, vec},
	imports::net::Request,
	prelude::*,
};

use crate::{
	API_URL,
	model::{ComixManga, ComixResponse, ResultData},
};
use crate::{Comix, INCLUDES, NSFW_GENRE_IDS, settings};

impl Home for Comix {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut qs = QueryParameters::new();
		for item in INCLUDES {
			qs.push("includes[]", Some(item));
		}

		if settings::get_nsfw() {
			for item in NSFW_GENRE_IDS {
				qs.push("includes[]", Some(&format!("-{item}")));
			}
		}
		let url = format!("{API_URL}/manga?order[views_30d]=desc&limit=50&{qs}");
		let mut manga_request = Request::get(&url)?.send()?;
		let manga_response = manga_request.get_json::<ComixResponse<ResultData<ComixManga>>>()?;
		let manga_list: Vec<Manga> = manga_response
			.result
			.items
			.into_iter()
			.map(Into::into)
			.collect();

		let first_ten_entries = manga_list.iter().take(10).cloned().collect();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some("Popular Releases".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: first_ten_entries,
					auto_scroll_interval: Some(5.0),
				},
			}],
		})
	}
}

impl ListingProvider for Comix {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"latest" => {
				let mut qs = QueryParameters::new();
				for item in INCLUDES {
					qs.push("includes[]", Some(item));
				}

				if settings::get_nsfw() {
					for item in NSFW_GENRE_IDS {
						qs.push("includes[]", Some(&format!("-{item}")));
					}
				}
				let url = format!(
					"{API_URL}/manga?order[chapter_updated_at]=desc&limit=50&{qs}&page={page}"
				);
				let (entries, has_next_page) = Request::get(url)?
					.send()?
					.get_json::<ComixResponse<ResultData<ComixManga>>>()
					.map(|res| {
						(
							res.result
								.items
								.into_iter()
								.map(Into::into)
								.collect::<Vec<Manga>>(),
							res.result.pagination.current_page < res.result.pagination.last_page,
						)
					})?;
				Ok(MangaPageResult {
					entries,
					has_next_page,
				})
			}
			_ => return Err(AidokuError::Message(("Invalid listing id".into()))),
		}
	}
}
