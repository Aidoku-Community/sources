// reference: https://github.com/nobottomline/extensions-source/blob/c8fe930f315f3baee23587559edfceab5e969202/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/Signer.kt
use crate::BASE_URL;
use aidoku::{
	AidokuError, Result,
	alloc::string::String,
	imports::{js::WebView, net::Request},
	prelude::*,
};
use regex::Regex;

const GET_VMOBJ_JS: &str = "\
const vmKey = Object.keys(window).find(key => key.startsWith('vm'));\
const vmObj = window[vmKey];\
if (!vmObj || typeof vmObj !== 'object' || vmObj === window) {\
	return '';\
}";

pub struct ComixWebView {
	web_view: WebView,
	installer_fn: Option<String>,
}

pub fn create_web_view() -> Result<ComixWebView> {
	let web_view = WebView::new();
	web_view.load_blocking(Request::get(BASE_URL)?)?;
	let mut comix_web_view = ComixWebView {
		web_view,
		installer_fn: None,
	};
	if find_functions(&mut comix_web_view).is_err() {
		find_secure_module_src(&mut comix_web_view)?;
		find_functions(&mut comix_web_view)?;
	}
	Ok(comix_web_view)
}

fn find_secure_module_src(web_view: &mut ComixWebView) -> Result<()> {
	let main_module_src = Request::get(BASE_URL)?
		.html()?
		.select("head > script[type=\"module\"][src*=\"main\"]")
		.and_then(|e| e.first())
		.and_then(|e| e.attr("src"))
		.ok_or(AidokuError::message("Main module not found"))?;
	if let Some(js_asset_path_index) = main_module_src.rfind("/") {
		let js_asset_path = &main_module_src[0..js_asset_path_index + 1];
		let secure_script_regex = Regex::new("(secure-[A-Za-z0-9-_]+?\\.js)").unwrap();
		let main_module_contents =
			Request::get(format!("{BASE_URL}{main_module_src}"))?.string()?;
		if let Some(captures) = secure_script_regex.captures(main_module_contents.as_str()) {
			let secure_script_path = captures.get(1).unwrap().as_str();
			web_view.web_view.eval(&format!(
				"(() => {{
				import('{BASE_URL}{js_asset_path}{secure_script_path}')
					.then((m) => window['vm'] = m)
					.catch((e) => window['vm'] = {{}});
				return '';
			}})()"
			))?;
			while web_view
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

fn find_functions(web_view: &mut ComixWebView) -> Result<()> {
	let result = web_view.web_view.eval(&format!(
		"(() => {{
			try {{
				{GET_VMOBJ_JS}
				let fnames = Object.keys(vmObj);
				for (let j = 0; j < fnames.length; j++) {{
					let fn = vmObj[fnames[j]];
					if (typeof fn !== 'function') continue;
					try {{
						let got = false;
						fn({{
							interceptors: {{
								request: {{ use: function() {{}} }},
								response: {{ use: function() {{ got = true; }} }}
							}},
							defaults: {{
								headers: {{ common: {{}} }},
								transformRequest: [],
								transformResponse: []
							}}
						}});
						if (got) return 'window[' + JSON.stringify(vmKey) + '].' + fnames[j];
					}} catch (e) {{}}
				}}
			}} catch(e) {{}}
			return '';
		}})()",
	))?;
	if result.is_empty() {
		bail!("Failed to find installer function");
	};
	web_view.installer_fn = Some(result);
	Ok(())
}

/// * `path`: API path, e.g. "/manga/some-hash/chapters"
pub fn get_token(web_view: &ComixWebView, path: &str) -> Result<String> {
	let Some(installer_fn) = web_view.installer_fn.as_ref() else {
		bail!("Missing installer function")
	};
	let token = web_view.web_view.eval(&format!(
		"(() => {{
			try {{
				{GET_VMOBJ_JS}
				let captured = {{ req: null, res: null }};
				{installer_fn}({{
					interceptors: {{
						request: {{
							use: function (fn) {{ captured.req = fn; }},
						}},
						response: {{
							use: function (fn) {{ captured.res = fn; }},
						}},
					}},
					defaults: {{
						headers: {{ common: {{}} }},
						transformRequest: [],
						transformResponse: []
					}}
				}});
				return captured.req({{ url: '{path}', method: 'GET' }}).params['_'];
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

pub fn decode_response(web_view: &ComixWebView, url: &str, encoded_res: &str) -> Result<String> {
	let Some(installer_fn) = web_view.installer_fn.as_ref() else {
		bail!("Missing installer function")
	};

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
	let result = web_view.web_view.eval(&format!(
		"(() => {{
			try {{
				{GET_VMOBJ_JS}
				let captured = {{ req: null, res: null }};
				{installer_fn}({{
					interceptors: {{
						request: {{
							use: function (fn) {{ captured.req = fn; }},
						}},
						response: {{
							use: function (fn) {{ captured.res = fn; }},
						}},
					}},
					defaults: {{
						headers: {{ common: {{}} }},
						transformRequest: [],
						transformResponse: []
					}}
				}});
				if (!captured.res) {{
					return 'error: could not capture response handler';
				}}

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
				let decoded = captured.res(fakeResp);
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
