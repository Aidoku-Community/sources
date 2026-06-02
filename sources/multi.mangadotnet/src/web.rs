use crate::BASE_URL;
use crate::CF_CHALLENGE_ERROR_MESSAGE;
use aidoku::{
	Result, alloc::format, alloc::string::String, bail, error, imports::js::WebView,
	imports::net::Request, println,
};
use serde::Deserialize;

const FETCH_RESPONSE_TOKEN: &str = "__AIDOKU_FETCH_RESPONSE_TOKEN__";
const EMPTY_FETCH_RESPONSE_OBJECT: &str =
	"{ data: null, error: null, isDone: false, isAbort: false }";

pub struct MangaDotnetWebView {
	web_view: WebView,
	is_initialized: bool,
}

#[derive(Deserialize)]
struct FetchResponseObject {
	data: Option<String>,
	error: Option<String>,
}

impl MangaDotnetWebView {
	pub fn new() -> Self {
		let web_view = WebView::new();
		Self {
			web_view,
			is_initialized: false,
		}
	}

	fn load_webview(&mut self) -> Result<()> {
		self.web_view.load_blocking(Request::get(BASE_URL)?)?;
		self.is_initialized = true;
		Ok(())
	}

	pub fn fetch(&mut self, url: &str, retry: i32) -> Result<String> {
		if !self.is_initialized {
			self.load_webview()?;
		}

		println!("Fetching {}", url);

		self.web_view.eval(&format!(
			"(() => {{
			window['{FETCH_RESPONSE_TOKEN}'] = {EMPTY_FETCH_RESPONSE_OBJECT};

			const controller = new AbortController();
			const signal = controller.signal;

			let cancelled = false;

			const timeout = setTimeout(() => {{
				cancelled = true;
				controller.abort();
				window['{FETCH_RESPONSE_TOKEN}'].isAbort = true;
			}}, 30000);

			fetch('{url}', {{ signal: signal }})
				.then((response) => {{
					if (!response.ok) {{
						if (response.headers.get('cf-mitigated') === 'challenge') {{
							throw new Error(`{CF_CHALLENGE_ERROR_MESSAGE}`);
						}}
						throw new Error(`Response status: ${{response.status}}`);
					}}
					const contentType = response.headers.get('content-type');
					if (contentType.startsWith('image')) {{
						return response.blob();
					}} else {{
						return response.text();
					}}
				}})
				.then((data) => {{
					if (typeof data === 'string') {{
						return Promise.resolve(data);
					}} else {{
						if (cancelled) throw new Error('Fetch aborted');
						return createImageBitmap(data);
					}}
				}})
				.then((data) => {{
					if (typeof data === 'string') {{
						window['{FETCH_RESPONSE_TOKEN}'].data = data;
					}} else {{
						if (cancelled) throw new Error('Fetch aborted');
						const canvas = document.createElement('canvas');
						canvas.width = data.width;
						canvas.height = data.height;
						const ctx = canvas.getContext('2d');
						ctx.drawImage(data, 0, 0);
						window['{FETCH_RESPONSE_TOKEN}'].data = canvas.toDataURL();
					}}
				}})
				.catch((error) => window['{FETCH_RESPONSE_TOKEN}'].error = error.message)
				.finally(() => {{
					clearTimeout(timeout);
					window['{FETCH_RESPONSE_TOKEN}'].isDone = true;
				}});
			return '';
		}})()"
		))?;

		while self.web_view.eval(&format!(
			"(() => {{ return window['{FETCH_RESPONSE_TOKEN}'].isDone ? 'true' : 'false'; }})()"
		))? == "false"
		{
			if self.web_view.eval(&format!(
				"(() => {{ return window['{FETCH_RESPONSE_TOKEN}'].isAbort ? 'true' : 'false'; }})()"
			))? == "true"
			{
				self.load_webview()?;
				bail!("Fetch aborted after 30 seconds.");
			}
		}

		let result = self.web_view.eval(&format!(
			"(() => {{ return JSON.stringify(window['{FETCH_RESPONSE_TOKEN}']); }})()"
		))?;

		let json = serde_json::from_str::<FetchResponseObject>(&result)?;

		if let Some(error) = json.error {
			if retry >= 1 {
				bail!("{error}");
			}

			if error == CF_CHALLENGE_ERROR_MESSAGE {
				self.load_webview()?;
				return self.fetch(url, retry + 1);
			}

			bail!("{error}");
		}

		json.data.ok_or(error!("Fetch data is empty"))
	}
}
