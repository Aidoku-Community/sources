//! Remove the promotional credit line the site injects into chapters.
//! Its wording rotates through Unicode look-alikes, so odd characters are matched, not keywords.

use aidoku::alloc::{String, Vec};

/// Accepted limits: blocks are dropped even when the prose needs them (CJK
/// brackets, Korean narration markers), and plain ASCII credits pass.
const PROSE_BLOCKS: &[(u32, u32)] = &[
	(0x0000, 0x007F), // Basic Latin
	(0x0080, 0x00FF), // Latin-1 Supplement
	(0x2000, 0x206F), // General Punctuation
];

/// Asymmetric delimiter pairs. Symmetric quotes are excluded: neither `"` nor
/// `'` tells an opener from a closer, and `'` is every contraction's apostrophe.
#[cfg(test)]
const BASIC_LATIN_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];

#[cfg(test)]
const LATIN_1_PAIRS: &[(char, char)] = &[('«', '»')];

#[cfg(test)]
const GENERAL_PUNCTUATION_PAIRS: &[(char, char)] =
	&[('‘', '’'), ('“', '”'), ('‹', '›'), ('⁅', '⁆')];

/// Pairs closable by extending to the matching closer past the cut.
/// General Punctuation only: their closers were never seen inside a credit.
const EXTEND_PAIRS: &[(char, char)] = &[('‘', '’'), ('“', '”'), ('‹', '›'), ('⁅', '⁆')];

/// Pairs closable by appending the missing closer. The credit frames its
/// own domain, so no forward search: the closer is created instead.
const CREATE_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('«', '»')];

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

/// Append a block, keeping blocks one blank line apart.
///
/// Only mixed lines are cut; the stranded opener is closed by extending for
/// General Punctuation and Latin-1, or by creating the closer for Basic Latin.
fn append_block(output: &mut String, block: &str) {
	let mut kept: Vec<String> = Vec::new();
	for line in block.trim().lines() {
		// `find` yields a byte index, unlike `chars().position`: slicing
		// with a character index would split multi-byte punctuation.
		match line.find(|ch| !is_prose_char(ch)) {
			None => kept.push(line.into()),
			Some(index) => {
				// No sentence end in front of the credit means the line
				// is the credit. `…` ends sentences here; `;` `:` `—` do not.
				let head = &line[..index];
				// `end` is the byte index where the terminator starts; the
				// slice has to cover it whole because `…` is three bytes.
				let Some(start) = head.rfind(['.', '!', '?', '…']) else {
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
	// A hard break left dangling at the end of a block carries no
	// meaning, so only the block's own trailing whitespace goes.
	output.push_str(kept.join("\n").trim_end());
}

/// Drop the lines the site injected, and the blank line that followed each, so
/// the remaining blocks stay one blank line apart.
///
/// Blocks are delimited by blank lines, not by a literal `\n\n`: the API
/// interleaves raw newlines between elements, which would otherwise stack up
/// into extra blank lines.
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
	fn keeps_paragraphs_inside_the_whitelisted_blocks() {
		// Latin-1 accents plus General Punctuation, all verified in live
		// prose on the site.
		let out = strip("Caf\u{00E9} \u{201C}wait\u{2026}\u{201D} \u{2014} it\u{2019}s fine\n\n");
		assert!(out.contains("Caf\u{00E9}"), "accent: {out}");
		assert!(out.contains('\u{2014}'), "dash: {out}");
	}

	#[aidoku_test]
	fn drops_paragraph_with_look_alike_characters() {
		let out = strip(
			"Before\\.\n\nThis \u{1D4EC}ontent is from f\u{0433}\u{0435}e site\\.\n\nAfter\\.\n\n",
		);
		assert_eq!(out, "Before\\.\n\nAfter\\.");
	}

	#[aidoku_test]
	fn drops_look_alike_paragraph_at_edges() {
		let out = strip("\u{0433}\u{0435}\n\nMiddle\\.\n\n\u{0433}\u{0435}\n\n");
		assert_eq!(out, "Middle\\.");
	}

	#[aidoku_test]
	fn drops_paragraph_outside_the_whitelisted_blocks() {
		// Accepted cost of whitelisting whole blocks: these are all real
		// prose on the site, sitting in blocks that are not whitelisted.
		let out = strip("\u{2E22}This is a lie.\u{2E25}\n\nKept\n\n");
		assert_eq!(out, "Kept");
	}

	#[aidoku_test]
	fn drops_attribution_paragraph_in_place() {
		// Verbatim from the live API, chapter 13: the line sits mid-chapter, so
		// the blocks around it must end up one blank line apart.
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
	fn keeps_prose_when_the_credit_shares_its_block() {
		// Verified on a live chapter: the credit is appended to the end of
		// a real paragraph as its own line, inside the same block.
		let out = strip(
			"Rosalyn profoundly asked\\.  \n\
			 \u{1D4B7}\u{1D4BB}\u{1D4EE}\u{1D4EE}\u{1D66C}\u{1D4E2}\u{1D483}\u{1D48F}\
			 \u{1D4F8}\u{1D66B}\u{1D4EE}\u{1D661}\\.\u{1D4EC}\u{1D4F8}\u{1D4EE}\n\n",
		);
		assert_eq!(out, "Rosalyn profoundly asked\\.");
	}

	#[aidoku_test]
	fn keeps_prose_when_the_credit_shares_its_line() {
		// Verified live: no line break at all — the credit sits at the end of
		// the sentence, even glued onto the full stop. Only the credit goes.
		let out = strip(
			"The aura of fire\\! \u{1D627}\u{1D45F}\u{1D452}\u{1D638}\u{1D4B7}\
			 \u{1D4C3}\u{1D45C}\u{1D4CB}\u{1D4C1}\\.\u{1D4B8}\u{1D45E}\n\n",
		);
		assert_eq!(out, "The aura of fire\\!");
		let out = strip(
			"The rumbling sounded different from before\\.\u{1D65B}\u{1D45F}\
			 \u{1D452}\u{1D638}\u{1D5EF}\u{1D62F}\u{1D5FC}\u{1D603}\u{1D484}\u{1D490}\n\n",
		);
		assert_eq!(out, "The rumbling sounded different from before\\.");
	}

	#[aidoku_test]
	fn cuts_at_an_ellipsis_like_at_a_full_stop() {
		// `…` ends sentences here as often as `.` does, so it cuts the same way.
		// `;` `:` `—` do not end sentences and stay out of the cut.
		let out = strip("She trailed off\u{2026} \u{2E22}x\u{2E25} end\n\n");
		assert_eq!(out, "She trailed off\u{2026}");
	}

	#[aidoku_test]
	fn separates_blocks_with_a_single_blank_line() {
		// The API interleaves raw newlines between block elements; they
		// must not stack up into extra blank lines.
		let out = strip("First\n\n  \n     \n  Second\n\n");
		assert_eq!(out, "First\n\nSecond");
	}

	#[aidoku_test]
	fn keeps_hard_breaks_inside_a_block() {
		// A `br` hard break is two spaces then a newline: the spaces are
		// not prose, but the break has to survive the filter.
		let out = strip("First  \nsecond\n\n");
		assert_eq!(out, "First  \nsecond");
	}

	#[aidoku_test]
	fn keeps_blockquote_lines() {
		// The bare quote marker is a line of its own and must not be read
		// as a dropped line.
		let out = strip("> one\n> \n> two\n\n");
		assert_eq!(out, "> one\n> \n> two");
	}

	#[aidoku_test]
	fn creates_closers_for_basic_latin() {
		// Basic Latin pairs close by appending the missing closer: the credit
		// frames its own domain, so there is no forward search.
		let out = strip("[Come! \u{25A0}\u{25A0}\u{25A0}\u{25A0}\u{25A0}!!]\n\n");
		assert_eq!(out, "[Come!]");
		let out = strip("He said (to me. \u{2E22}x\u{2E25} done) end\n\n");
		assert_eq!(out, "He said (to me.)");
	}

	#[aidoku_test]
	fn extends_the_cut_to_the_matching_closer() {
		let out = strip("She said \u{201C}hi. \u{2E22}x\u{2E25} there\u{201D} end\n\n");
		assert_eq!(out, "She said \u{201C}hi. \u{2E22}x\u{2E25} there\u{201D}");
	}

	#[aidoku_test]
	fn repair_pairs_come_from_whitelisted_blocks() {
		// Every block table holds asymmetric prose pairs only: `"` and `'` are
		// left out because they cannot tell an opener from a closer.
		for table in [BASIC_LATIN_PAIRS, LATIN_1_PAIRS, GENERAL_PUNCTUATION_PAIRS] {
			for &(open, close) in table.iter() {
				assert_ne!(open, close, "symmetric pair: {open}");
				assert!(is_prose_char(open) && is_prose_char(close));
			}
		}
		// Every repair pair is listed in one of the block tables, so the
		// tables cannot rot.
		for &(open, close) in EXTEND_PAIRS.iter().chain(CREATE_PAIRS.iter()) {
			let listed = BASIC_LATIN_PAIRS.contains(&(open, close))
				|| LATIN_1_PAIRS.contains(&(open, close))
				|| GENERAL_PUNCTUATION_PAIRS.contains(&(open, close));
			assert!(listed, "unlisted pair: {open}{close}");
		}
	}
}
