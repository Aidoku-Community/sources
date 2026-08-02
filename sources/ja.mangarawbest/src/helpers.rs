use aidoku::{
	ContentRating,
	alloc::{String, Vec, string::ToString},
	helpers::uri::encode_uri_component,
	imports::std::current_date,
	prelude::*,
};

/// Sort values accepted by the `sort` query parameter, ordered to match the
/// options declared in `filters.json`.
pub const SORT_VALUES: [&str; 8] = [
	"-updated_at",
	"-created_at",
	"created_at",
	"-views",
	"-views_day",
	"-views_week",
	"name",
	"-name",
];

/// Genres that mark a series as explicit.
const NSFW_TAGS: [&str; 6] = ["成人向け", "成年", "アダルト", "hentai", "adult", "smut"];

/// Genres that mark a series as suggestive but not explicit.
const SUGGESTIVE_TAGS: [&str; 4] = ["ecchi", "mature", "巨乳", "エッチ"];

/// Parses a Japanese relative date such as "6日前" into a unix timestamp.
///
/// The site renders every date through a `timeago` helper, so absolute dates
/// are never present in the markup.
pub fn parse_relative_date(text: &str) -> Option<i64> {
	let text = text.trim().strip_suffix('前')?;
	let digits_end = text.find(|c: char| !c.is_ascii_digit())?;
	let amount: i64 = text[..digits_end].parse().ok()?;

	let seconds = match text[digits_end..].trim() {
		"秒" => 1,
		"分" => 60,
		"時間" => 3600,
		"日" => 86400,
		"週間" => 604800,
		"ヶ月" | "ヵ月" | "カ月" | "か月" => 2_592_000,
		"年" => 31_536_000,
		_ => return None,
	};

	Some(current_date() - amount * seconds)
}

/// Extracts a chapter number from a title such as "第990話" or "第37.5話".
pub fn parse_chapter_number(title: &str) -> Option<f32> {
	// Chapter titles are consistently formatted as 第<number>話, but fall back
	// to the first number in the string so unusual titles still get a number.
	let rest = match title.find('第') {
		Some(index) => &title[index + '第'.len_utf8()..],
		None => {
			let start = title.find(|c: char| c.is_ascii_digit())?;
			&title[start..]
		}
	};

	let end = rest
		.find(|c: char| !c.is_ascii_digit() && c != '.')
		.unwrap_or(rest.len());
	rest[..end].trim_end_matches('.').parse().ok()
}

/// Returns true when a chapter title carries nothing beyond the chapter number,
/// such as "第41話".
///
/// Every chapter on this site is named that way, and the app already renders the
/// number from `chapter_number`, so keeping the title would show it twice.
pub fn is_plain_chapter_title(title: &str) -> bool {
	let Some(number) = title
		.trim()
		.strip_prefix('第')
		.and_then(|rest| rest.strip_suffix('話'))
	else {
		return false;
	};

	!number.is_empty() && number.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Maps the `filter[status]` value a series page links to onto a status.
///
/// The value is preferred over the label next to it ("進行中" / "完了") because
/// it is what the site filters on, and so is not affected by wording changes.
pub fn parse_status_value(href: &str) -> Option<&'static str> {
	// ex: /manga-list?sort=-updated_at&page=1&filter%5Bstatus%5D=2
	let start = href.rfind('=')? + 1;
	match href[start..].trim() {
		"1" => Some("completed"),
		"2" => Some("ongoing"),
		_ => None,
	}
}

/// Derives a content rating from a series' genres.
///
/// The site has no rating of its own, but tags such as "成人向け" and "Ecchi"
/// are assigned consistently enough to classify the explicit entries.
pub fn content_rating_from_tags(tags: &[String]) -> ContentRating {
	let matches = |list: &[&str], tag: &str| {
		let tag = tag.to_lowercase();
		list.iter().any(|needle| tag.contains(needle))
	};

	if tags.iter().any(|tag| matches(&NSFW_TAGS, tag)) {
		ContentRating::NSFW
	} else if tags.iter().any(|tag| matches(&SUGGESTIVE_TAGS, tag)) {
		ContentRating::Suggestive
	} else {
		ContentRating::Safe
	}
}

/// Strips the " raw" suffix the site appends to some of its Japanese genres.
pub fn clean_tag(tag: &str) -> Option<String> {
	let tag = tag.trim();
	let tag = tag.strip_suffix(" raw").unwrap_or(tag).trim();
	(!tag.is_empty()).then(|| tag.to_string())
}

/// Rewrites a page image url for the given image server, mirroring the
/// `switchImageServer` helper the site ships in its reader.
pub fn build_image_url(server: &str, original: &str) -> String {
	match server {
		"2" => {
			let stripped = original
				.strip_prefix("https://")
				.or_else(|| original.strip_prefix("http://"))
				.unwrap_or(original);
			format!("https://i0.wp.com/{stripped}")
		}
		"3" => format!(
			"https://external-content.duckduckgo.com/iu/?u={}",
			encode_uri_component(original)
		),
		_ => original.to_string(),
	}
}

/// Extracts the `page` query parameter from a pagination link.
pub fn parse_page_param(href: &str) -> Option<i32> {
	let start = href.find("page=")? + "page=".len();
	let rest = &href[start..];
	let end = rest
		.find(|c: char| !c.is_ascii_digit())
		.unwrap_or(rest.len());
	rest[..end].parse().ok()
}

/// Reduces a series url to its path, tolerating the scheme and host variations
/// a shared link may use.
pub fn strip_base_url(url: &str) -> Option<&str> {
	let path = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))
		.unwrap_or(url);
	let path = path.strip_prefix("www.").unwrap_or(path);
	path.strip_prefix("mangaraw.best")
}

/// Appends `value` to `values` unless it is already present.
///
/// The responsive series layout renders each genre twice, so the tag list has
/// to be deduplicated without disturbing the site's ordering.
pub fn push_unique(values: &mut Vec<String>, value: String) {
	if !values.contains(&value) {
		values.push(value);
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_parse_relative_date() {
		let now = current_date();

		assert_eq!(parse_relative_date("6日前"), Some(now - 6 * 86400));
		assert_eq!(parse_relative_date("2週間前"), Some(now - 2 * 604800));
		assert_eq!(parse_relative_date("1ヶ月前"), Some(now - 2_592_000));
		assert_eq!(parse_relative_date(" 1時間前 "), Some(now - 3600));
		assert_eq!(parse_relative_date("30分前"), Some(now - 1800));
		assert_eq!(parse_relative_date("1年前"), Some(now - 31_536_000));

		assert_eq!(parse_relative_date("昨日"), None);
		assert_eq!(parse_relative_date(""), None);
		assert_eq!(parse_relative_date("前"), None);
	}

	#[aidoku_test]
	fn test_parse_chapter_number() {
		assert_eq!(parse_chapter_number("第990話"), Some(990.0));
		assert_eq!(parse_chapter_number("第37.5話"), Some(37.5));
		assert_eq!(parse_chapter_number("第8.1話"), Some(8.1));
		assert_eq!(parse_chapter_number("12"), Some(12.0));

		assert_eq!(parse_chapter_number("番外編"), None);
		assert_eq!(parse_chapter_number(""), None);
	}

	#[aidoku_test]
	fn test_is_plain_chapter_title() {
		assert!(is_plain_chapter_title("第41話"));
		assert!(is_plain_chapter_title("第37.5話"));
		assert!(is_plain_chapter_title(" 第990話 "));

		assert!(!is_plain_chapter_title("第41話 サブタイトル"));
		assert!(!is_plain_chapter_title("番外編"));
		assert!(!is_plain_chapter_title("第話"));
	}

	#[aidoku_test]
	fn test_parse_status_value() {
		assert_eq!(
			parse_status_value("/manga-list?sort=-updated_at&page=1&filter%5Bstatus%5D=1"),
			Some("completed")
		);
		assert_eq!(
			parse_status_value("/manga-list?sort=-updated_at&page=1&filter%5Bstatus%5D=2"),
			Some("ongoing")
		);
		assert_eq!(parse_status_value("/manga-list"), None);
	}

	#[aidoku_test]
	fn test_content_rating_from_tags() {
		let rating = |tags: &[&str]| {
			let tags = tags.iter().map(|t| String::from(*t)).collect::<Vec<_>>();
			content_rating_from_tags(&tags)
		};

		assert_eq!(rating(&["Action", "Comedy"]), ContentRating::Safe);
		assert_eq!(rating(&[]), ContentRating::Safe);
		assert_eq!(rating(&["Action", "Ecchi"]), ContentRating::Suggestive);
		assert_eq!(rating(&["Mature"]), ContentRating::Suggestive);
		assert_eq!(rating(&["成人向け"]), ContentRating::NSFW);
		// explicit wins over suggestive regardless of ordering
		assert_eq!(rating(&["Ecchi", "成人向け"]), ContentRating::NSFW);
	}

	#[aidoku_test]
	fn test_clean_tag() {
		assert_eq!(clean_tag("アクション raw").as_deref(), Some("アクション"));
		assert_eq!(clean_tag(" Action ").as_deref(), Some("Action"));
		assert_eq!(clean_tag("   "), None);
	}

	#[aidoku_test]
	fn test_build_image_url() {
		let original = "https://rbest.mgcdnxyz.cfd/a/b/1.jpg";

		assert_eq!(build_image_url("1", original), original);
		assert_eq!(
			build_image_url("2", original),
			"https://i0.wp.com/rbest.mgcdnxyz.cfd/a/b/1.jpg"
		);
		assert_eq!(
			build_image_url("3", original),
			"https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Frbest.mgcdnxyz.cfd%2Fa%2Fb%2F1.jpg"
		);
	}

	#[aidoku_test]
	fn test_parse_page_param() {
		assert_eq!(
			parse_page_param("https://mangaraw.best/manga-list?page=484"),
			Some(484)
		);
		assert_eq!(
			parse_page_param("https://mangaraw.best/manga-list?page=2&sort=-views"),
			Some(2)
		);
		assert_eq!(parse_page_param("https://mangaraw.best/manga-list"), None);
	}

	#[aidoku_test]
	fn test_strip_base_url() {
		assert_eq!(
			strip_base_url("https://mangaraw.best/raw/tu-long-nobai"),
			Some("/raw/tu-long-nobai")
		);
		assert_eq!(
			strip_base_url("http://www.mangaraw.best/raw/tu-long-nobai"),
			Some("/raw/tu-long-nobai")
		);
		assert_eq!(strip_base_url("https://example.com/raw/x"), None);
	}

	#[aidoku_test]
	fn test_push_unique() {
		let mut values = Vec::new();
		push_unique(&mut values, String::from("a"));
		push_unique(&mut values, String::from("b"));
		push_unique(&mut values, String::from("a"));

		assert_eq!(values.len(), 2);
		assert_eq!(values[0], "a");
		assert_eq!(values[1], "b");
	}
}
