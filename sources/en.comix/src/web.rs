use crate::{BASE_URL, helpers::create_request_get, models::ErrorResponse};
use aidoku::{
	HashMap, Result,
	alloc::{string::String, string::ToString, vec::Vec},
	helpers::uri::QueryParameters,
	imports::{
		js::WebView,
		net::{Request, Response},
	},
	prelude::*,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

const INSTALLER_REQUEST_TOKEN: &str = "__AIDOKU_INSTALLER_REQUEST_TOKEN__";
const INSTALLER_RESPONSE_TOKEN: &str = "__AIDOKU_INSTALLER_RESPONSE_TOKEN__";

const CF_CHALLENGE_HTML_ERROR_MESSAGE: &str = "Response returned CF challenge page. If problem persist, please clear the source cache and restart the application to resolve this issue.";
const CF_CHALLENGE_ERROR_MESSAGE: &str = "Response returned CF challenge page instead of JSON data. If problem persist, please clear the source cache and restart the application to resolve this issue.";

const WAF_CHALLENGE_KEY: &str = "captcha_required";
const WAF_CHALLENGE_HTML_ERROR_MESSAGE: &str = "Response returned WAF challenge page. Please open Comix Settings and Verify Captcha to resolve this issue.";
const WAF_CHALLENGE_ERROR_MESSAGE: &str = "Response returned WAF challenge page instead of JSON data. Please open Comix Settings and Verify Captcha to resolve this issue.";

#[derive(Deserialize)]
struct AxiosRequest {
	url: String,
	params: Option<HashMap<String, Value>>,
}

pub struct ComixWebView {
	web_view: WebView,
	is_initialized: bool,
}

impl ComixWebView {
	pub fn new() -> Self {
		Self {
			web_view: WebView::new(),
			is_initialized: false,
		}
	}

	fn load_webview(&mut self) -> Result<()> {
		let request = create_request_get(BASE_URL)?;
		let response = request.send()?;

		let status_code = response.status_code();

		if status_code == 403
			&& response
				.get_header("cf-mitigated")
				.is_some_and(|value| value == "challenge")
		{
			bail!("{CF_CHALLENGE_HTML_ERROR_MESSAGE}")
		} else if status_code >= 400 {
			bail!("Response Error: {}", response.status_code())
		} else if response
			.get_html()?
			.select_first("head > title")
			.is_some_and(|e| e.text().is_some_and(|t| t == "Security check"))
		{
			bail!("{}", WAF_CHALLENGE_HTML_ERROR_MESSAGE)
		}

		self.web_view
			.load_html_blocking(response.get_string()?.as_str(), Some(BASE_URL))?;
		self.find_functions(&response)?;
		self.is_initialized = true;
		Ok(())
	}

	fn find_functions(&mut self, response: &Response) -> Result<()> {
		let main_module_src = response
			.get_html()?
			.select_first("head > script[type=\"module\"][src*=\"main\"]")
			.and_then(|e| e.attr("src"))
			.ok_or_else(|| error!("Main module not found"))?;
		let main_module_url = Self::module_url(BASE_URL, &main_module_src)?;
		let main_module = create_request_get(&main_module_url)?.string()?;
		let api_module = main_module
			.split(['\'', '"'])
			.find(|path| {
				path.strip_prefix("./")
					.unwrap_or(path)
					.starts_with("apicore-")
					&& path.ends_with(".js")
			})
			.ok_or_else(|| error!("API module not found"))?;
		let api_module_url = Self::module_url(&main_module_url, api_module)?;
		let source = create_request_get(&api_module_url)?.string()?;
		let module_body = Self::load_module_body(&api_module_url, &source, 0)?;

		let names: Vec<&str> = {
			let words: Vec<&str> = source
				.split(|c: char| !c.is_ascii() || !Self::is_identifier_byte(c as u8))
				.filter(|s| !s.is_empty())
				.collect();
			let mut names: Vec<&str> = words
				.windows(2)
				.filter(|pair| {
					matches!(pair[0], "var" | "let" | "const") && Self::is_identifier(pair[1])
				})
				.map(|pair| pair[1])
				.collect();
			names.sort_unstable();
			names.dedup();
			names
		};
		let names = serde_json::to_string(&names)?;
		let result = self.web_view.eval(&format!(
			"(() => {{
				try {{
					{module_body}
					for (const name of {names}) {{
						try {{
							const client = eval(name);
							const requests = client?.interceptors?.request?.handlers;
							const responses = client?.interceptors?.response?.handlers;
							if (!requests?.length || !responses?.length) continue;
							const request = requests[requests.length - 1]?.fulfilled;
							const response = responses[responses.length - 1]?.fulfilled;
							if (typeof request !== 'function' || typeof response !== 'function') continue;
							window['{INSTALLER_REQUEST_TOKEN}'] = request;
							window['{INSTALLER_RESPONSE_TOKEN}'] = response;
							return 'ok';
						}} catch (e) {{}}
					}}
					return 'API interceptors not found';
				}} catch (e) {{
					return 'JS error: ' + e;
				}}
			}})()"
		))?;
		if result != "ok" {
			bail!("Failed to load API client: {result}");
		}
		Ok(())
	}

	fn module_url(base: &str, path: &str) -> Result<String> {
		if path.starts_with("https://") {
			return Ok(path.into());
		}
		if path.starts_with('/') {
			return Ok(format!("{BASE_URL}{path}"));
		}
		let path = path.strip_prefix("./").unwrap_or(path);
		let directory = base
			.rsplit_once('/')
			.filter(|(dir, _)| dir.len() > "https://".len());
		Ok(format!(
			"{}/{}",
			directory.map_or(base, |(dir, _)| dir),
			path
		))
	}

	fn load_module_body(url: &str, source: &str, depth: usize) -> Result<String> {
		if depth > 4 {
			bail!("API module dependency limit exceeded");
		}
		let (body, _) = Self::split_module(source)?;

		let mut result = String::new();
		let mut offset = 0;

		let keyword = "import";
		let keyword_positions = body.match_indices(keyword).filter_map(move |(start, _)| {
			let end = start + keyword.len();
			let before = start.checked_sub(1).map(|i| body.as_bytes()[i]);
			let after = body.as_bytes().get(end).copied();
			(!before.is_some_and(Self::is_identifier_byte)
				&& !after.is_some_and(Self::is_identifier_byte))
			.then_some(start)
		});

		for start in keyword_positions {
			let Some(rest) = body[start + keyword.len()..].trim_start().strip_prefix('{') else {
				continue;
			};
			let (bindings, rest) = rest
				.split_once('}')
				.ok_or_else(|| error!("Invalid API module import"))?;
			let rest = rest
				.trim_start()
				.strip_prefix("from")
				.ok_or_else(|| error!("Invalid API module import"))?
				.trim_start();
			let quote = rest
				.chars()
				.next()
				.filter(|c| matches!(c, '\'' | '"'))
				.ok_or_else(|| error!("Invalid API module import path"))?;
			let (path, rest) = rest[1..]
				.split_once(quote)
				.ok_or_else(|| error!("Invalid API module import path"))?;
			result.push_str(&body[offset..start]);
			offset = body.len() - rest.len();
			let dependency_url = Self::module_url(url, path)?;
			let dependency = create_request_get(&dependency_url)?.string()?;
			let (_, exports) = Self::split_module(&dependency)?;
			let dependency_body = Self::load_module_body(&dependency_url, &dependency, depth + 1)?;
			let bindings = Self::module_bindings(bindings, true)?;
			let exports = Self::module_bindings(exports, false)?;
			result.push_str(&format!(
				"const {{{bindings}}} = (() => {{{dependency_body}; return {{{exports}}};}})();"
			));
		}
		result.push_str(&body[offset..]);
		Ok(result)
	}

	fn split_module(source: &str) -> Result<(&str, &str)> {
		let (body, exports) = source
			.rsplit_once("export")
			.ok_or_else(|| error!("API module exports not found"))?;
		let exports = exports.trim().trim_end_matches(';').trim_end();
		let exports = exports
			.strip_prefix('{')
			.and_then(|s| s.strip_suffix('}'))
			.filter(|s| !s.contains(['{', '}']))
			.ok_or_else(|| error!("API module exports not found"))?;
		Ok((body, exports))
	}

	fn module_bindings(bindings: &str, import: bool) -> Result<String> {
		let mut result = Vec::new();
		for binding in bindings.split(',').map(str::trim).filter(|s| !s.is_empty()) {
			let parts: Vec<&str> = binding.split_whitespace().collect();
			let (local, exported) = match parts.as_slice() {
				[name] => (*name, *name),
				[name, "as", alias] => (*name, *alias),
				_ => bail!("Unsupported API module binding"),
			};
			if !Self::is_identifier(local) || !Self::is_identifier(exported) {
				bail!("Invalid API module binding");
			}
			result.push(if import {
				format!("{local}:{exported}")
			} else {
				format!("{exported}:{local}")
			});
		}
		Ok(result.join(","))
	}

	fn is_identifier_byte(byte: u8) -> bool {
		byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
	}

	fn is_identifier(name: &str) -> bool {
		!name.is_empty()
			&& !name.as_bytes()[0].is_ascii_digit()
			&& name.bytes().all(Self::is_identifier_byte)
	}
}

impl ComixWebView {
	pub fn build_request(&mut self, url: &str) -> Result<Request> {
		if !self.is_initialized {
			self.load_webview()?
		}

		let result = self.web_view.eval(&format!(
			"(() => {{
			const url = new URL('{url}');
			const result = {{}};

			for (const [key, rawValue] of url.searchParams) {{
				const value = rawValue.length > 0 && [...rawValue].every(c => c >= '0' && c <= '9')
					? Number(rawValue)
					: rawValue;

				const parts = key.split(']').join('').split('[');

				let current = result;

				for (let i = 0; i < parts.length; i++) {{
					const part = parts[i];
					const last = i === parts.length - 1;

					if (last) {{
						if (part === '') {{
							current.push(value);
						}} else if (current[part] === undefined) {{
							current[part] = value;
						}} else if (Array.isArray(current[part])) {{
							current[part].push(value);
						}} else {{
							current[part] = [current[part], value];
						}}
					}} else {{
						const nextPart = parts[i + 1];

						current[part] ??= nextPart === '' ? [] : {{}};
						current = current[part];
					}}
				}}
			}}

			const request = window['{INSTALLER_REQUEST_TOKEN}']({{
				url: `${{url.origin}}${{url.pathname}}`,
				method: 'GET',
				params: result,
			}});

			return JSON.stringify(request);
		}})()"
		))?;

		let axios_request: AxiosRequest = serde_json::from_str(result.as_str())?;

		fn build_query(params_map: &HashMap<String, Value>) -> QueryParameters {
			let mut params = QueryParameters::new();

			for (key, value) in params_map {
				push_value(&mut params, key, value);
			}

			params
		}

		fn push_value(params: &mut QueryParameters, key: &str, value: &Value) {
			match value {
				Value::Null => {
					params.push_key(key);
				}

				Value::Bool(_) | Value::Number(_) | Value::String(_) => {
					let value_str = value.to_string();

					// Remove JSON string quotes
					let value_str = match value {
						Value::String(s) => s.as_str(),
						_ => value_str.as_str(),
					};

					params.push(key, Some(value_str));
				}

				Value::Array(arr) => {
					let array_key = format!("{key}[]");

					for item in arr {
						match item {
							Value::String(s) => {
								params.push(&array_key, Some(s));
							}
							_ => {
								let value_str = item.to_string();
								params.push(&array_key, Some(&value_str));
							}
						}
					}
				}

				Value::Object(obj) => {
					for (child_key, child_value) in obj {
						let nested_key = format!("{key}[{child_key}]");
						push_value(params, &nested_key, child_value);
					}
				}
			}
		}

		if let Some(params) = axios_request.params {
			let query = build_query(&params);
			create_request_get(&format!("{}?{query}", axios_request.url))
		} else {
			create_request_get(&axios_request.url)
		}
	}

	pub fn decode_json_owned<T>(&mut self, response: &Response) -> Result<T>
	where
		T: DeserializeOwned,
	{
		if !self.is_initialized {
			self.load_webview()?;
		}

		let status_code = response.status_code();

		if status_code == 403
			&& response
				.get_header("cf-mitigated")
				.is_some_and(|value| value == "challenge")
		{
			bail!("{CF_CHALLENGE_ERROR_MESSAGE}")
		} else if status_code >= 400 {
			if response.status_code() == 403
				&& serde_json::from_slice::<ErrorResponse>(&response.get_data()?)
					.is_ok_and(|e| e.error == WAF_CHALLENGE_KEY)
			{
				bail!("{}", WAF_CHALLENGE_ERROR_MESSAGE)
			} else {
				bail!("Response Error: {}", response.status_code())
			}
		} else if let Some(enc) = response.get_header("x-enc") {
			let encoded_response = serde_json::to_string(&response.get_string()?)?;
			let enc = serde_json::to_string(&enc)?;

			let result = self.web_view.eval(&format!(
				"(() => {{
					try {{
						let decoded = window['{INSTALLER_RESPONSE_TOKEN}']({{
							data: JSON.parse({encoded_response}),
							status: 200,
							headers: {{
								'x-enc': {enc},
							}},
						}});
						return JSON.stringify({{ result: decoded && decoded.data }});
					}} catch(e) {{
						return 'error: ' + e;
					}}
				}})()",
			))?;

			if result.starts_with("error:") {
				bail!("{result}");
			} else if result.is_empty() {
				bail!("Failed to fetch result")
			}

			serde_json::from_str(&result).map_err(|e| error!("Invalid json: {}", e))
		} else {
			let json_str = response.get_string()?;
			serde_json::from_str(&json_str).map_err(|e| error!("Invalid json: {}", e))
		}
	}
}
