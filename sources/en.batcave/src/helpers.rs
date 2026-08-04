use crate::{REFERER, TRUST_COOKIE_KEY, USER_AGENT, VERIFY_KEY};
use aidoku::{
	Result,
	imports::{defaults::defaults_get_map, html::Document, net::Request},
	prelude::*,
};

pub trait BatCaveHtml {
	fn batcave_html(self) -> Result<Document>;
}

impl BatCaveHtml for Request {
	fn batcave_html(self) -> Result<Document> {
		let mut request = self
			.header("Referer", REFERER)
			.header("User-Agent", USER_AGENT);
		if let Some(token) =
			defaults_get_map(VERIFY_KEY).and_then(|cookies| cookies.get(TRUST_COOKIE_KEY).cloned())
		{
			request = request.header("Cookie", &format!("{TRUST_COOKIE_KEY}={token}"));
		}
		let html = request.html()?;

		let is_title_empty = html
			.select_first("title")
			.and_then(|el| el.text())
			.is_none_or(|s| s.is_empty());
		if is_title_empty {
			bail!("Verification required in settings.")
		}

		Ok(html)
	}
}
