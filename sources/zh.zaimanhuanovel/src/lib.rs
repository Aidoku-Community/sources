#![no_std]
use aidoku::{
	Chapter, DeepLinkResult, Filter, FilterValue, HomeLayout, Listing, Manga, MangaPageResult,
	Page, PageContent, Result, SelectFilter, Source,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::uri::QueryParameters,
	imports::net::Request,
	prelude::*,
};
use core::cell::RefCell;
use helpers::SiteLink;
use markdown::Block;
use models::{NovelDetailData, NovelVolumesData};
use zaimanhua_template::{Impl, Kind, Params, ZaimanhuaTemplate, models::tag_filter, net};

const M_URL: &str = "https://m.zaimanhua.com";
const IMAGE_URL: &str = "https://images.zaimanhua.com";

const POPULARITY: &str = "0";

mod helpers;
mod home;
mod markdown;
mod models;
mod urls;

#[derive(Default)]
struct ZaimanhuaNovel {
	tag_id: RefCell<Option<(String, i64)>>,
}

impl Impl for ZaimanhuaNovel {
	fn new() -> Self {
		Self::default()
	}

	fn params(&self) -> Params {
		Params { kind: Kind::Novel }
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
		let mut tag = None;
		for filter in filters {
			let (id, value) = match filter {
				// novels have no author list, but search matches author names
				FilterValue::Text { id, value } if id == "author" => {
					let url = urls::search(params.kind, &value, page);
					return Ok(helpers::fetch_list(&url)?.into_page_result(page));
				}
				FilterValue::Text { id, value } if id == "keyword" => {
					return self.search(params, &value, page);
				}
				FilterValue::Select { id, value } => (id, value),
				_ => continue,
			};
			match id.as_str() {
				"genre" => {
					let tag_id = helpers::tag_id(&self.tag_id, &value, helpers::tags)?;
					tag = Some(tag_id.to_string());
				}
				"tagId" => tag = Some(value),
				"sortType" | "status" => qs.push(&id, Some(&value)),
				_ => {}
			}
		}
		if let Some(tag) = tag {
			qs.push("tagId", Some(&tag));
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
		let id = manga.key.as_str();
		let mut requests = vec![net::request(&urls::detail(params.kind, id), None)?];
		if needs_chapters {
			requests.push(net::request(&urls::chapters(id), None)?);
		}
		let mut responses = Request::send_all(requests).into_iter();
		let detail = net::parse::<NovelDetailData>(responses.next())?.into_detail()?;
		if needs_chapters {
			let volumes = net::parse::<NovelVolumesData>(responses.next())?
				.data
				.unwrap_or_default();
			manga.chapters = Some(detail.chapters(id, volumes));
		}
		if needs_details {
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
		let (volume_id, chapter_id) = chapter
			.key
			.split_once('/')
			.ok_or_else(|| error!("章节不存在"))?;
		let text = Request::get(urls::text(volume_id, chapter_id))?.string()?;
		let blocks = markdown::blocks(&text);
		let page = |content| Page {
			content,
			..Default::default()
		};
		if blocks.iter().all(|block| matches!(block, Block::Image(_))) {
			return Ok(blocks
				.into_iter()
				.filter_map(|block| match block {
					Block::Image(src) => Some(page(PageContent::url(src))),
					Block::Text(_) => None,
				})
				.collect());
		}
		let content = PageContent::text(markdown::from_blocks(&blocks));
		Ok(vec![page(content)])
	}

	fn get_manga_list(
		&self,
		params: &Params,
		listing: Listing,
		page: i32,
	) -> Result<MangaPageResult> {
		let query = match listing.id.as_str() {
			"subscribe" => return helpers::subscriptions(params.kind, page),
			"rank" => return helpers::rank_list(POPULARITY, page),
			"latest" => "sortType=1",
			"ongoing" => "status=1",
			"completed" => "status=2",
			_ => bail!("Invalid listing"),
		};
		helpers::list(params.kind, query, page)
	}

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		home::home()
	}

	fn get_dynamic_filters(&self, _params: &Params) -> Result<Vec<Filter>> {
		Ok(vec![
			tag_filter("tagId", "题材", true, helpers::tags()?),
			SelectFilter {
				id: "status".into(),
				title: Some("状态".into()),
				uses_tag_style: true,
				options: ["全部", "连载中", "已完结"].map(Into::into).into(),
				ids: Some(["0", "1", "2"].map(Into::into).into()),
				..Default::default()
			}
			.into(),
		])
	}

	fn handle_deep_link(&self, _params: &Params, url: &str) -> Result<Option<DeepLinkResult>> {
		let Some(link) = SiteLink::parse(url) else {
			return Ok(None);
		};
		let manga = |key: &str| DeepLinkResult::Manga { key: key.into() };
		Ok(match link.segments.as_slice() {
			["pages", "qing", "detail"] => link.param("id").map(manga),
			["pages", "qing", "page"] => link.param("comic_id").map(manga),
			["pages", "qing", "reader"] => link
				.param("novelid")
				.zip(link.param("volid"))
				.zip(link.param("cptid"))
				.map(|((id, volume_id), chapter_id)| DeepLinkResult::Chapter {
					manga_key: id.into(),
					key: format!("{volume_id}/{chapter_id}"),
				}),
			_ => None,
		})
	}
}

register_source!(
	ZaimanhuaTemplate<ZaimanhuaNovel>,
	Home,
	ListingProvider,
	DeepLinkHandler,
	BasicLoginHandler,
	NotificationHandler,
	DynamicSettings,
	DynamicFilters
);
