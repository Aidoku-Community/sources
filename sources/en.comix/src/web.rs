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

pub fn probe_for_function(web_view: &WebView, path: &str) -> Result<String> {
	let result = web_view.eval(&format!(
		"(() => {{
			try {{
				const probe = '{path}';
                const tokenRe = /^[A-Za-z0-9_-]{{40,200}}$/;
                const shortRe = /^[A-Za-z]{{1,3}}$/;
                const nameRe  = /^vm[A-Za-z]_/;

				function tryProbe(ns, topName) {{
                    var sig = '', inst = '';
                    var fnames;
                    try {{ fnames = Object.keys(ns); }} catch (e) {{ return null; }}
                    for (var j = 0; j < fnames.length; j++) {{
                        var fn = ns[fnames[j]];
                        if (typeof fn !== 'function') continue;
                        var ref = fnames[j];
                        if (!sig) {{
                            try {{
                                var out = fn(probe);
                                if (typeof out === 'string' && out !== probe && tokenRe.test(out)) {{
                                    sig = ref;
                                }}
                            }} catch (e) {{}}
                        }}
                        if (!inst) {{
                            try {{
                                var got = false;
                                fn({{
                                    interceptors: {{
                                        request:  {{ use: function() {{}} }},
                                        response: {{ use: function() {{ got = true; }} }}
                                    }},
                                    defaults: {{ headers: {{ common: {{}} }}, transformRequest: [], transformResponse: [] }}
                                }});
                                if (got) inst = ref;
                            }} catch (e) {{}}
                        }}
                        if (sig && inst) return {{ topName: topName, sig: sig, inst: inst }};
                    }}
                    return null;
                }}

				var keys = Object.keys(window);

                // Fast path: matches every observed deploy.
                for (var i = 0; i < keys.length; i++) {{
                    var topName = keys[i];
                    if (!nameRe.test(topName)) continue;
                    var ns = window[topName];
                    if (!ns || typeof ns !== 'object' || ns === window) continue;
                    var hit = tryProbe(ns, topName);
                    if (hit) return JSON.stringify(hit);
                }}

				// Fallback: structural fingerprint, no name constraint.
                for (var i = 0; i < keys.length; i++) {{
                    var topName = keys[i];
                    if (nameRe.test(topName)) continue; // already tried
                    var ns = window[topName];
                    if (!ns || typeof ns !== 'object' || ns === window) continue;
                    var fnames;
                    try {{ fnames = Object.keys(ns); }} catch (e) {{ continue; }}
                    if (fnames.length < 5) continue;
                    var shortAlpha = 0;
                    for (var s = 0; s < fnames.length; s++) {{
                        if (shortRe.test(fnames[s])) shortAlpha++;
                    }}
                    if (shortAlpha < 3) continue;
                    var hit = tryProbe(ns, topName);
                    if (hit) return JSON.stringify(hit);
                }}

				// This probably won't happen but just in case
				return '';
			}} catch(e) {{
				return '';
			}}
		}})()"
	))?;
	if result.is_empty() {
		bail!("Failed to fetch token")
	}
	Ok(result)
}

/// * `path`: API path, e.g. "/manga/some-hash/chapters"
pub fn get_token(web_view: &WebView, path: &str, js_function: &str) -> Result<String> {
	let token = web_view.eval(&format!(
		"(() => {{
			try {{
				const vmFnName = JSON.parse('{js_function}');
				const vmObj = window[vmFnName.topName];
				if (!vmObj || typeof vmObj[vmFnName.sig] !== 'function') {{
				    return '';
				}}
				return vmObj[vmFnName.sig]('{path}');
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

pub fn decode_response(web_view: &WebView, url: &str, encoded_res: &str, js_function: &str) -> Result<String> {
	let result = web_view.eval(&format!(
		"(() => {{
			try {{
                const vmFnName = JSON.parse('{js_function}');
				const vmObj = window[vmFnName.topName];
				if (!vmObj || typeof vmObj[vmFnName.sig] !== 'function') {{
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
				vmObj[vmFnName.inst](fakeAxios);

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
