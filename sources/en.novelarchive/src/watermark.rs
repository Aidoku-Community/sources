//! Match characters rather than wording: the injected credit rotates through Unicode look-alikes.

use aidoku::alloc::{String, Vec};

const PROSE_BLOCKS: &[(u32, u32)] = &[
	(0x0000, 0x007F), // Basic Latin
	(0x0080, 0x00FF), // Latin-1 Supplement
	(0x2000, 0x206F), // General Punctuation
];

const EXTEND_PAIRS: &[(char, char)] = &[
	('\u{2018}', '\u{2019}'),
	('\u{201C}', '\u{201D}'),
	('\u{2039}', '\u{203A}'),
];

const CREATE_PAIRS: &[(char, char)] = &[
	('(', ')'),
	('[', ']'),
	('{', '}'),
	('\u{AB}', '\u{BB}'),
	('\u{2045}', '\u{2046}'),
];

fn is_prose_char(ch: char) -> bool {
	let code = u32::from(ch);
	ch.is_whitespace()
		|| PROSE_BLOCKS
			.iter()
			.any(|(start, end)| (*start..=*end).contains(&code))
}

fn extend_close(open: char) -> Option<char> {
	EXTEND_PAIRS
		.iter()
		.find(|pair| pair.0 == open)
		.map(|pair| pair.1)
}

fn create_close(open: char) -> Option<char> {
	CREATE_PAIRS
		.iter()
		.find(|pair| pair.0 == open)
		.map(|pair| pair.1)
}

fn matching_opener(close: char) -> Option<char> {
	EXTEND_PAIRS
		.iter()
		.chain(CREATE_PAIRS.iter())
		.find(|pair| pair.1 == close)
		.map(|pair| pair.0)
}

fn unbalanced(text: &str) -> Vec<char> {
	let mut stack: Vec<char> = Vec::new();
	for ch in text.chars() {
		if extend_close(ch).is_some() || create_close(ch).is_some() {
			stack.push(ch);
		} else if let Some(open) = matching_opener(ch)
			&& stack.last() == Some(&open)
		{
			stack.pop();
		}
	}
	stack
}

fn append_block(output: &mut String, block: &str) {
	let mut kept: Vec<String> = Vec::new();
	for line in block.trim().lines() {
		match line.find(|ch| !is_prose_char(ch)) {
			None => kept.push(line.into()),
			Some(index) => {
				// No sentence end before the credit: the whole line is the credit.
				let head = &line[..index];
				let Some(start) = head.rfind(['.', '!', '?', '\u{2026}']) else {
					continue;
				};
				let end = start + head[start..].chars().next().map_or(1, |ch| ch.len_utf8());
				let mut fixed = String::from(&head[..end]);
				let mut rest = &line[end..];
				let mut open = unbalanced(&fixed);
				while let Some(opener) = open.last() {
					if let Some(close) = extend_close(*opener) {
						let Some(at) = rest.find(close) else {
							break;
						};
						let (skipped, after) = rest.split_at(at + close.len_utf8());
						fixed.push_str(skipped);
						rest = after;
						open.pop();
					} else if let Some(close) = create_close(*opener) {
						fixed.push(close);
						open.pop();
					} else {
						break;
					}
				}
				kept.push(fixed);
			}
		}
	}
	if kept.is_empty() {
		return;
	}
	if !output.is_empty() {
		output.push_str("\n\n");
	}
	output.push_str(kept.join("\n").trim_end());
}

pub fn strip(markdown: &str) -> String {
	let mut output = String::default();
	let mut block = String::default();
	for line in markdown.lines() {
		if line.trim().is_empty() {
			if !block.is_empty() {
				append_block(&mut output, &block);
				block.clear();
			}
		} else {
			if !block.is_empty() {
				block.push('\n');
			}
			block.push_str(line);
		}
	}
	append_block(&mut output, &block);
	output
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn drops_injected_credits_and_keeps_paragraph_layout() {
		let out = strip(
			"Before\\.\n\nThis \u{1D4EC}ontent is from f\u{0433}\u{0435}e site\\.\n\nAfter\\.\n\n",
		);
		assert_eq!(out, "Before\\.\n\nAfter\\.");
		assert_eq!(
			strip("\u{0433}\u{0435}\n\nMiddle\\.\n\n\u{0433}\u{0435}\n\n"),
			"Middle\\."
		);
		assert_eq!(strip("\u{2E22}This is a lie.\u{2E25}\n\nKept\n\n"), "Kept");
		let out = strip(
			"She pointed up\n\nThe sourc\u{1D5F2} of this content is \
			 fre\u{113}w\u{113}b\u{3B7}ovel.c\u{0AE6}m\\.\n\n\u{201C}How do you know that\u{201D}\n\n",
		);
		assert_eq!(
			out,
			"She pointed up\n\n\u{201C}How do you know that\u{201D}"
		);
	}

	#[aidoku_test]
	fn keeps_prose_layout_breaks_and_quotes() {
		let out = strip("Caf\u{00E9} \u{201C}wait\u{2026}\u{201D} \u{2014} it\u{2019}s fine\n\n");
		assert_eq!(
			out,
			"Caf\u{00E9} \u{201C}wait\u{2026}\u{201D} \u{2014} it\u{2019}s fine"
		);
		assert_eq!(strip("First\n\n  \n     \n  Second\n\n"), "First\n\nSecond");
		assert_eq!(strip("First  \nsecond\n\n"), "First  \nsecond");
		assert_eq!(strip("> one\n> \n> two\n\n"), "> one\n> \n> two");
	}

	#[aidoku_test]
	fn cuts_credits_and_repairs_delimiters() {
		assert_eq!(
			strip("She trailed off\u{2026} \u{2E22}x\u{2E25} end\n\n"),
			"She trailed off\u{2026}"
		);
		assert_eq!(
			strip("[Come! \u{25A0}\u{25A0}\u{25A0}\u{25A0}\u{25A0}!!]\n\n"),
			"[Come!]"
		);
		assert_eq!(
			strip("He said (to me. \u{2E22}x\u{2E25} done) end\n\n"),
			"He said (to me.)"
		);
		assert_eq!(
			strip("She said \u{201C}hi. \u{2E22}x\u{2E25} there\u{201D} end\n\n"),
			"She said \u{201C}hi. \u{2E22}x\u{2E25} there\u{201D}"
		);
	}

	#[aidoku_test]
	fn cuts_credit_sharing_a_line_or_block() {
		let out = strip(
			"Rosalyn profoundly asked\\.  \n\
			 \u{1D4B7}\u{1D4BB}\u{1D4EE}\u{1D4EE}\u{1D66C}\u{1D4E2}\u{1D483}\u{1D48F}\
			 \u{1D4F8}\u{1D66B}\u{1D4EE}\u{1D661}\\.\u{1D4EC}\u{1D4F8}\u{1D4EE}\n\n",
		);
		assert_eq!(out, "Rosalyn profoundly asked\\.");
		let out = strip(
			"The aura of fire\\! \u{1D627}\u{1D45F}\u{1D452}\u{1D638}\u{1D4B7}\
			 \u{1D4C3}\u{1D45C}\u{1D4CB}\u{1D4C1}\\.\u{1D4B8}\u{1D45E}\n\n",
		);
		assert_eq!(out, "The aura of fire\\!");
	}
}
