use crate::models::ComixChapter;
use aidoku::{
	HashMap,
	alloc::string::{String, ToString},
	imports::std::current_date,
};

fn is_official_like(ch: &ComixChapter) -> bool {
	ch.group.as_ref().is_some_and(|g| g.id == 10702) || ch.is_official
}

fn is_better(new_ch: &ComixChapter, cur: &ComixChapter) -> bool {
	let official_new = is_official_like(new_ch);
	let official_cur = is_official_like(cur);

	if official_new && !official_cur {
		return true;
	}
	if !official_new && official_cur {
		return false;
	}

	if new_ch.votes > cur.votes {
		return true;
	}
	if new_ch.votes < cur.votes {
		return false;
	}

	let new_created_at = new_ch.created_at();
	let cur_created_at = cur.created_at();
	new_created_at > cur_created_at
}

pub fn dedup_insert(map: &mut HashMap<String, ComixChapter>, ch: ComixChapter) {
	let key = ch.number.to_string();
	match map.get(&key) {
		None => {
			map.insert(key, ch);
		}
		Some(current) => {
			if is_better(&ch, current) {
				map.insert(key, ch);
			}
		}
	}
}

// parse strings like "3d", "2w", "1mo", "5mos", "1yr"
pub fn parse_relative_date_string(string: &str) -> i64 {
	let now = current_date();

	let s = string.trim();
	let mut num = String::new();
	let mut unit = String::new();

	for c in s.chars() {
		if c.is_whitespace() && !unit.is_empty() {
			break;
		}
		if c.is_ascii_digit() {
			num.push(c);
		} else {
			unit.push(c);
		}
	}

	let n: i64 = num.parse().unwrap_or(0);

	let seconds = match unit.to_lowercase().as_str() {
		"s" | "sec" | "secs" | "second" | "seconds" => n,
		"m" | "min" | "mins" | "minute" | "minutes" => n * 60,
		"h" | "hr" | "hrs" | "hour" | "hours" => n * 3600,
		"d" | "day" | "days" => n * 86400,
		"w" | "wk" | "wks" | "week" | "weeks" => n * 7 * 86400,
		"mo" | "mos" | "month" | "months" => n * 30 * 86400,
		"y" | "yr" | "yrs" | "year" | "years" => n * 365 * 86400,
		_ => 0,
	};

	now - seconds
}
