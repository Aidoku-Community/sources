#![no_std]

mod html;
mod json;
mod net;

use aidoku::{
	Chapter, DynamicFilters, Filter, FilterValue, Manga, MangaPageResult, Page, Result, Source,
	alloc::{String, Vec},
	imports::std::send_partial_result,
	register_source,
};
use html::{FiltersPage as _, GenresPage as _, MangaPage as _};
use json::{chapter_list, search};
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
		let url = Url::from_query_or_filters(query.as_deref(), page, &filters)?;
		let request = url.request()?;
		let manga_page_result = if url.is_filters() {
			request.html()?.manga_page_result()?
		} else {
			request.json_owned::<search::Root>()?.into()
		};
		Ok(manga_page_result)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_page = Url::manga(&manga.key).request()?.html()?;
		if needs_details {
			manga_page.update_details(&mut manga)?;

			if needs_chapters {
				send_partial_result(&manga);
			} else {
				return Ok(manga);
			}
		}

		let key = manga_page.key()?;
		manga.chapters = Url::chapter_list(&manga.key)
			.request()?
			.json_owned::<chapter_list::Root>()?
			.chapters(&key)?;

		Ok(manga)
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
