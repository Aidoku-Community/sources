// reference: https://github.com/nobottomline/extensions-source/blob/c8fe930f315f3baee23587559edfceab5e969202/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/Signer.kt
use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::string::String,
	alloc::vec::Vec,
	helpers::uri::QueryParameters,
	imports::net::Response,
	imports::{js::WebView, net::Request},
	prelude::*,
};
use regex::Regex;
use serde::Deserialize;
use serde::de::DeserializeOwned;

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
const DESCRAMBLER_FN_TOKEN: &str = "__AIDOKU_DESCRAMBLER_FN_TOKEN__";

const JS_PATCHER: &str = "<head>\
<script>window['__AIDOKU_CANVAS_TO_DATA_URL_TOKEN__'] = HTMLCanvasElement.prototype.toDataURL;</script>";

#[derive(Deserialize)]
struct AxiosRequest {
	params: Option<AxiosRequestParams>,
}

#[derive(Deserialize)]
struct AxiosRequestParams {
	#[serde(rename = "_")]
	token: Option<String>,
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
				const vmKey = Object.keys(window).find(key => key.startsWith('vm'));
				const vmObj = window[vmKey];
				if (!vmObj || typeof vmObj !== 'object' || vmObj === window) {{
					return '';
				}}
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
		if expr.len() < 2 || expr[1].is_empty() {
			bail!("Failed to find descrambler canvas function");
		}
		if expr.len() < 3 || expr[2].is_empty() {
			bail!("Failed to find descrambler blob function");
		}
		Ok(())
	}

	pub fn create_request(&mut self, url: &str) -> Result<Request> {
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

		if let Some(axios_token) = axios_request.params.and_then(|p| p.token) {
			let mut params = QueryParameters::new();
			params.push("_", Some(axios_token.as_str()));

			if url.contains("?") {
				println!("{}&{}", &url, &params);
				Request::get(format!("{url}&{params}")).map_err(Into::into)
			} else {
				println!("{}?{}", &url, &params);
				Request::get(format!("{url}?{params}")).map_err(Into::into)
			}
		} else {
			println!("{}", &url);
			Request::get(url).map_err(Into::into)
		}
	}

	pub fn decode_json_owned<T>(&mut self, response: &Response) -> Result<T>
	where
		T: DeserializeOwned,
	{
		if !self.is_initialized {
			self.load_webview()?;
		}

		let encoded_response = response.get_string()?;
		let json = serde_json::from_str::<serde_json::Value>(&encoded_response)
			.map_err(|_| error!("Invalid api response"))?;
		let is_encoded = match json {
			serde_json::Value::Object(ref map) => map.contains_key("e"),
			_ => false,
		};
		if !is_encoded {
			return serde_json::from_str(&encoded_response)
				.map_err(|e| error!("Invalid json: {}", e));
		}

		let encoded_res_escaped = encoded_response.replace("'", "\\'");
		let result = self.web_view.eval(&format!(
			"(() => {{
			try {{
				let raw = JSON.parse('{encoded_res_escaped}');
				let fakeResp = {{
					data: raw,
					status: 200,
					statusText: '',
					headers: {{
						'x-enc': '1',
					}},
				}};
				let decoded = window['{INSTALLER_RESPONSE_TOKEN}'](fakeResp);
				return JSON.stringify({{ result: decoded && decoded.data }});
			}} catch(e) {{
				return 'error: ' + e;
			}}
		}})()",
		))?;
		if result.starts_with("error:") {
			bail!("{result}");
		} else if result.is_empty() {
			bail!("Failed to fetch token")
		}
		serde_json::from_str(&result).map_err(|e| error!("Invalid json: {}", e))
	}

	/// * `path`: API path, e.g. "/manga/some-hash/chapters"
	pub fn get_token(&mut self, path: &str) -> Result<String> {
		if !self.is_initialized {
			self.load_webview()?
		}

		let token = self.web_view.eval(&format!(
			"(() => {{
				try {{
					return window['{INSTALLER_REQUEST_TOKEN}']({{ url: '{path}', method: 'GET' }}).params['_'];
				}} catch(e) {{
					return '';
				}}
			}})()"
		))?;
		if token.is_empty() {
			bail!("Failed to fetch token")
		}
		Ok(token)
	}

	pub fn decode_response(&mut self, url: &str, encoded_res: &str) -> Result<String> {
		if !self.is_initialized {
			self.load_webview()?
		}

		let json = serde_json::from_str::<serde_json::Value>(encoded_res)
			.map_err(|_| error!("Invalid api response"))?;
		let is_encoded = match json {
			serde_json::Value::Object(ref map) => map.contains_key("e"),
			_ => false,
		};
		if !is_encoded {
			return Ok(encoded_res.into());
		};

		let encoded_res_escaped = encoded_res.replace("'", "\\'");
		let result = self.web_view.eval(&format!(
			"(() => {{
			try {{
				let raw = JSON.parse('{encoded_res_escaped}');
				let fakeResp = {{
					data: raw,
					status: 200,
					statusText: '',
					headers: {{
						'x-enc': '1',
					}},
					config: {{ url: '{url}', method: 'get', baseURL: '/api/v1' }},
					request: {{}},
				}};
				let decoded = window['{INSTALLER_RESPONSE_TOKEN}'](fakeResp);
				return JSON.stringify({{ result: decoded && decoded.data }});
			}} catch(e) {{
				return 'error: ' + e;
			}}
		}})()",
		))?;
		if result.starts_with("error:") {
			bail!("{result}");
		} else if result.is_empty() {
			bail!("Failed to fetch token")
		}
		Ok(result)
	}

	pub fn descramble_image(&mut self, width: f32, height: f32, url: &str) -> Result<String> {
		if !self.is_initialized {
			self.load_webview()?
		}

		self.web_view.eval(&format!(
			"(() => {{
				const canvas = document.createElement('canvas');
				canvas.width = {width};
				canvas.height = {height};

				window['TEMP_CANVAS'] = canvas;
				window['TEMP_STATE'] = {{ isDone: false, error: null }}

                const controller = new AbortController();
                const signal = controller.signal;

                window['{DESCRAMBLER_FN_TOKEN}']('{url}', signal)
                    .then((data) => {{
                        const url = URL.createObjectURL(data);
                        const image = new Image();
                        image.src = url;
                        image.onload = () => {{
                            URL.revokeObjectURL(url);
                            const ctx = canvas.getContext('2d');
                            ctx.drawImage(image, 0, 0);
                            window['TEMP_STATE'].isDone = true;
                        }}
                    }})
                    .catch((e) => {{ window['TEMP_STATE'].isDone = true; window['TEMP_STATE'].error = e }});

				return '';
			}})()"
		))?;

		while self
			.web_view
			.eval("(() => { return window['TEMP_STATE'].isDone ? 'true' : 'false'; })()")?
			== "false"
		{}

		let result = self.web_view.eval(
			"(() => {{
				if (window['TEMP_STATE'].error) return '';
				const data = window['originalGetImageData'].call(window['TEMP_CANVAS']);
				return data;
			}})()",
		)?;

		if result.is_empty() {
			bail!("Failed to descramble image")
		} else {
			Ok(result)
		}
	}
}
