use crate::{BASE_URL, Comix, models::ComixPage, web};
use aidoku::{
	ImageResponse, Page, PageContent, PageContext, PageImageProcessor, Result,
	alloc::{String, Vec, string::ToString, vec},
	imports::{
		canvas::{Canvas, ImageRef, Rect},
		net::{Request, Response},
	},
	prelude::*,
};

const SCRAMBLED_PAGE_GRID_CONTEXT_KEY: &str = "comixScrambleGrid";
const SCRAMBLED_PAGE_MAP_CONTEXT_KEY: &str = "comixScrambleMap";
pub const PAGE_S_FLAG_CONTEXT_KEY: &str = "comixPageS";
const PAGE_WIDTH_CONTEXT_KEY: &str = "comixPageWidth";
const PAGE_HEIGHT_CONTEXT_KEY: &str = "comixPageHeight";

/// Builds the page list, batching the s:1 clean-route and scramble-metadata
/// probes through `send_all` so latency scales with round-trips, not page count.
pub fn build_page_list(source: &Comix, base_url: &str, items: Vec<ComixPage>) -> Result<Vec<Page>> {
	let resolved: Vec<ResolvedPage> = items
		.into_iter()
		.enumerate()
		.map(|(index, page)| ResolvedPage::new(base_url, page, index))
		.collect::<Result<Vec<_>>>()?;

	let clean_available = probe_clean_routes(&resolved);
	let scramble_headers = probe_scramble_headers(&resolved, &clean_available);

	let pages = resolved
		.into_iter()
		.enumerate()
		.map(|(i, page)| Page {
			content: page.into_content(
				source,
				clean_available.get(&i).copied().unwrap_or(false),
				scramble_headers.get(&i),
			),
			..Default::default()
		})
		.collect();
	Ok(pages)
}

struct ResolvedPage {
	url: String,
	index: usize,
	s_flag: bool,
	scrambled_bounds: Option<(usize, usize)>,
	width: f32,
	height: f32,
}

impl ResolvedPage {
	fn new(base_url: &str, page: ComixPage, index: usize) -> Result<Self> {
		let (width, height) = (page.width, page.height);
		let url = absolute_page_url(base_url, page.url)?;
		let s_flag = page.s == Some(1);
		let scrambled_bounds = s_flag.then(|| scrambled_route_bounds(&url)).flatten();
		Ok(Self {
			url,
			index,
			s_flag,
			scrambled_bounds,
			width,
			height,
		})
	}

	fn clean_probe(&self) -> Option<(usize, String)> {
		self.scrambled_bounds
			.map(|(start, end)| (self.index, clean_image_url(&self.url, start, end)))
	}

	fn into_content(
		self,
		source: &Comix,
		clean_available: bool,
		scramble: Option<&PageScramble>,
	) -> PageContent {
		if !self.s_flag {
			return PageContent::url(self.url);
		}
		let (width, height) = (self.width, self.height);
		let Some((start, end)) = self.scrambled_bounds else {
			// s:1 but the URL doesn't match a known scrambled route — still
			// carry the s:1 context so get_image_request adds Origin.
			return PageContent::url_context(self.url, with_dims(s_flag_context(), width, height));
		};
		if clean_available {
			return PageContent::url_context(
				clean_image_url(&self.url, start, end),
				with_dims(s_flag_context(), width, height),
			);
		}
		// Build the descramble context locally so image processing at render
		// time doesn't need to touch the WebView.
		let context = scramble
			.and_then(|s| source.prepare_scrambled_page_context(&s.seed, &s.grid))
			.unwrap_or_else(s_flag_context);
		// When the seed/grid came from a cache-busted probe, fetch the image from
		// the same busted URL so its bytes match the seed the map was built from.
		let url = if scramble.is_some_and(|s| s.cache_bust) {
			let separator = if self.url.contains('?') { '&' } else { '?' };
			format!("{}{separator}r=1", self.url)
		} else {
			self.url
		};
		PageContent::url_context(url, with_dims(context, width, height))
	}
}

fn probe_clean_routes(resolved: &[ResolvedPage]) -> aidoku::HashMap<usize, bool> {
	let requests: Vec<(usize, Request)> = resolved
		.iter()
		.filter_map(|p| {
			let (index, clean_url) = p.clean_probe()?;
			Some((index, apply_s1_headers(Request::head(&clean_url).ok()?)))
		})
		.collect();
	batch_responses(requests)
		.into_iter()
		.map(|(index, response)| (index, is_image_response(response.as_ref())))
		.collect()
}

enum ScrambleProbe {
	Valid(String, String),
	/// No usable seed/grid (e.g. the CDN's `X-Scramble-Seed: 0` response); the
	/// caller retries with a cache-bust, then falls through to the raw image.
	Unknown,
}

struct PageScramble {
	seed: String,
	grid: String,
	/// The plain URL returned a degraded `seed:0` response and the real seed/grid
	/// only came back from a cache-busted request, so the image must be fetched
	/// from the busted URL too.
	cache_bust: bool,
}

/// Fetches X-Scramble-Seed/Grid for s:1 pages without a clean `/i/` route via a
/// HEAD batch. The CDN sometimes caches a degraded response (`seed:0`/no grid)
/// for a page that is actually scrambled; a second HEAD with a cache-busting
/// param forces a fresh response with the real seed/grid (the website recovers
/// the same way). Pages still without seed/grid aren't scrambled and fall
/// through to the raw image.
fn probe_scramble_headers(
	resolved: &[ResolvedPage],
	clean_available: &aidoku::HashMap<usize, bool>,
) -> aidoku::HashMap<usize, PageScramble> {
	let targets: aidoku::HashMap<usize, &str> = resolved
		.iter()
		.filter(|p| {
			p.s_flag
				&& p.scrambled_bounds.is_some()
				&& !clean_available.get(&p.index).copied().unwrap_or(false)
		})
		.map(|p| (p.index, p.url.as_str()))
		.collect();
	if targets.is_empty() {
		return aidoku::HashMap::new();
	}

	let mut results = aidoku::HashMap::new();
	let head_requests: Vec<(usize, Request)> = targets
		.iter()
		.filter_map(|(&i, &url)| Some((i, apply_s1_headers(Request::head(url).ok()?))))
		.collect();
	let mut unresolved = Vec::new();
	for (index, response) in batch_responses(head_requests) {
		match extract_scramble_probe(response) {
			ScrambleProbe::Valid(seed, grid) => {
				results.insert(
					index,
					PageScramble {
						seed,
						grid,
						cache_bust: false,
					},
				);
			}
			ScrambleProbe::Unknown => unresolved.push(index),
		}
	}

	let bust_requests: Vec<(usize, Request)> = unresolved
		.into_iter()
		.filter_map(|index| {
			let url = *targets.get(&index)?;
			let separator = if url.contains('?') { '&' } else { '?' };
			let busted = format!("{url}{separator}r=1");
			Some((index, apply_s1_headers(Request::head(&busted).ok()?)))
		})
		.collect();
	for (index, response) in batch_responses(bust_requests) {
		if let ScrambleProbe::Valid(seed, grid) = extract_scramble_probe(response) {
			results.insert(
				index,
				PageScramble {
					seed,
					grid,
					cache_bust: true,
				},
			);
		}
	}
	results
}

/// Sends every `(index, request)` in parallel, flattening transport errors to `None`.
fn batch_responses(requests: Vec<(usize, Request)>) -> Vec<(usize, Option<Response>)> {
	if requests.is_empty() {
		return Vec::new();
	}
	let (indices, reqs): (Vec<usize>, Vec<Request>) = requests.into_iter().unzip();
	Request::send_all(reqs)
		.into_iter()
		.zip(indices)
		.map(|(result, index)| (index, result.ok()))
		.collect()
}

fn is_image_response(response: Option<&Response>) -> bool {
	let Some(response) = response else {
		return false;
	};
	let content_type = response
		.get_header("Content-Type")
		.unwrap_or_default()
		.to_lowercase();
	(200..400).contains(&response.status_code()) && content_type.starts_with("image/")
}

fn extract_scramble_probe(response: Option<Response>) -> ScrambleProbe {
	let Some(response) = response.filter(|r| (200..400).contains(&r.status_code())) else {
		return ScrambleProbe::Unknown;
	};
	let seed = response.get_header("X-Scramble-Seed");
	let grid = response.get_header("X-Scramble-Grid");
	match (seed, grid) {
		(Some(seed), Some(grid)) => ScrambleProbe::Valid(seed, grid),
		// `seed:0`/missing grid means the page isn't scrambled — use the raw image.
		_ => ScrambleProbe::Unknown,
	}
}

fn absolute_page_url(base_url: &str, page_url: String) -> Result<String> {
	if page_url.starts_with("http://") || page_url.starts_with("https://") {
		Ok(page_url)
	} else {
		let base_url = base_url.trim_end_matches('/');
		if base_url.is_empty() {
			bail!("Comix page list is missing an image base URL")
		}
		Ok(format!("{base_url}/{}", page_url.trim_start_matches('/')))
	}
}

// Comix s:1 images need Origin alongside Referer; missing Origin returns 404
// from the CDN. Non-s:1 images must not use these headers.
fn apply_s1_headers(request: Request) -> Request {
	request
		.header("Referer", &format!("{BASE_URL}/"))
		.header("Origin", BASE_URL)
}

fn s_flag_context() -> PageContext {
	let mut context = PageContext::new();
	context.insert(PAGE_S_FLAG_CONTEXT_KEY.into(), "1".into());
	context
}

fn scrambled_page_context(grid: &str, map: &[usize]) -> PageContext {
	let mut context = s_flag_context();
	context.insert(SCRAMBLED_PAGE_GRID_CONTEXT_KEY.into(), grid.into());
	context.insert(SCRAMBLED_PAGE_MAP_CONTEXT_KEY.into(), encode_tile_map(map));
	context
}

fn encode_tile_map(map: &[usize]) -> String {
	map.iter()
		.map(usize::to_string)
		.collect::<Vec<_>>()
		.join(",")
}

fn parse_tile_map(value: &str) -> Result<Vec<usize>> {
	value
		.split(',')
		.map(|item| {
			item.parse::<usize>()
				.map_err(|_| error!("Comix image descrambler tile map is invalid"))
		})
		.collect()
}

fn parse_scramble_grid(grid: &str) -> Result<(usize, usize)> {
	grid.split_once('x')
		.and_then(|(c, r)| Some((c.parse::<usize>().ok()?, r.parse::<usize>().ok()?)))
		.filter(|(c, r)| (1..=20).contains(c) && (1..=20).contains(r))
		.ok_or_else(|| error!("Comix image scramble grid is invalid: {grid}"))
}

fn descramble_image(image: &ImageRef, map: &[usize], cols: usize, rows: usize) -> Result<ImageRef> {
	let total = cols * rows;
	if map.len() != total {
		bail!("Comix image descrambler returned an invalid tile count")
	}

	let mut seen = vec![false; total];
	for &dest in map {
		if dest >= total || seen[dest] {
			bail!("Comix image descrambler returned an invalid tile map")
		}
		seen[dest] = true;
	}

	let image_width = image.width();
	let image_height = image.height();
	let tile_width = (image_width as usize / cols) as f32;
	let tile_height = (image_height as usize / rows) as f32;
	if tile_width <= 0.0 || tile_height <= 0.0 {
		bail!("Comix image is too small for its scramble grid")
	}

	let mut canvas = Canvas::new(image_width, image_height);
	// Base layer: integer tile division can leave a few-pixel remainder strip on
	// the right/bottom edges that the per-tile copies below never overwrite.
	canvas.draw_image(image, Rect::new(0.0, 0.0, image_width, image_height));

	for (source, &dest) in map.iter().enumerate() {
		let source_x = (source % cols) as f32 * tile_width;
		let source_y = (source / cols) as f32 * tile_height;
		let dest_x = (dest % cols) as f32 * tile_width;
		let dest_y = (dest / cols) as f32 * tile_height;
		canvas.copy_image(
			image,
			Rect::new(source_x, source_y, tile_width, tile_height),
			Rect::new(dest_x, dest_y, tile_width, tile_height),
		);
	}

	Ok(canvas.get_image())
}

fn clean_image_url(url: &str, start: usize, end: usize) -> String {
	let mut clean_url = url.to_string();
	clean_url.replace_range(start..end, "i");
	clean_url
}

/// Returns the byte offsets of the `si`/`sii` segment within `url` so the caller
/// can rewrite it to `i` in-place; `None` for any URL that doesn't match the
/// `<host>/(si|sii)/<token>/<filename>` shape Comix uses for scrambled images.
fn scrambled_route_bounds(url: &str) -> Option<(usize, usize)> {
	let path_end = url.find(['?', '#']).unwrap_or(url.len());
	let path_start = if let Some(i) = url.find("://") {
		i + 3 + url.get(i + 3..path_end)?.find('/')?
	} else if url.starts_with('/') {
		0
	} else {
		return None;
	};

	let mut segments = url[path_start + 1..path_end].split('/');
	let route = segments.next()?;
	let token = segments.next()?;
	let filename = segments.next()?;
	if segments.next().is_some()
		|| !matches!(route, "si" | "sii")
		|| !is_comix_image_token(token)
		|| !filename.contains('.')
	{
		return None;
	}
	let start = path_start + 1;
	Some((start, start + route.len()))
}

fn is_comix_image_token(token: &str) -> bool {
	token.len() >= 16
		&& token
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '='))
}

impl Comix {
	fn prepare_scrambled_page_context(&self, seed: &str, grid: &str) -> Option<PageContext> {
		let map_key = format!("{seed}:{grid}");
		if let Some(map) = self.scramble_maps.borrow().get(&map_key) {
			return Some(scrambled_page_context(grid, map));
		}
		let map = web::get_scramble_map(&mut self.web_view.borrow_mut(), seed, grid).ok()?;
		self.scramble_maps.borrow_mut().insert(map_key, map.clone());
		Some(scrambled_page_context(grid, &map))
	}
}

impl PageImageProcessor for Comix {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let context = context.as_ref();
		let source_url = response.request.url.clone();

		// Descramble when we have a tile map, otherwise use the raw image.
		let image = finish_image(response.image, context);
		if is_valid_image(&image) {
			return Ok(image);
		}

		// The CDN intermittently serves a truncated, undecodable image; the
		// website recovers by re-requesting with a cache-busting param, so mirror
		// that to keep the page rather than losing it.
		if let Some(retried) = source_url
			.and_then(|url| refetch_page_image(&url, context))
			.filter(is_valid_image)
		{
			return Ok(retried);
		}

		// A genuinely broken page (truncated even after retry) would decode to a
		// zero-sized image; the reader divides by the page height for layout and a
		// NaN frame crashes the app (Aidoku#991). Fall back to a blank page at the
		// real dimensions so it lays out like the site's errored-page placeholder.
		Ok(placeholder_image(context))
	}
}

/// Descrambles `image` when the context carries a tile map, otherwise returns it
/// unchanged.
fn finish_image(image: ImageRef, context: Option<&PageContext>) -> ImageRef {
	try_descramble(&image, context).unwrap_or(image)
}

/// Re-requests a page image with a cache-busting param (mirroring the website's
/// own retry) and processes it, to recover the occasional truncated image.
fn refetch_page_image(url: &str, context: Option<&PageContext>) -> Option<ImageRef> {
	let separator = if url.contains('?') { '&' } else { '?' };
	let mut request = Request::get(format!("{url}{separator}r=1"))
		.ok()?
		.header("Referer", &format!("{BASE_URL}/"));
	if context.is_some_and(|c| c.get(PAGE_S_FLAG_CONTEXT_KEY).is_some()) {
		request = request.header("Origin", BASE_URL);
	}
	Some(finish_image(request.image().ok()?, context))
}

fn is_valid_image(image: &ImageRef) -> bool {
	let (width, height) = (image.width(), image.height());
	width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0
}

/// A blank image sized to the page's real dimensions when known, so a broken
/// page lays out at the correct size instead of collapsing the reader layout.
fn placeholder_image(context: Option<&PageContext>) -> ImageRef {
	let (width, height) = context.and_then(page_dimensions).unwrap_or((1.0, 1.0));
	Canvas::new(width, height).get_image()
}

/// Stores the page's pixel dimensions in `context` so a failed page can be laid
/// out at its real size (the API provides these per page).
fn with_dims(mut context: PageContext, width: f32, height: f32) -> PageContext {
	if width >= 1.0 && height >= 1.0 {
		context.insert(PAGE_WIDTH_CONTEXT_KEY.into(), width.to_string());
		context.insert(PAGE_HEIGHT_CONTEXT_KEY.into(), height.to_string());
	}
	context
}

fn page_dimensions(context: &PageContext) -> Option<(f32, f32)> {
	let width = context.get(PAGE_WIDTH_CONTEXT_KEY)?.parse::<f32>().ok()?;
	let height = context.get(PAGE_HEIGHT_CONTEXT_KEY)?.parse::<f32>().ok()?;
	(width >= 1.0 && height >= 1.0).then_some((width, height))
}

fn try_descramble(image: &ImageRef, context: Option<&PageContext>) -> Option<ImageRef> {
	let context = context?;
	let grid = context.get(SCRAMBLED_PAGE_GRID_CONTEXT_KEY)?;
	let map = parse_tile_map(context.get(SCRAMBLED_PAGE_MAP_CONTEXT_KEY)?).ok()?;
	let (cols, rows) = parse_scramble_grid(grid).ok()?;
	descramble_image(image, &map, cols, rows).ok()
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	const TOKEN: &str = "testImageRouteToken0123456789";

	fn clean(path: &str) -> Option<String> {
		let url = format!("https://cdn.example.com{path}");
		scrambled_route_bounds(&url).map(|(start, end)| clean_image_url(&url, start, end))
	}

	#[aidoku_test]
	fn normalizes_known_scrambled_routes() {
		assert_eq!(
			clean(&format!("/si/{TOKEN}/03.webp")),
			Some(format!("https://cdn.example.com/i/{TOKEN}/03.webp"))
		);
		assert_eq!(
			clean(&format!("/sii/{TOKEN}/03.webp")),
			Some(format!("https://cdn.example.com/i/{TOKEN}/03.webp"))
		);
	}

	#[aidoku_test]
	fn does_not_guess_unknown_scrambled_routes() {
		assert_eq!(clean(&format!("/future/{TOKEN}/03.webp")), None);
	}

	#[aidoku_test]
	fn keeps_normal_pages_and_preserves_suffixes() {
		assert_eq!(clean(&format!("/i/{TOKEN}/03.webp")), None);
		assert_eq!(
			clean(&format!("/sii/{TOKEN}/03.webp?size=large#page")),
			Some(format!(
				"https://cdn.example.com/i/{TOKEN}/03.webp?size=large#page"
			))
		);
	}

	#[aidoku_test]
	fn avoids_substring_and_unknown_shape_rewrites() {
		assert_eq!(clean(&format!("/sii-extra/{TOKEN}/03.webp")), None);
		assert_eq!(clean(&format!("/path/sii/{TOKEN}/03.webp")), None);
		assert_eq!(clean("/si/short/03.webp"), None);
	}
}
