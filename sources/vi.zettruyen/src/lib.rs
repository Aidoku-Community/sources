#![no_std]
use aidoku::{
	Chapter, FilterValue, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Manga,
	Result, Source, Viewer,
	alloc::{borrow::ToOwned, string::ToString, *},
	helpers::uri::QueryParameters,
	imports::{html::Element, net::Request, std::send_partial_result},
	prelude::*,
};
use wpcomics::{Cache, Impl, Params, WpComics};

const BASE_URL: &str = "https://www.zettruyen.space";

mod models;
use models::*;

struct ZetTruyen;

impl Impl for ZetTruyen {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			next_page: "a[href*=nang-cao].items-center.bg-theme-color.cursor-pointer:last-child",
			viewer: Viewer::RightToLeft,
			manga_cell: ".grid > a",
			manga_cell_title: "span.line-clamp-2",
			manga_cell_url: "a",
			manga_cell_image: "img",
			manga_cell_image_attr: "abs:src",

			manga_details_title: "h1.comic-title-content",
			manga_details_cover: ".thumb-cover img.text-transparent",
			manga_details_chapters: "#chapters-list-container > div",
			chapter_anchor_selector: "a",
			chapter_date_selector: "div > div.text-xs",

			manga_parse_id: |url| {
				url.split("truyen-tranh/")
					.nth(1)
					.and_then(|s| s.split('/').next())
					.unwrap_or_default()
					.trim_end_matches(".html")
					.into()
			},
			chapter_parse_id: |url| {
				url.trim_end_matches('/')
					.rsplit('/')
					.next()
					.unwrap_or_default()
					.into()
			},
			manga_viewer_page: ".w-full.mx-auto.center > img",

			manga_details_authors: "div:contains(Tác giả) + div",
			manga_details_description: ".comic-content",
			manga_details_tags: ".hidden div > div.flex.flex-row.flex-wrap.gap-3 > a",
			manga_details_tags_splitter: "",
			manga_details_status: "div.text-\\[\\#D9D9D9\\], div:contains(Trạng thái) + div",

			manga_page: |params, manga| format!("{}/truyen-tranh/{}", params.base_url, manga.key),
			page_list_page: |params, manga, chapter| {
				format!(
					"{}/truyen-tranh/{}/{}",
					params.base_url,
					manga.key,
					chapter.key.replace("chapter-", "chuong-")
				)
			},

			get_search_url: |params, q, page, filters| {
				let mut query = QueryParameters::new();
				query.push("name", q.as_deref());
				query.push("page", Some(&page.to_string()));

				for filter in filters {
					match filter {
						FilterValue::MultiSelect { included, .. } => {
							query.push("category", Some(&included.join(",")));
						}
						FilterValue::Select { id, value } => {
							query.push(&id, Some(&value));
						}
						FilterValue::Sort { id, index, .. } => {
							query.push(&id, Some(&index.to_string()));
						}
						_ => {}
					}
				}

				Ok(format!("{}/tim-kiem-nang-cao?{query}", params.base_url))
			},

			home_manga_link: "a.truncate.uppercase, .mt-2 > a",
			home_chapter_link: ".mt-1 > a.text-txt-secondary",

			home_sliders_selector: ".owl-carousel",
			home_sliders_title_selector: "h2",
			home_sliders_item_selector: ".thumb-cover",

			home_grids_selector: ".manga-horizontal-bar-container, #LatestUpdate",
			home_grids_title_selector: ".manga-horizontal-bar-title",
			home_grids_item_selector: ".manga-horizontal-bar-content > div",

			time_formats: Some(vec!["%d/%m/%Y", "%m-%d-%Y", "%Y-%d-%m"]),

			..Default::default()
		}
	}

	fn get_home(&self, cache: &mut Cache, params: &Params) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: None,
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Top".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Top ngày".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Top tháng".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Top tuần".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
				HomeComponent {
					title: Some("Mới cập nhật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
			],
		}));

		let base_url = &params.base_url.clone();
		let html = self.create_request(cache, params, base_url, None)?.html()?;

		let parse_manga = |el: &Element, slider: bool| -> Option<Manga> {
			let manga_link = el
				.select_first(params.home_manga_link)
				.or_else(|| el.select_first(".widget-title a"))?;
			let cover = el
				.select_first(params.home_manga_cover_selector)
				.and_then(|img| {
					img.attr(if slider {
						params
							.home_manga_cover_slider_attr
							.unwrap_or(params.home_manga_cover_attr)
					} else {
						params.home_manga_cover_attr
					})
					.or_else(|| img.attr("data-cfsrc"))
				})
				.map(|src| {
					if slider {
						(params.home_manga_cover_slider_transformer)(src)
					} else {
						src
					}
				});
			let url = manga_link.attr("abs:href")?;
			Some(Manga {
				key: (params.manga_parse_id)(&url),
				title: manga_link.text()?,
				cover,
				url: Some(url),
				..Default::default()
			})
		};

		if let Some(popular_sliders) = html.select(params.home_sliders_selector) {
			for popular_slider in popular_sliders {
				let title = popular_slider
					.select_first(params.home_sliders_title_selector)
					.and_then(|el| el.text());
				let items = popular_slider
					.select(params.home_sliders_item_selector)
					.map(|els| {
						els.filter_map(|el| parse_manga(&el, true))
							.collect::<Vec<_>>()
					})
					.unwrap_or_default();
				if !items.is_empty() {
					send_partial_result(&HomePartialResult::Component(HomeComponent {
						title,
						subtitle: None,
						value: HomeComponentValue::BigScroller {
							entries: items.into_iter().collect(),
							auto_scroll_interval: Some(10.0),
						},
					}));
				}
			}
		}

		let top = Request::get(format!("{BASE_URL}/api/comics/top"))?
			.send()?
			.get_json::<Top>()?;

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Top".to_owned()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: top
					.data
					.top_all
					.into_iter()
					.map(|v| Manga::from(v).into())
					.collect::<Vec<_>>(),
				listing: None,
			},
		}));
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Top ngày".to_owned()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: top
					.data
					.top_day
					.into_iter()
					.map(|v| Manga::from(v).into())
					.collect::<Vec<_>>(),
				listing: None,
			},
		}));
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Top tháng".to_owned()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: top
					.data
					.top_month
					.into_iter()
					.map(|v| Manga::from(v).into())
					.collect::<Vec<_>>(),
				listing: None,
			},
		}));
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Top tuần".to_owned()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: top
					.data
					.top_week
					.into_iter()
					.map(|v| Manga::from(v).into())
					.collect::<Vec<_>>(),
				listing: None,
			},
		}));

		let related = Request::get(format!("{BASE_URL}/api/comics/related?limit=10"))?
			.send()?
			.get_json::<Related>()?
			.data;
		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some("Mới cập nhật".to_owned()),
			subtitle: None,
			value: HomeComponentValue::MangaChapterList {
				entries: related.into_iter().map(|v| v.into()).collect::<Vec<_>>(),
				page_size: Some(4),
				listing: None,
			},
		}));

		Ok(HomeLayout::default())
	}

	fn get_manga_update(
		&self,
		cache: &mut Cache,
		params: &Params,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = (params.manga_page)(params, &manga);

		if needs_details {
			let new_manga = self.parse_manga_element(cache, params, url.clone())?;

			manga.copy_from(new_manga);

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(self.get_chapter_list(cache, params, manga.key.to_owned())?);
		}

		Ok(manga)
	}

	fn get_chapter_list(&self, _: &mut Cache, _: &Params, key: String) -> Result<Vec<Chapter>> {
		let limit = 100;
		let mut page = 1;

		let mut chapters: Vec<Chapter> = vec![];
		loop {
			let url = format!(
				"{BASE_URL}/api/comics/{key}/chapters?page={page}&per_page={limit}&order=desc",
			);

			let data = Request::get(url)?.send()?.get_json::<ChaptersData>()?.data;
			let chunk: Vec<Chapter> = data
				.chapters
				.into_iter()
				.map(|c| {
					let slug = c
						.chapter_num
						.map(|v| format!("chuong-{v}"))
						.unwrap_or_else(|| c.chapter_slug.replace("chapter-", "chuong-"));
					let mut chapter: Chapter = c.into();

					chapter.url = Some(format!("{BASE_URL}/truyen-tranh/{key}/{slug}"));

					chapter
				})
				.collect();

			let size = chunk.len();
			chapters.extend(chunk);

			if data.current_page >= data.last_page || size == 25 {
				break;
			}

			page += 1;
		}

		Ok(chapters)
	}
}

register_source!(
	WpComics<ZetTruyen>,
	ImageRequestProvider,
	DeepLinkHandler,
	Home
);
