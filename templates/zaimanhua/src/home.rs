use crate::{
	Kind, NEWS_URL,
	models::{ListData, ListItem, NewsData, UnifiedData},
	net::{self, urls},
};
use aidoku::{
	HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Link, LinkValue, Listing,
	Manga, MangaWithChapter, Result,
	alloc::{String, Vec, format},
	imports::{
		net::{Request, RequestError, Response},
		std::send_partial_result,
	},
	prelude::*,
};

const FEATURED_TITLE: &str = "精品推荐";
const RANKING_TITLE: &str = "人气推荐";
const LATEST_TITLE: &str = "最近更新";

pub fn send_skeleton(rows: impl IntoIterator<Item = (&'static str, HomeComponentValue)>) {
	send_partial_result(&HomePartialResult::Layout(HomeLayout {
		components: [
			(None, HomeComponentValue::empty_image_scroller()),
			(
				Some(FEATURED_TITLE),
				HomeComponentValue::empty_big_scroller(),
			),
			(Some(RANKING_TITLE), HomeComponentValue::empty_manga_list()),
			(
				Some(LATEST_TITLE),
				HomeComponentValue::empty_manga_chapter_list(),
			),
		]
		.into_iter()
		.chain(rows.into_iter().map(|(title, value)| (Some(title), value)))
		.map(|(title, value)| HomeComponent {
			title: title.map(Into::into),
			value,
			..Default::default()
		})
		.collect(),
	}));
}

pub fn news_banner(
	response: Option<core::result::Result<Response, RequestError>>,
) -> Option<HomeComponent> {
	let news = net::parse::<NewsData>(response).ok()?.news_list;
	(!news.is_empty()).then(|| HomeComponent {
		value: HomeComponentValue::ImageScroller {
			links: news
				.into_iter()
				.take(5)
				.map(|item| Link {
					title: item.title,
					image_url: item.row_pic_url,
					value: Some(LinkValue::Url(format!(
						"{NEWS_URL}/article/{}.html",
						item.article_id
					))),
					..Default::default()
				})
				.collect(),
			auto_scroll_interval: None,
			width: Some(252),
			height: Some(162),
		},
		..Default::default()
	})
}

// banner items have wide artwork instead of a cover
pub fn featured(
	kind: Kind,
	banners: Vec<(i64, Option<String>)>,
	token: Option<&str>,
) -> Result<Option<HomeComponent>> {
	let requests = banners
		.iter()
		.map(|(id, _)| net::request(&urls::search_id(kind, id), token))
		.collect::<Result<Vec<_>>>()?;
	let entries: Vec<Manga> = banners
		.into_iter()
		.zip(Request::send_all(requests))
		.filter_map(|((id, tagline), response)| {
			let manga = net::parse::<UnifiedData>(Some(response))
				.ok()?
				.into_featured(id)?;
			Some(Manga {
				description: tagline.or(manga.description),
				..manga
			})
		})
		.collect();
	Ok((!entries.is_empty()).then(|| HomeComponent {
		title: Some(FEATURED_TITLE.into()),
		value: HomeComponentValue::BigScroller {
			entries,
			auto_scroll_interval: Some(8.0),
		},
		..Default::default()
	}))
}

pub fn ranking(entries: Vec<Link>, rows: i32, listing: &str) -> Option<HomeComponent> {
	(!entries.is_empty()).then(|| HomeComponent {
		title: Some(RANKING_TITLE.into()),
		value: HomeComponentValue::MangaList {
			ranking: true,
			page_size: Some(rows),
			entries,
			listing: Some(Listing {
				id: listing.into(),
				name: RANKING_TITLE.into(),
				..Default::default()
			}),
		},
		..Default::default()
	})
}

pub fn latest(entries: Vec<MangaWithChapter>) -> Option<HomeComponent> {
	(!entries.is_empty()).then(|| HomeComponent {
		title: Some(LATEST_TITLE.into()),
		value: HomeComponentValue::MangaChapterList {
			page_size: Some(4),
			entries,
			listing: Some(Listing {
				id: "latest".into(),
				name: "更新".into(),
				..Default::default()
			}),
		},
		..Default::default()
	})
}

pub fn scroller(entries: Vec<Link>, title: &str, listing: &str) -> Option<HomeComponent> {
	(!entries.is_empty()).then(|| HomeComponent {
		title: Some(title.into()),
		value: HomeComponentValue::Scroller {
			entries,
			listing: Some(Listing {
				id: listing.into(),
				name: title.into(),
				..Default::default()
			}),
		},
		..Default::default()
	})
}

pub fn items(response: Option<core::result::Result<Response, RequestError>>) -> Vec<ListItem> {
	net::parse::<ListData>(response)
		.map(|data| data.list)
		.unwrap_or_default()
}

pub fn layout(components: Vec<HomeComponent>) -> Result<HomeLayout> {
	if components.is_empty() {
		bail!("首页加载失败");
	}
	Ok(HomeLayout { components })
}
