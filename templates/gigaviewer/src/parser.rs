use aidoku::{
	alloc::{String, Vec},
	helpers::date::parse_local_date,
	imports::{
		defaults::defaults_get,
		html::{Document, Html},
	},
	prelude::*,
	Chapter, Manga,
};

#[allow(clippy::too_many_arguments)]
pub fn parse_response<T: AsRef<str>>(
	html: &Document,
	base_url: &str,
	item_selector: T,
	title_selector: T,
	cover_selector: T,
	cover_attr: T,
	authors_selector: Option<T>,
	description_selector: Option<T>,
) -> Vec<Manga> {
	html.select(&item_selector)
		.map(|x| {
			x.filter_map(|element| {
				let key = element
					.select_first("a")?
					.attr("href")?
					.strip_prefix(base_url)
					.map(String::from)?;
				let title = element.select_first(&title_selector)?.text()?;
				let cover = element
					.select_first(&cover_selector)
					.and_then(|x| x.attr(&cover_attr));
				let authors = authors_selector.as_ref().and_then(|selector| {
					let el = element.select_first(selector)?;
					let text = el.text()?;
					Some(text.split('/').map(String::from).collect())
				});
				let description = description_selector
					.as_ref()
					.and_then(|selector| element.select_first(selector)?.text());

				Some(Manga {
					key,
					title,
					cover,
					authors,
					description,
					..Default::default()
				})
			})
			.collect::<Vec<Manga>>()
		})
		.unwrap_or_default()
}

pub fn parse_chapter_elements(
	html: String,
	base_url: &str,
	manga_title: &str,
	chapter_list_selector: &str,
) -> Option<Vec<Chapter>> {
	let document = Html::parse(html).ok()?;
	let skip_locked = !defaults_get::<bool>("showLocked").unwrap_or(true);
	document
		.select(format!("ul.series-episode-list {chapter_list_selector}"))
		.map(|episodes| {
			let mut chapters = episodes
				.filter_map(|e| {
					let date_uploaded = e
						.select_first("span.series-episode-list-date")
						.and_then(|e| parse_local_date(e.text()?, "%Y/%m/%d"));

					let locked = e.select_first(".series-episode-list-price").is_some();

					if skip_locked && locked {
						return None;
					}

					let info = e
						.select_first("a.series-episode-list-container")
						.unwrap_or(e);

					let url = info.attr("href")?;
					let key = url
						.strip_prefix(base_url)
						.map(String::from)
						.unwrap_or(url.clone());
					let title = info
						.select_first("h4.series-episode-list-title")
						.and_then(|e| e.text());
					let chapter_number = title.clone().and_then(parse_chapter_number);
					let thumbnail = info
						.select_first(".series-episode-list-thumb-container img")
						.and_then(|e| e.attr("src"));

					Some(Chapter {
						key,
						title,
						chapter_number,
						date_uploaded,
						url: Some(url),
						thumbnail,
						locked,
						..Default::default()
					})
				})
				.collect::<Vec<_>>();
			// check for oneshot
			if chapters.len() == 1 {
				let only_chapter_has_manga_title = chapters[0]
					.title
					.as_ref()
					.map(|str| str == manga_title)
					.unwrap_or(false);
				if only_chapter_has_manga_title {
					chapters[0].chapter_number = Some(1.0);
				}
			}
			chapters
		})
}

// Parse chapter number from title string containing japanese characters
pub fn parse_chapter_number(title_str: String) -> Option<f32> {
	let mut digits = String::new();
	let mut kanji_num: f32 = 0.0;
	let mut found_digit = false;
	let mut found_kanji = false;

	for c in title_str.chars() {
		let is_wide_digit = ('０'..='９').contains(&c);
		if (is_wide_digit || c.is_ascii_digit()) && !found_kanji {
			// parse wide digits or regular digits
			let regular_digit = if is_wide_digit {
				(c as u32 - 0xfee0) as u8 as char
			} else {
				c
			};
			digits.push(regular_digit);
			found_digit = true;
		} else if ([
			'一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千',
		])
		.contains(&c)
			&& !found_digit
		{
			// parse kanji
			match c {
				'一' => kanji_num += 1.0,
				'二' => kanji_num += 2.0,
				'三' => kanji_num += 3.0,
				'四' => kanji_num += 4.0,
				'五' => kanji_num += 5.0,
				'六' => kanji_num += 6.0,
				'七' => kanji_num += 7.0,
				'八' => kanji_num += 8.0,
				'九' => kanji_num += 9.0,
				'十' => {
					kanji_num = if kanji_num == 0.0 {
						10.0
					} else {
						kanji_num * 10.0
					}
				}
				'百' => {
					kanji_num = if kanji_num == 0.0 {
						100.0
					} else {
						kanji_num * 100.0
					}
				}
				'千' => {
					kanji_num = if kanji_num == 0.0 {
						1000.0
					} else {
						kanji_num * 1000.0
					}
				}
				_ => {}
			}
			found_kanji = true;
		} else if found_digit {
			break;
		}
	}

	let num = if found_digit {
		digits.parse::<f32>().ok()
	} else if found_kanji {
		Some(kanji_num)
	} else {
		None
	};

	// parse part 1 and part 2
	if title_str.contains("前編") || title_str.contains("①") {
		num.map(|n| n + 0.1)
	} else if title_str.contains("後編") || title_str.contains("②") {
		num.map(|n| n + 0.2)
	} else {
		num
	}
}
