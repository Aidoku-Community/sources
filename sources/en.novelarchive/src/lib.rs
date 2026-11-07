#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, DynamicSettings, Filter, FilterValue,
	GroupSetting, Home, HomeComponent, HomeComponentValue, HomeLayout, HomePartialResult, Listing,
	ListingProvider, Manga, MangaPageResult, MultiSelectFilter, MultiSelectSetting, Page,
	PageContent, Result, SelectFilter, Setting, Source,
	alloc::{String, Vec, vec},
	helpers::string::PlainText,
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod markdown;
mod models;
mod settings;

use helpers::{SearchParams, browse, get, listed_genres, mirror_chapters, parse, request};
use markdown::{html_to_markdown, paragraphs};
use models::{ChapterResponse, MirrorChapterText, MirrorList, Novel, NovelDetails, NovelList};

const BASE_URL: &str = "https://novelarchive.st";
const API_URL: &str = "https://novelarchive.st/api/novels";
const PAGE_SIZE: i32 = 24;
// The fixed feeds return at most 50, whatever the limit.
const FEED_SIZE: i32 = 50;

struct NovelArchive;

impl Source for NovelArchive {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// A pasted novel or reader link answers with that novel's card; anything else is searched.
		if let Some(url) = query
			.as_deref()
			.map(str::trim)
			.filter(|q| q.starts_with(BASE_URL))
			&& let Ok(Some(
				DeepLinkResult::Manga { key } | DeepLinkResult::Chapter { manga_key: key, .. },
			)) = self.handle_deep_link(url.into())
			&& let Ok(details) = get::<NovelDetails>(&format!("{API_URL}/{key}"))
		{
			return Ok(MangaPageResult {
				entries: vec![details.novel.into()],
				has_next_page: false,
			});
		}
		let mut params = SearchParams {
			query,
			..SearchParams::from_settings()
		};
		for filter in filters {
			match filter {
				// Recently updated is the site's default and isn't sent.
				FilterValue::Sort { index, .. } => {
					params.sort = match index {
						1 => Some("popular"),
						2 => Some("rating"),
						3 => Some("chapters"),
						_ => None,
					}
				}
				FilterValue::Select { id, value } => match id.as_str() {
					"status" => params.status = (value != "all").then_some(value),
					"source" => params.source = (value != "all").then_some(value),
					"genre_match" => params.any_genre = value == "any",
					// A tapped tag carries the genre's label.
					"genre" => params.genres = vec![value.to_lowercase()],
					_ => {}
				},
				FilterValue::MultiSelect {
					id,
					included,
					excluded,
				} if id == "genre" => {
					params.genres = included;
					params.hidden_genres = excluded;
				}
				// Ticked (1) hides adult novels, excluded (2) shows them, overriding the setting.
				FilterValue::Check { id, value } if id == "hide_adult" => {
					params.hide_adult = value == 1
				}
				_ => {}
			}
		}
		browse(params, page)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let novel = get::<NovelDetails>(&format!("{API_URL}/{}", manga.key))?.novel;
		if needs_chapters {
			let mut chapters = novel.chapters();
			let mirrors = mirror_chapters(&manga.key);
			if !mirrors.is_empty() {
				for chapter in &mut chapters {
					chapter.scanlators = Some(vec!["Novel Archive".into()]);
				}
				chapters.extend(mirrors);
				// Stable, so the site's own copy stays above a mirror's of the same number.
				chapters.sort_by(|a, b| {
					let number = |chapter: &Chapter| chapter.chapter_number.unwrap_or_default();
					number(b).total_cmp(&number(a))
				});
			}
			manga.chapters = Some(chapters);
		}
		if needs_details {
			let mut details = novel.into_details();
			// The details page renders Markdown; the big scroller shows the text as it is.
			details.description = details.description.map(|text| text.escape_markdown());
			manga.copy_from(details);
		}
		Ok(manga)
	}

	fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let text = match chapter.key.split_once(':') {
			Some((mirror, number)) => html_to_markdown(
				&get::<MirrorChapterText>(&format!(
					"{API_URL}/{}/sources/{mirror}/chapters/{number}",
					manga.key
				))?
				.content_html
				.unwrap_or_default(),
			)?,
			// The archive's own copies are plain text, which the site prints as is.
			None => paragraphs(
				&get::<ChapterResponse>(&format!(
					"{API_URL}/{}/chapters/{}?prefetch=1",
					manga.key, chapter.key
				))?
				.chapter
				.content
				.unwrap_or_default(),
			),
		};
		if text.is_empty() {
			bail!("This chapter has no text on Novel Archive yet");
		}
		Ok(vec![Page {
			content: PageContent::text(text),
			..Default::default()
		}])
	}
}

impl Home for NovelArchive {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut layout = HomeLayout {
			components: [
				("Trending", HomeComponentValue::empty_big_scroller()),
				("Popular", HomeComponentValue::empty_scroller()),
				(
					"Recently Updated",
					HomeComponentValue::empty_manga_chapter_list(),
				),
				("Top Rated", HomeComponentValue::empty_scroller()),
				("Editor's Choice", HomeComponentValue::empty_big_scroller()),
				("Recently Added", HomeComponentValue::empty_scroller()),
			]
			.into_iter()
			.map(|(title, value)| HomeComponent {
				title: Some(title.into()),
				subtitle: None,
				value,
			})
			.collect(),
		};
		send_partial_result(&HomePartialResult::Layout(layout.clone()));

		let sorted = |sort| {
			request(
				SearchParams {
					sort,
					..SearchParams::from_settings()
				}
				.url(1),
			)
		};
		let mut feeds: Vec<Result<Vec<Novel>>> = Request::send_all([
			request(format!("{API_URL}/trending?limit={FEED_SIZE}"))?,
			sorted(Some("popular"))?,
			request(format!("{API_URL}/recently-updated?limit={FEED_SIZE}"))?,
			sorted(Some("rating"))?,
			request(format!("{API_URL}/editors-choice?limit={FEED_SIZE}"))?,
			sorted(None)?,
		])
		.into_iter()
		.map(|response| Ok(parse::<NovelList>(response?)?.novels))
		.collect();
		if feeds.iter().all(Result::is_err)
			&& let Err(error) = feeds.swap_remove(0)
		{
			return Err(error);
		}
		let mut feeds = feeds
			.into_iter()
			.map(|feed| feed.ok().filter(|novels| !novels.is_empty()));
		let mut feed = || feeds.next().flatten();

		let big = |novels: Vec<Novel>, auto_scroll_interval| HomeComponentValue::BigScroller {
			entries: novels.into_iter().map(Novel::into_details).collect(),
			auto_scroll_interval,
		};
		let scroller = |novels: Vec<Novel>, id: &str, name: &str| HomeComponentValue::Scroller {
			entries: novels
				.into_iter()
				.map(|novel| Manga::from(novel).into())
				.collect(),
			listing: Some(Listing {
				id: id.into(),
				name: name.into(),
				..Default::default()
			}),
		};
		// Array elements are evaluated in order, so each takes its own feed.
		let values = [
			feed().map(|novels| big(novels, Some(8.0))),
			feed().map(|novels| scroller(novels, "popular", "Popular")),
			feed().map(|novels| HomeComponentValue::MangaChapterList {
				page_size: Some(5),
				entries: novels.into_iter().filter_map(Novel::into_latest).collect(),
				listing: None,
			}),
			feed().map(|novels| scroller(novels, "rating", "Top Rated")),
			// The feed carries each pick's description and genres.
			feed().map(|novels| big(novels, None)),
			feed().map(|novels| scroller(novels, "recent", "Recently Added")),
		];
		layout.components = layout
			.components
			.into_iter()
			.zip(values)
			.filter_map(|(component, value)| {
				Some(HomeComponent {
					value: value?,
					..component
				})
			})
			.collect();
		Ok(layout)
	}
}

impl ListingProvider for NovelArchive {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"popular" => Some("popular"),
			"rating" => Some("rating"),
			"recent" => None,
			_ => bail!("Unknown listing"),
		};
		browse(
			SearchParams {
				sort,
				..SearchParams::from_settings()
			},
			page,
		)
	}
}

impl DynamicFilters for NovelArchive {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut responses = Request::send_all([
			request(format!("{API_URL}/genres"))?,
			request(format!("{API_URL}/source-list"))?,
		])
		.into_iter();
		let (Some(genres), Some(sources)) = (responses.next(), responses.next()) else {
			bail!("Novel Archive's filters didn't load");
		};
		let (options, ids) = listed_genres(parse(genres?)?)
			.map(|genre| (genre.label.into(), genre.value.into()))
			.unzip();
		let (mut source_options, mut source_ids): (Vec<_>, Vec<_>) = parse::<MirrorList>(sources?)?
			.sources
			.into_iter()
			.map(|source| (source.name().into(), source.id.into()))
			.unzip();
		source_options.insert(0, "All".into());
		source_ids.insert(0, "all".into());
		Ok(vec![
			// Where the archive's copy of a novel came from, as the site's library filters it.
			SelectFilter {
				id: "source".into(),
				title: Some("Source".into()),
				options: source_options,
				ids: Some(source_ids),
				..Default::default()
			}
			.into(),
			MultiSelectFilter {
				id: "genre".into(),
				title: Some("Genres".into()),
				is_genre: true,
				can_exclude: true,
				uses_tag_style: true,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}
}

impl DynamicSettings for NovelArchive {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		let (values, titles): (Vec<_>, Vec<_>) = listed_genres(get(&format!("{API_URL}/genres"))?)
			.map(|genre| (genre.value.into(), genre.label.into()))
			.unzip();
		Ok(vec![
			GroupSetting {
				title: "Genres".into(),
				footer: Some(
					"Applies to search, listings and the sorted Discover rows. A genre a search \
					 includes itself overrides it."
						.into(),
				),
				items: vec![
					MultiSelectSetting {
						key: "hiddenGenres".into(),
						title: "Hidden Genres".into(),
						values,
						titles: Some(titles),
						refreshes: Some(vec!["content".into(), "listings".into()]),
						..Default::default()
					}
					.into(),
				],
				..Default::default()
			}
			.into(),
		])
	}
}

impl DeepLinkHandler for NovelArchive {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(rest) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		let rest = rest.split('#').next().unwrap_or_default();
		let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
		let param = |name: &str| {
			query
				.split('&')
				.find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
				.filter(|value| !value.is_empty())
		};
		Ok(match path {
			"/novel" => param("id").map(|id| DeepLinkResult::Manga { key: id.into() }),
			"/reader" => param("novel")
				.zip(param("chapter"))
				.map(|(novel, chapter)| DeepLinkResult::Chapter {
					manga_key: novel.into(),
					key: match param("source") {
						Some(mirror) => format!("{mirror}:{chapter}"),
						None => chapter.into(),
					},
				}),
			_ => None,
		})
	}
}

register_source!(
	NovelArchive,
	Home,
	ListingProvider,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler
);
