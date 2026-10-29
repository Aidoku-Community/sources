use crate::{
	POPULARITY,
	models::{HotKeyword, NovelRecommendData, RankData, novel_title},
	urls,
};
use aidoku::{
	Chapter, FilterItem, HomeComponent, HomeComponentValue, HomeLayout, Link, MangaWithChapter,
	Result,
	alloc::{Vec, format},
	imports::net::{Request, RequestError, Response},
};
use zaimanhua_template::{
	Kind,
	home::{featured, items, latest, layout, news_banner, ranking, send_skeleton},
	models::ListItem,
	net, settings,
};

pub fn home() -> Result<HomeLayout> {
	let hot_title = "热门搜索";
	send_skeleton([(hot_title, HomeComponentValue::empty_filters())]);

	net::sync_account();

	let token = settings::get_token();
	let token = token.as_deref();
	let requests = [
		(urls::news(Kind::Novel), None),
		(urls::recommend(), None),
		(urls::rank(POPULARITY, 1), token),
		(urls::list(Kind::Novel, "sortType=1", 1), token),
		(urls::hot_keywords(), None),
	]
	.into_iter()
	.map(|(url, token)| net::request(&url, token))
	.collect::<Result<Vec<_>>>()?;
	let mut responses = Request::send_all(requests).into_iter();

	let news = news_banner(responses.next());
	let banners = net::parse::<NovelRecommendData>(responses.next())
		.unwrap_or_default()
		.recommend_list
		.into_iter()
		.find(|category| category.category_id == 57)
		.map(|category| {
			category
				.data
				.into_iter()
				.map(|item| (item.obj_id, None))
				.collect()
		})
		.unwrap_or_default();
	let popular = rank_links(responses.next());
	let updates: Vec<MangaWithChapter> = items(responses.next())
		.into_iter()
		.filter_map(latest_chapter)
		.collect();
	let hot_searches: Vec<FilterItem> = net::parse::<Vec<HotKeyword>>(responses.next())
		.unwrap_or_default()
		.into_iter()
		.map(Into::into)
		.collect();
	let hot = (!hot_searches.is_empty()).then(|| HomeComponent {
		title: Some(hot_title.into()),
		value: HomeComponentValue::Filters(hot_searches),
		..Default::default()
	});

	let components = [
		news,
		featured(Kind::Novel, banners, token)?,
		ranking(popular, 3, "rank"),
		latest(updates),
		hot,
	]
	.into_iter()
	.flatten()
	.collect();
	layout(components)
}

fn rank_links(response: Option<core::result::Result<Response, RequestError>>) -> Vec<Link> {
	net::parse::<RankData>(response)
		.ok()
		.and_then(|data| data.rank_list)
		.unwrap_or_default()
		.into_iter()
		.map(ListItem::into_link)
		.collect()
}

fn latest_chapter(item: ListItem) -> Option<MangaWithChapter> {
	let chapter_id = item.last_update_chapter_id.filter(|id| *id > 0)?;
	let chapter = Chapter {
		key: format!("{}/{chapter_id}", item.last_update_volume_id?),
		title: novel_title(
			item.last_update_volume_name.as_deref()?,
			item.last_update_chapter_name.as_deref()?,
			true,
		),
		..Default::default()
	};
	Some(MangaWithChapter {
		chapter,
		manga: item.into_manga(),
	})
}
