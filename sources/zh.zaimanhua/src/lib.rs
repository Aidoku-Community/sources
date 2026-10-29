#![no_std]
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, HomeLayout, Listing, Manga, MangaPageResult,
	Page, PageContent, Result, Source,
	alloc::{String, Vec, format, string::ToString},
	helpers::uri::QueryParameters,
	imports::defaults::defaults_get,
	prelude::*,
};
use core::cell::RefCell;
use helpers::{SiteLink, is_id};
use models::{ChapterData, ClassifyData, DetailData};
use zaimanhua_template::{
	Impl, Kind, Params, ZaimanhuaTemplate,
	models::{ApiResponse, tag_filter},
	net,
};

const WEB_URL: &str = "https://manhua.zaimanhua.com";
const CHAPTER_COMMENTS_KEY: &str = "chapterComments";

const AUDIENCES: [(&str, &str, &str); 4] = [
	("shounen", "少年漫画", "cate=3262"),
	("shoujo", "少女漫画", "cate=3263"),
	("seinen", "男青漫画", "cate=3264"),
	("josei", "女青漫画", "cate=13626"),
];

mod chapter;
mod comments;
mod helpers;
mod home;
mod models;
mod urls;

#[derive(Default)]
struct Zaimanhua {
	author_tag: RefCell<Option<(String, Option<i64>)>>,
	tag_id: RefCell<Option<(String, i64)>>,
}

impl Impl for Zaimanhua {
	fn new() -> Self {
		Self::default()
	}

	fn params(&self) -> Params {
		Params { kind: Kind::Comic }
	}

	fn get_search_manga_list(
		&self,
		params: &Params,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query {
			return self.search(params, &query, page);
		}

		let mut qs = QueryParameters::new();
		let (mut rank, mut theme) = (None, None);
		for filter in filters {
			let (id, value) = match filter {
				FilterValue::Text { id, value } if id == "author" => {
					return helpers::search_author(&self.author_tag, &value, page);
				}
				FilterValue::Select { id, value } => (id, value),
				_ => continue,
			};
			match id.as_str() {
				"genre" => {
					let tag_id = helpers::tag_id(&self.tag_id, &value, helpers::theme_tags)?;
					theme = Some(tag_id.to_string());
				}
				"theme" => theme = Some(value),
				"rank" => rank = Some(value),
				"sortType" | "cate" | "status" | "zone" => qs.push(&id, Some(&value)),
				_ => {}
			}
		}
		if let Some(by_time) = rank {
			return helpers::rank_list(&by_time, page);
		}
		if let Some(theme) = theme {
			qs.push("theme", Some(&theme));
		}
		helpers::list(params.kind, &qs.to_string(), page)
	}

	fn get_manga_update(
		&self,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let mut detail = helpers::with_access(|token| {
			net::request(&urls::detail(params.kind, &manga.key), token)?
				.json_owned::<ApiResponse<DetailData>>()
		})?
		.data
		.ok_or_else(|| error!("漫画不存在"))?;
		if needs_chapters {
			let is_member =
				detail.is_fee == Some(1) && net::user_info().is_some_and(|info| info.is_member);
			manga.chapters = Some(detail.take_chapters(&manga.key, is_member));
		}
		if needs_details {
			if let Some(author) = detail.first_author() {
				self.author_tag.replace(Some(author));
			}
			manga.copy_from(detail.into_manga(&manga.key));
		}
		Ok(manga)
	}

	fn get_page_list(
		&self,
		_params: &Params,
		_manga: Manga,
		chapter: Chapter,
	) -> Result<Vec<Page>> {
		let Some(chapter_pages) = helpers::with_access(|token| {
			net::request(&urls::chapter(&chapter.key), token)?
				.json_owned::<ApiResponse<ChapterData>>()
		})?
		.data
		else {
			return Ok(Vec::new());
		};
		let mut pages: Vec<Page> = chapter_pages
			.page_url_hd
			.filter(|urls| !urls.is_empty())
			.or(chapter_pages.page_url)
			.unwrap_or_default()
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect();
		let has_comments = chapter_pages.comment_count > 0 && !pages.is_empty();
		if has_comments && defaults_get(CHAPTER_COMMENTS_KEY).unwrap_or(false) {
			pages.extend(comments::page(&chapter.key));
		}
		Ok(pages)
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let query = match listing.id.as_str() {
			"subscribe" => return helpers::subscriptions(params.kind, page),
			"rank-monthly" => return helpers::rank_list("2", page),
			"latest" => "sortType=1",
			"ongoing" => "status=2309",
			"completed" => "status=2310",
			"short" => "status=29205",
			id => AUDIENCES
				.iter()
				.find_map(|(key, _, query)| (*key == id).then_some(*query))
				.ok_or_else(|| error!("Invalid listing"))?,
		};
		helpers::list(params.kind, query, page)
	}

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		home::home()
	}

	fn get_dynamic_filters(&self, _params: &Params) -> Result<Vec<Filter>> {
		Ok(net::fetch::<ClassifyData>(&urls::classify(), None)?
			.classify_list
			.into_iter()
			.filter_map(|group| {
				let (id, title, is_genre) = match group.id {
					1 => ("theme", "题材", true),
					6 => ("cate", "受众", false),
					5 => ("status", "状态", false),
					4 => ("zone", "地区", false),
					_ => return None,
				};
				Some(tag_filter(id, title, is_genre, group.list))
			})
			.collect())
	}

	fn handle_deep_link(&self, _params: &Params, url: &str) -> Result<Option<DeepLinkResult>> {
		let Some(link) = SiteLink::parse(url) else {
			return Ok(None);
		};
		let manga = |key: &str| DeepLinkResult::Manga { key: key.into() };
		let chapter = |manga_key: &str, chapter_id: &str| DeepLinkResult::Chapter {
			manga_key: manga_key.into(),
			key: format!("{manga_key}/{chapter_id}"),
		};
		Ok(match link.segments.as_slice() {
			["details", id] if is_id(id) => Some(manga(id)),
			["view", _, id, chapter_id] if is_id(id) && is_id(chapter_id) => {
				Some(chapter(id, chapter_id))
			}
			["pages", "comic", "detail"] => link.param("id").map(manga),
			["pages", "comic", "page"] => link
				.param("comic_id")
				.zip(link.param("chapter_id"))
				.map(|(id, chapter_id)| chapter(id, chapter_id)),
			["info", file] => match file.strip_suffix(".html") {
				Some(pinyin) => net::fetch::<DetailData>(&urls::detail_by_pinyin(pinyin), None)?
					.data
					.map(|detail| DeepLinkResult::Manga {
						key: detail.id.to_string(),
					}),
				None => None,
			},
			_ => None,
		})
	}
}

register_source!(
	ZaimanhuaTemplate<Zaimanhua>,
	Home,
	ListingProvider,
	DeepLinkHandler,
	BasicLoginHandler,
	NotificationHandler,
	DynamicSettings,
	DynamicFilters
);
