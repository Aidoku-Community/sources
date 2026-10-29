use crate::{AUDIENCES, models::RankItem, urls};
use aidoku::{
	Chapter, HomeComponentValue, HomeLayout, Link, MangaWithChapter, Result,
	alloc::{Vec, format},
	imports::net::Request,
};
use zaimanhua_template::{
	Kind,
	home::{featured, items, latest, layout, news_banner, ranking, scroller, send_skeleton},
	models::{ListItem, RecommendCategory},
	net, settings,
};

pub fn home() -> Result<HomeLayout> {
	send_skeleton(
		AUDIENCES
			.iter()
			.map(|(_, title, _)| (*title, HomeComponentValue::empty_scroller())),
	);

	net::sync_account();

	let token = settings::get_token();
	let token = token.as_deref();
	let requests = [
		(urls::news(Kind::Comic), None),
		(urls::recommend(), None),
		(urls::rank("2", 1), token),
		(urls::list(Kind::Comic, "sortType=1", 1), token),
	]
	.into_iter()
	.chain(
		AUDIENCES
			.iter()
			.map(|(_, _, query)| (urls::list(Kind::Comic, query, 1), token)),
	)
	.map(|(url, token)| net::request(&url, token))
	.collect::<Result<Vec<_>>>()?;
	let mut responses = Request::send_all(requests).into_iter();

	let news = news_banner(responses.next());
	// comic banners have a tagline instead of a title
	let banners = responses
		.next()
		.and_then(|response| {
			response
				.ok()?
				.get_json_owned::<Vec<RecommendCategory>>()
				.ok()
		})
		.and_then(|categories| {
			categories
				.into_iter()
				.find(|category| category.category_id == 109)
		})
		.map(|category| {
			category
				.data
				.into_iter()
				.filter(|item| item.kind == 1)
				.map(|item| (item.obj_id, Some(item.title)))
				.collect()
		})
		.unwrap_or_default();
	let rank: Vec<Link> = net::parse::<Vec<RankItem>>(responses.next())
		.unwrap_or_default()
		.into_iter()
		.map(Into::into)
		.collect();
	let updates: Vec<MangaWithChapter> = items(responses.next())
		.into_iter()
		.filter_map(latest_chapter)
		.collect();
	let rows = AUDIENCES
		.iter()
		.zip(responses)
		.filter_map(|((id, title, _), response)| {
			let entries = items(Some(response))
				.into_iter()
				.map(ListItem::into_link)
				.collect();
			scroller(entries, title, id)
		});

	let components = [
		news,
		featured(Kind::Comic, banners, token)?,
		ranking(rank, 2, "rank-monthly"),
		latest(updates),
	]
	.into_iter()
	.flatten()
	.chain(rows)
	.collect();
	layout(components)
}

fn latest_chapter(mut item: ListItem) -> Option<MangaWithChapter> {
	let chapter_id = item.last_update_chapter_id.filter(|id| *id > 0)?;
	let chapter = Chapter {
		key: format!("{}/{chapter_id}", item.id),
		title: item.last_update_chapter_name.take(),
		date_uploaded: item.last_updatetime,
		..Default::default()
	};
	Some(MangaWithChapter {
		chapter,
		manga: item.into_manga(),
	})
}
