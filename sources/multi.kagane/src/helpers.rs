use aidoku::{
	ContentRating, Manga, MangaStatus, Result, Viewer,
	alloc::{format, string::String, vec, vec::Vec},
	imports::{defaults::defaults_get, js::WebView, net::Request},
	prelude::*,
};
use core::cell::RefCell;
use serde::de::DeserializeOwned;

pub const BASE_URL: &str = "https://kagane.to";
pub const API_BASE: &str = "https://kagane.to/api/v2";

#[derive(serde::Deserialize)]
struct FetchResult {
	ok: bool,
	status: i32,
	text: String,
	aborted: bool,
}

/// A unique-enough global name for the web view to stash its in-flight
/// fetch result under, polled from Rust via repeated synchronous `eval`s.
const RESULT_TOKEN: &str = "__aidoku_kagane_result";

/// Every kagane.to API route sits behind a Cloudflare managed challenge, and
/// (confirmed by testing) the `cf_clearance` cookie a web view earns by
/// solving it is *not* shared with the app's plain `net::Request` client. So
/// instead of solving the challenge once and retrying with `Request`, all
/// API calls run as a `fetch()` executed inside the web view itself, which
/// carries its own cookies. The web view is kept alive for the lifetime of
/// the source so the challenge is only solved once per session.
#[derive(Default)]
pub struct ApiClient {
	webview: RefCell<Option<WebView>>,
}

impl ApiClient {
	/// Ensures the web view exists and has passed Cloudflare's challenge.
	fn ensure_ready(&self) -> Result<()> {
		if self.webview.borrow().is_some() {
			return Ok(());
		}

		let webview = WebView::new();
		webview.load_blocking(Request::get(BASE_URL)?)?;

		// The interstitial's own "load" event fires before its JS finishes
		// and redirects to the real page, so poll a few follow-up loads
		// rather than trusting the first one.
		let mut solved = false;
		for _ in 0..5 {
			let title = webview
				.eval("document.title")
				.unwrap_or_else(|_| String::from("Just a moment"));
			if !title.contains("Just a moment") {
				solved = true;
				break;
			}
			webview.wait_for_load();
		}
		if !solved {
			bail!("Failed to bypass Cloudflare's challenge. Please try again.");
		}

		*self.webview.borrow_mut() = Some(webview);
		Ok(())
	}

	/// Runs `body_js` in the web view and waits for the result it stashes.
	///
	/// `body_js` is responsible for eventually assigning a
	/// `{ done, ok, status, text, aborted }` object to the result global.
	/// `eval_async` turned out to be unreliable here, so this follows the
	/// approach `en.comix` uses: kick the work off with plain (synchronous)
	/// `eval`, then poll the global with further synchronous `eval`s rather
	/// than awaiting the promise directly.
	fn run_js(&self, body_js: &str) -> Result<String> {
		self.ensure_ready()?;
		let borrow = self.webview.borrow();
		let webview = borrow
			.as_ref()
			.ok_or_else(|| error!("Web view not initialized"))?;

		// The trailing `return ''` matters: `eval` treats a missing value as
		// a failure, so the kickoff has to hand back *some* string.
		webview.eval(&format!(
			"(() => {{
				window.{RESULT_TOKEN} = {{ done: false, ok: false, status: 0, text: '', aborted: false }};
				{body_js}
				return '';
			}})()"
		))?;

		while webview.eval(&format!("window.{RESULT_TOKEN}.done ? 'true' : 'false'"))? != "true" {}

		let raw = webview.eval(&format!("JSON.stringify(window.{RESULT_TOKEN})"))?;
		let result: FetchResult = serde_json::from_str(&raw)?;
		if result.aborted {
			bail!("Kagane request timed out. Please try again.");
		}
		if !result.ok {
			bail!(
				"Kagane request failed (status {}): {}",
				result.status,
				result.text.chars().take(200).collect::<String>()
			);
		}
		Ok(result.text)
	}

	/// Runs a `fetch()` inside the web view and returns the response body.
	fn fetch(
		&self,
		method: &str,
		url: &str,
		body: Option<&str>,
		header: Option<(&str, &str)>,
	) -> Result<String> {
		// The request is passed to the web view as a JSON value (rather than
		// interpolating it into the JS source directly) so serde_json
		// handles all string escaping.
		let payload = serde_json::to_string(&serde_json::json!({
			"url": url,
			"method": method,
			"body": body,
			"headerName": header.map(|h| h.0),
			"headerValue": header.map(|h| h.1),
		}))?;
		self.run_js(&format!(
			"const req = {payload};
			const headers = {{ 'Content-Type': 'application/json' }};
			if (req.headerName != null) headers[req.headerName] = req.headerValue;
			const controller = new AbortController();
			const timeout = setTimeout(() => {{
				controller.abort();
				window.{RESULT_TOKEN}.aborted = true;
				window.{RESULT_TOKEN}.done = true;
			}}, 30000);
			const opts = {{ method: req.method, headers, signal: controller.signal }};
			if (req.body != null) opts.body = req.body;
			fetch(req.url, opts)
				.then(res => res.text().then(text => {{
					clearTimeout(timeout);
					window.{RESULT_TOKEN} = {{ done: true, ok: res.ok, status: res.status, text, aborted: false }};
				}}))
				.catch(e => {{
					clearTimeout(timeout);
					const message = e && e.message ? e.message : String(e);
					window.{RESULT_TOKEN} = {{ done: true, ok: false, status: 0, text: message, aborted: false }};
				}});"
		))
	}

	/// Replaces each manga's cover URL with an inline `data:` URI.
	///
	/// Cover images sit behind the same Cloudflare challenge as the API, but
	/// the app loads them with its own HTTP client, which has no clearance
	/// cookie — so a plain URL always 403s. Fetching them in the web view and
	/// inlining the bytes is the only way to get them to render. All covers
	/// for a page are fetched in one round trip; any that fail keep their
	/// original URL rather than failing the whole listing.
	pub fn apply_covers(&self, mangas: &mut [Manga]) -> Result<()> {
		let urls: Vec<&str> = mangas
			.iter()
			.filter_map(|m| m.cover.as_deref())
			.collect::<Vec<_>>();
		if urls.is_empty() {
			return Ok(());
		}

		let payload = serde_json::to_string(&urls)?;
		let text = self.run_js(&format!(
			"const urls = {payload};
			const controller = new AbortController();
			const timeout = setTimeout(() => {{
				controller.abort();
				window.{RESULT_TOKEN}.aborted = true;
				window.{RESULT_TOKEN}.done = true;
			}}, 30000);
			const toDataUri = (url) => fetch(url, {{ signal: controller.signal }})
				.then(res => res.ok ? res.blob() : null)
				.then(blob => blob == null ? null : new Promise(resolve => {{
					const reader = new FileReader();
					reader.onloadend = () => resolve(typeof reader.result === 'string' ? reader.result : null);
					reader.onerror = () => resolve(null);
					reader.readAsDataURL(blob);
				}}))
				.catch(() => null);
			Promise.all(urls.map(toDataUri)).then(list => {{
				clearTimeout(timeout);
				window.{RESULT_TOKEN} = {{ done: true, ok: true, status: 200, text: JSON.stringify(list), aborted: false }};
			}});"
		))?;

		let data_uris: Vec<Option<String>> = serde_json::from_str(&text)?;
		let mut next = data_uris.into_iter();
		for manga in mangas.iter_mut().filter(|m| m.cover.is_some()) {
			if let Some(Some(uri)) = next.next() {
				manga.cover = Some(uri);
			}
		}
		Ok(())
	}

	pub fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
		let text = self.fetch("GET", url, None, None)?;
		Ok(serde_json::from_str(&text)?)
	}

	pub fn post_json<T: DeserializeOwned>(&self, url: &str, body: &str) -> Result<T> {
		let text = self.fetch("POST", url, Some(body), None)?;
		Ok(serde_json::from_str(&text)?)
	}

	pub fn post_json_with_header<T: DeserializeOwned>(
		&self,
		url: &str,
		body: &str,
		header: (&str, &str),
	) -> Result<T> {
		let text = self.fetch("POST", url, Some(body), Some(header))?;
		Ok(serde_json::from_str(&text)?)
	}
}

/// The content languages to request from the API. Reads the app's built-in
/// language selection (populated from the `languages` array in source.json)
/// and maps each canonical code to the code kagane's API expects. Falls back
/// to English when nothing is selected.
fn languages() -> Vec<String> {
	defaults_get::<Vec<String>>("languages")
		.filter(|langs| !langs.is_empty())
		.map(|langs| {
			langs
				.into_iter()
				.map(|lang| match lang.as_str() {
					"pt-BR" => String::from("pt-br"),
					_ => lang,
				})
				.collect()
		})
		.unwrap_or_else(|| vec![String::from("en")])
}

/// The content ratings to request from the API, from the "Content Rating"
/// setting. Falls back to Safe + Suggestive when the setting is unset.
fn content_ratings() -> Vec<String> {
	defaults_get::<Vec<String>>("contentRating")
		.unwrap_or_else(|| vec![String::from("Safe"), String::from("Suggestive")])
}

/// The source types to request from the API, from the "Source Type" setting.
/// Falls back to all types when the setting is unset.
fn source_types() -> Vec<String> {
	defaults_get::<Vec<String>>("sourceType").unwrap_or_else(|| {
		vec![
			String::from("Official"),
			String::from("Unofficial"),
			String::from("Mixed"),
		]
	})
}

pub fn parse_status(s: &str) -> MangaStatus {
	match s.to_uppercase().as_str() {
		"ONGOING" => MangaStatus::Ongoing,
		"COMPLETED" => MangaStatus::Completed,
		"HIATUS" => MangaStatus::Hiatus,
		"ABANDONED" => MangaStatus::Cancelled,
		_ => MangaStatus::Unknown,
	}
}

pub fn parse_viewer(format: Option<&str>) -> Viewer {
	match format {
		Some("Manga") => Viewer::RightToLeft,
		Some("Comic") => Viewer::LeftToRight,
		_ => Viewer::Webtoon,
	}
}

pub fn parse_content_rating(s: Option<&str>) -> ContentRating {
	let lower = s.map(|s| s.to_lowercase());
	match lower.as_deref() {
		Some("safe") => ContentRating::Safe,
		Some("suggestive") => ContentRating::Suggestive,
		Some("erotica") | Some("pornographic") => ContentRating::NSFW,
		_ => ContentRating::Suggestive,
	}
}

pub fn build_search_body(
	query: Option<&str>,
	statuses: &[String],
	formats: &[String],
	genres_included: &[String],
	genres_excluded: &[String],
) -> String {
	let mut body = serde_json::Map::new();

	if let Some(q) = query.filter(|q| !q.is_empty()) {
		body.insert(String::from("title"), serde_json::json!(q));
	}

	body.insert(String::from("content_lang"), serde_json::json!(languages()));
	body.insert(
		String::from("source_type"),
		serde_json::json!(source_types()),
	);
	body.insert(
		String::from("content_rating"),
		serde_json::json!(content_ratings()),
	);

	if !statuses.is_empty() {
		body.insert(String::from("upload_status"), serde_json::json!(statuses));
	}

	if !formats.is_empty() {
		body.insert(String::from("format"), serde_json::json!(formats));
	}

	// Genres are sent as an object with included `values`, `exclude`, and a
	// constant `match_all: true` (the website never toggles it).
	if !genres_included.is_empty() || !genres_excluded.is_empty() {
		let mut genres = serde_json::Map::new();
		genres.insert(String::from("values"), serde_json::json!(genres_included));
		genres.insert(String::from("match_all"), serde_json::json!(true));
		if !genres_excluded.is_empty() {
			genres.insert(String::from("exclude"), serde_json::json!(genres_excluded));
		}
		body.insert(String::from("genres"), serde_json::Value::Object(genres));
	}

	serde_json::to_string(&serde_json::Value::Object(body)).unwrap_or_default()
}
