#![no_std]
use aidoku::{
	alloc::{vec, String, Vec},
	imports::{html::Document, net::Request},
	prelude::*,
	HomeComponent, HomeLayout, Link, LinkValue, Listing, ListingKind, Manga, MangaPageResult,
	Result, Source,
};
use gigaviewer::{GigaViewer, Impl, Params};

const BASE_URL: &str = "https://comic-days.com";
const CDN_URL: &str = "https://cdn-img.comic-days.com/public/page";

struct ComicDays;

impl Impl for ComicDays {
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
			"series" => {
				item_selector = "ul.daily-series li.daily-series-item";
				title_selector = ".daily-series-title";
				cover_selector = "div.daily-series-thumb img";
				cover_attr = "data-src";
				authors_selector = Some(".daily-series-author");
			}
			"newcomer" | "oneshot" | "daysneo" => {
				item_selector = "li.yomikiri-item-box";
				title_selector = ".yomikiri-link-title h4";
				cover_selector = "img.yomikiri-image";
				cover_attr = "src";
				authors_selector = Some(".yomikiri-link-title h5");
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
				Some("p"),
			)
			.into_iter()
			.map(|manga| manga.into())
			.collect()
		}

		// banner
		let manga_prefix = format!("{BASE_URL}/episode");
		let links = html
			.select_first(".swiper")
			.and_then(|x| {
				Some(
					x.select(".swiper-slide:not(.swiper-slide-duplicate) a")?
						.filter_map(|e| {
							let image = e.select_first("img").and_then(|x| x.attr("src"))?;
							let url = e.attr("href")?;
							let value = if url.starts_with(&manga_prefix) {
								let key = url.strip_prefix(BASE_URL).map(String::from)?;
								LinkValue::Manga(Manga {
									key,
									..Default::default()
								})
							} else {
								LinkValue::Url(url)
							};
							let link = Link {
								title: String::default(),
								image_url: Some(image),
								value: Some(value),
								..Default::default()
							};
							Some(link)
						})
						.collect(),
				)
			})
			.unwrap_or_default();

		// sections
		let ranking_subtitle = html.select_first("#ranking p").and_then(|x| x.text());
		let new_topics = parse_home_section(&html, ".gtm-top-newtopic-item");
		let ranking = parse_home_section(&html, ".gtm-top-ranking-オリジナル-item");
		let originals = parse_home_section(&html, ".gtm-top-days-original-item");
		let newcomers = parse_home_section(&html, ".gtm-top-newcomer-item");

		Ok(HomeLayout {
			components: vec![
				HomeComponent {
					title: None,
					subtitle: None,
					value: aidoku::HomeComponentValue::ImageScroller {
						links,
						auto_scroll_interval: Some(4.0),
						width: Some(341),
						height: Some(128),
					},
				},
				HomeComponent {
					title: Some("新作＆話題作".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: new_topics,
						listing: None,
					},
				},
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
					title: Some("DAYSオリジナル".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: originals,
						listing: None,
					},
				},
				HomeComponent {
					title: Some("新人作家・読み切り".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::Scroller {
						entries: newcomers,
						listing: Some(Listing {
							id: "oneshot".into(),
							name: "読み切り".into(),
							kind: ListingKind::List,
						}),
					},
				},
			],
		})
	}
}

register_source!(
	GigaViewer<ComicDays>,
	PageImageProcessor,
	Home,
	BasicLoginHandler,
	NotificationHandler,
	DeepLinkHandler
);
