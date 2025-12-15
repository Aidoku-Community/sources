use aidoku::{
	Home, HomeComponent, HomeLayout, Manga, Result,
	alloc::{string::ToString, vec},
	imports::net::Request,
	prelude::*,
};

use crate::Comix;
use crate::{
	API_URL,
	model::{ComixChapter, ComixManga, ComixResponse},
};

impl Home for Comix {
	fn get_home(&self) -> Result<HomeLayout> {
		let url = format!("{API_URL}/manga?order[views_30d]=desc&limit=28");

		let mut manga_request = Request::get(&url)?.send()?;
		let manga_response = manga_request.get_json::<ComixResponse<ComixManga>>()?;
		println!("{:?}", manga_response);
		let manga_list = manga_response
			.result
			.items
			.into_iter()
			.map(|item| Manga {
				key: item.hash_id.to_string(),
				title: item.title,
				cover: Some(item.poster.medium.to_string()),
				..Default::default()
			})
			.collect();

		Ok(HomeLayout {
			components: vec![HomeComponent {
				title: Some("Hot Updates".into()),
				subtitle: None,
				value: aidoku::HomeComponentValue::BigScroller {
					entries: manga_list,
					auto_scroll_interval: None,
				},
			}],
		})
	}
}
