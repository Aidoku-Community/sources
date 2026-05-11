// reference: https://github.com/nobottomline/extensions-source/blob/c8fe930f315f3baee23587559edfceab5e969202/src/en/comix/src/eu/kanade/tachiyomi/extension/en/comix/Signer.kt
use crate::BASE_URL;
use aidoku::{
	Result,
	alloc::string::String,
	imports::{js::WebView, net::Request},
	prelude::*,
};

const GET_VMOBJ_JS: &str = "\
const vmKey = Object.keys(window).find(key => key.startsWith('vm'));\
const vmObj = window[vmKey];\
if (!vmObj || typeof vmObj !== 'object' || vmObj === window) {\
	return '';\
}";

pub struct ComixWebView {
	web_view: WebView,
	signer_fn: Option<String>,
	installer_fn: Option<String>,
}

pub fn create_web_view() -> Result<ComixWebView> {
	let web_view = WebView::new();
	web_view.load_blocking(Request::get(BASE_URL)?)?;
	let mut comix_web_view = ComixWebView {
		web_view,
		signer_fn: None,
		installer_fn: None,
	};
	find_functions(&mut comix_web_view)?;
	Ok(comix_web_view)
}

fn find_functions(web_view: &mut ComixWebView) -> Result<()> {
	let result = web_view.web_view.eval(&format!(
		"(() => {{
			try {{
				{GET_VMOBJ_JS}
				let probe = '/manga/x/chapters';
				let tokenRe = /^[A-Za-z0-9_-]{{40,200}}$/;
				let sig = '', inst = '';
				let fnames = Object.keys(vmObj);
				for (let j = 0; j < fnames.length; j++) {{
					let fn = vmObj[fnames[j]];
					if (typeof fn !== 'function') continue;
					let ref = 'window[' + JSON.stringify(vmKey) + '].' + fnames[j];
					if (!sig) {{
						try {{
							let out = fn(probe);
							if (typeof out === 'string' && out !== probe && tokenRe.test(out)) {{
								sig = ref;
								continue;
							}}
						}} catch (e) {{}}
					}}
					if (!inst) {{
						try {{
							let got = false;
							fn({{
								interceptors: {{
									request:{{ use: function() {{}} }},
									response: {{ use: function() {{ got = true; }} }}
								}},
								defaults: {{
									headers: {{ common: {{}} }},
									transformRequest: [],
									transformResponse: []
								}}
							}});
							if (got) inst = ref;
						}} catch (e) {{}}
					}}
				}}
				return sig + '||' + inst;
			}} catch(e) {{
				return '';
			}}
		}})()",
	))?;
	let Some((sig_expr, inst_expr)) = result.split_once("||") else {
		bail!("Failed to find signer and installer functions")
	};
	web_view.signer_fn = Some(sig_expr.into());
	web_view.installer_fn = Some(inst_expr.into());
	Ok(())
}

/// * `path`: API path, e.g. "/manga/some-hash/chapters"
pub fn get_token(web_view: &ComixWebView, path: &str) -> Result<String> {
	let Some(signer_fn) = web_view.signer_fn.as_ref() else {
		bail!("Missing installer function")
	};
	let token = web_view.web_view.eval(&format!(
		"(() => {{
			try {{
				{GET_VMOBJ_JS}
				return {signer_fn}('{path}');
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
	let result = web_view.web_view.eval(&format!(
		"(() => {{
			try {{
				{GET_VMOBJ_JS}
				let captured = {{ req: null, res: null }};
				{installer_fn}({{
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
				}});

				let raw = JSON.parse('{encoded_res}');
				let bodyOut;
				if (raw && typeof raw === 'object' && 'e' in raw && captured.res) {{
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
