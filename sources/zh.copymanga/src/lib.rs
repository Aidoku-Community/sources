#![no_std]

use aidoku::{
	Chapter, FilterValue, Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	register_source,
};

struct Copymanga;

impl Source for Copymanga {
	fn new() -> Self {
		todo!()
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

register_source!(Copymanga);
