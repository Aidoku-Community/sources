use crate::{
	models::{NovelTagData, RankData},
	urls,
};
use aidoku::{Manga, MangaPageResult, Result, alloc::Vec};
use zaimanhua_template::{
	models::{ClassifyTag, ListItem},
	net, settings,
};

pub use zaimanhua_template::helpers::*;

pub fn tags() -> Result<Vec<ClassifyTag>> {
	Ok(net::fetch::<NovelTagData>(&urls::tags(), None)?.tag_list)
}

pub fn rank_list(rank_type: &str, page: i32) -> Result<MangaPageResult> {
	let url = urls::rank(rank_type, page);
	let entries: Vec<Manga> = net::fetch::<RankData>(&url, settings::get_token().as_deref())?
		.rank_list
		.unwrap_or_default()
		.into_iter()
		.map(ListItem::into_manga)
		.collect();
	Ok(MangaPageResult {
		has_next_page: !entries.is_empty(),
		entries,
	})
}
