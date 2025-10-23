use aidoku::alloc::{String, Vec, string::ToString};

pub fn get_volume_and_chapter_number(
	title_input: String,
) -> (Option<f32>, Option<f32>, Option<String>) {
	let mut volume_number: Option<f32> = None;
	let mut chapter_number: Option<f32> = None;
	let mut title: Option<String> = None;
	let tokens: Vec<&str> = title_input.split_whitespace().collect();
	let mut i = 0;
	while i < tokens.len() {
			let token = tokens[i];
			let token_lower = token.to_lowercase();
			
			// Check for volume (case-insensitive)
			if token_lower.starts_with("vol") {
				volume_number = token
					.get(4..)  // After "vol."
					.or_else(|| token.get(6..))  // After "volume"
					.filter(|s| !s.is_empty())
					.and_then(|num| num.parse::<f32>().ok())
					.or_else(|| tokens.get(i + 1).and_then(|s| s.parse::<f32>().ok()));
			}
			
			// Check for chapter (case-insensitive)
			if token_lower.starts_with("ch") {
				// Try to extract number from the same token first
				chapter_number = token
					.get(3..)  // After "ch."
					.or_else(|| token.get(7..))  // After "chapter"
					.or_else(|| token.get(3..))  // After "chap"
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
						// If not in same token, check next token
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
	if title_input.contains(":") {
		let parts = title_input.split(':').collect::<Vec<&str>>();
		title = Some(parts[1..].join(":").trim().to_string());
	}
	(volume_number, chapter_number, title)
}
