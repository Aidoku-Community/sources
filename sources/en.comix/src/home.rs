use aidoku::alloc::string::ToString;
use aidoku::helpers::uri::QueryParameters;
use aidoku::{
	Home, HomeComponent, HomeLayout, Manga, Result,
	alloc::{Vec, vec},
	imports::net::Request,
	prelude::*,
};

use crate::{
	API_URL,
	model::{ComixChapter, ComixManga, ComixResponse, ResultData},
};
use crate::{Comix, INCLUDES};

impl Home for Comix {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut qs = QueryParameters::new();
		for item in INCLUDES {
			qs.push("includes[]", Some(item));
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
