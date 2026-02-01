use aidoku::alloc::{string::String, vec::Vec};

/// Returns the ID of a manga from a URL.
pub fn get_manga_key(url: &str) -> Option<String> {
	// remove query parameters
	let path = url.split_once('?').map(|t| t.0).unwrap_or(url);

	// find the segment after "title"
	let manga_segment = path
		.split('/')
		.skip_while(|segment| *segment != "title" && *segment != "series")
		.nth(1)?;

	// remove after the first '-'
	if let Some(dash_pos) = manga_segment.find('-') {
		let prefix = &manga_segment[..dash_pos];
		Some(prefix.into())
	} else {
		Some(manga_segment.into())
	}
}

/// Returns the ID of a chapter from a URL
pub fn get_chapter_key(url: &str) -> Option<String> {
	let path = url.split_once('?').map(|t| t.0).unwrap_or(url);

	// find the last segment (the chapter part)
	let chapter_segment = path.rsplit('/').next()?;

	// get the part before the first '-'
	if let Some(dash_pos) = chapter_segment.find('-') {
		let prefix = &chapter_segment[..dash_pos];
		Some(prefix.into())
	} else {
		Some(chapter_segment.into())
	}
}

pub struct ChapterInfo {
	pub volume: Option<f32>,
	pub chapter: Option<f32>,
	pub title: Option<String>,
}

/// Parses volume, chapter, and title from a chapter title string.
pub fn parse_chapter_title(input: &str) -> ChapterInfo {
	let input = input.trim();
	let mut rest = input;

	let mut volume = None;
	let mut chapter = None;
	let mut title = None;

	let volume_keywords = ["volume ", "vol.", "season "];
	let chapter_keywords = ["chapter ", "capítulo ", "capitulo ", "caoitulo ", "ch."];

	fn find_keyword(string: &str, keywords: &[&str]) -> Option<(usize, usize)> {
		let lower = string.to_ascii_lowercase();
		for &kw in keywords {
			if let Some(idx) = lower.find(kw) {
				return Some((idx, kw.len()));
			}
		}
		None
	}

	if let Some((vol_idx, key_len)) = find_keyword(rest, &volume_keywords) {
		rest = &rest[vol_idx + key_len..];
		let mut end = 0;
		for (i, c) in rest.char_indices() {
			if !(c.is_ascii_digit() || c == '.') {
				break;
			}
			end = i + c.len_utf8();
		}
		if end > 0
			&& let Ok(v) = rest[..end].parse::<f32>()
		{
			volume = Some(v);
			rest = rest[end..].trim_start();
		}
	}

	if let Some((chap_idx, key_len)) = find_keyword(rest, &chapter_keywords) {
		rest = &rest[chap_idx + key_len..];
		let mut end = 0;
		for (i, c) in rest.char_indices() {
			if !(c.is_ascii_digit() || c == '.') {
				break;
			}
			end = i + c.len_utf8();
		}
		if end > 0
			&& let Ok(c) = rest[..end].parse::<f32>()
		{
			chapter = Some(c);
			rest = rest[end..].trim_start();
		}
	}

	let rest =
		rest.trim_start_matches(|c: char| c == ':' || c == '-' || c == ',' || c.is_whitespace());
	if !rest.is_empty() {
		title = Some(rest.into());
	}

	ChapterInfo {
		volume,
		chapter,
		title,
	}
}

/// Extracts the cdn page images from a qwik/json script.
pub fn extract_image_urls(input: &str) -> Vec<String> {
	let mut urls = Vec::new();
	let pattern = "https://";
	let mut start = 0;

	// find the first occurrence of a url
	while let Some(pos) = input[start..].find(pattern) {
		let abs_pos = start + pos;

		// check if this is the start of a quoted, comma-separated value
		// (either at the start, or after a comma, and preceded by a quote)
		let is_valid_start = abs_pos == 0
			|| input[..abs_pos].ends_with(",\"")
			|| input[..abs_pos].ends_with("[\"")
			|| input[..abs_pos].ends_with(" \"");

		if is_valid_start {
			// find the end quote
			let after = &input[abs_pos..];
			if let Some(end_quote) = after.find('"') {
				let candidate = &after[..end_quote];
				// check for "https://{letter}{digit}{digit}." (cdn url)
				let bytes = candidate.as_bytes();
				if bytes.len() > 12
					&& bytes[8].is_ascii_alphabetic()
					&& bytes[9].is_ascii_digit()
					&& bytes[10].is_ascii_digit()
					&& bytes[11] == b'.'
				{
					urls.push(candidate.into());
					// move start to after this quote
					start = abs_pos + end_quote + 1;
					// continue to next url
					continue;
				}
			}
		}
		// if not valid, move past this url and keep searching
		start = abs_pos + pattern.len();
	}

	urls
}

pub fn get_language_iso(language: &str) -> &str {
	match language.to_lowercase().as_str() {
		"abkhaz" => "ab",
		"afar" => "aa",
		"afrikaans" => "af",
		"akan" => "ak",
		"albanian" => "sq",
		"amharic" => "am",
		"arabic" => "ar",
		"aragonese" => "an",
		"armenian" => "hy",
		"assamese" => "as",
		"avaric" => "av",
		"avestan" => "ae",
		"aymara" => "ay",
		"azerbaijani" => "az",
		"bambara" => "bm",
		"bashkir" => "ba",
		"basque" => "eu",
		"belarusian" => "be",
		"bengali; bangla" => "bn",
		"bihari" => "bh",
		"bislama" => "bi",
		"bosnian" => "bs",
		"breton" => "br",
		"bulgarian" => "bg",
		"burmese" => "my",
		"catalan; valencian" => "ca",
		"chamorro" => "ch",
		"chechen" => "ce",
		"chichewa; chewa; nyanja" => "ny",
		"chinese" => "zh",
		"chuvash" => "cv",
		"cornish" => "kw",
		"corsican" => "co",
		"cree" => "cr",
		"croatian" => "hr",
		"czech" => "cs",
		"danish" => "da",
		"divehi; dhivehi; maldivian;" => "dv",
		"dutch" => "nl",
		"dzongkha" => "dz",
		"english" => "en",
		"esperanto" => "eo",
		"estonian" => "et",
		"ewe" => "ee",
		"faroese" => "fo",
		"fijian" => "fj",
		"finnish" => "fi",
		"french" => "fr",
		"fula; fulah; pulaar; pular" => "ff",
		"galician" => "gl",
		"georgian" => "ka",
		"german" => "de",
		"greek, modern" => "el",
		"guaranã\u{AD}" => "gn",
		"gujarati" => "gu",
		"haitian; haitian creole" => "ht",
		"hausa" => "ha",
		"hebrew (modern)" => "he",
		"herero" => "hz",
		"hindi" => "hi",
		"hiri motu" => "ho",
		"hungarian" => "hu",
		"interlingua" => "ia",
		"indonesian" => "id",
		"interlingue" => "ie",
		"irish" => "ga",
		"igbo" => "ig",
		"inupiaq" => "ik",
		"ido" => "io",
		"icelandic" => "is",
		"italian" => "it",
		"inuktitut" => "iu",
		"japanese" => "ja",
		"javanese" => "jv",
		"kalaallisut, greenlandic" => "kl",
		"kannada" => "kn",
		"kanuri" => "kr",
		"kashmiri" => "ks",
		"kazakh" => "kk",
		"khmer" => "km",
		"kikuyu, gikuyu" => "ki",
		"kinyarwanda" => "rw",
		"kyrgyz" => "ky",
		"komi" => "kv",
		"kongo" => "kg",
		"korean" => "ko",
		"kurdish" => "ku",
		"kwanyama, kuanyama" => "kj",
		"latin" => "la",
		"luxembourgish, letzeburgesch" => "lb",
		"ganda" => "lg",
		"limburgish, limburgan, limburger" => "li",
		"lingala" => "ln",
		"lao" => "lo",
		"lithuanian" => "lt",
		"luba-katanga" => "lu",
		"latvian" => "lv",
		"manx" => "gv",
		"macedonian" => "mk",
		"malagasy" => "mg",
		"malay" => "ms",
		"malayalam" => "ml",
		"maltese" => "mt",
		"mäori" => "mi",
		"marathi (maräá¹\u{AD}hä«)" => "mr",
		"marshallese" => "mh",
		"mongolian" => "mn",
		"nauru" => "na",
		"navajo, navaho" => "nv",
		"norwegian bokmã¥l" => "nb",
		"north ndebele" => "nd",
		"nepali" => "ne",
		"ndonga" => "ng",
		"norwegian nynorsk" => "nn",
		"norwegian" => "no",
		"nuosu" => "ii",
		"south ndebele" => "nr",
		"occitan" => "oc",
		"ojibwe, ojibwa" => "oj",
		"old church slavonic, church slavic, church slavonic, old bulgarian, old slavonic" => "cu",
		"oromo" => "om",
		"oriya" => "or",
		"ossetian, ossetic" => "os",
		"panjabi, punjabi" => "pa",
		"päli" => "pi",
		"persian (farsi)" => "fa",
		"polish" => "pl",
		"pashto, pushto" => "ps",
		"portuguese" => "pt",
		"quechua" => "qu",
		"romansh" => "rm",
		"kirundi" => "rn",
		"romanian, [])" => "ro",
		"russian" => "ru",
		"sanskrit (saá¹ská¹›ta)" => "sa",
		"sardinian" => "sc",
		"sindhi" => "sd",
		"northern sami" => "se",
		"samoan" => "sm",
		"sango" => "sg",
		"serbian" => "sr",
		"scottish gaelic; gaelic" => "gd",
		"shona" => "sn",
		"sinhala, sinhalese" => "si",
		"slovak" => "sk",
		"slovene" => "sl",
		"somali" => "so",
		"southern sotho" => "st",
		"spanish; castilian" => "es",
		"sundanese" => "su",
		"swahili" => "sw",
		"swati" => "ss",
		"swedish" => "sv",
		"tamil" => "ta",
		"telugu" => "te",
		"tajik" => "tg",
		"thai" => "th",
		"tigrinya" => "ti",
		"tibetan standard, tibetan, central" => "bo",
		"turkmen" => "tk",
		"tagalog" => "tl",
		"tswana" => "tn",
		"tonga (tonga islands)" => "to",
		"turkish" => "tr",
		"tsonga" => "ts",
		"tatar" => "tt",
		"twi" => "tw",
		"tahitian" => "ty",
		"uyghur, uighur" => "ug",
		"ukrainian" => "uk",
		"urdu" => "ur",
		"uzbek" => "uz",
		"venda" => "ve",
		"vietnamese" => "vi",
		"volapã¼k" => "vo",
		"walloon" => "wa",
		"welsh" => "cy",
		"wolof" => "wo",
		"western frisian" => "fy",
		"xhosa" => "xh",
		"yiddish" => "yi",
		"yoruba" => "yo",
		"zhuang, chuang" => "za",
		"zulu" => "zu",
		_ => "",
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn test_manga_keys() {
		assert_eq!(
			get_manga_key("https://bato.to/series/181119").as_deref(),
			Some("181119")
		);
		assert_eq!(
			get_manga_key("https://bato.to/series/181119/a-familiar-feeling").as_deref(),
			Some("181119")
		);
		assert_eq!(
			get_manga_key("https://bato.to/title/181119").as_deref(),
			Some("181119")
		);
		assert_eq!(
			get_manga_key("https://bato.to/title/181119-a-familiar-feeling").as_deref(),
			Some("181119")
		);
		assert_eq!(
			get_manga_key("https://bato.to/title/269-aishiteru-uso-dakedo?blahblah").as_deref(),
			Some("269")
		);
		assert_eq!(
			get_manga_key("https://bato.to/title/181119-a-familiar-feeling/3249793-ch_1")
				.as_deref(),
			Some("181119")
		);
	}

	#[aidoku_test]
	fn test_chapter_keys() {
		assert_eq!(
			get_chapter_key(
				"https://bato.to/title/209639-en-concubine-official-uncensored/4001767-ch_18"
			)
			.as_deref(),
			Some("4001767")
		);
		assert_eq!(
			get_chapter_key("https://bato.to/title/181119-a-familiar-feeling/3249793-ch_1")
				.as_deref(),
			Some("3249793")
		);
	}

	#[aidoku_test]
	fn test_chapter_titles() {
		let info = parse_chapter_title("Chapter 1");
		assert_eq!(info.volume, None);
		assert_eq!(info.chapter, Some(1.0));
		assert_eq!(info.title, None);

		let info = parse_chapter_title("Volume 3 Chapter 13.5 : Title");
		assert_eq!(info.volume, Some(3.0));
		assert_eq!(info.chapter, Some(13.5));
		assert_eq!(info.title.as_deref(), Some("Title"));

		let info = parse_chapter_title("Oneshot");
		assert_eq!(info.volume, None);
		assert_eq!(info.chapter, None);
		assert_eq!(info.title.as_deref(), Some("Oneshot"));

		let info = parse_chapter_title("Volume 2 Chapter 1");
		assert_eq!(info.volume, Some(2.0));
		assert_eq!(info.chapter, Some(1.0));
		assert_eq!(info.title.as_deref(), None);

		let info = parse_chapter_title("vol.1 ch.2 - , END");
		assert_eq!(info.volume, Some(1.0));
		assert_eq!(info.chapter, Some(2.0));
		assert_eq!(info.title.as_deref(), Some("END"));
	}
}
