use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::{string::String, vec::Vec},
	helpers::uri::encode_uri_component,
	imports::{js::WebView, net::Request, std::sleep},
	prelude::*,
};

// The reader imports signing/image helpers from a hashed `secure-*.js` module.
// Find the import from the entry module; the install step below validates it.
const DISCOVER_SECURITY_MODULE_JS: &str = r#"
(() => {
	try {
		const HOME = "https://comix.to/";
		const ORIGIN = new URL(HOME).origin;
		const ENTRY_SELECTOR = "script[type='module'][src],link[rel~='modulepreload'][href]";
		const SECURE_REF_RE = /["']([^"']*\/?secure-[^"']+\.js(?:\?[^"']*)?)["']/g;

		const sameOriginUrl = (value, base = HOME) => {
			try {
				const url = new URL(value, base);
				return url.origin === ORIGIN ? url.href : "";
			} catch (_) { return ""; }
		};
		const fetchText = (url) => {
			const req = new XMLHttpRequest();
			req.open("GET", url, false);
			req.withCredentials = true;
			req.send(null);
			if (req.status < 200 || req.status >= 300) throw new Error(`fetch ${url} failed: ${req.status}`);
			return req.responseText || "";
		};
		const findEntry = (root, base) => {
			const node = root.querySelector(ENTRY_SELECTOR);
			return node ? sameOriginUrl(node.getAttribute("src") || node.getAttribute("href"), base) : "";
		};
		const findSecureModule = (code, base) => {
			SECURE_REF_RE.lastIndex = 0;
			let match;
			while ((match = SECURE_REF_RE.exec(code))) {
				const url = sameOriginUrl(match[1], base);
				if (url) return url;
			}
			return "";
		};

		let entryUrl = findEntry(document, location.href);
		let cfg = document.querySelector("meta[name='cfg']")?.getAttribute("content") || "";
		if (!entryUrl || !cfg) {
			const homepage = new DOMParser().parseFromString(fetchText(HOME), "text/html");
			if (!entryUrl) entryUrl = findEntry(homepage, HOME);
			if (!cfg) cfg = homepage.querySelector("meta[name='cfg']")?.getAttribute("content") || "";
		}
		if (!entryUrl) throw new Error("module entry not found");

		const secureUrl = findSecureModule(fetchText(entryUrl), entryUrl);
		if (!secureUrl) throw new Error("secure module import not found");
		return JSON.stringify({ url: secureUrl, cfg });
	} catch (e) {
		return JSON.stringify({ error: e && e.message ? e.message : String(e) });
	}
})()
"#;

const INSTALL_SECURITY_HTML_TEMPLATE: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<script>
(() => {
	const safeMessage = (value) => {
		const text = value && value.message ? value.message : String(value);
		return text.length > 160 ? `${text.slice(0, 160)}...` : text;
	};
	const setState = (state) => {
		window.__aidokuComixInstallState = JSON.stringify({
			ok: !!state.ok,
			done: !!state.done,
			stage: String(state.stage || "unknown"),
			message: safeMessage(state.message || ""),
			hasSigner: typeof window.__aidokuComixSigner === "function",
			hasDecoder: typeof window.__aidokuComixDecodeResponse === "function",
		});
	};
	const fail = (stage, message) => setState({ done: true, stage, message });
	window.__aidokuComixSetInstallState = setState;
	setState({ stage: "init", message: "install started" });

	window.addEventListener("error", (e) => fail("module-load", e.message || "module script failed"));
	window.addEventListener("unhandledrejection", (e) => fail("module-import", e.reason || "module import rejected"));

	const cfg = __CFG_JSON__;
	if (cfg) {
		const meta = document.createElement("meta");
		meta.name = "cfg";
		meta.content = cfg;
		document.head.appendChild(meta);
	}

	const securityUrl = __SECURITY_URL_JSON__;
	const SIGNER_PROBE_PATHS = ["/manga/x/chapters", "/chapters/x"];
	const TOKEN_RE = /^[A-Za-z0-9._~+/=-]{20,512}$/;
	const SCRAMBLE_PROBE_WEBP_BASE64 = "UklGRogAAABXRUJQVlA4THwAAAAvY8AYALkyRPQ/NmLBZP7QnTFE9D/JQ1FsK9XroXTRJWHsoV0+OR+wfjOKGklS2sDikYL1/z4JJEohI0lSqSzBkr3ncc5BJm2TGtg5DSYJMHlV6RavB6hva0caC2k+JPlHTFDOkaZ3kvwjHtIMUPYnyT+iraCAT7kABDkA";

	const isSignatureValue = (value, originalPath) =>
		typeof value === "string" && value !== originalPath && TOKEN_RE.test(value);

	const findSignatureParam = (config, originalPath) => {
		const params = config && config.params && typeof config.params === "object" ? config.params : {};
		for (const [key, value] of Object.entries(params)) {
			if (isSignatureValue(value, originalPath)) return { key, value };
		}
		try {
			const signedUrl = new URL(config && config.url || "", location.origin);
			if (signedUrl.pathname === new URL(originalPath, location.origin).pathname) {
				for (const [key, value] of signedUrl.searchParams.entries()) {
					if (isSignatureValue(value, originalPath)) return { key, value };
				}
			}
		} catch (_) {}
		return null;
	};

	const captureInstaller = (fn) => {
		let requestHandler = null;
		let responseHandler = null;
		const client = {
			interceptors: {
				request: { use: (h) => { requestHandler = h; } },
				response: { use: (h) => { responseHandler = h; } },
			},
			defaults: { headers: { common: {} }, transformRequest: [], transformResponse: [] },
		};
		const result = fn(client);
		if (result && typeof result.then === "function") return { error: "installer returned async result" };
		if (typeof requestHandler !== "function") return null;
		const supports = SIGNER_PROBE_PATHS.some((path) => {
			const signed = requestHandler({ url: path, method: "get", params: {} }) || {};
			return !!findSignatureParam(signed, path);
		});
		return supports ? { requestHandler, responseHandler } : null;
	};

	const installScrambleMapHelper = (security) => {
		const probeBlob = () => {
			const raw = atob(SCRAMBLE_PROBE_WEBP_BASE64);
			const bytes = new Uint8Array(raw.length);
			for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
			return new Blob([bytes], { type: "image/webp" });
		};
		const parseGrid = (grid) => {
			const match = String(grid || "").match(/^(\d+)x(\d+)$/);
			if (!match) throw new Error("invalid scramble grid");
			const cols = Number(match[1]);
			const rows = Number(match[2]);
			if (cols < 1 || rows < 1 || cols > 20 || rows > 20) throw new Error("unsupported scramble grid");
			return { cols, rows, total: cols * rows };
		};
		const isPermutation = (map, total) =>
			Array.isArray(map) && map.length === total && new Set(map).size === total &&
			map.every((v) => Number.isInteger(v) && v >= 0 && v < total);
		const computeWithRenderer = async (renderer, seed, grid) => {
			const { cols, rows, total } = parseGrid(grid);
			const calls = [];
			const originalFetch = window.fetch;
			const originalDrawImage = CanvasRenderingContext2D.prototype.drawImage;
			window.fetch = async () => new Response(probeBlob(), {
				status: 200,
				headers: {
					"Content-Type": "image/webp",
					"X-Scramble-Seed": String(seed),
					"X-Scramble-Grid": String(grid),
				},
			});
			CanvasRenderingContext2D.prototype.drawImage = function(...args) {
				if (args.length >= 9 && typeof args[1] === "number") calls.push(args.slice(1, 9));
			};
			try {
				const canvas = document.createElement("canvas");
				canvas.width = 100;
				canvas.height = 100;
				const result = renderer("https://comix.to/__aidoku_scramble_probe.webp", canvas);
				if (!result || typeof result.then !== "function") throw new Error("renderer did not return a promise");
				await result;
			} finally {
				window.fetch = originalFetch;
				CanvasRenderingContext2D.prototype.drawImage = originalDrawImage;
			}
			const cellWidth = Math.floor(100 / cols);
			const cellHeight = Math.floor(100 / rows);
			const map = [];
			for (const [sx, sy, , , dx, dy] of calls) {
				const source = Math.floor(sx / cellWidth) + Math.floor(sy / cellHeight) * cols;
				const dest = Math.floor(dx / cellWidth) + Math.floor(dy / cellHeight) * cols;
				if (source >= 0 && source < total && dest >= 0 && dest < total && map[source] === undefined) {
					map[source] = dest;
				}
			}
			if (!isPermutation(map, total)) throw new Error(`invalid tile map (${calls.length} calls)`);
			return map;
		};

		let renderer = null;
		let queue = Promise.resolve();
		const states = (window.__aidokuComixScrambleMapState = {});
		window.__aidokuComixStartScrambleMap = (id, seed, grid) => {
			const key = String(id || "");
			if (!key) return JSON.stringify({ ok: false, done: true, stage: "map-start", message: "missing id" });
			states[key] = { ok: false, done: false, stage: "queued", message: "", map: [] };
			queue = queue.then(async () => {
				let lastError = "";
				const candidates = renderer ? [renderer] : Object.values(security || {}).filter((v) => typeof v === "function");
				for (const candidate of candidates) {
					try {
						const map = await computeWithRenderer(candidate, seed, grid);
						renderer = candidate;
						states[key] = { ok: true, done: true, stage: "ready", message: "", map };
						return;
					} catch (e) {
						lastError = safeMessage(e);
					}
				}
				states[key] = { ok: false, done: true, stage: "renderer-detect", message: lastError || "no scramble renderer matched", map: [] };
			});
			return JSON.stringify({ ok: true, done: false, stage: "queued", message: "" });
		};
		window.__aidokuComixReadScrambleMap = (id) => {
			const key = String(id || "");
			const state = states[key];
			if (!state) return JSON.stringify({ ok: false, done: true, stage: "map-read", message: "request not found", map: [] });
			if (state.done) delete states[key];
			return JSON.stringify(state);
		};
	};

	const installFromModule = (security) => {
		let captured = null;
		let lastError = "";
		for (const value of Object.values(security || {})) {
			if (typeof value !== "function") continue;
			try {
				const candidate = captureInstaller(value);
				if (candidate && candidate.error) { lastError = candidate.error; continue; }
				if (candidate) { captured = candidate; break; }
			} catch (e) { lastError = safeMessage(e); }
		}
		if (!captured) return fail("installer-detect", `no signer installer matched (last=${lastError || "none"})`);

		window.__aidokuComixSigner = (path) => {
			const signed = captured.requestHandler({ url: path, method: "get", params: {} }) || {};
			if (signed && typeof signed.then === "function") return "";
			const param = findSignatureParam(signed, path);
			return param ? JSON.stringify(param) : "";
		};

		if (typeof captured.responseHandler === "function") {
			window.__aidokuComixDecodeResponse = (url, encodedText) => {
				const raw = typeof encodedText === "string" ? JSON.parse(encodedText) : encodedText;
				const decoded = captured.responseHandler({
					data: raw,
					status: 200,
					statusText: "",
					headers: { "x-enc": "1" },
					config: { url, method: "get", baseURL: "/api/v1" },
					request: {},
				}) || { data: null };
				if (decoded && typeof decoded.then === "function") return "error: decoder returned async result";
				return JSON.stringify({ result: decoded && decoded.data });
			};
		}
		installScrambleMapHelper(security);

		try {
			const probe = window.__aidokuComixSigner("/manga/x/chapters");
			const parsed = probe ? JSON.parse(probe) : null;
			if (!parsed || typeof parsed.key !== "string" || typeof parsed.value !== "string") {
				return fail("wrapper-check", "signer wrapper did not return a token-shaped value");
			}
		} catch (e) {
			return fail("wrapper-check", safeMessage(e));
		}
		setState({ ok: true, done: true, stage: "ready", message: "ready" });
	};

	try {
		const importResult = import(securityUrl);
		if (!importResult || typeof importResult.then !== "function") {
			fail("module-import", "dynamic import did not return a promise");
			return;
		}
		importResult.then(installFromModule, (error) => fail("module-import", safeMessage(error)));
	} catch (e) {
		fail("module-import", safeMessage(e));
	}
})();
</script>
</head>
<body></body>
</html>"#;

const READ_INSTALL_STATE_JS: &str = r#"
(() => {
	const state = window.__aidokuComixInstallState;
	if (typeof state === "string") return state;
	return JSON.stringify({
		ok: false,
		done: false,
		stage: "state-read",
		message: "install state missing",
		hasSigner: typeof window.__aidokuComixSigner === "function",
		hasDecoder: typeof window.__aidokuComixDecodeResponse === "function",
	});
})()
"#;

const EMPTY_WEBVIEW_HTML: &str =
	r#"<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>"#;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ApiResponseKind {
	PlainJson,
	EncodedJson,
	Other,
}

pub struct ComixWebView {
	web_view: WebView,
}

pub fn get_api_response(
	web_view: &mut Option<ComixWebView>,
	path: &str,
	url: &str,
) -> Result<String> {
	let mut bodies = get_api_responses(web_view, path, &[url])?;
	bodies
		.pop()
		.ok_or_else(|| error!("Comix signed response missing"))
}

/// Fetches all `urls` (which share one API `path`) in parallel: signs the path
/// once, applies that signature to every URL, then sends them all via
/// `Request::send_all`. Decoding stays sequential because WebView eval can't
/// run concurrently.
pub fn get_api_responses(
	web_view: &mut Option<ComixWebView>,
	path: &str,
	urls: &[&str],
) -> Result<Vec<String>> {
	if urls.is_empty() {
		return Ok(Vec::new());
	}
	let web_view_ref = get_or_create_web_view(web_view)?;
	let (key, value) = match get_signature(web_view_ref, path) {
		Ok(signature) => signature,
		Err(error) => {
			*web_view = None;
			return Err(error);
		}
	};

	let mut signed_urls = Vec::with_capacity(urls.len());
	let mut requests = Vec::with_capacity(urls.len());
	for &url in urls {
		let signed_url = append_query_param(url, &key, &value);
		requests.push(Request::get(&signed_url)?);
		signed_urls.push(signed_url);
	}

	let responses = Request::send_all(requests);

	let mut bodies = Vec::with_capacity(urls.len());
	for (response_result, signed_url) in responses.into_iter().zip(signed_urls.iter()) {
		let response = response_result?;
		let status = response.status_code();
		let body = response.get_string()?;
		bodies.push(match response_kind(&body) {
			ApiResponseKind::PlainJson => body,
			ApiResponseKind::EncodedJson => decode_response(web_view_ref, signed_url, &body)?,
			ApiResponseKind::Other => {
				*web_view = None;
				bail!("Comix signed response did not include result data (status={status})")
			}
		});
	}
	Ok(bodies)
}

pub fn create_web_view() -> Result<ComixWebView> {
	let web_view = WebView::new();
	// The discover script does its own homepage fetch when the empty doc has
	// no module entry, so no Rust-side retry is needed.
	web_view.load_html_blocking(EMPTY_WEBVIEW_HTML, Some(BASE_URL))?;
	install_security_helpers(&web_view)?;
	Ok(ComixWebView { web_view })
}

fn get_or_create_web_view(web_view: &mut Option<ComixWebView>) -> Result<&ComixWebView> {
	if web_view.is_none() {
		*web_view = Some(create_web_view()?);
	}
	Ok(web_view.as_ref().expect("just initialized"))
}

fn install_security_helpers(web_view: &WebView) -> Result<()> {
	let raw = web_view.eval(DISCOVER_SECURITY_MODULE_JS)?;
	let discovery: serde_json::Value = serde_json::from_str(&raw)
		.map_err(|_| error!("Failed to discover Comix security module: invalid result"))?;
	if let Some(error) = json_str(&discovery, "error") {
		bail!("Failed to discover Comix security module: {error}")
	}
	let security_url = json_str(&discovery, "url")
		.ok_or_else(|| error!("Failed to discover Comix security module: missing URL"))?;
	let cfg = json_str(&discovery, "cfg").unwrap_or_default();

	let html = INSTALL_SECURITY_HTML_TEMPLATE
		.replace(
			"__SECURITY_URL_JSON__",
			&serde_json::to_string(security_url)?,
		)
		.replace("__CFG_JSON__", &serde_json::to_string(cfg)?);
	web_view.load_html_blocking(&html, Some(BASE_URL))?;
	wait_for_security_install(web_view)
}

fn wait_for_security_install(web_view: &WebView) -> Result<()> {
	let mut last_stage = String::from("unknown");
	let mut last_message = String::new();

	for attempt in 0..=15 {
		let raw_state = web_view.eval(READ_INSTALL_STATE_JS)?;
		let state: serde_json::Value = serde_json::from_str(&raw_state)
			.map_err(|_| error!("Comix install state was not JSON"))?;

		let done = json_bool(&state, "done");
		let ok = json_bool(&state, "ok");
		let has_signer = json_bool(&state, "hasSigner");
		last_stage = json_str(&state, "stage").unwrap_or("unknown").into();
		last_message = json_str(&state, "message").unwrap_or("").into();

		if done {
			if ok && has_signer {
				return Ok(());
			}
			bail!("Failed to install Comix security helpers: stage={last_stage}: {last_message}");
		}
		if (3..15).contains(&attempt) {
			sleep(1);
		}
	}
	bail!("Comix security helper install timed out: stage={last_stage}: {last_message}")
}

fn json_bool(value: &serde_json::Value, key: &str) -> bool {
	value.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
	value.get(key).and_then(|v| v.as_str())
}

/// Signs `path` once via the JS signer wrapper. The signature is a pure
/// function of the path, so paginated callers reuse it across every URL.
fn get_signature(web_view: &ComixWebView, path: &str) -> Result<(String, String)> {
	let path_json = serde_json::to_string(path)?;
	let result = web_view.web_view.eval(&format!(
		"(() => {{ try {{ \
			if (typeof window.__aidokuComixSigner !== 'function') return ''; \
			return window.__aidokuComixSigner({path_json}); \
		}} catch (_) {{ return ''; }} }})()"
	))?;

	let invalid = || error!("Comix signer wrapper returned an invalid signature");
	if result.is_empty() {
		return Err(invalid());
	}
	let payload: serde_json::Value = serde_json::from_str(&result).map_err(|_| invalid())?;
	let key = json_str(&payload, "key").ok_or_else(invalid)?;
	let value = json_str(&payload, "value").ok_or_else(invalid)?;
	if key.is_empty() || value.is_empty() {
		return Err(invalid());
	}
	Ok((key.into(), value.into()))
}

fn decode_response(web_view: &ComixWebView, url: &str, encoded_res: &str) -> Result<String> {
	let url = serde_json::to_string(url)?;
	let encoded_res = serde_json::to_string(encoded_res)?;
	let result = web_view.web_view.eval(&format!(
		"(() => {{ try {{ \
			if (typeof window.__aidokuComixDecodeResponse !== 'function') return 'error: decoder not installed'; \
			return window.__aidokuComixDecodeResponse({url}, {encoded_res}); \
		}} catch(e) {{ return 'error: ' + e; }} }})()"
	))?;
	if result.starts_with("error:") {
		bail!("Failed to decode Comix response: {result}");
	}
	if result.is_empty() || response_kind(&result) == ApiResponseKind::Other {
		bail!("Comix decoder returned an unexpected response shape")
	}
	Ok(result)
}

pub fn get_scramble_map(
	web_view: &mut Option<ComixWebView>,
	seed: &str,
	grid: &str,
) -> Result<Vec<usize>> {
	let web_view = get_or_create_web_view(web_view)?;
	let request_id = serde_json::to_string(&format!("{seed}:{grid}"))?;
	let seed = serde_json::to_string(seed)?;
	let grid = serde_json::to_string(grid)?;

	let start = scramble_map_eval(
		web_view,
		&format!("window.__aidokuComixStartScrambleMap({request_id}, {seed}, {grid})"),
	)?;
	if json_bool(&start, "done") && !json_bool(&start, "ok") {
		bail!(
			"Failed to start Comix image descrambler: {}",
			scramble_error(&start)
		);
	}

	for attempt in 0..=8 {
		let state = scramble_map_eval(
			web_view,
			&format!("window.__aidokuComixReadScrambleMap({request_id})"),
		)?;
		if json_bool(&state, "done") {
			if !json_bool(&state, "ok") {
				bail!(
					"Failed to build Comix image descrambler map: {}",
					scramble_error(&state)
				);
			}
			return state
				.get("map")
				.and_then(|v| v.as_array())
				.ok_or_else(|| error!("Comix image descrambler returned no tile map"))?
				.iter()
				.map(|v| {
					v.as_u64().map(|n| n as usize).ok_or_else(|| {
						error!("Comix image descrambler returned an invalid tile map")
					})
				})
				.collect();
		}
		if attempt >= 2 {
			sleep(1);
		}
	}

	bail!("Timed out while building Comix image descrambler map")
}

fn scramble_map_eval(web_view: &ComixWebView, body: &str) -> Result<serde_json::Value> {
	let raw = web_view.web_view.eval(&format!(
		"(() => {{ try {{ return {body}; }} \
		catch (e) {{ return JSON.stringify({{ done: true, ok: false, message: String(e) }}); }} }})()"
	))?;
	serde_json::from_str(&raw)
		.map_err(|_| error!("Comix image descrambler returned an invalid WebView result"))
}

fn scramble_error(value: &serde_json::Value) -> String {
	let stage = json_str(value, "stage").unwrap_or("unknown");
	let message = json_str(value, "message").unwrap_or("");
	format!("stage={stage}: {message}")
}

fn response_kind(body: &str) -> ApiResponseKind {
	let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(body) else {
		return ApiResponseKind::Other;
	};

	if map.contains_key("e") {
		ApiResponseKind::EncodedJson
	} else if map.contains_key("result") {
		ApiResponseKind::PlainJson
	} else {
		ApiResponseKind::Other
	}
}

fn append_query_param(url: &str, key: &str, value: &str) -> String {
	let separator = if url.contains('?') { '&' } else { '?' };
	format!(
		"{url}{separator}{}={}",
		encode_uri_component(key),
		encode_uri_component(value)
	)
}
