#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeLayout, ImageRequestProvider,
	Manga, MangaPageResult, Page, PageContext, Result, Source,
	alloc::{String, Vec, borrow::Cow},
	imports::net::Request,
};

pub mod helpers;
mod imp;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub manga_url_directory: Cow<'static, str>,
	pub date_format: Cow<'static, str>,
	pub date_locale: Cow<'static, str>,
	pub mark_all_nsfw: bool,
	pub series_title_selector: Cow<'static, str>,
	pub series_cover_selector: Cow<'static, str>,
	pub series_artist_selector: Cow<'static, str>,
	pub series_author_selector: Cow<'static, str>,
	pub series_description_selector: Cow<'static, str>,
	pub series_genre_selector: Cow<'static, str>,
	pub series_type_selector: Cow<'static, str>,
	pub series_status_selector: Cow<'static, str>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			manga_url_directory: "/manga".into(),
			date_format: "MMMM dd, yyyy".into(),
			date_locale: "en_US_POSIX".into(),
			mark_all_nsfw: false,
			series_title_selector: "h1.entry-title, .ts-breadcrumb li:last-child span".into(),
			series_cover_selector: ".infomanga > div[itemprop=image] img, .thumb img".into(),
			series_artist_selector: helpers::selector(
				".infotable tr:contains({}) td:last-child, .tsinfo .imptdt:contains({}) i, .fmed b:contains({})+span, span:contains({})",
				&[
					"artist",
					"Artist",
					"الرسام",
					"الناشر",
					"İllüstratör",
					"Çizer",
				],
			).into(),
			series_author_selector: helpers::selector(
				".infotable tr:contains({}) td:last-child, .tsinfo .imptdt:contains({}) i, .fmed b:contains({})+span, span:contains({})",
				&[
					"Author",
					"Auteur",
					"autor",
					"المؤلف",
					"Mangaka",
					"seniman",
					"Pengarang",
					"Yazar",
				],
			).into(),
			series_description_selector: ".desc, .entry-content[itemprop=description]".into(),
			series_genre_selector: (String::from("div.gnr a, .mgen a, .seriestugenre a, ")
				+ &helpers::selector("span:contains({})", &["genre", "التصنيف"])).into(),
			series_type_selector: helpers::selector(
				".infotable tr:contains({}) td:last-child, .tsinfo .imptdt:contains({}) i, \
				 .tsinfo .imptdt:contains({}) a, .fmed b:contains({})+span, span:contains({}) a",
				&["type", "ประเภท", "النوع", "tipe", "Türü"],
			).into(),
			series_status_selector: helpers::selector(
				".infotable tr:contains({}) td:last-child, .tsinfo .imptdt:contains({}) i, .fmed b:contains({})+span, span:contains({})",
				&[
					"status",
					"Statut",
					"Durum",
					"連載状況",
					"Estado",
					"الحالة",
					"حالة العمل",
					"สถานะ",
					"stato",
					"Statüsü",
				],
			).into(),
		}
	}
}

pub struct MangaThemesia<T: Impl> {
	inner: T,
	params: Params,
}

impl<T: Impl> Source for MangaThemesia<T> {
	fn new() -> Self {
		let inner = T::new();
		let params = inner.params();
		Self { inner, params }
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		self.inner
			.get_search_manga_list(&self.params, query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		self.inner
			.get_manga_update(&self.params, manga, needs_details, needs_chapters)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		self.inner.get_page_list(&self.params, manga, chapter)
	}
}

impl<T: Impl> ImageRequestProvider for MangaThemesia<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		self.inner.get_image_request(&self.params, url, context)
	}
}

impl<T: Impl> Home for MangaThemesia<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		self.inner.get_home(&self.params)
	}
}

impl<T: Impl> DeepLinkHandler for MangaThemesia<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		self.inner.handle_deep_link(&self.params, url)
	}
}
