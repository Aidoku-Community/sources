use aidoku::{
	alloc::String,
	helpers::string::StripPrefixOrSelf,
	imports::html::{Document, Element},
	Manga, MangaPageResult,
};

pub trait ElementImageAttr {
	fn img_attr(&self) -> Option<String>;
}

impl ElementImageAttr for Element {
	fn img_attr(&self) -> Option<String> {
		self.attr("abs:data-lazy-src")
			.or_else(|| self.attr("abs:data-src"))
			.or_else(|| self.attr("abs:src"))
	}
}

pub fn url_host(url: &str) -> &str {
	let Some(scheme_end) = url.find("://") else {
		return url;
	};
	let after_scheme = &url[scheme_end + 3..];

	let host_end = after_scheme
		.find(|c| ['/', ':', '?', '#'].contains(&c))
		.unwrap_or(after_scheme.len());

	&after_scheme[..host_end]
}

pub fn find_first_f32(s: &str) -> Option<f32> {
	let mut num = String::new();
	let mut found_digit = false;
	let mut dot_found = false;

	for c in s.chars() {
		if c.is_ascii_digit() {
			num.push(c);
			found_digit = true;
		} else if c == '.' && found_digit && !dot_found {
			num.push(c);
			dot_found = true;
		} else if found_digit {
			break;
		}
	}

	if found_digit {
		num.parse::<f32>().ok()
	} else {
		None
	}
}

pub fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
	s.find(start).and_then(|start_idx| {
		let after_start = &s[start_idx + start.len()..];
		after_start.find(end).map(|end_idx| &after_start[..end_idx])
	})
}

pub fn parse_manga_page(html: &Document, base_url: &str) -> MangaPageResult {
	MangaPageResult {
		entries: html
			.select("div#main div.grid > div")
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first(".text-center a")?;
					Some(Manga {
						key: link.attr("href")?.strip_prefix_or_self(base_url).into(),
						title: link.text()?,
						cover: el.select_first("img")?.img_attr(),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default(),
		has_next_page: html
			.select(".blog-pager > span.pagecurrent + span")
			.is_some(),
	}
}
