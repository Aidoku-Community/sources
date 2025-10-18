use aidoku::alloc::{String, Vec};

pub fn get_volume_and_chapter_number(title: String) -> (Option<f32>, Option<f32>) {
	let mut volume_number: Option<f32> = None;
	let mut chapter_number: Option<f32> = None;
	let tokens: Vec<&str> = title.split_whitespace().collect();

	let mut i = 0;
	while i < tokens.len() {
		let token = tokens[i];
		if token.starts_with("vol") {
			volume_number = token
				.strip_prefix("vol.")
				.or_else(|| token.strip_prefix("volume"))
				.filter(|s| !s.is_empty())
				.and_then(|num| num.parse::<f32>().ok())
				.or_else(|| tokens.get(i + 1).and_then(|s| s.parse::<f32>().ok()));
		}
		if token.starts_with("ch") {
			chapter_number = token
				.strip_prefix("ch.")
				.or_else(|| token.strip_prefix("chapter"))
				.or_else(|| token.strip_prefix("chap"))
				.or_else(|| token.strip_prefix("ch-"))
				.filter(|s| !s.is_empty())
				.and_then(|num_str| {
					num_str
						.chars()
						.take_while(|c| c.is_numeric() || *c == '.')
						.collect::<String>()
						.parse::<f32>()
						.ok()
				})
				.or_else(|| {
					tokens.get(i + 1).and_then(|s| {
						s.chars()
							.take_while(|c| c.is_numeric() || *c == '.')
							.collect::<String>()
							.parse::<f32>()
							.ok()
					})
				});
		}
		i += 1;
	}
	(volume_number, chapter_number)
}
