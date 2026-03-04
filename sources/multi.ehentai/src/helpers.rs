use crate::USER_AGENT;
use crate::settings::{
	build_cookie_header, get_base_url, get_domain, get_ipb_member_id, get_ipb_pass_hash,
	refresh_igneous_from_set_cookie,
};
use aidoku::{
	Result,
	alloc::{Vec, string::String, string::ToString},
	imports::{
		html::Document,
		net::{Request, Response},
	},
	prelude::*,
};

pub fn eh_get_html(url: &str, cookies: &str, user_agent: &str) -> Result<Document> {
	let do_request = |cookie_header: &str| -> Result<Response> {
		Ok(Request::get(url)?
			.header("Cookie", cookie_header)
			.header("User-Agent", user_agent)
			.send()?)
	};

	let mut resp = do_request(cookies)?;

	if url.contains("exhentai.org")
		&& let Some(set_cookie) = resp.get_header("Set-Cookie")
	{
		refresh_igneous_from_set_cookie(&set_cookie);
	}

	let doc = resp.get_html()?;

	if url.contains("exhentai.org") {
		let is_rejected = doc
			.select_first("body")
			.and_then(|b| b.select_first("div"))
			.is_none();

		if is_rejected {
			let base_cookie = {
				let member_id = get_ipb_member_id();
				let pass_hash = get_ipb_pass_hash();
				let mut parts: Vec<String> = Vec::new();
				parts.push("nw=1".into());
				if !member_id.is_empty() {
					parts.push(format!("ipb_member_id={}", member_id));
				}
				if !pass_hash.is_empty() {
					parts.push(format!("ipb_pass_hash={}", pass_hash));
				}
				parts.join("; ")
			};

			if let Ok(probe_req) = Request::get("https://exhentai.org")
				&& let Ok(probe) = probe_req
					.header("Cookie", &base_cookie)
					.header("User-Agent", user_agent)
					.send() && let Some(set_cookie) = probe.get_header("Set-Cookie")
			{
				refresh_igneous_from_set_cookie(&set_cookie);
			}

			let refreshed_cookies = build_cookie_header();
			resp = do_request(&refreshed_cookies)?;

			if let Some(set_cookie) = resp.get_header("Set-Cookie") {
				refresh_igneous_from_set_cookie(&set_cookie);
			}

			let retry_doc = resp.get_html()?;
			let still_rejected = retry_doc
				.select_first("body")
				.and_then(|b| b.select_first("div"))
				.is_none();
			if still_rejected {
				bail!(
					"Access denied by ExHentai. Please check your account permissions or re-login."
				);
			}
			return Ok(retry_doc);
		}
	}

	Ok(doc)
}

pub fn rewrite_domain(url: &str) -> String {
	let domain = get_domain();
	if url.contains("exhentai.org") {
		url.replacen("exhentai.org", &domain, 1)
	} else if url.contains("e-hentai.org") {
		url.replacen("e-hentai.org", &domain, 1)
	} else {
		url.to_string()
	}
}

pub fn get_api_url() -> &'static str {
	"https://api.e-hentai.org/api.php"
}

pub fn api_showpage(
	gid: &str,
	imgkey: &str,
	page: u32,
	showkey: &str,
	nl: Option<&str>,
	cookies: &str,
) -> Option<(String, Option<String>)> {
	let nl_val = nl.unwrap_or("");
	let body = format!(
		r#"{{"method":"showpage","gid":{gid},"imgkey":"{imgkey}","page":{page},"showkey":"{showkey}","nl":"{nl_val}"}}"#
	);
	let mut resp = Request::post(get_api_url())
		.ok()?
		.header("Content-Type", "application/json")
		.header("Cookie", cookies)
		.header("User-Agent", USER_AGENT)
		.header("Referer", &get_base_url())
		.body(body.as_bytes())
		.send()
		.ok()?;

	let json: serde_json::Value = resp.get_json().ok()?;
	let i3 = json.get("i3").and_then(|v| v.as_str())?;
	let img_url = extract_src_from_img_html(i3)?;
	let nl_out = json
		.get("i6")
		.and_then(|v| v.as_str())
		.and_then(extract_nl_from_i6);

	Some((img_url.to_string(), nl_out))
}

pub fn extract_src_from_img_html(html: &str) -> Option<&str> {
	let start = html
		.find("src=\"")
		.map(|i| (i + 5, '"'))
		.or_else(|| html.find("src='").map(|i| (i + 5, '\'')))?;
	let (idx, quote) = start;
	let end = html[idx..].find(quote)?;
	Some(&html[idx..idx + end])
}

pub fn extract_nl_from_i6(i6: &str) -> Option<String> {
	let start = i6.find("nl('")?;
	let after = &i6[start + 4..];
	let end = after.find('\'')?;
	Some(after[..end].to_string())
}

pub fn api_imagedispatch(
	gid: &str,
	imgkey: &str,
	page: u32,
	mpvkey: &str,
	nl: Option<&str>,
	cookies: &str,
) -> Option<(String, Option<String>)> {
	let nl_val = nl.unwrap_or("");
	let body = format!(
		r#"{{"method":"imagedispatch","gid":{gid},"imgkey":"{imgkey}","page":{page},"mpvkey":"{mpvkey}","nl":"{nl_val}"}}"#
	);
	let mut resp = Request::post(get_api_url())
		.ok()?
		.header("Content-Type", "application/json")
		.header("Cookie", cookies)
		.header("User-Agent", USER_AGENT)
		.header("Referer", &get_base_url())
		.body(body.as_bytes())
		.send()
		.ok()?;

	let json: serde_json::Value = resp.get_json().ok()?;
	let img_url = json.get("i").and_then(|v| v.as_str())?.to_string();
	let nl_out = json
		.get("s")
		.and_then(|v| v.as_str())
		.filter(|s| !s.is_empty())
		.map(|s| s.to_string());

	Some((img_url, nl_out))
}
