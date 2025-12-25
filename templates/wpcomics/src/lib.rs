#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter,
	FilterValue, Home, HomeLayout, ImageRequestProvider, ListingProvider, Manga, MangaPageResult,
	MangaStatus, Page, PageContext, Result, Source, Viewer,
	alloc::{String, Vec, borrow::Cow},
	imports::{html::Element, net::Request},
	prelude::*,
};
use core::cell::RefCell;
use helpers::{get_search_url, parse_chapter_date};

pub mod helpers;
mod imp;

pub use imp::Impl;

pub struct Params {
	pub base_url: Cow<'static, str>,
	pub cookie: Option<String>,
	pub custom_headers: Option<Vec<(&'static str, &'static str)>>,
	pub status_mapping: fn(String) -> MangaStatus,
	pub time_converter: fn(&Params, &str) -> i64,
	pub nsfw: ContentRating,
	pub viewer: Viewer,

	pub next_page: &'static str,
	pub manga_cell: &'static str,
	pub manga_cell_url: &'static str,
	pub manga_cell_title: &'static str,
	pub manga_cell_image: &'static str,
	pub manga_cell_image_attr: &'static str,
	pub manga_cell_no_data: fn(&Element) -> bool,
	pub manga_parse_id: fn(&str) -> String,

	pub manga_details_title: &'static str,
	pub manga_details_title_transformer: fn(String) -> String,
	pub manga_details_cover: &'static str,
	pub manga_details_cover_attr: &'static str,
	pub manga_details_cover_transformer: fn(String) -> String,
	pub manga_details_authors: &'static str,
	pub manga_details_authors_transformer: fn(Vec<String>) -> Vec<String>,
	pub manga_details_description: &'static str,
	pub manga_details_tags: &'static str,
	pub manga_details_tags_splitter: &'static str,
	pub manga_details_status: &'static str,
	pub manga_details_status_transformer: fn(String) -> String,

	pub manga_details_chapters: &'static str,
	pub chapter_skip_first: bool,
	pub chapter_date_selector: &'static str,
	pub chapter_anchor_selector: &'static str,
	pub chapter_parse_id: fn(String) -> String,

	pub manga_viewer_page: &'static str,
	pub manga_viewer_page_url_suffix: &'static str,
	pub page_url_transformer: fn(String) -> String,

	pub user_agent: Option<&'static str>,

	pub datetime_format: &'static str,
	pub datetime_locale: &'static str,
	pub datetime_timezone: &'static str,

	pub genre_endpoint: &'static str,

	pub search_page: fn(i32) -> String,
	pub manga_page: fn(&Params, &Manga) -> String,
	pub page_list_page: fn(&Params, &Manga, &Chapter) -> String,

	pub get_search_url: fn(
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<String>,

	pub home_manga_link: &'static str,
	pub home_chapter_link: &'static str,
	pub home_date_uploaded: &'static str,
	pub home_date_uploaded_attr: &'static str,

	pub home_sliders_selector: &'static str,
	pub home_sliders_title_selector: &'static str,
	pub home_sliders_item_selector: &'static str,

	pub home_grids_selector: &'static str,
	pub home_grids_title_selector: &'static str,
	pub home_grids_item_selector: &'static str,

	pub home_manga_cover_selector: &'static str,
	pub home_manga_cover_attr: &'static str,
	pub home_manga_cover_slider_attr: Option<&'static str>,
	pub home_manga_cover_slider_transformer: fn(String) -> String,
	pub time_formats: Option<Vec<&'static str>>,
}

impl Default for Params {
	fn default() -> Self {
		Self {
			base_url: "".into(),
			cookie: None,
			custom_headers: None,
			status_mapping: |status| match status.to_lowercase().as_str() {
				"ongoing"
				| "продолжается"
				| "updating"
				| "em lançamento"
				| "em andamento"
				| "en cours"
				| "en cours de publication"
				| "ativo"
				| "lançando"
				| "đang tiến hành"
				| "đang cập nhật"
				| "devam ediyor"
				| "in corso"
				| "in arrivo"
				| "مستمرة"
				| "مستمر"
				| "en curso"
				| "emision"
				| "curso"
				| "en marcha"
				| "publicandose"
				| "publicándose"
				| "en emision"
				| "连载中"
				| "devam ediyo"
				| "đang làm"
				| "em postagem"
				| "devam eden"
				| "em progresso"
				| "em curso"
				| "atualizações semanais" => MangaStatus::Ongoing,
				"completed" | "completo" | "completado" | "concluído" | "concluido"
				| "finalizado" | "achevé" | "terminé" | "hoàn thành" | "مكتملة" | "مكتمل"
				| "已完结" | "tamamlandı" | "đã hoàn thành" | "завершено" | "tamamlanan"
				| "complété" => MangaStatus::Completed,
				"hiatus"
				| "on hold"
				| "pausado"
				| "en espera"
				| "durduruldu"
				| "beklemede"
				| "đang chờ"
				| "متوقف"
				| "en pause"
				| "заморожено"
				| "en attente" => MangaStatus::Hiatus,
				"canceled" | "cancelado" | "i̇ptal edildi" | "güncel" | "đã hủy" | "ملغي"
				| "abandonné" | "заброшено" | "annulé" => MangaStatus::Cancelled,
				_ => MangaStatus::Unknown,
			},
			time_converter: |params, date| parse_chapter_date(params, date),
			nsfw: ContentRating::Safe,
			viewer: Viewer::LeftToRight,

			next_page: "li > a[rel=next]",
			manga_cell: "div.items > div.row > div.item > figure.clearfix",
			manga_cell_title: "figcaption > h3 > a",
			manga_cell_url: "figcaption > h3 > a",
			manga_cell_image: "div.image > a > img",
			manga_cell_image_attr: "abs:data-original",
			manga_cell_no_data: |_| false,
			manga_parse_id: |url| url.into(),

			manga_details_title: "h1.title-detail",
			manga_details_title_transformer: |title| title,
			manga_details_cover: "div.col-image > img",
			manga_details_cover_attr: "abs:src",
			manga_details_cover_transformer: |src| src,
			manga_details_authors: "ul.list-info > li.author > p.col-xs-8",
			manga_details_authors_transformer: |titles| titles,
			manga_details_description: "div.detail-content > p",
			manga_details_tags: "li.kind.row > p.col-xs-8",
			manga_details_tags_splitter: " - ",
			manga_details_status: "li.status.row > p.col-xs-8",
			manga_details_status_transformer: |title| title,
			manga_details_chapters: "div.list-chapter > nav > ul > li",

			chapter_skip_first: false,
			chapter_anchor_selector: "div.chapter > a",
			chapter_date_selector: "div.col-xs-4",
			chapter_parse_id: |url| url,

			manga_viewer_page: "div.page-chapter > img",
			manga_viewer_page_url_suffix: "",
			page_url_transformer: |url| url,

			user_agent: None,

			datetime_format: "MMMM dd, yyyy",
			datetime_locale: "en_US_POSIX",
			datetime_timezone: "current",

			genre_endpoint: "tim-kiem-nang-cao.html",

			search_page: |page| {
				if page == 1 {
					String::new()
				} else {
					format!("page/{page}/")
				}
			},
			manga_page: |params, manga| format!("{}/{}", params.base_url, manga.key),
			page_list_page: |params, manga, chapter| {
				format!("{}/{}/{}", params.base_url, manga.key, chapter.key)
			},

			get_search_url: |params, query, page, filters| {
				get_search_url(params, query, page, filters)
			},

			home_manga_link: ".book_info a",
			home_chapter_link: ".last_chapter a, .chapter-item a",
			home_date_uploaded: ".time-ago, .timediff a",
			home_date_uploaded_attr: "title",

			home_sliders_selector: ".homepage_suggest",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: "li",

			home_grids_selector: ".list_grid_out",
			home_grids_title_selector: "h1",
			home_grids_item_selector: "li",

			home_manga_cover_selector: "img",
			home_manga_cover_attr: "abs:src",
			home_manga_cover_slider_attr: None,
			home_manga_cover_slider_transformer: |src| src,
			time_formats: None,
		}
	}
}

#[derive(Default)]
pub struct Cache {
	manga_id: Option<String>,
	manga_value: Option<Vec<u8>>,
}

pub struct WpComics<T: Impl> {
	inner: T,
	params: Params,
	cache: RefCell<Cache>,
}

impl<T: Impl> Source for WpComics<T> {
	fn new() -> Self {
		let inner = T::new();
		let params = inner.params();
		Self {
			inner,
			params,
			cache: RefCell::new(Cache::default()),
		}
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut cache = self.cache.borrow_mut();
		self.inner
			.get_search_manga_list(&mut cache, &self.params, query, page, filters)
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut cache = self.cache.borrow_mut();
		self.inner.get_manga_update(
			&mut cache,
			&self.params,
			manga,
			needs_details,
			needs_chapters,
		)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let mut cache = self.cache.borrow_mut();
		self.inner
			.get_page_list(&mut cache, &self.params, manga, chapter)
	}
}

impl<T: Impl> ListingProvider for WpComics<T> {
	fn get_manga_list(&self, _listing: aidoku::Listing, _page: i32) -> Result<MangaPageResult> {
		Err(AidokuError::Unimplemented)
	}
}

impl<T: Impl> Home for WpComics<T> {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut cache = self.cache.borrow_mut();
		self.inner.get_home(&mut cache, &self.params)
	}
}

impl<T: Impl> DynamicFilters for WpComics<T> {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut cache = self.cache.borrow_mut();
		self.inner.get_dynamic_filters(&mut cache, &self.params)
	}
}

impl<T: Impl> ImageRequestProvider for WpComics<T> {
	fn get_image_request(&self, url: String, context: Option<PageContext>) -> Result<Request> {
		let mut cache = self.cache.borrow_mut();
		self.inner
			.get_image_request(&mut cache, &self.params, url, context)
	}
}

impl<T: Impl> DeepLinkHandler for WpComics<T> {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let mut cache = self.cache.borrow_mut();
		self.inner.handle_deep_link(&mut cache, &self.params, url)
	}
}
