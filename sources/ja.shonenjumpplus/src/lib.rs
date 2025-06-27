#![no_std]
use aidoku::{
	alloc::{vec, Vec},
	imports::{html::Document, net::Request},
	prelude::*,
	HomeComponent, HomeLayout, Link, Listing, MangaPageResult, Result, Source,
};
use gigaviewer::{GigaViewer, Impl, Params};

const BASE_URL: &str = "https://shonenjumpplus.com";
const CDN_URL: &str = "https://cdn-ak-img.shonenjumpplus.com";

struct ShonenJumpPlus;

impl Impl for ShonenJumpPlus {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			cdn_url: CDN_URL.into(),
			..Default::default()
		}
	}

	fn get_manga_list(
		&self,
		_params: &Params,
		listing: Listing,
		_page: i32,
	) -> Result<MangaPageResult> {
		let item_selector: &str;
		let title_selector: &str;
		let cover_selector: &str;
		let cover_attr: &str;
		let authors_selector: Option<&str>;

		match listing.id.as_str() {
			"series" | "series/oneshot" | "series/finished" => {
				item_selector = ".series-list-item";
				title_selector = ".series-list-title";
				cover_selector = ".series-list-thumb img";
				cover_attr = "data-src";
				authors_selector = Some(".series-list-author");
			}
			_ => return Impl::get_manga_list(self, _params, listing, _page),
		}

		let base_url = self.params().base_url;
		let html = Request::get(format!("{}/{}", base_url, listing.id))?.html()?;

		let entries = gigaviewer::parser::parse_response(
			&html,
			&base_url,
			item_selector,
			title_selector,
			cover_selector,
			cover_attr,
			authors_selector,
			None,
		);

		Ok(MangaPageResult {
			entries,
			has_next_page: false,
		})
	}

	fn get_home(&self, _params: &Params) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?.html()?;

		fn parse_home_section(html: &Document, item_selector: &str) -> Vec<Link> {
			gigaviewer::parser::parse_response(
				html,
				BASE_URL,
				item_selector,
				"h3",
				"img",
				"src",
				None,
				None,
			)
			.into_iter()
			.map(|manga| manga.into())
			.collect()
		}

		// sections
		let ranking_subtitle = html.select_first(".date-wrapper").and_then(|x| x.text());
		let ranking = gigaviewer::parser::parse_response(
			&html,
			BASE_URL,
			".daily:first-child .daily-series-item",
			"h2",
			"img",
			"src",
			Some(".daily-series-author"),
			Some("p"),
		)
		.into_iter()
		.map(|manga| manga.into())
		.collect();
		let total_ranking = parse_home_section(&html, ".total-ranking-list-sp a");
		let free_campaign = parse_home_section(&html, ".free-campaign a");

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("今日のランキング".into()),
					subtitle: ranking_subtitle,
					value: aidoku::HomeComponentValue::MangaList {
						ranking: true,
						page_size: Some(3),
						entries: ranking,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("総合ランキング".into()),
					subtitle: Some("今話題の作品をチェック！".into()),
					value: aidoku::HomeComponentValue::Scroller {
						entries: total_ranking,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("無料キャンペーン・復刻連載".into()),
					subtitle: Some("話題作や名作が今だけ無料の大公開！".into()),
					value: aidoku::HomeComponentValue::Scroller {
						entries: free_campaign,
						listing: None,
					},
				},
				// rookies are on a different subdomain, which we aren't supporting (idek if it's gigaviewer)
				// HomeComponent {
				//     title: Some("ジャンプルーキー！".into()),
				//     subtitle: Some("才能溢れる投稿作が読み放題！".into()),
				//     value: aidoku::HomeComponentValue::Scroller {
				//         entries: rookie,
				//         listing: None,
				//     },
				// },
			],
		})
	}
}

register_source!(
	GigaViewer<ShonenJumpPlus>,
	ListingProvider,
	Home,
	PageImageProcessor,
	BasicLoginHandler,
	NotificationHandler,
	DeepLinkHandler
);
