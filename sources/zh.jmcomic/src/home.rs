use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, Link, Listing, Manga, MangaPageResult,
	Result,
	alloc::{String, Vec},
	bail, error,
	imports::net::Request,
	prelude::format,
};

use crate::{
	JMComic, block_ctx, extract_id,
	models::{BlockState, ComicItem, PromoteGroup},
	net::{self, ApiContext},
};

const PROMOTE_WEB_URL: &str = "https://jmcomic-zzz.one";
const PROMOTE_PAGE_SIZE: usize = 80;

impl Home for JMComic {
	fn get_home(&self) -> Result<HomeLayout> {
		let api = net::context()?;
		let block = block_ctx(Some(&api));
		let (groups, single) = net::home_data(&api)?;

		let mut components: Vec<HomeComponent> = groups
			.into_iter()
			.filter(PromoteGroup::is_visible)
			.filter_map(|mut g| {
				let listing = g.listing();
				let title = g.title.take();
				let entries: Vec<Link> = g
					.into_manga_list(&api.cdn_base, &block)
					.into_iter()
					.map(Manga::into)
					.collect();
				scroller_component(title, listing, entries)
			})
			.collect();

		let single_entries: Vec<Link> = single
			.into_manga_list(&api.cdn_base, &block)
			.into_iter()
			.take(30)
			.map(Manga::into)
			.collect();
		if let Some(component) = scroller_component(
			Some("单行本推荐".into()),
			Some(Listing {
				id: "cat:single".into(),
				name: "单行本推荐".into(),
				..Default::default()
			}),
			single_entries,
		) {
			components.push(component);
		}

		Ok(HomeLayout { components })
	}
}

pub fn listing_page(
	api: &ApiContext,
	group_id: &str,
	page: i32,
	block: &BlockState,
) -> Result<MangaPageResult> {
	let page = page.max(1);
	if PromoteGroup::is_large_listing_id(group_id) {
		return large_promote_page(group_id, page, &api.cdn_base, block);
	}
	if page > 1 {
		return Ok(MangaPageResult::default());
	}
	let groups: Vec<PromoteGroup> = api.get(&net::url::promote(0))?;
	let entries = groups
		.into_iter()
		.find(|g| g.id == group_id)
		.map(|g| g.into_manga_list(&api.cdn_base, block))
		.unwrap_or_default();
	Ok(MangaPageResult {
		has_next_page: false,
		entries,
	})
}

fn large_promote_page(
	group_id: &str,
	page: i32,
	cdn_base: &str,
	ctx: &BlockState,
) -> Result<MangaPageResult> {
	let url = if page <= 1 {
		format!("{PROMOTE_WEB_URL}/promotes/{group_id}")
	} else {
		format!("{PROMOTE_WEB_URL}/promotes/{group_id}?page={page}")
	};
	let html = Request::get(&url)?
		.header("user-agent", net::JM_UA)
		.header("referer", "https://jmcomic-zzz.one/")
		.html()
		.map_err(|_| error!("页面加载失败"))?;

	let items: Vec<ComicItem> = html
		.select(".list-col")
		.ok_or_else(|| error!("页面解析失败"))?
		.filter_map(|el| {
			let href = el.select_first("a")?.attr("href")?;
			let id = extract_id(&href, "/album/")?;
			let img = el.select_first("img")?;
			let name: String = img.attr("title").or_else(|| img.attr("alt"))?.trim().into();
			if name.is_empty() {
				return None;
			}
			let image = img
				.attr("data-original")
				.or_else(|| img.attr("data-src"))
				.or_else(|| img.attr("src"))
				.and_then(|u| normalize_img(&u));
			Some(ComicItem {
				id,
				name: Some(name),
				author: None,
				image,
				description: None,
				category: None,
				category_sub: None,
			})
		})
		.collect();

	if items.is_empty() {
		bail!("页面解析失败");
	}
	Ok(MangaPageResult {
		has_next_page: items.len() >= PROMOTE_PAGE_SIZE,
		entries: items
			.into_iter()
			.filter(|i| !i.is_blocked(ctx))
			.map(|i| i.into_manga(cdn_base))
			.collect(),
	})
}

fn normalize_img(url: &str) -> Option<String> {
	let url = url.trim();
	if url.is_empty() || url.ends_with("/blank.jpg") {
		return None;
	}
	if url.starts_with("http") {
		return Some(url.into());
	}
	if let Some(p) = url.strip_prefix("//") {
		return Some(format!("https://{p}"));
	}
	url.starts_with('/')
		.then(|| format!("{PROMOTE_WEB_URL}{url}"))
}

fn scroller_component(
	title: Option<String>,
	listing: Option<Listing>,
	entries: Vec<Link>,
) -> Option<HomeComponent> {
	(!entries.is_empty()).then_some(HomeComponent {
		title,
		subtitle: None,
		value: HomeComponentValue::Scroller { entries, listing },
	})
}
