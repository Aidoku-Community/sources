#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, DynamicFilters, Filter, FilterValue, ImageResponse,
	Listing, ListingProvider, Manga, MangaPageResult, MultiSelectFilter, Page, PageContent,
	PageContext, PageImageProcessor, Result, Source,
	alloc::{String, Vec, borrow::Cow, string::ToString, vec},
	canvas::{Rect, Transform},
	imports::{
		canvas::{Canvas, ImageRef},
		std::{current_date, send_partial_result},
	},
	prelude::*,
};
use core::cell::RefCell;

mod catalogue;
mod helpers;
mod home;
mod models;
#[cfg(test)]
mod test;

use catalogue::*;
use helpers::*;
use models::*;

const BASE_URL: &str = "https://soraraw.com";
const THUMBNAIL_URL: &str = "https://i.mangaraw.lat";
const IMAGE_API_URL: &str = "https://api.mangarawgo.site";
// both hosts answer 403 to a user agent containing "aidoku", such as the test runner's default
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 26_6 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148 Version/26.6 Safari/605.1.15";
const DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss.SSSXXX";

const PAYLOAD_KEY: &[u8] = b"/fuCkYou!!!";
const PATH_SECRET: &[u8] = b"202508055d0db38bae2e86cc41649f90";
const SCRAMBLE_SECRET: &str = "6a0248ad1ca4208275aed64d336e81595ecb149422a8e621f70e23b9f01b9c1c";
const SCRAMBLE_GRID: u32 = 8;

// enough to reach the header unless the file leads with a large colour profile
const HEADER_BYTES: usize = 16 * 1024;
// a strip holds a handful of images at most, a scanned chapter one per page, so this keeps the
// request that measures them off the common case
const STRIP_IMAGE_LIMIT: usize = 4;
// the tallest image seen is 49152 and the narrowest 800, which divide into 43 pages. a count past
// this comes from a misread header rather than an image that deep, and slicing on it would hand
// the reader hundreds of slivers
const STACKED_PAGE_LIMIT: u32 = 64;
// the site lists over 1800 genres, sorted by how many series they hold
const GENRE_LIMIT: usize = 100;

#[derive(Default)]
struct Soraraw {
	catalogue: RefCell<Option<Catalogue>>,
}

impl Source for Soraraw {
	fn new() -> Self {
		Self::default()
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let mut author = None;
		let mut content = None;
		let mut status = None;
		let mut selected_genre = None;
		let mut included = Vec::new();
		let mut excluded = Vec::new();
		for filter in filters {
			match filter {
				FilterValue::Text { id, value } if id == "author" && !value.is_empty() => {
					author = Some(value)
				}
				FilterValue::Select { id, value } if !value.is_empty() => match id.as_str() {
					"content" => content = Some(value),
					"status" => status = Some(value),
					"genre" => selected_genre = Some(value),
					_ => bail!("invalid select filter id: `{id}`"),
				},
				FilterValue::MultiSelect {
					id,
					included: selected,
					excluded: rejected,
				} if id == "genre-id" => {
					included.extend(selected);
					excluded.extend(rejected);
				}
				_ => {}
			}
		}

		let parse_id = |value: String| {
			value
				.parse::<i64>()
				.map_err(|_| error!("invalid genre id: `{value}`"))
		};
		let mut included = included
			.into_iter()
			.map(&parse_id)
			.collect::<Result<Vec<_>>>()?;
		let excluded = excluded
			.into_iter()
			.map(&parse_id)
			.collect::<Result<Vec<_>>>()?;
		if let Some(name) = selected_genre {
			// selecting tag from series page only gives the name, so the id must be fetched
			let genres =
				request(format!("{BASE_URL}/genres.json"))?.json_owned::<Vec<GenreEntry>>()?;
			let id = genres
				.iter()
				.find(|genre| genre.name == name || genre.slug == name)
				.map(|genre| genre.id)
				.ok_or_else(|| error!("unknown genre: {name}"))?;
			included.push(id);
		}
		let params = SearchParams {
			query: query.as_deref(),
			author: author.as_deref(),
			content: content.as_deref(),
			status: status.as_deref(),
			included: &included,
			excluded: &excluded,
			page,
		};
		self.search_catalogue(params)
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
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			let chapters = details
				.chapters
				.into_iter()
				.map(|chapter| chapter.into_chapter(details.id, &details.slug))
				.collect::<Vec<Chapter>>();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let Some((manga_id, chapter_id)) = chapter.key.split_once('/') else {
			bail!("malformed chapter key {}", chapter.key);
		};
		let chapter_id = chapter_id
			.parse::<i64>()
			.map_err(|_| error!("malformed chapter key {}", chapter.key))?;

		// the page list holds the paths of the images encrypted, and the key to them lives on the
		// chapter page rather than alongside the list
		let details = {
			let url = chapter.url.as_deref().ok_or_else(|| {
				error!("no url to read the image key of chapter {chapter_id} from")
			})?;
			next_data::<DataProps<ChapterData>>(url)?
				.data
				.chapter
				.ok_or_else(|| error!("no chapter data at {url}"))?
		};
		let (Some(uuid), Some(host)) = (details.uuid, details.base) else {
			bail!("chapter {chapter_id} hands out no image key");
		};
		let scrambled = details.mode.as_deref() == Some("canva2");

		let images = {
			let payload = request(format!("{IMAGE_API_URL}/{manga_id}/{chapter_id}.json"))?
				.json_owned::<ImagePayload>()?;
			let json = deobfuscate(&payload.d, PAYLOAD_KEY)
				.ok_or_else(|| error!("could not decode the page list of chapter {chapter_id}"))?;
			serde_json::from_str::<Vec<PageImage>>(&json)
				.map_err(|error| error!("unexpected page list for chapter {chapter_id}: {error}"))?
		};

		let mut urls = Vec::with_capacity(images.len());
		for image in images {
			let (Some(order), Some(path)) = (
				image.order.as_f32(),
				decrypt_path(&image.b, &uuid, PATH_SECRET),
			) else {
				bail!("could not read a page of chapter {chapter_id}");
			};
			urls.push((order, format!("{host}/{path}")));
		}
		if urls.is_empty() {
			bail!("no pages returned for chapter {chapter_id}");
		}
		// the endpoint returns them in order, but the site sorts them anyway before reading. some
		// chapters number an inserted page as a fraction, so the order can't be taken as an integer
		urls.sort_by(|(left, _), (right, _)| left.total_cmp(right));

		let possibly_uses_strip_images = urls.len() <= STRIP_IMAGE_LIMIT;
		let mut pages = Vec::with_capacity(urls.len());
		for (_, url) in urls {
			// the tile plan covers the whole image, so these can't be sliced
			if scrambled {
				let mut context = PageContext::new();
				context.insert(String::from("seed"), chapter_id.to_string());
				pages.push(Page {
					content: PageContent::url_context(url, context),
					..Default::default()
				});
				continue;
			}
			let slice_count = if possibly_uses_strip_images {
				stacked_page_count(&url)
			} else {
				1
			};
			if slice_count == 1 {
				pages.push(Page {
					content: PageContent::url(url),
					..Default::default()
				});
				continue;
			}
			for slice in 0..slice_count {
				let mut context = PageContext::new();
				context.insert(String::from("slice"), slice.to_string());
				context.insert(String::from("slices"), slice_count.to_string());
				pages.push(Page {
					content: PageContent::url_context(&url, context),
					..Default::default()
				});
			}
		}

		Ok(pages)
	}
}

impl Soraraw {
	fn search_catalogue(&self, params: SearchParams<'_>) -> Result<MangaPageResult> {
		let now = current_date();
		let needs_refresh = self
			.catalogue
			.borrow()
			.as_ref()
			.is_none_or(|catalogue| !catalogue.is_fresh(now));
		if needs_refresh {
			*self.catalogue.borrow_mut() = Some(Catalogue::fetch()?);
		}

		let catalogue = self.catalogue.borrow();
		Ok(catalogue
			.as_ref()
			.expect("catalogue was loaded")
			.filter(params))
	}

	fn parse_list(url: &str) -> Result<MangaPageResult> {
		next_data::<DataProps<ListData>>(url).map(|d| d.data.into())
	}

	fn parse_top(period: &str) -> Result<MangaPageResult> {
		let top = request(format!("{BASE_URL}/top/{period}.json"))?.json_owned::<TopList>()?;
		if top.mangas.is_empty() {
			bail!("the {period} ranking came back empty");
		}
		Ok(MangaPageResult {
			entries: top.mangas.into_iter().map(Manga::from).collect(),
			has_next_page: false,
		})
	}

	fn unscramble(image: &ImageRef, seed: &str) -> Option<ImageRef> {
		let (width, height) = (image.width(), image.height());
		let tiles = scramble_plan(width as u32, height as u32, seed)?;

		// only a bare rotation is set: the app applies a translation in a different order than the
		// sdk splits it out for, so tiles are placed through the dst rect instead
		let mut canvas = Canvas::new(width, height);
		for turns in 0..4 {
			match turns {
				1 => canvas.set_transform(&Transform::new(0.0, -1.0, 1.0, 0.0, 0.0, 0.0)),
				2 => canvas.set_transform(&Transform::new(-1.0, 0.0, 0.0, -1.0, 0.0, 0.0)),
				3 => canvas.set_transform(&Transform::new(0.0, 1.0, -1.0, 0.0, 0.0, 0.0)),
				_ => {}
			}
			for tile in tiles.iter().filter(|tile| tile.turns == turns) {
				let source = &tiles[tile.source];
				let (x, y) = (tile.x as f32, tile.y as f32);
				let (width, height) = (tile.width as f32, tile.height as f32);
				let (dst_x, dst_y) = match turns {
					1 => (-(y + height), x),
					2 => (-(x + width), -(y + height)),
					3 => (y, -(x + width)),
					_ => (x, y),
				};
				canvas.copy_image(
					image,
					Rect::new(source.x as f32, source.y as f32, width, height),
					Rect::new(dst_x, dst_y, width, height),
				);
			}
		}
		Some(canvas.get_image())
	}
}

impl PageImageProcessor for Soraraw {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let Some(context) = context else {
			return Ok(response.image);
		};
		if let Some(seed) = context.get("seed") {
			return Ok(Self::unscramble(&response.image, seed).unwrap_or(response.image));
		}

		let number = |key: &str| context.get(key).and_then(|value| value.parse::<u32>().ok());
		let (Some(slice), Some(slices)) = (number("slice"), number("slices")) else {
			return Ok(response.image);
		};
		if slices < 2 || slice >= slices {
			return Ok(response.image);
		}

		let width = response.image.width();
		let height = response.image.height() as u32;
		// whole pixel rows: the app's crop rounds a fractional rect, which would draw a row twice
		// or drop it at the boundary between two slices
		let top = height * slice / slices;
		let bottom = height * (slice + 1) / slices;
		let (top, slice_height) = (top as f32, (bottom - top) as f32);

		// the canvas has to match the slice: app versions before the AidokuRunner fix in e44d774
		// placed the destination rect off any canvas shorter than the image, drawing nothing
		let mut canvas = Canvas::new(width, slice_height);
		canvas.copy_image(
			&response.image,
			Rect::new(0.0, top, width, slice_height),
			Rect::new(0.0, 0.0, width, slice_height),
		);
		Ok(canvas.get_image())
	}
}

impl ListingProvider for Soraraw {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		match listing.id.as_str() {
			"rising" => Self::parse_top("rising"),
			"trending" => Self::parse_top("last30Days"),
			"lifetime" => Self::parse_top("lifetime"),
			"newest" => Self::parse_list(&paginated(&format!("{BASE_URL}/newest"), page)),
			_ => bail!("invalid listing id: `{}`", listing.id),
		}
	}
}

impl DynamicFilters for Soraraw {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let genres = request(format!("{BASE_URL}/genres.json"))?.json_owned::<Vec<GenreEntry>>()?;

		let mut options: Vec<Cow<'static, str>> = Vec::new();
		let mut ids: Vec<Cow<'static, str>> = Vec::new();
		for genre in genres.into_iter().take(GENRE_LIMIT) {
			if genre.slug.is_empty() || genre.name.trim().is_empty() {
				continue;
			}
			options.push(String::from(genre.name.trim()).into());
			ids.push(genre.id.to_string().into());
		}

		Ok(vec![
			MultiSelectFilter {
				id: "genre-id".into(),
				title: Some("ジャンル".into()),
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

register_source!(
	Soraraw,
	Home,
	ListingProvider,
	DynamicFilters,
	DeepLinkHandler,
	PageImageProcessor
);
