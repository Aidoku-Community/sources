use aidoku::{
	alloc::{String, Vec},
	imports::html::Document,
	Manga,
};

pub fn parse_response<T: AsRef<str>>(
	html: &Document,
	base_url: &str,
	item_selector: T,
) -> Vec<Manga> {
	html.select(&item_selector)
		.map(|x| {
			x.filter_map(|element| {
				let href = element.attr("href")?;
				let key = href
					.strip_prefix(base_url)
					.map(String::from)
					.unwrap_or(href);
				let img = element.select_first("img")?;
				let title = img.attr("alt")?;
				let cover = img.attr("abs:src");

				Some(Manga {
					key,
					title,
					cover,
					..Default::default()
				})
			})
			.collect::<Vec<Manga>>()
		})
		.unwrap_or_default()
}
