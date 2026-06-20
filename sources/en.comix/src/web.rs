// reference: https://github.com/nobottomline/extensions-source/blob/c8fe930f315f3baee23587559edfceab5e969202/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/Signer.kt
use crate::BASE_URL;
use aidoku::{
	HashMap, Result,
	alloc::string::String,
	alloc::string::ToString,
	alloc::vec::Vec,
	helpers::uri::QueryParameters,
	imports::net::Response,
	imports::{js::WebView, net::Request},
	prelude::*,
};
use regex::Regex;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const GET_VMOBJ_JS: &str = "\
const vmKey = Object.keys(window).find(key => key.startsWith('vm'));\
const vmObj = window[vmKey];\
if (!vmObj || typeof vmObj !== 'object' || vmObj === window) {\
	return '';\
}";

const CANVAS_TO_DATA_URL_TOKEN: &str = "__AIDOKU_CANVAS_TO_DATA_URL_TOKEN__";

const INSTALLER_REQUEST_TOKEN: &str = "__AIDOKU_INSTALLER_REQUEST_TOKEN__";
const INSTALLER_RESPONSE_TOKEN: &str = "__AIDOKU_INSTALLER_RESPONSE_TOKEN__";

const DESCRAMBLER_BLOB_TOKEN: &str = "__AIDOKU_DESCRAMBLER_BLOB_TOKEN__";
const DESCRAMBLER_CANVAS_TOKEN: &str = "__AIDOKU_DESCRAMBLER_CANVAS_TOKEN__";

const DESCRAMBLER_RESPONSE_TOKEN: &str = "__AIDOKU_DESCRAMBLER_RESPONSE_TOKEN__";
const EMPTY_DESCRAMBLER_RESPONSE_OBJECT: &str =
	"{ data: null, error: null, isDone: false, isAbort: false }";
const FETCH_TIMEOUT_RESPONSE: &str =
	"Fetch timeout after 30s. If problem persist, please restart the application.";

const JS_PATCHER: &str = "<head>\
<script>window['__AIDOKU_CANVAS_TO_DATA_URL_TOKEN__'] = HTMLCanvasElement.prototype.toDataURL;</script>";

const CF_CHALLENGE_ERROR_MESSAGE: &str = "Response returned CF challenge page instead of JSON data. If problem persist, please clear the source cache and restart the application to resolve this issue.";

#[derive(Deserialize)]
struct AxiosRequest {
	url: String,
	params: Option<HashMap<String, Value>>,
}

#[derive(Deserialize)]
struct DescrambleResponseObject {
	data: Option<String>,
	error: Option<String>,
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
		self.web_view.load_html_blocking(
			Request::get(BASE_URL)?
				.string()?
				.replace("<head>", JS_PATCHER)
				.as_str(),
			Some(BASE_URL),
		)?;
		if self.find_functions().is_err() {
			self.find_secure_module_src()?;
			self.find_functions()?;
		}
		self.is_initialized = true;
		Ok(())
	}

	fn find_secure_module_src(&mut self) -> Result<()> {
		let main_module_src = Request::get(BASE_URL)?
			.html()?
			.select("head > script[type=\"module\"][src*=\"main\"]")
			.and_then(|e| e.first())
			.and_then(|e| e.attr("src"))
			.ok_or(error!("Main module not found"))?;
		if let Some(js_asset_path_index) = main_module_src.rfind("/") {
			let js_asset_path = &main_module_src[0..js_asset_path_index + 1];
			let secure_script_regex = Regex::new("(secure-[A-Za-z0-9-_]+?\\.js)").unwrap();
			let main_module_contents =
				Request::get(format!("{BASE_URL}{main_module_src}"))?.string()?;
			if let Some(secure_script_path) = secure_script_regex
				.captures(main_module_contents.as_str())
				.and_then(|captures| captures.get(1).map(|m| m.as_str()))
			{
				self.web_view.eval(&format!(
					"(() => {{
						import('{BASE_URL}{js_asset_path}{secure_script_path}')
						.then((m) => window['vm'] = m)
						.catch((e) => window['vm'] = {{}});
						return '';
					}})()"
				))?;
				while self
					.web_view
					.eval("(() => { return window['vm'] == null ? 'true' : 'false'; })()")?
					== "true"
				{}
				Ok(())
			} else {
				bail!("Secure module not found");
			}
		} else {
			bail!("Invalid path")
		}
	}

	fn find_functions(&mut self) -> Result<()> {
		let result = self.web_view.eval(&format!(
			"(() => {{
			try {{
				{GET_VMOBJ_JS}
				let fnames = Object.keys(vmObj);
				let inst = '', descBlob = '', descCanvas = '';
				const isPromise = (v) => v && (typeof v === 'object' || typeof v === 'function') && typeof v.then === 'function';
				const canvas = document.createElement('canvas');
				const controller = new AbortController();
                const signal = controller.signal;
				for (let j = 0; j < fnames.length; j++) {{
					let fn = vmObj[fnames[j]];
					if (typeof fn !== 'function') continue;
					let ref = 'window[' + JSON.stringify(vmKey) + '].' + fnames[j];
					if (!inst) {{
						try {{
							let got = false;
							fn({{
								interceptors: {{
									request: {{ use: function() {{ got = true; }} }},
									response: {{ use: function() {{ got = true; }} }}
								}}
							}});
							if (got) {{
								inst = ref;
								fn({{
									interceptors: {{
										request: {{
											use: function (fn) {{ window['{INSTALLER_REQUEST_TOKEN}'] = fn; }},
										}},
										response: {{
											use: function (fn) {{ window['{INSTALLER_RESPONSE_TOKEN}'] = fn; }},
										}},
									}}
								}});
							}}
						}} catch (e) {{}}
					}}
					if (!descCanvas) {{
						try {{
							if (fn.length == 3) {{
								let res = fn('about:blank', canvas, signal);
								if (isPromise(res)) {{
									descCanvas = ref;
									window['{DESCRAMBLER_CANVAS_TOKEN}'] = fn;
								}}
							}}
						}} catch (e) {{}}
					}}
					if (!descBlob) {{
						try {{
							if (fn.length == 2) {{
								let res = fn('about:blank', signal);
								if (isPromise(res)) {{
									descBlob = ref;
									window['{DESCRAMBLER_BLOB_TOKEN}'] = fn;
								}}
							}}
						}} catch (e) {{}}
					}}
				}}
				return inst + '||' + descCanvas + '||' + descBlob;
			}} catch (e) {{}}
			return '';
		}})()",
		))?;
		let expr: Vec<&str> = result.split("||").collect();
		if expr.is_empty() {
			bail!("Failed to find installer and descrambler functions")
		}
		if expr[0].is_empty() {
			bail!("Failed to find installer function");
		}
		if expr[1].is_empty() && expr[2].is_empty() {
			bail!("Failed to find descrambler canvas/blob function");
		}
		Ok(())
	}

	pub fn build_request(&mut self, url: &str) -> Result<Request> {
		if !self.is_initialized {
			self.load_webview()?
		}

		let result = self.web_view.eval(&format!(
			"(() => {{
			const url = new URL('{url}');
			const result = {{}};

			for (const [key, rawValue] of url.searchParams) {{
				const value = /^\\d+$/.test(rawValue)
					? Number(rawValue)
					: rawValue;

				const parts = key.replace(/\\]/g, '').split('[');

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
			Request::get(format!("{}?{query}", axios_request.url)).map_err(Into::into)
		} else {
			Request::get(axios_request.url).map_err(Into::into)
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
			bail!("Response Error: {}", status_code)
		} else if response
			.get_header("x-enc")
			.is_some_and(|value| value == "1")
		{
			let encoded_response = response
				.get_string()?
				.replace("\\", "\\\\")
				.replace("'", "\\'");

			let result = self.web_view.eval(&format!(
				"(() => {{
					try {{
						let decoded = window['{INSTALLER_RESPONSE_TOKEN}']({{
							data: JSON.parse('{encoded_response}'),
							status: 200,
							headers: {{
								'x-enc': '1',
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

	pub fn descramble_image(&mut self, width: f32, height: f32, url: &str) -> Result<String> {
		if !self.is_initialized {
			self.load_webview()?
		}

		self.web_view.eval(&format!(
			"(() => {{
				window['{DESCRAMBLER_RESPONSE_TOKEN}'] = {EMPTY_DESCRAMBLER_RESPONSE_OBJECT};

				const controller = new AbortController();
                const signal = controller.signal;

				const canvas = document.createElement('canvas');
				canvas.width = {width};
				canvas.height = {height};

				const timeout = setTimeout(() => {{
					controller.abort();
					window['{DESCRAMBLER_RESPONSE_TOKEN}'].isAbort = true;
				}}, 30000);

				if (window['{DESCRAMBLER_BLOB_TOKEN}'] != null) {{
					window['{DESCRAMBLER_BLOB_TOKEN}']('{url}', signal)
						.then((data) => {{
							if (typeof data === 'object' && data.mode) {{
								// So they are using new format:
								if (data.mode === 'blob') {{
									return new Promise((resolve, reject) => {{
										const url = URL.createObjectURL(data.blob);
										const image = new Image();
										image.src = url;
										image.onload = () => resolve(image);
										image.onerror = reject;
									}})
								}} else if (data.mode === 'canvas') {{
									data.apply(canvas)
									const output = window['{CANVAS_TO_DATA_URL_TOKEN}'].call(canvas);
									window['{DESCRAMBLER_RESPONSE_TOKEN}'].data = output;
									window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone = true;
									clearTimeout(timeout);
								}}
								return null;
							}} else {{
								// Let's assume they undo this so let keep this as old behaviour
								return new Promise((resolve, reject) => {{
									const url = URL.createObjectURL(data);
									const image = new Image();
									image.src = url;
									image.onload = () => resolve(image);
									image.onerror = reject;
								}})
							}}
						}})
						.then((obj) => {{
							if (typeof obj === 'object' && obj) {{
								URL.revokeObjectURL(obj.src);
								const ctx = canvas.getContext('2d');
								ctx.drawImage(obj, 0, 0);
								const data = window['{CANVAS_TO_DATA_URL_TOKEN}'].call(canvas);
								window['{DESCRAMBLER_RESPONSE_TOKEN}'].data = data;
								window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone = true;
								clearTimeout(timeout);
							}}
						}})
						.catch((error) => {{
							if (window['{DESCRAMBLER_CANVAS_TOKEN}'] != null) {{
								window['{DESCRAMBLER_CANVAS_TOKEN}']('{url}', canvas, signal)
									.then(() => {{
										const data = window['{CANVAS_TO_DATA_URL_TOKEN}'].call(canvas);
										window['{DESCRAMBLER_RESPONSE_TOKEN}'].data = data;
									}})
									.catch((error) => {{
										window['{DESCRAMBLER_RESPONSE_TOKEN}'].error = error.message;
									}})
									.finally(() => {{
										window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone = true;
										clearTimeout(timeout);
									}});
							}} else {{
								window['{DESCRAMBLER_RESPONSE_TOKEN}'].error = error.message;
								window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone = true;
								clearTimeout(timeout);
							}}
						}});
				}} else if (window['{DESCRAMBLER_CANVAS_TOKEN}'] != null) {{
					window['{DESCRAMBLER_CANVAS_TOKEN}']('{url}', canvas, signal)
						.then(() => {{
							const data = window['{CANVAS_TO_DATA_URL_TOKEN}'].call(canvas);
							window['{DESCRAMBLER_RESPONSE_TOKEN}'].data = data;
						}})
						.catch((error) => {{
							window['{DESCRAMBLER_RESPONSE_TOKEN}'].error = error.message;
						}})
						.finally(() => {{
							window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone = true;
							clearTimeout(timeout);
						}});
				}} else {{
					window['{DESCRAMBLER_RESPONSE_TOKEN}'].error = 'No suitable descrambler found.';
					window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone = true;
					clearTimeout(timeout);
				}}

				return '';
			}})()"
		))?;

		while self.web_view.eval(&format!(
			"(() => {{ return window['{DESCRAMBLER_RESPONSE_TOKEN}'].isDone ? 'true' : 'false'; }})()"
		))? == "false"
		{
			if self.web_view.eval(&format!(
				"(() => {{ return window['{DESCRAMBLER_RESPONSE_TOKEN}'].isAbort ? 'true' : 'false'; }})()"
			))? == "true"
			{
				self.load_webview()?;
				bail!("{FETCH_TIMEOUT_RESPONSE}");
			}
		}

		let result = self.web_view.eval(&format!(
			"(() => {{ return JSON.stringify(window['{DESCRAMBLER_RESPONSE_TOKEN}']); }})()"
		))?;

		let json = serde_json::from_str::<DescrambleResponseObject>(&result)?;

		if let Some(error) = json.error {
			bail!("{error}");
		}

		json.data.ok_or(error!("Fetch data is null"))
	}
}
