#![cfg_attr(target_arch = "wasm32", no_std)]
use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	Home, HomeComponent, HomeComponentValue, HomeLayout, ImageRequestProvider, Manga,
	MangaPageResult, MangaStatus, MultiSelectFilter, Page, PageContent, PageContext, Result,
	Source, Viewer,
	alloc::{String, Vec, format, string::ToString, vec},
	helpers::{element::ElementHelpers, uri::QueryParameters},
	imports::{html::Html, net::Request, std::send_partial_result},
	prelude::*,
};
use serde::{Deserialize, Deserializer};

const BASE_URL: &str = "https://scans.gg";
const API_URL: &str = "https://api.scans.gg";
const CDN_URL: &str = "https://cdn.scans.gg/uploads";

struct ScansGG;

#[derive(Deserialize)]
struct Response<T> {
	data: T,
}

#[derive(Deserialize)]
struct Tag {
	id: i32,
	title: String,
}

#[derive(Deserialize)]
struct ScanGroup {
	id: i32,
	title: String,
}

#[derive(Deserialize)]
struct HomeResponse {
	featured: Option<Vec<Series>>,
	latest_updates: Option<Vec<Series>>,
	series: Option<Vec<Series>>,
	popular: Option<PopularHome>,
}

#[derive(Deserialize)]
struct PopularHome {
	daily: Option<Vec<Series>>,
	weekly: Option<Vec<Series>>,
	monthly: Option<Vec<Series>>,
	#[serde(rename = "3months")]
	three_months: Option<Vec<Series>>,
	#[serde(rename = "6months")]
	six_months: Option<Vec<Series>>,
	#[serde(rename = "1year")]
	one_year: Option<Vec<Series>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringList {
	List(Vec<String>),
	One(String),
}

#[derive(Deserialize)]
struct Series {
	id: i32,
	cover: Option<String>,
	title: String,
	summary: Option<String>,
	status: Option<i32>,
	#[serde(rename = "type")]
	series_type: Option<i32>,
	content_rating: Option<i32>,
	tags: Option<Vec<i32>>,
	#[serde(default, deserialize_with = "string_list")]
	author: Option<Vec<String>>,
	#[serde(default, deserialize_with = "string_list")]
	artist: Option<Vec<String>>,
	#[serde(default, deserialize_with = "string_list")]
	themes: Option<Vec<String>>,
	#[serde(default, deserialize_with = "string_list")]
	keywords: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ScansChapter {
	id: i32,
	group_id: Option<i32>,
	group: Option<ScanGroup>,
	series_id: i32,
	number: f32,
	volume: Option<f32>,
	title: Option<String>,
	created_at: Option<String>,
	updated_at: Option<String>,
	release_at: Option<String>,
	collab_groups: Option<Vec<i32>>,
}

#[derive(Deserialize)]
struct ChapterNavigation {
	chapter: ChapterWithPages,
}

#[derive(Deserialize)]
struct ChapterWithPages {
	id: i32,
	pages: Option<Vec<ScansPage>>,
}

#[derive(Deserialize)]
struct ScansPage {
	path: String,
}

fn cover_url(cover: Option<&str>) -> Option<String> {
	cover.map(|cover| format!("{CDN_URL}/covers/{cover}"))
}

fn page_url(chapter_id: i32, path: &str) -> String {
	format!("{CDN_URL}/pages/{chapter_id}/{path}")
}

fn id_from_key(key: &str) -> &str {
	key.split_once('-').map(|(id, _)| id).unwrap_or(key)
}

fn timestamp(date: Option<&str>) -> Option<i64> {
	let date = date?;
	chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S")
		.ok()
		.map(|d| d.and_utc().timestamp())
}

fn html_text(html: Option<&str>) -> Option<String> {
	html.and_then(|html| Html::parse_fragment(html).ok())
		.and_then(|html| {
			html.select_first("body")
				.expect("parsed fragment must have body")
				.text_with_newlines()
		})
		.map(|s| s.trim().into())
}

fn string_list<'de, D>(deserializer: D) -> core::result::Result<Option<Vec<String>>, D::Error>
where
	D: Deserializer<'de>,
{
	Ok(match Option::<StringList>::deserialize(deserializer)? {
		Some(StringList::List(list)) => Some(list),
		Some(StringList::One(value)) if !value.is_empty() => Some(vec![value]),
		_ => None,
	})
}

fn push_scroller(components: &mut Vec<HomeComponent>, title: &str, entries: Option<Vec<Series>>) {
	if let Some(entries) = entries
		&& !entries.is_empty()
	{
		components.push(HomeComponent {
			title: Some(title.into()),
			subtitle: None,
			value: HomeComponentValue::Scroller {
				entries: entries
					.into_iter()
					.map(|series| series.basic_manga().into())
					.collect(),
				listing: None,
			},
		});
	}
}

fn push_unique_tag(tags: &mut Vec<String>, tag: String) {
	if !tag.is_empty() && !tags.iter().any(|existing| existing == &tag) {
		tags.push(tag);
	}
}

fn tag_title(tags: &[Tag], id: i32) -> Option<String> {
	tags.iter()
		.find(|tag| tag.id == id)
		.map(|tag| tag.title.clone())
}

fn fetch_tags() -> Result<Vec<Tag>> {
	let mut response = Request::get(format!("{API_URL}/tags"))?.send()?;
	Ok(response.get_json::<Response<Vec<Tag>>>()?.data)
}

fn fetch_groups() -> Result<Vec<ScanGroup>> {
	let mut response = Request::get(format!("{API_URL}/groups"))?.send()?;
	Ok(response.get_json::<Response<Vec<ScanGroup>>>()?.data)
}

fn group_title(groups: &[ScanGroup], id: i32) -> Option<String> {
	groups
		.iter()
		.find(|group| group.id == id)
		.map(|group| group.title.clone())
}

impl Series {
	fn basic_manga(&self) -> Manga {
		Manga {
			key: self.id.to_string(),
			title: self.title.clone(),
			cover: cover_url(self.cover.as_deref()),
			..Default::default()
		}
	}

	fn manga(&self, tag_list: Option<&[Tag]>) -> Manga {
		let mut tags = Vec::new();
		if let Some(ids) = &self.tags
			&& let Some(tag_list) = tag_list
		{
			for id in ids {
				if let Some(tag) = tag_title(tag_list, *id) {
					push_unique_tag(&mut tags, tag);
				}
			}
		}
		if let Some(themes) = &self.themes {
			for theme in themes {
				push_unique_tag(&mut tags, theme.clone());
			}
		}
		if let Some(keywords) = &self.keywords {
			for keyword in keywords {
				push_unique_tag(&mut tags, keyword.clone());
			}
		}

		Manga {
			authors: self.author.clone().filter(|v| !v.is_empty()),
			artists: self.artist.clone().filter(|v| !v.is_empty()),
			description: html_text(self.summary.as_deref()),
			url: Some(format!("{BASE_URL}/series/{}", self.id)),
			tags: if tags.is_empty() { None } else { Some(tags) },
			status: match self.status {
				Some(1) => MangaStatus::Ongoing,
				Some(2) | Some(3) => MangaStatus::Completed,
				Some(4) => MangaStatus::Hiatus,
				_ => MangaStatus::Unknown,
			},
			viewer: match self.series_type {
				Some(2) => Viewer::RightToLeft,
				Some(3) | Some(4) => Viewer::Webtoon,
				_ => Viewer::Unknown,
			},
			content_rating: match self.content_rating {
				Some(2) => ContentRating::Suggestive,
				Some(3) => ContentRating::NSFW,
				_ => ContentRating::Safe,
			},
			..self.basic_manga()
		}
	}
}

impl ScansChapter {
	fn chapter(&self, groups: Option<&[ScanGroup]>) -> Chapter {
		let mut scanlators = Vec::new();
		if let Some(group) = &self.group {
			push_unique_tag(&mut scanlators, group.title.clone());
		}
		if let Some(groups) = groups {
			if let Some(group_id) = self.group_id
				&& let Some(title) = group_title(groups, group_id)
			{
				push_unique_tag(&mut scanlators, title);
			}
			if let Some(collab_groups) = &self.collab_groups {
				for group_id in collab_groups {
					if let Some(title) = group_title(groups, *group_id) {
						push_unique_tag(&mut scanlators, title);
					}
				}
			}
		}

		Chapter {
			key: self.id.to_string(),
			title: self.title.as_ref().filter(|s| !s.is_empty()).cloned(),
			chapter_number: Some(self.number),
			volume_number: self.volume,
			date_uploaded: timestamp(self.updated_at.as_deref())
				.or_else(|| timestamp(self.created_at.as_deref())),
			scanlators: if scanlators.is_empty() {
				None
			} else {
				Some(scanlators)
			},
			url: Some(format!("{BASE_URL}/series/{}/{}", self.series_id, self.id)),
			locked: self.release_at.is_some(),
			..Default::default()
		}
	}
}

impl Source for ScansGG {
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
		if let Some(query) = query.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
			qs.push("q", Some(query));
		}
		let limit = 21;
		let offset = (page.saturating_sub(1) * limit).to_string();
		qs.push("limit", Some(&limit.to_string()));
		qs.push("offset", Some(&offset));

		for filter in filters {
			if let FilterValue::MultiSelect { id, included, .. } = filter {
				if included.is_empty() {
					continue;
				}
				let values = format!("[{}]", included.join(","));
				match id.as_str() {
					"type" => qs.push("q_type", Some(&values)),
					"status" => qs.push("q_status", Some(&values)),
					"tags" => qs.push("q_tags", Some(&values)),
					_ => {}
				}
			}
		}

		let mut response = Request::get(format!("{API_URL}/series?{qs}"))?.send()?;
		let data = response.get_json::<Response<Vec<Series>>>()?;
		let entries: Vec<Manga> = data
			.data
			.into_iter()
			.map(|series| series.basic_manga())
			.collect();

		Ok(MangaPageResult {
			has_next_page: entries.len() == limit as usize,
			entries,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let manga_id = id_from_key(&manga.key).to_string();

		if needs_details {
			let mut response = Request::get(format!("{API_URL}/series?id={manga_id}"))?.send()?;
			let data = response.get_json::<Response<Series>>()?;
			let tag_list = fetch_tags().ok();
			manga.copy_from(data.data.manga(tag_list.as_deref()));
			send_partial_result(&manga);
		}

		if needs_chapters {
			let mut response =
				Request::get(format!("{API_URL}/chapters?series_id={manga_id}"))?.send()?;
			let data = response.get_json::<Response<Vec<ScansChapter>>>()?;
			let groups = fetch_groups().ok();
			manga.chapters = Some(
				data.data
					.into_iter()
					.map(|chapter| chapter.chapter(groups.as_deref()))
					.collect(),
			);
		}

		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let manga_id = id_from_key(&manga.key);
		let mut response = Request::get(format!(
			"{API_URL}/chapter-navigation?series_id={}&chapter_id={}",
			manga_id, chapter.key
		))?
		.send()?;
		let data = response.get_json::<Response<ChapterNavigation>>()?;

		Ok(data
			.data
			.chapter
			.pages
			.unwrap_or_default()
			.into_iter()
			.map(|page| Page {
				content: PageContent::url(page_url(data.data.chapter.id, &page.path)),
				..Default::default()
			})
			.collect())
	}
}

impl Home for ScansGG {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut response = Request::get(format!("{API_URL}/home"))?.send()?;
		let data = response.get_json::<Response<HomeResponse>>()?;
		let mut components = Vec::new();

		if let Some(entries) = data.data.featured
			&& !entries.is_empty()
		{
			components.push(HomeComponent {
				title: Some("Featured".into()),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: entries
						.into_iter()
						.map(|series| series.basic_manga())
						.collect(),
					auto_scroll_interval: None,
				},
			});
		}

		push_scroller(&mut components, "Latest Updates", data.data.latest_updates);
		push_scroller(&mut components, "Series", data.data.series);

		if let Some(popular) = data.data.popular {
			push_scroller(&mut components, "Popular Today", popular.daily);
			push_scroller(&mut components, "Popular This Week", popular.weekly);
			push_scroller(&mut components, "Popular This Month", popular.monthly);
			push_scroller(
				&mut components,
				"Popular Last 3 Months",
				popular.three_months,
			);
			push_scroller(&mut components, "Popular Last 6 Months", popular.six_months);
			push_scroller(&mut components, "Popular This Year", popular.one_year);
		}

		Ok(HomeLayout { components })
	}
}

impl ImageRequestProvider for ScansGG {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?.header("Referer", &format!("{BASE_URL}/")))
	}
}

impl DynamicFilters for ScansGG {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let type_filter = MultiSelectFilter {
			id: "type".into(),
			title: Some("Type".into()),
			options: vec![
				"Comic".into(),
				"Manga".into(),
				"Manhwa".into(),
				"Manhua".into(),
				"Mangatoon".into(),
				"Webtoon".into(),
				"One Shot".into(),
				"Doujinshi".into(),
			],
			ids: Some(vec![
				"1".into(),
				"2".into(),
				"3".into(),
				"4".into(),
				"5".into(),
				"6".into(),
				"7".into(),
				"8".into(),
			]),
			..Default::default()
		};

		let status_filter = MultiSelectFilter {
			id: "status".into(),
			title: Some("Status".into()),
			options: vec![
				"Ongoing".into(),
				"Completed".into(),
				"Hiatus".into(),
				"Dropped".into(),
				"Upcoming".into(),
				"Paused".into(),
				"Cancelled".into(),
			],
			ids: Some(vec![
				"1".into(),
				"2".into(),
				"3".into(),
				"4".into(),
				"5".into(),
				"6".into(),
				"7".into(),
			]),
			..Default::default()
		};

		let tags = fetch_tags()?;
		let genre_filter = MultiSelectFilter {
			id: "tags".into(),
			title: Some("Genres".into()),
			is_genre: true,
			uses_tag_style: true,
			options: tags.iter().map(|tag| tag.title.clone().into()).collect(),
			ids: Some(
				tags.into_iter()
					.map(|tag| tag.id.to_string().into())
					.collect(),
			),
			..Default::default()
		};

		Ok(vec![
			type_filter.into(),
			status_filter.into(),
			genre_filter.into(),
		])
	}
}

impl DeepLinkHandler for ScansGG {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let Some(path) = path.strip_prefix("/series/") else {
			return Ok(None);
		};
		let path = path
			.split_once('?')
			.map(|(path, _)| path)
			.unwrap_or(path)
			.split_once('#')
			.map(|(path, _)| path)
			.unwrap_or(path);
		let mut parts = path.split('/');
		let Some(series_id) = parts.next().filter(|s| !s.is_empty()) else {
			return Ok(None);
		};
		let series_id = id_from_key(series_id);

		if let Some(chapter_id) = parts.next().filter(|s| !s.is_empty()) {
			Ok(Some(DeepLinkResult::Chapter {
				manga_key: series_id.into(),
				key: chapter_id.into(),
			}))
		} else {
			Ok(Some(DeepLinkResult::Manga {
				key: series_id.into(),
			}))
		}
	}
}

register_source!(
	ScansGG,
	Home,
	ImageRequestProvider,
	DeepLinkHandler,
	DynamicFilters
);
