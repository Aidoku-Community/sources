use aidoku::{
	Home, HomeComponent, HomeComponentValue, HomeLayout, Link, MangaPageResult, Result, alloc::Vec,
	bail,
};

use crate::{
	JMComic, PROMOTE_PAGE_SIZE,
	models::{BlockState, ComicItem, PromoteGroup, SearchResp},
	net::{self, ApiContext},
	settings,
};

impl Home for JMComic {
	fn get_home(&self) -> Result<HomeLayout> {
		let api = self.api()?;
		let mut block = settings::block_state();
		let groups: Vec<PromoteGroup> =
			api.get_list(&net::url::promote(0), "mr", "", &mut block)?;
		let components: Vec<_> = groups
			.into_iter()
			.filter(PromoteGroup::is_visible)
			.filter_map(|g| {
				let listing = g.listing();
				let convert = if g.is_novels() {
					ComicItem::into_novel
				} else {
					ComicItem::into_manga
				};
				let entries: Vec<Link> = g
					.content
					.into_iter()
					.filter(|item| !block.matches(item.block_fields()))
					.map(|item| item.into_home_manga(&api.cdn_base, convert))
					.filter(|manga| !block.blocks_key(&manga.key))
					.take(30)
					.map(Into::into)
					.collect();
				(!entries.is_empty()).then_some(HomeComponent {
					title: g.title,
					subtitle: None,
					value: HomeComponentValue::Scroller { entries, listing },
				})
			})
			.collect();
		if components.is_empty() {
			bail!("首頁載入失敗");
		}
		Ok(HomeLayout { components })
	}
}

pub fn listing_page(
	api: &ApiContext,
	group_id: &str,
	page: i32,
	block: &mut BlockState,
) -> Result<MangaPageResult> {
	let path = net::url::promote_list(group_id, page - 1);
	let resp: SearchResp = api.get_list(&path, "mr", "", block)?;
	Ok(resp.into_page_result(page, PROMOTE_PAGE_SIZE, &api.cdn_base, block))
}
