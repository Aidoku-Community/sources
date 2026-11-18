use crate::{
	helpers::{manga_url, parse_page_urls},
	models::*,
	settings,
};
use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::{
		js::{WebView, WebViewUserScript},
		net::Request,
		std::sleep,
	},
	prelude::*,
};

const RESULT_TOKEN: &str = "__AIDOKU_MKISSA_PAGES__";
const WAIT_TOKEN: &str = "__AIDOKU_MKISSA_WAIT__";
const CHALLENGE_TOKEN: &str = "__AIDOKU_MKISSA_CHALLENGE__";

fn is_parser_key_byte(byte: u8) -> bool {
	byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn parse_quoted_key(input: &str) -> Option<String> {
	let bytes = input.as_bytes();
	let quote = *bytes.first()?;
	if quote != b'\'' && quote != b'"' {
		return None;
	}
	let end = bytes[1..].iter().position(|byte| *byte == quote)? + 1;
	let key = &input[1..end];
	(!key.is_empty() && key.bytes().all(is_parser_key_byte)).then(|| String::from(key))
}

fn resolve_parser_key_assignment(script: &str, target: &str) -> Option<String> {
	let mut offset = 0;
	while let Some(relative) = script[offset..].find(target) {
		let start = offset + relative;
		let end = start + target.len();
		let before_is_key = start > 0 && is_parser_key_byte(script.as_bytes()[start - 1]);
		let after_is_key = end < script.len() && is_parser_key_byte(script.as_bytes()[end]);
		if !before_is_key && !after_is_key {
			let rest = script[end..].trim_start();
			if rest.starts_with('=') && !rest.starts_with("==") && !rest.starts_with("=>") {
				let expression = rest[1..].split(';').next()?;
				let bytes = expression.as_bytes();
				let mut key = String::new();
				let mut found_literal = false;
				let mut index = 0;
				while index < bytes.len() {
					let quote = bytes[index];
					if quote != b'\'' && quote != b'"' {
						index += 1;
						continue;
					}
					let literal_start = index + 1;
					index = literal_start;
					while index < bytes.len() && bytes[index] != quote {
						index += 1;
					}
					if index >= bytes.len() {
						return None;
					}
					let literal = &expression[literal_start..index];
					if literal.bytes().all(is_parser_key_byte) {
						key.push_str(literal);
						found_literal = true;
					}
					index += 1;
				}
				if found_literal && !key.is_empty() {
					return Some(key);
				}
			}
		}
		offset = end;
	}
	None
}

// The page pins a pristine JSON.parse under a window key, written whole or built from literals
fn pinned_parser_key(html: &str) -> Option<String> {
	let mut offset = 0;
	while let Some(relative) = html[offset..].find("defineProperty") {
		let call_start = offset + relative;
		let script_start = html[..call_start].rfind("<script")?;
		let body_start = script_start + html[script_start..].find('>')? + 1;
		let script_end = call_start + html[call_start..].find("</script>")?;
		let script = &html[body_start..script_end];
		if !script.contains("JSON") {
			offset = call_start + "defineProperty".len();
			continue;
		}

		let after_name = &html[call_start + "defineProperty".len()..script_end];
		let open = after_name.find('(')?;
		let mut rest = after_name[open + 1..].trim_start();
		if !rest.starts_with("window") {
			offset = call_start + "defineProperty".len();
			continue;
		}
		rest = rest["window".len()..].trim_start();
		if !rest.starts_with(',') {
			offset = call_start + "defineProperty".len();
			continue;
		}
		rest = rest[1..].trim_start();

		if let Some(key) = parse_quoted_key(rest) {
			return Some(key);
		}

		let target_len = rest
			.bytes()
			.take_while(|byte| is_parser_key_byte(*byte))
			.count();
		if target_len > 0 {
			let target = &rest[..target_len];
			if let Some(key) = resolve_parser_key_assignment(script, target) {
				return Some(key);
			}
		}
		offset = call_start + "defineProperty".len();
	}
	None
}

// The site decodes `chapterPages` in its own script, so the page is captured as it parses them
fn capture_script(chapter_path: &str, chapter: &str, pinned_key: Option<&str>) -> String {
	let chapter_path = serde_json::to_string(chapter_path).unwrap_or_else(|_| String::from("\"\""));
	let chapter = serde_json::to_string(chapter).unwrap_or_else(|_| String::from("\"\""));
	let pin = pinned_key
		.and_then(|key| serde_json::to_string(key).ok())
		.map(|key| {
			format!(
				"try {{
		Object.defineProperty(window, {key}, {{
			value: captureParse,
			writable: false,
			configurable: false,
			enumerable: false
		}});
	}} catch (_) {{}}"
			)
		})
		.unwrap_or_default();

	format!(
		"(function () {{
	if (window['{RESULT_TOKEN}']) return;
	window['{RESULT_TOKEN}'] = {{ data: '', done: false, chapter: {chapter} }};
	const state = window['{RESULT_TOKEN}'];
	const finish = (payload) => {{
		if (state.done) return;
		state.data = payload;
		state.done = true;
	}};
	// A page kept for later chapters can parse others too, so only the one asked for counts
	const capture = (parsed, raw) => {{
		try {{
			const pages = parsed && (parsed.chapterPages || (parsed.data && parsed.data.chapterPages));
			if (!pages) return;
			const edges = pages.edges || [];
			if (edges.some((edge) => edge.chapterString != null && String(edge.chapterString) !== state.chapter)) return;
			finish(raw);
		}} catch (_) {{}}
	}};

	const originalJson = Response.prototype.json;
	Response.prototype.json = function () {{
		return originalJson.call(this).then((data) => {{
			capture(data, JSON.stringify(data));
			return data;
		}});
	}};

	const originalParse = JSON.parse;
	const captureParse = new Proxy(originalParse, {{
		apply(target, thisArg, args) {{
			const result = Reflect.apply(target, thisArg, args);
			capture(result, args[0]);
			return result;
		}}
	}});
	JSON.parse = captureParse;
	{pin}

	// The page takes a pristine JSON.parse from a new iframe as it boots; with no frame window it
	// keeps ours. Later frames, such as the site's human check, keep theirs.
	const hideFrame = (element) => {{
		if (document.readyState === 'loading' && element && String(element.tagName).toUpperCase() === 'IFRAME') {{
			Object.defineProperty(element, 'contentWindow', {{ get: () => null, configurable: false }});
		}}
		return element;
	}};
	for (const name of ['createElement', 'createElementNS']) {{
		const create = Document.prototype[name];
		Document.prototype[name] = function (...args) {{
			return hideFrame(create.apply(this, args));
		}};
	}}

	// SvelteKit's router listens for clicks on the document element once the first page has
	// rendered, about a second before that page's own links appear
	let routerReady = false;
	const addListener = EventTarget.prototype.addEventListener;
	EventTarget.prototype.addEventListener = function (type, ...rest) {{
		if (type === 'click' && this === document.documentElement) {{
			routerReady = true;
			EventTarget.prototype.addEventListener = addListener;
		}}
		return addListener.call(this, type, ...rest);
	}};

	function triggerChapterNav(path) {{
		const a = document.createElement('a');
		a.href = a.dataset.href = path;
		document.body.append(a);
		a.click();
	}}

	function route(path) {{
		let attempts = 0;
		const check = () => {{
			if (state.done) return;
			if (routerReady || document.querySelector('[data-href]')) {{
				triggerChapterNav(path);
			}} else if (attempts < 300) {{
				attempts++;
				setTimeout(check, 50);
			}} else {{
				triggerChapterNav(path);
			}}
		}};
		check();
	}}

	state.open = (path, chapter) => {{
		state.chapter = chapter;
		state.data = '';
		state.done = false;
		route(path);
		return 'opened'; // eval fails on undefined
	}};
	route({chapter_path});
}})()"
	)
}

// Polls once a second; an eval that fails while the page is busy is skipped
fn wait_for_pages(web_view: &WebView, seconds: usize) -> Result<Option<String>> {
	let mut challenged = 0;
	for _ in 0..seconds {
		match web_view.eval(&format!(
			"(() => {{
				const state = window['{RESULT_TOKEN}'];
				if (state && state.done) return state.data;
				const challenge = document.title.includes('Just a moment')
					|| document.querySelector('.captcha-overlay--visible') != null;
				return challenge ? '{CHALLENGE_TOKEN}' : '{WAIT_TOKEN}';
			}})()"
		)) {
			// The check can flash while the page loads, so only five polls in a row count
			Ok(value) if value == CHALLENGE_TOKEN => {
				challenged += 1;
				if challenged >= 5 {
					bail!(
						"Pass the site's check under Pass Human Check in MKissa's settings, then retry"
					);
				}
			}
			Ok(value) if value != WAIT_TOKEN && !value.is_empty() => return Ok(Some(value)),
			_ => challenged = 0,
		}
		sleep(1);
	}
	Ok(None)
}

// Keeps the last series page loaded, so later chapters route inside it instead of reloading
#[derive(Default)]
pub struct Reader {
	web_view: Option<WebView>,
	opened: Vec<String>,
}

impl Reader {
	pub fn page_urls(&mut self, manga_id: &str, chapter: &str) -> Result<Vec<String>> {
		let chapter_path = format!("/manga/{manga_id}/chapter-{chapter}-sub");

		// The site answers a chapter the page already showed from its own cache, which parses nothing
		let mut found = None;
		if let Some(web_view) = self.web_view.take()
			&& !self.opened.contains(&chapter_path)
			&& web_view
				.eval(&format!(
					"window['{RESULT_TOKEN}'].open({}, {})",
					serde_json::to_string(&chapter_path)?,
					serde_json::to_string(chapter)?
				))
				.is_ok() && let Some(result) = wait_for_pages(&web_view, 10)?
		{
			found = Some((web_view, result));
		}
		let (web_view, result) = match found {
			Some(found) => found,
			None => {
				self.opened.clear();
				let web_view = load_chapter(manga_id, &chapter_path, chapter)?;
				let result = wait_for_pages(&web_view, 30)?
					.ok_or(error!("The reader did not produce any pages"))?;
				(web_view, result)
			}
		};
		self.web_view = Some(web_view);
		self.opened.push(chapter_path);

		// The page hands back the text it parsed, not a response body
		let payload: ReaderPayload = serde_json::from_str(&result)?;
		let pages = payload
			.chapter_pages
			.or_else(|| payload.data.and_then(|data| data.chapter_pages))
			.ok_or(error!("The reader did not produce any pages"))?;
		parse_page_urls(&pages, &settings::image_quality())
	}
}

fn load_chapter(manga_id: &str, chapter_path: &str, chapter: &str) -> Result<WebView> {
	let manga_url = manga_url(manga_id);

	// A plain request lets the app clear Cloudflare and keep its cookie
	let response = Request::get(&manga_url)?
		.header("Referer", &format!("{BASE_URL}/"))
		.header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
		.send()?;
	let status = response.status_code();
	if status >= 400 {
		if response.get_header("cf-mitigated").as_deref() == Some("challenge") {
			bail!("The reader returned HTTP {status}. Complete the Cloudflare check and retry");
		}
		bail!("The reader returned HTTP {status}");
	}
	let html = response.get_string()?;
	let parser_key = pinned_parser_key(&html);

	// The chapter route only answers a navigation from inside the site
	let web_view = WebView::new();
	let mut user_script =
		WebViewUserScript::new(capture_script(chapter_path, chapter, parser_key.as_deref()));
	user_script.at_document_end = false;
	// The human check runs Turnstile in a frame of its own, which the capture would break
	user_script.for_main_frame_only = true;
	web_view.add_user_script(user_script)?;
	// The blocking load never returns if the navigation fails; the poll has a deadline
	web_view.load_html(&html, Some(&manga_url))?;
	Ok(web_view)
}
