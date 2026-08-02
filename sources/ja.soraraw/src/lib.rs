#![no_std]
use aidoku::{
	AidokuError, Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue,
	Listing, ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, SelectFilter,
	Source,
	alloc::{String, Vec, borrow::Cow, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod models;
use helpers::*;
use models::*;

const BASE_URL: &str = "https://soraraw.com";
/// Host the cover images are served from.
const THUMBNAIL_URL: &str = "https://i.mangaraw.lat";
/// Endpoint holding the page list of a chapter, which no page of the site embeds.
const IMAGE_API_URL: &str = "https://api.mangarawgo.site";
/// Page images are spread over four subdomains of this host.
const IMAGE_HOST: &str = "rawcontent.top";
const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss.SSSXXX";
/// Key the site xors the payload of the image endpoint with.
const PAYLOAD_KEY: &[u8] = b"/fuCkYou!!!";
/// How many genres to offer as filter options. The site lists over 1800 of them, most holding a
/// handful of entries, and hands them out sorted by how many series they hold.
const GENRE_LIMIT: usize = 100;
/// How many matches to collect before leaving the rest of the catalogue alone.
const SEARCH_RESULT_LIMIT: usize = 50;
/// Upper bound on the catalogue pages a search walks. The dump held 13 at the time of writing and
/// ends with a 404; this only guards against a host that stops answering with one.
const CATALOGUE_PAGE_LIMIT: i32 = 40;

struct Soraraw;

impl Source for Soraraw {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		// searching walks the whole catalogue, so it can't be combined with the other filters;
		// "hidesFiltersWhileSearching" in "source.json" keeps the app from offering them together.
		// an empty query is filtered out rather than trusted: it would match every entry and walk
		// the catalogue for nothing
		if let Some(query) = query
			.as_deref()
			.map(str::trim)
			.filter(|query| !query.is_empty())
		{
			// every match is collected in one go, leaving no page for the app to ask for
			if page > 1 {
				return Ok(MangaPageResult {
					entries: Vec::new(),
					has_next_page: false,
				});
			}

			return Ok(MangaPageResult {
				entries: Self::search_catalogue(query)?,
				has_next_page: false,
			});
		}

		let genre = filters.into_iter().find_map(|filter| match filter {
			FilterValue::Select { id, value } if id == "genre" && !value.is_empty() => Some(value),
			_ => None,
		});

		let url = match genre {
			Some(genre) => paginated(&format!("{BASE_URL}/genre/{genre}"), page),
			None => paginated(&format!("{BASE_URL}/newest"), page),
		};
		Self::parse_list(&url)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = manga_url(&manga.key);
		let Some(details) = next_data::<DataProps<MangaData>>(&url)?.data.manga else {
			bail!("no details for manga {}", manga.key);
		};

		if needs_details {
			manga.title = details.name.trim().into();
			manga.cover = details.cover();
			manga.authors = details.authors();
			manga.description = details.description();
			manga.url = Some(url);
			manga.status = status(details.kind.as_deref());
			manga.viewer = viewer(details.mode.as_deref());
			manga.content_rating = content_rating(details.is_adult.as_deref());

			let tags = details
				.genres
				.into_iter()
				.filter_map(Genre::into_tag)
				.collect::<Vec<String>>();
			manga.tags = (!tags.is_empty()).then_some(tags);

			if needs_chapters {
				// the chapter list is parsed out of the same response, but every entry costs a
				// date to parse, so the details are handed over before that starts
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let manga_id = details.id;
			let slug = details.slug;
			let chapters = details
				.chapters
				.into_iter()
				.map(|chapter| chapter.into_chapter(manga_id, &slug))
				.collect::<Vec<Chapter>>();
			if chapters.is_empty() {
				// surfaces in `aidoku logcat` when a series is listed without a readable chapter
				println!("no chapters returned for manga {}", manga.key);
			}
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let Some((manga_id, chapter_id)) = chapter.key.split_once('/') else {
			bail!("malformed chapter key {}", chapter.key);
		};
		let Ok(chapter_id) = chapter_id.parse::<i64>() else {
			bail!("malformed chapter key {}", chapter.key);
		};

		let payload = Request::get(format!("{IMAGE_API_URL}/{manga_id}/{chapter_id}.json"))?
			.json_owned::<ImagePayload>()?;
		let Some(json) = deobfuscate(&payload.d, PAYLOAD_KEY) else {
			bail!("could not decode the page list of chapter {chapter_id}");
		};
		let mut images = serde_json::from_str::<Vec<PageImage>>(&json).map_err(|error| {
			AidokuError::Message(format!(
				"unexpected page list for chapter {chapter_id}: {error}"
			))
		})?;
		// the endpoint returns them in order, but the site sorts them anyway before reading
		images.sort_by_key(PageImage::order);

		let names = images
			.iter()
			.filter_map(PageImage::file_name)
			.collect::<Vec<String>>();
		let Some(first_page) = names.first() else {
			// an empty list is indistinguishable from a failed request once it reaches the app
			bail!("no pages returned for chapter {chapter_id}");
		};

		// the four hosts serve the same images, and the one a chapter is on follows from its id
		let host = format!("https://lh{}.{IMAGE_HOST}", chapter_id % 4 + 1);
		let extension = image_extension(&host, chapter_id, first_page);

		Ok(names
			.iter()
			.map(|name| Page {
				content: PageContent::url(format!("{host}/c{chapter_id}/{name}.{extension}")),
				..Default::default()
			})
			.collect())
	}
}

impl Soraraw {
	/// Searches the catalogue dump the site publishes, because nothing else on it can be queried.
	///
	/// "/search?q=" renders a fixed batch that ignores the query entirely — the page it serves is
	/// statically generated, and the browser filters a catalogue it downloads itself. The api host
	/// the site is configured with does expose "/search", but it answers 500 for every query
	/// (`Unknown column 'Manga.number_views' in 'ORDER BY'`), and its "/mangas" endpoint ignores
	/// every query parameter it was tried with. That leaves walking the same dump, which is 13
	/// pages of 2000 entries and around 4.7 MB over the wire with the gzip the host serves.
	fn search_catalogue(query: &str) -> Result<Vec<Manga>> {
		let mut entries = Vec::new();

		for page in 1..=CATALOGUE_PAGE_LIMIT {
			let response = Request::get(format!("{BASE_URL}/mangas_{page}.json"))?.send()?;
			// the dump ends with a 404, which is how the site's own search stops walking it
			if response.status_code() != 200 {
				break;
			}
			let Ok(catalogue) = response.get_json_owned::<CataloguePage>() else {
				// a page that stopped being json is worth reporting, but the matches already
				// collected are still worth returning
				println!("could not read catalogue page {page}");
				break;
			};

			for entry in catalogue.list {
				if !entry.matches(query) {
					continue;
				}
				entries.push(Manga::from(entry));
				if entries.len() >= SEARCH_RESULT_LIMIT {
					return Ok(entries);
				}
			}
		}

		Ok(entries)
	}

	fn parse_list(url: &str) -> Result<MangaPageResult> {
		let data = next_data::<DataProps<ListData>>(url)?.data;
		Ok(MangaPageResult {
			has_next_page: data
				.pagination
				.is_some_and(|pagination| pagination.has_next_page()),
			entries: data.results.into_iter().map(Manga::from).collect(),
		})
	}
}

impl ListingProvider for Soraraw {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			// both lists are embedded in the home page as a single batch, with no page to follow
			"hot" | "trending" => {
				if page > 1 {
					return Ok(MangaPageResult {
						entries: Vec::new(),
						has_next_page: false,
					});
				}

				let props = next_data::<HomeProps>(BASE_URL)?;
				let entries = if listing.id == "hot" {
					props.data.hot
				} else {
					props
						.initial_trending
						.map(|trending| trending.mangas)
						.unwrap_or_default()
				};

				Ok(MangaPageResult {
					entries: entries.into_iter().map(Manga::from).collect(),
					has_next_page: false,
				})
			}
			_ => Self::parse_list(&paginated(&format!("{BASE_URL}/newest"), page)),
		}
	}
}

impl DynamicFilters for Soraraw {
	// the genre list is fetched instead of hardcoded, so new genres are picked up automatically
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let genres =
			Request::get(format!("{BASE_URL}/genres.json"))?.json_owned::<Vec<GenreEntry>>()?;

		let mut options: Vec<Cow<'static, str>> = vec![Cow::Borrowed("All")];
		let mut ids: Vec<Cow<'static, str>> = vec![Cow::Borrowed("")];
		for genre in genres.into_iter().take(GENRE_LIMIT) {
			if genre.slug.is_empty() || genre.name.trim().is_empty() {
				continue;
			}
			options.push(String::from(genre.name.trim()).into());
			ids.push(genre.slug.into());
		}

		Ok(vec![
			SelectFilter {
				id: "genre".into(),
				title: Some("Genre".into()),
				is_genre: true,
				options,
				ids: Some(ids),
				..Default::default()
			}
			.into(),
		])
	}
}

impl DeepLinkHandler for Soraraw {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};
		// shared links can carry a query string or a fragment
		let path = path.split(['?', '#']).next().unwrap_or_default();
		let segments = path
			.split('/')
			.filter(|segment| !segment.is_empty())
			.collect::<Vec<&str>>();

		Ok(match segments.as_slice() {
			// https://soraraw.com/manga/majo-to-youhei-57539
			["manga", slug] => Some(DeepLinkResult::Manga {
				key: String::from(*slug),
			}),
			// https://soraraw.com/manga/majo-to-youhei-57539/ch-74-2
			//
			// chapter keys hold ids that the url doesn't, so the page has to be read to build one
			["manga", slug, _] => {
				let data = next_data::<DataProps<ChapterData>>(&format!("{BASE_URL}{path}"))?;
				data.data.chapter.map(|chapter| DeepLinkResult::Chapter {
					manga_key: String::from(*slug),
					key: chapter_key(chapter.manga_id, chapter.id),
				})
			}
			_ => None,
		})
	}
}

register_source!(Soraraw, ListingProvider, DynamicFilters, DeepLinkHandler);

#[cfg(test)]
mod test {
	use super::*;
	use aidoku::{ContentRating, FilterKind, MangaStatus, Viewer};
	use aidoku_test::aidoku_test;

	/// "Majo to Youhei", a long running series used to check parsing against.
	const MANGA_KEY: &str = "majo-to-youhei-57539";
	/// "Hard Worker Nakata", the stable entry with a vertical reading direction and an adult flag.
	const WEBTOON_KEY: &str = "haadowaakaa-nakata-740";
	/// "Tonari no Kurokawa-san", which holds one of the chapters stored as jpg.
	const JPG_MANGA_KEY: &str = "my-neighbor-ms-kurokawa-tonari-no-kurokawa-san-1";
	const JPG_CHAPTER_KEY: &str = "1/786104";
	/// "Kobayashi-san Chi no Maid Dragon", used to check that searching finds a series by both its
	/// japanese and its english title.
	const SEARCHED_KEY: &str = "kobayashi-san-chino-meidoragon-57605";

	fn listing(id: &str) -> Listing {
		Listing {
			id: String::from(id),
			..Default::default()
		}
	}

	fn resolves(url: &str) -> bool {
		Request::head(url)
			.and_then(|request| request.send())
			.map(|response| response.status_code() == 200)
			.unwrap_or(false)
	}

	fn page_urls(pages: &[Page]) -> Vec<&String> {
		pages
			.iter()
			.map(|page| {
				let PageContent::Url(url, _) = &page.content else {
					panic!("expected a page url");
				};
				url
			})
			.collect()
	}

	#[aidoku_test]
	fn test_listings() {
		for id in ["newest", "hot", "trending"] {
			let result = Soraraw.get_manga_list(listing(id), 1).expect("listing");
			assert!(!result.entries.is_empty(), "{id} returned no entries");

			let entry = &result.entries[0];
			assert!(!entry.key.is_empty());
			assert!(!entry.title.is_empty());
			assert!(
				entry
					.cover
					.as_ref()
					.is_some_and(|cover| cover.starts_with("http")),
				"{id} entry has no absolute cover"
			);
			// listings carry enough to fill these in, and leaving them for the details request
			// would show the wrong reading direction until a series is opened
			assert_ne!(entry.viewer, Viewer::Unknown, "{id} entry has no viewer");
			assert_ne!(
				entry.content_rating,
				ContentRating::Unknown,
				"{id} entry has no content rating"
			);
		}
	}

	// only the paginated listing walks pages; the other two hand out a single batch
	#[aidoku_test]
	fn test_listing_pagination() {
		let first = Soraraw
			.get_manga_list(listing("newest"), 1)
			.expect("page 1");
		assert!(first.has_next_page);

		let second = Soraraw
			.get_manga_list(listing("newest"), 2)
			.expect("page 2");
		assert!(!second.entries.is_empty());
		assert_ne!(first.entries[0].key, second.entries[0].key);

		let hot = Soraraw.get_manga_list(listing("hot"), 1).expect("hot");
		assert!(!hot.has_next_page);
	}

	// searching walks the catalogue dump; the series below is the one that showed the site's own
	// "/search" page answering with the same fixed batch no matter what it is asked for
	#[aidoku_test]
	fn test_search() {
		let result = Soraraw
			.get_search_manga_list(
				Some(String::from("小林さんちのメイドラゴン")),
				1,
				Vec::new(),
			)
			.expect("search");
		assert!(
			result.entries.iter().any(|manga| manga.key == SEARCHED_KEY),
			"{:?}",
			result
				.entries
				.iter()
				.map(|it| &it.title)
				.collect::<Vec<_>>()
		);
		// every match is returned at once, so there is no page to follow
		assert!(!result.has_next_page);

		let empty = Soraraw
			.get_search_manga_list(
				Some(String::from("小林さんちのメイドラゴン")),
				2,
				Vec::new(),
			)
			.expect("second search page");
		assert!(empty.entries.is_empty());
	}

	// about half the catalogue carries an author, which the same walk matches against; this is
	// what "supportsAuthorSearch" in "source.json" tells the app it can do
	#[aidoku_test]
	fn test_search_by_author() {
		let result = Soraraw
			.get_search_manga_list(Some(String::from("伊藤京介")), 1, Vec::new())
			.expect("author search");
		assert!(
			result
				.entries
				.iter()
				.any(|manga| manga.key == JPG_MANGA_KEY),
			"{:?}",
			result
				.entries
				.iter()
				.map(|it| &it.title)
				.collect::<Vec<_>>()
		);
	}

	// alternative titles are romanised or english, and have to match regardless of case
	#[aidoku_test]
	fn test_search_alternative_title() {
		let result = Soraraw
			.get_search_manga_list(Some(String::from("miss kobayashi")), 1, Vec::new())
			.expect("search");
		let entry = result
			.entries
			.iter()
			.find(|manga| manga.key == SEARCHED_KEY)
			.expect("the series matched by its english title");
		assert_eq!(entry.title, "小林さんちのメイドラゴン");
		assert!(
			entry
				.cover
				.as_ref()
				.is_some_and(|cover| cover.starts_with("http")),
			"catalogue entries name their cover field differently, so it is easy to lose"
		);
		assert_eq!(entry.content_rating, ContentRating::NSFW);
		assert_eq!(entry.viewer, Viewer::RightToLeft);
	}

	#[aidoku_test]
	fn test_genre_filter() {
		let filters = vec![FilterValue::Select {
			id: String::from("genre"),
			value: String::from("akushon"),
		}];
		let result = Soraraw
			.get_search_manga_list(None, 1, filters)
			.expect("filtered list");
		assert!(!result.entries.is_empty());
		assert!(result.has_next_page);

		// an empty selection is the "All" option, which has to fall back to the plain listing
		let cleared = vec![FilterValue::Select {
			id: String::from("genre"),
			value: String::new(),
		}];
		let result = Soraraw
			.get_search_manga_list(None, 1, cleared)
			.expect("cleared filter");
		assert!(!result.entries.is_empty());
	}

	#[aidoku_test]
	fn test_dynamic_filters() {
		let filters = Soraraw.get_dynamic_filters().expect("dynamic filters");
		assert_eq!(filters.len(), 1);

		let FilterKind::Select { options, ids, .. } = &filters[0].kind else {
			panic!("expected a select filter");
		};
		// the site listed over 1800 genres at the time of writing, so the cap is what decides the
		// count; the lower bound only guards against the list coming back empty or broken
		assert!(options.len() > 1, "got {} options", options.len());
		assert!(options.len() <= GENRE_LIMIT + 1, "the cap is not applied");
		let ids = ids.as_ref().expect("genre ids");
		assert_eq!(ids.len(), options.len());
		// the first option clears the filter, every other one has to name a genre
		assert!(ids[0].is_empty());
		assert!(ids[1..].iter().all(|id| !id.is_empty()));
	}

	#[aidoku_test]
	fn test_manga_details() {
		let manga = Manga {
			key: String::from(MANGA_KEY),
			..Default::default()
		};
		let manga = Soraraw
			.get_manga_update(manga, true, true)
			.expect("manga details");

		assert_eq!(manga.title, "魔女と傭兵");
		assert_eq!(
			manga.url.as_deref(),
			Some("https://soraraw.com/manga/majo-to-youhei-57539")
		);
		assert!(manga.cover.is_some_and(|cover| cover.starts_with("http")));
		// the author field holds several names separated by commas
		assert!(manga.authors.is_some_and(|authors| authors.len() > 1));
		assert!(manga.tags.is_some_and(|tags| !tags.is_empty()));
		assert_eq!(manga.status, MangaStatus::Ongoing);
		assert_eq!(manga.viewer, Viewer::RightToLeft);
		assert_eq!(manga.content_rating, ContentRating::Safe);
		// the synopsis is stored as an editor document, which has to come out as plain text
		let description = manga.description.expect("description");
		assert!(!description.contains('<'), "{description}");
		assert!(!description.contains("blocks"), "{description}");

		let chapters = manga.chapters.expect("chapters");
		assert!(chapters.len() > 100, "got {} chapters", chapters.len());
		let chapter = &chapters[0];
		assert!(chapter.key.contains('/'));
		assert!(chapter.chapter_number.is_some());
		assert!(chapter.url.as_deref().is_some_and(|url| {
			url.starts_with("https://soraraw.com/manga/majo-to-youhei-57539/ch-")
		}));
		// language stays unset so the app's chapter language filter can't hide these
		assert_eq!(chapter.language, None);
		// decimal chapters exist and have to keep their number
		assert!(
			chapters
				.iter()
				.any(|chapter| chapter.chapter_number.is_some_and(|it| it.fract() != 0.0))
		);
		// date_uploaded isn't checked here: the test runner doesn't implement the quoting,
		// fractional seconds or ISO 8601 zones that DATE_FORMAT relies on, so it only ever
		// parses on device
	}

	// webtoons have to open in a vertical viewer, and the adult flag has to reach the app
	#[aidoku_test]
	fn test_webtoon_details() {
		let manga = Manga {
			key: String::from(WEBTOON_KEY),
			..Default::default()
		};
		let manga = Soraraw
			.get_manga_update(manga, true, false)
			.expect("webtoon details");

		assert_eq!(manga.viewer, Viewer::Webtoon);
		assert_eq!(manga.content_rating, ContentRating::NSFW);
	}

	#[aidoku_test]
	fn test_page_list() {
		let manga = Manga {
			key: String::from(MANGA_KEY),
			..Default::default()
		};
		let mut manga = Soraraw
			.get_manga_update(manga, false, true)
			.expect("chapters");
		// taken rather than cloned; the page request doesn't read the chapter list back
		let chapter = manga
			.chapters
			.take()
			.expect("chapters")
			.into_iter()
			.find(|chapter| chapter.chapter_number == Some(1.0))
			.expect("chapter 1");

		let pages = Soraraw.get_page_list(manga, chapter).expect("page list");
		// chapter 1 of this series holds 72 pages, and can only ever gain them
		assert!(pages.len() >= 72, "got {} pages", pages.len());

		let urls = page_urls(&pages);
		for url in &urls {
			assert!(url.starts_with("https://lh"), "{url} is not absolute");
			assert!(url.ends_with(".webp"), "{url} is not a webp");
		}
		// page numbers are built rather than handed out, so they have to stay in order and unique
		assert!(
			urls[0].contains("/001_"),
			"{} is not the first page",
			urls[0]
		);
		assert!(
			urls[1].contains("/002_"),
			"{} is not the second page",
			urls[1]
		);

		// the urls are assembled from ids, so one has to be requested to prove the shape resolves
		assert!(resolves(urls[0]), "{} did not resolve", urls[0]);
		assert!(
			resolves(urls[urls.len() - 1]),
			"{} did not resolve",
			urls[urls.len() - 1]
		);
	}

	// a few chapters are stored as jpg, which nothing in the page list gives away
	#[aidoku_test]
	fn test_jpg_chapter_pages() {
		let manga = Manga {
			key: String::from(JPG_MANGA_KEY),
			..Default::default()
		};
		let chapter = Chapter {
			key: String::from(JPG_CHAPTER_KEY),
			..Default::default()
		};
		let pages = Soraraw
			.get_page_list(manga, chapter)
			.expect("jpg page list");

		let urls = page_urls(&pages);
		assert!(!urls.is_empty());
		assert!(urls[0].ends_with(".jpg"), "{} is not a jpg", urls[0]);
		assert!(resolves(urls[0]), "{} did not resolve", urls[0]);
	}

	// a chapter numbered "74.2" has to survive the round trip into a page request
	#[aidoku_test]
	fn test_decimal_chapter_pages() {
		let manga = Manga {
			key: String::from(MANGA_KEY),
			..Default::default()
		};
		let mut manga = Soraraw
			.get_manga_update(manga, false, true)
			.expect("chapters");
		let chapter = manga
			.chapters
			.take()
			.expect("chapters")
			.into_iter()
			.find(|chapter| {
				chapter
					.chapter_number
					.is_some_and(|number| number.fract() != 0.0)
			})
			.expect("a decimal chapter");

		let pages = Soraraw
			.get_page_list(manga, chapter)
			.expect("decimal chapter pages");
		assert!(!pages.is_empty());
	}

	#[aidoku_test]
	fn test_deep_link() {
		let manga = Soraraw
			.handle_deep_link(String::from(
				"https://soraraw.com/manga/majo-to-youhei-57539",
			))
			.expect("manga deep link");
		assert_eq!(
			manga,
			Some(DeepLinkResult::Manga {
				key: String::from(MANGA_KEY)
			})
		);

		// shared links carry tracking parameters the key must not pick up
		let shared = Soraraw
			.handle_deep_link(String::from(
				"https://soraraw.com/manga/majo-to-youhei-57539?utm_source=share",
			))
			.expect("shared manga deep link");
		assert_eq!(
			shared,
			Some(DeepLinkResult::Manga {
				key: String::from(MANGA_KEY)
			})
		);

		let chapter = Soraraw
			.handle_deep_link(String::from(
				"https://soraraw.com/manga/majo-to-youhei-57539/ch-1",
			))
			.expect("chapter deep link");
		assert_eq!(
			chapter,
			Some(DeepLinkResult::Chapter {
				manga_key: String::from(MANGA_KEY),
				key: String::from("57539/508048"),
			})
		);

		let unknown = Soraraw
			.handle_deep_link(String::from("https://soraraw.com/newest"))
			.expect("unknown deep link");
		assert_eq!(unknown, None);

		let foreign = Soraraw
			.handle_deep_link(String::from(
				"https://example.com/manga/majo-to-youhei-57539",
			))
			.expect("foreign deep link");
		assert_eq!(foreign, None);
	}

	// malformed keys can reach the source from a stale library entry, and have to fail loudly
	#[aidoku_test]
	fn test_malformed_chapter_key() {
		let chapter = Chapter {
			key: String::from("not-a-key"),
			..Default::default()
		};
		assert!(Soraraw.get_page_list(Manga::default(), chapter).is_err());

		let chapter = Chapter {
			key: String::from("57539/not-a-number"),
			..Default::default()
		};
		assert!(Soraraw.get_page_list(Manga::default(), chapter).is_err());
	}

	// the payload decoding is pure, so it can be checked without touching the network
	#[aidoku_test]
	fn test_deobfuscate() {
		// "[{\"id\":1,\"order\":1}]" xored with the key and encoded, padding left off
		let payload = "dB1XKg97VUQNA05dAhAxSWNeCHw";
		let json = deobfuscate(payload, PAYLOAD_KEY).expect("decoded payload");
		let images = serde_json::from_str::<Vec<PageImage>>(&json).expect("page list");
		assert_eq!(images.len(), 1);
		assert_eq!(images[0].id, 1);
		assert_eq!(images[0].file_name().as_deref(), Some("001_1"));

		// the endpoint gives the order as a number, but strings appear in the same shape of
		// payload elsewhere on the site, so both have to survive
		let text_order =
			serde_json::from_str::<Vec<PageImage>>(r#"[{"id":2,"order":"12"}]"#).expect("text");
		assert_eq!(text_order[0].file_name().as_deref(), Some("012_2"));

		assert_eq!(deobfuscate("*", PAYLOAD_KEY), None);
		assert_eq!(decode_base64("QUJD"), Some(Vec::from(*b"ABC")));
		assert_eq!(decode_base64("QUJD="), Some(Vec::from(*b"ABC")));
		assert_eq!(decode_base64("*"), None);
	}
}
