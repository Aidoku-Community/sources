#![no_std]
use aidoku::{
	Chapter, DynamicFilters, Filter, FilterKind, FilterValue, Home, HomeComponent, HomeLayout,
	HomePartialResult, Manga, MangaPageResult, MangaWithChapter, Page, PageContent, Result,
	SelectFilter, Source,
	alloc::{
		String, Vec,
		borrow::{Cow, ToOwned},
		string::ToString,
		vec,
	},
	helpers::uri::QueryParameters,
	imports::{defaults::defaults_get, net::Request, std::send_partial_result},
	prelude::*,
};

mod models;

use models::*;
use regex::Regex;
use serde_json::Value;

pub const BASE_URL: &str = "https://minotruyenv1.xyz";
pub const API_URL: &str = "https://api.cloudkk.art";

struct MinoTruyen;

impl Home for MinoTruyen {
	fn get_home(&self) -> Result<HomeLayout> {
		send_partial_result(&HomePartialResult::Layout(HomeLayout {
			components: vec![
				HomeComponent {
					title: Some("Truyện Nổi Bật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_big_scroller(),
				},
				HomeComponent {
					title: Some("Mới Cập Nhật".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_manga_chapter_list(),
				},
				HomeComponent {
					title: Some("Top".into()),
					subtitle: None,
					value: aidoku::HomeComponentValue::empty_scroller(),
				},
			],
		}));

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Truyện Nổi Bật")),
			subtitle: None,
			value: aidoku::HomeComponentValue::BigScroller {
				entries: Request::get(format!(
					"{}/api/books/top/featured?category=comics&take=10",
					API_URL,
				))?
				.send()?
				.get_json::<FeaturedRoot>()?
				.data
				.books
				.into_iter()
				.map(|v| v.into())
				.collect::<Vec<_>>(),
				auto_scroll_interval: Some(10.0),
			},
		}));

		let html = Request::get(format!("{BASE_URL}/comics"))?
			.send()?
			.get_html()?;
		let text = html
			.select("script")
			.and_then(|v| {
				v.filter(|node| node.html().unwrap_or_default().contains("grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-4 xl:grid-cols-5 gap-x-4 gap-y-3")).last()
			})
			.and_then(|f| f.html())
			.and_then(|t| extract_next_object(&t, None));

		if let Some(text) = text {
			let json = serde_json::from_str::<
				FlightRoot<Vec<FlightAny<FlightRoot<FlightChild<FlightNode>>>>>,
			>(&text)?;

			let entries = json
				.children
				.into_iter()
				.filter_map(|t| match t {
					FlightAny::Arr(v) => Some(v),
					FlightAny::Str(_) => None,
				})
				.map(|n| n.3.children.3.book.into())
				.collect::<Vec<MangaWithChapter>>();

			send_partial_result(&HomePartialResult::Component(HomeComponent {
				title: Some(String::from("Mới Cập Nhật")),
				subtitle: None,
				value: aidoku::HomeComponentValue::MangaChapterList {
					entries,
					page_size: Some(4),
					listing: None,
				},
			}));
		}

		send_partial_result(&HomePartialResult::Component(HomeComponent {
			title: Some(String::from("Top")),
			subtitle: None,
			value: aidoku::HomeComponentValue::MangaList {
				entries: Request::get(format!("{}/api/books/side-home?category=comics", API_URL,))?
					.send()?
					.get_json::<SideHomeRoot>()?
					.top_books_view
					.into_iter()
					.map(|v| Manga::from(v).into())
					.collect::<Vec<_>>(),
				listing: None,
				ranking: true,
				page_size: Some(3),
			},
		}));

		Ok(HomeLayout::default())
	}
}

impl Source for MinoTruyen {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut qs = QueryParameters::new();

		qs.push("take", "24".into());
		qs.push("page", Some(&page.to_string()));

		let r#type = defaults_get::<String>("type");
		qs.push("category", r#type.as_deref());

		if let Some(query) = query {
			qs.push("q", Some(&query))
		}

		for filter in filters {
			match filter {
				FilterValue::MultiSelect {
					included, excluded, ..
				} => {
					qs.push("genres", Some(&included.join(",").to_lowercase()));
					qs.push("notgenres", Some(&excluded.join(",").to_lowercase()));
				}
				_ => {}
			}
		}

		println!("{}", format!("{API_URL}/api/books?{qs}"));
		let (entries, has_next_page) = Request::get(format!("{API_URL}/api/books?{qs}"))?
			.send()?
			.get_json::<FeaturedBooksData>()
			.map(|res| {
				(
					res.books.into_iter().map(Manga::from).collect(),
					res.count_books
						.map(|v| v > (page * 24).into())
						.unwrap_or_default(),
				)
			})?;

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if needs_details {
			let html = Request::get(format!("{BASE_URL}/{}", manga.key.replace("/", "/books/")))?
				.send()?
				.get_html()?;

			let text = html
				.select("script")
				.and_then(|mut f| {
					f.find(|n| {
						n.html()
							.unwrap_or_default()
							.contains("dangerouslySetInnerHTML")
					})
				})
				.and_then(|n| n.html())
				.unwrap_or_default();

			let Some(json) = extract_next_object(&text, Some(3)) else {
				bail!("extract Next error");
			};

			manga.copy_from(serde_json::from_str::<BookItem>(&json)?.into());
		}

		if needs_chapters {
			let chapters = Request::get(format!(
				"{API_URL}/api/chapters/{}?order=desc&take=5000",
				manga.key.split("/").last().unwrap_or_default()
			))?
			.send()?
			.get_json::<Chapters>()?
			.chapters
			.into_iter()
			.map(|c| VChapterF::to(c, &manga))
			.collect::<Vec<_>>();

			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		todo!();
		// let pages = Request::get(format!(
		// 	"{BASE_URL}/api/chapter_image?chapter={}&v=0",
		// 	chapter.key
		// ))?
		// .send()?
		// .get_json::<ChapterImages>()?
		// .image
		// .into_iter()
		// .map(|p| Page {
		// 	content: PageContent::url(p),
		// 	..Default::default()
		// })
		// .collect();

		// Ok(pages)
	}
}

// impl DynamicFilters for MinoTruyen {
// 	fn get_dynamic_filters<'a>(&'a self) -> Result<Vec<Filter>> {
// 		let type_f = defaults_get::<String>("type").unwrap_or("all".to_owned());

// 		let list_data: Vec<Request> = if type_f == "all" {
// 			vec![
// 				Request::get(format!("{BASE_URL}/manga/search-advanced"))?,
// 				Request::get(format!("{BASE_URL}/comics/search-advanced"))?,
// 			]
// 		} else {
// 			vec![Request::get(format!(
// 				"{BASE_URL}/{type_f}/search-advanced"
// 			))?]
// 		};

// 		let results = list_data
// 			.into_iter()
// 			.filter_map(|page| {
// 				let document = page.html().ok()?;

// 				let script = document
// 					.select("script")?
// 					.find(|node| node.html().unwrap_or_default().contains("genresTag"))?;

// 				let script_text = script.html()?.to_string();

// 				let re = Regex::new(r#"\{\\?"content\\?":.*?\}\]"#).ok()?;

// 				let caps = re.find(&script_text)?;
// 				let raw_json = caps.as_str();

// 				let cleaned = raw_json.replace(r#"\""#, r#"""#).replace("\\/", "/");

// 				let json_value: Value = serde_json::from_str(&cleaned).ok()?;

// 				let categories = json_value.get("category")?.as_array()?;

// 				let parsed: Vec<(String, String, i64)> = categories
// 					.iter()
// 					.filter_map(|c| {
// 						Some((
// 							c.get("name")?.as_str()?.to_owned(),
// 							c.get("tagId")?.as_str()?.to_owned(),
// 							c.get("tagging_count")?.as_i64()?,
// 						))
// 					})
// 					.collect();

// 				Some(parsed)
// 			})
// 			.flatten()
// 			.collect::<Vec<_>>();

// 		let options: Vec<Cow<'_, str>> = results.iter().map(|v| v.0.clone().into()).collect();
// 		let ids: Option<Vec<Cow<'_, str>>> =
// 			Some(results.iter().map(|v| v.1.clone().into()).collect());

// 		Ok(vec![Filter {
// 			id: "genres".into(),
// 			title: Some("Thể loại".into()),
// 			hide_from_header: None,
// 			kind: FilterKind::MultiSelect {
// 				is_genre: true,
// 				uses_tag_style: true,
// 				options,
// 				ids,
// 				can_exclude: true,
// 				default_excluded: None,
// 				default_included: None,
// 			},
// 		}])
// 	}
// }

register_source!(MinoTruyen, Home);
