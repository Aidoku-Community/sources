// reference: https://github.com/nobottomline/extensions-source/blob/c8fe930f315f3baee23587559edfceab5e969202/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/Signer.kt
use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::string::String,
	imports::{js::WebView, net::Request},
	prelude::*,
};

pub fn create_web_view() -> Result<WebView> {
	let web_view = WebView::new();
	web_view.load_blocking(Request::get(BASE_URL)?)?;
	Ok(web_view)
}

/// * `path`: API path, e.g. "/manga/some-hash/chapters"
pub fn get_token(web_view: &WebView, path: &str) -> Result<String> {
	let token = web_view.eval(&format!(
		"(() => {{
			try {{
				const vmKey = Object.keys(window).find(key => key.startsWith('vm'));
				const vmObj = window[vmKey];
				if (!vmObj || typeof vmObj.Qi !== 'function') {{
				    return '';
				}}
				return vmObj.Qi('{path}');
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

pub fn decode_response(web_view: &WebView, url: &str, encoded_res: &str) -> Result<String> {
	let result = web_view.eval(&format!(
		"(() => {{
			try {{
				const vmKey = Object.keys(window).find(key => key.startsWith('vm'));
				const vmObj = window[vmKey];
				if (!vmObj || typeof vmObj.Qi !== 'function') {{
				    return '';
				}}
				var captured = {{ req: null, res: null }};
				var fakeAxios = {{
					interceptors: {{
						request: {{
							use: function (fn) {{
								captured.req = fn;
							}},
						}},
						response: {{
							use: function (fn) {{
								captured.res = fn;
							}},
						}},
					}},
					defaults: {{
						headers: {{ common: {{}} }},
						transformRequest: [],
						transformResponse: [],
					}},
				}};
				vmObj.v(fakeAxios);

				var raw = JSON.parse('{encoded_res}');
				var bodyOut;
				if (raw && typeof raw === 'object' && 'e' in raw && captured.res) {{
					var fakeResp = {{
						data: raw,
						status: 200,
						statusText: '',
						headers: {{
							'x-enc': '1',
						}},
						config: {{ url: '{url}', method: 'get', baseURL: '/api/v1' }},
						request: {{}},
					}};
					var decoded = captured.res(fakeResp);
					bodyOut = JSON.stringify({{ result: decoded && decoded.data }});
				}} else if (raw && typeof raw === 'object' && 'result' in raw) {{
					bodyOut = text;
				}} else {{
					bodyOut = JSON.stringify({{ result: raw }});
				}}
				return bodyOut;
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
