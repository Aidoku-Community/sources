#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, Listing,
	ListingProvider, Manga, MangaPageResult, Page, PageContent, Result, SelectFilter, Source,
	alloc::{String, Vec, borrow::Cow, vec},
	imports::{net::Request, std::send_partial_result},
	prelude::*,
};

mod helpers;
mod models;
#[cfg(test)]
mod test;
use helpers::*;
use models::*;

const BASE_URL: &str = "https://soraraw.com";
const THUMBNAIL_URL: &str = "https://i.mangaraw.lat";
const IMAGE_API_URL: &str = "https://api.mangarawgo.site";
const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss.SSSXXX";
const PAYLOAD_KEY: &[u8] = b"/fuCkYou!!!";
const PATH_SECRET: &[u8] = b"202508055d0db38bae2e86cc41649f90";
// a strip holds a handful of images at most, a scanned chapter one per page, so this keeps the
// request that measures them off the common case
const STRIP_IMAGE_LIMIT: usize = 4;
// the texture size the gpu takes
const MAX_DRAWABLE_HEIGHT: u32 = 16384;
// enough to reach the header unless the file leads with a large colour profile
const HEADER_BYTES: usize = 16 * 1024;
// the site lists over 1800 genres, sorted by how many series they hold
const GENRE_LIMIT: usize = 100;
const SEARCH_RESULT_LIMIT: usize = 50;
// the dump held 13 pages at the time of writing and ends with a 404; this only guards against a
// host that stops answering with one
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
		let mut author = None;
		let mut genre = None;
		for filter in filters {
			match filter {
				// the text field "supportsAuthorSearch" adds to the search filters, which the app
				// fills in on its own rather than folding the name into the query
				FilterValue::Text { id, value } if id == "author" && !value.is_empty() => {
					author = Some(value)
				}
				FilterValue::Select { id, value } if id == "genre" && !value.is_empty() => {
					genre = Some(value)
				}
				_ => {}
			}
		}

		// searching walks the whole catalogue, so it can't be combined with the genre filter;
		// "hidesFiltersWhileSearching" in "source.json" keeps the app from offering them together
		let (query, author) = (query.as_deref(), author.as_deref());
		if query.is_some() || author.is_some() {
			// every match is collected in one go, leaving no page for the app to ask for
			return Ok(MangaPageResult {
				entries: Self::search_catalogue(query, author)?,
				has_next_page: false,
			});
		}

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
			manga.viewer = details.viewer();
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

		// the page list holds the paths of the images encrypted, and the key to them lives on the
		// chapter page rather than alongside the list
		let Some(url) = chapter.url.as_deref() else {
			bail!("no url to read the image key of chapter {chapter_id} from");
		};
		let Some(details) = next_data::<DataProps<ChapterData>>(url)?.data.chapter else {
			bail!("no chapter data at {url}");
		};
		let (Some(uuid), Some(host)) = (details.uuid, details.base) else {
			bail!("chapter {chapter_id} hands out no image key");
		};

		let payload = Request::get(format!("{IMAGE_API_URL}/{manga_id}/{chapter_id}.json"))?
			.json_owned::<ImagePayload>()?;
		let Some(json) = deobfuscate(&payload.d, PAYLOAD_KEY) else {
			bail!("could not decode the page list of chapter {chapter_id}");
		};
		let images = serde_json::from_str::<Vec<PageImage>>(&json)
			.map_err(|error| error!("unexpected page list for chapter {chapter_id}: {error}"))?;

		let mut urls = Vec::with_capacity(images.len());
		for image in images {
			// a page that can't be placed or decrypted is not skipped: the chapter would read as
			// complete while missing a page, which nothing downstream could tell apart
			let (Some(order), Some(path)) = (
				image.order.as_f32(),
				decrypt_path(&image.b, &uuid, PATH_SECRET),
			) else {
				bail!("could not read a page of chapter {chapter_id}");
			};
			urls.push((order, format!("{host}/{path}")));
		}
		if urls.is_empty() {
			// an empty list is indistinguishable from a failed request once it reaches the app
			bail!("no pages returned for chapter {chapter_id}");
		}
		// the endpoint returns them in order, but the site sorts them anyway before reading. some
		// chapters number an inserted page as a fraction, so the order can't be taken as an integer
		urls.sort_by(|(left, _), (right, _)| left.total_cmp(right));

		// a chapter holding few enough images to be a strip gets measured before it's handed over,
		// so one too tall to draw fails with a reason instead of turning up blank
		if urls.len() <= STRIP_IMAGE_LIMIT {
			for (_, url) in &urls {
				check_drawable(url)?;
			}
		}

		Ok(urls
			.into_iter()
			.map(|(_, url)| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl Soraraw {
	// nothing on the site can be queried: "/search?q=" is statically generated and renders a fixed
	// batch, the api host answers 500 for every query (`Unknown column 'Manga.number_views' in
	// 'ORDER BY'`), and its "/mangas" ignores every parameter tried. that leaves walking the dump
	// the browser filters itself, 13 pages of 2000 entries and about 4.7 MB gzipped
	fn search_catalogue(query: Option<&str>, author: Option<&str>) -> Result<Vec<Manga>> {
		let mut entries = Vec::new();

		for page in 1..=CATALOGUE_PAGE_LIMIT {
			let response = Request::get(format!("{BASE_URL}/mangas_{page}.json"))?.send()?;
			// the dump ends with a 404, which is how the site's own search stops walking it
			if response.status_code() != 200 {
				// the first page is the exception: with nothing walked yet, a dump that can't be
				// reached hands back the empty result of a query that matched nothing
				if page == 1 {
					bail!(
						"the catalogue is unreachable: page 1 answered {}",
						response.status_code()
					);
				}
				break;
			}
			let Ok(catalogue) = response.get_json_owned::<CataloguePage>() else {
				// a page that stopped being json leaves the matches collected before it worth
				// returning, except on the first page, which leaves none
				if page == 1 {
					bail!("could not read catalogue page 1");
				}
				break;
			};

			for entry in catalogue.list {
				let matched = query.is_none_or(|query| entry.matches(query))
					&& author.is_none_or(|author| entry.matches_author(author));
				if !matched {
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
