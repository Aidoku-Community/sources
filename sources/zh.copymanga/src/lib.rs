#![no_std]

mod html;
mod net;

use aidoku::{
	Chapter, DynamicFilters, Filter, FilterValue, Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	register_source,
};
use html::GenresPage as _;
use net::Url;

struct Copymanga;

impl Source for Copymanga {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		todo!()
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		todo!()
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		todo!()
	}
}

impl DynamicFilters for Copymanga {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let genre = Url::GenresPage.request()?.html()?.filter()?.into();
		Ok([genre].into())
	}
}

register_source!(Copymanga, DynamicFilters);
