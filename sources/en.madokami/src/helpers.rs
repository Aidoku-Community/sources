// parses a relative date string (e.g. "21 hours ago")
pub fn parse_relative_date(date: &str, current_date: i64) -> i64 {
	// extract the first number found in the string
	let number = date
		.split_whitespace()
		.find_map(|word| word.parse::<i64>().ok())
		.unwrap_or(0);

	let date_lc = date.to_lowercase();

	const SECOND: i64 = 1;
	const MINUTE: i64 = 60 * SECOND;
	const HOUR: i64 = 60 * MINUTE;
	const DAY: i64 = 24 * HOUR;
	const WEEK: i64 = 7 * DAY;
	const MONTH: i64 = 30 * DAY;
	const YEAR: i64 = 365 * DAY;

	let offset = if date_lc.contains("day") {
		number * DAY
	} else if date_lc.contains("hour") {
		number * HOUR
	} else if date_lc.contains("min") {
		number * MINUTE
	} else if date_lc.contains("sec") {
		number * SECOND
	} else if date_lc.contains("week") {
		number * WEEK
	} else if date_lc.contains("month") {
		number * MONTH
	} else if date_lc.contains("year") {
		number * YEAR
	} else {
		0
	};

	current_date - offset
}
