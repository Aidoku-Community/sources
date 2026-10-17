//! Restore the paragraph order that bilinovel's `chapterlog.js` shuffles.
//!
//! The site permutes the non-empty `<p>` elements of `#acontent` with a
//! deterministic Fisher-Yates shuffle: the first `fixedLength` paragraphs
//! stay in place and the rest are shuffled by an LCG seeded from the
//! chapter id. The browser runs the obfuscated `chapterlog.js` to undo it;
//! we extract the numeric parameters from that script at runtime (so site
//! updates that change the constants only require the regexes below to
//! still match) and replay the shuffle natively.
//!
//! Ported from bili_novel_packer's `BiliChapterLogTemplate` /
//! `BiliNovelRestore` (github.com/montaro2017/bili_novel_packer).

use aidoku::alloc::{String, Vec, format, string::ToString};
use aidoku::imports::defaults::{defaults_get, defaults_set_data};
use regex::Regex;

use crate::net::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShuffleParams {
	/// Number of leading paragraphs that are never shuffled.
	pub fixed_length: i64,
	pub seed_multiplier: i64,
	pub seed_offset: i64,
	pub a: i64,
	pub c: i64,
	pub modulus: i64,
}

const DEFAULT_FIXED_LENGTH: i64 = 20;

/// Extract shuffle parameters from the chapterlog script source.
///
/// Supports both the plain (readable constants) and the obfuscated script
/// variants; returns `None` when neither parses, in which case callers
/// should fall back to the document order.
pub fn parse_params(js: &str) -> Option<ShuffleParams> {
	parse_plain(js).or_else(|| parse_obfuscated(js))
}

/// The chapter pages carry `var ReadParams={...chapterid:'12345',...}`;
/// the shuffle seed derives from this decimal id.
pub fn parse_chapter_id(page_html: &str) -> Option<i64> {
	let re = Regex::new(r"chapterid:'(\d+)'").ok()?;
	let caps = re.captures(page_html)?;
	caps.get(1)?.as_str().parse().ok()
}

/// Read the cached shuffle parameters for a chapterlog script version,
/// fetching and extracting them on first use. Cached in source defaults so
/// the extraction survives across source instances and app restarts.
pub fn shuffle_params(src: String) -> Option<ShuffleParams> {
	let key = format!("shuffle_params:{src}");
	if let Some(saved) = defaults_get::<String>(&key)
		&& let Some(params) = parse_saved_params(&saved)
	{
		return Some(params);
	}
	let js = Url::ChapterLog { src }.request().ok()?.string().ok()?;
	let params = parse_params(&js)?;
	defaults_set_data(&key, save_params(&params));
	Some(params)
}

fn save_params(params: &ShuffleParams) -> String {
	format!(
		"{},{},{},{},{},{}",
		params.fixed_length,
		params.seed_multiplier,
		params.seed_offset,
		params.a,
		params.c,
		params.modulus
	)
}

fn parse_saved_params(saved: &str) -> Option<ShuffleParams> {
	let values: Option<Vec<i64>> = saved.split(',').map(|value| value.parse().ok()).collect();
	let values = values?;
	if values.len() != 6 {
		return None;
	}
	Some(ShuffleParams {
		fixed_length: values[0],
		seed_multiplier: values[1],
		seed_offset: values[2],
		a: values[3],
		c: values[4],
		modulus: values[5],
	})
}

fn parse_plain(js: &str) -> Option<ShuffleParams> {
	let fixed_re = Regex::new(r"if\s*\(\s*[_$a-zA-Z0-9]+\s*>\s*").ok()?;
	let fixed_expr = extract_trailing_expression(js, &fixed_re, ')')?;
	let fixed_length = eval_int_expression(&strip_outer_parentheses(&fixed_expr)?)?;

	// e.g. `seed = (Number(chapterId) * 123 + 456) % 789;` — find the
	// statement containing `Number(chapterId)` and evaluate it at
	// chapterId = 0 and chapterId = 1 to split multiplier and offset.
	let seed_re = Regex::new(r"=\s*(.+?Number\s*\(\s*chapterId\s*\).+?)\s*;").ok()?;
	let seed_expr = seed_re.captures(js)?.get(1)?.as_str();
	let offset = eval_with_variable(seed_expr, "chapterId", 0)?;
	let one_value = eval_with_variable(seed_expr, "chapterId", 1)?;

	// e.g. `state = (state * 9302 + 49397) % 233280;`
	let lcg_re = Regex::new(r"=\s*(\(\s*[_$a-zA-Z0-9]+\s*\*.+?\)\s*%\s*.+?)\s*;").ok()?;
	let lcg_expr = lcg_re.captures(js)?.get(1)?.as_str();
	let parts = split_top_level(lcg_expr, '%');
	if parts.len() != 2 {
		return None;
	}
	let modulus = eval_int_expression(&parts[1])?;
	let left = strip_outer_parentheses(&parts[0])?;
	let variable = first_identifier(&left)?;
	let c = eval_with_variable(&left, &variable, 0)?;
	let one = eval_with_variable(&left, &variable, 1)?;

	Some(ShuffleParams {
		fixed_length,
		seed_multiplier: one_value - offset,
		seed_offset: offset,
		a: one - c,
		c,
		modulus,
	})
}

fn parse_obfuscated(js: &str) -> Option<ShuffleParams> {
	// Seed: `var x = ...Number(y), <multiplier-expr>), <offset-expr>),`.
	let seed_re = Regex::new(
		r"var\s+[_$a-zA-Z0-9]+\s*=\s*[^;]*?Number\s*\(\s*[_$a-zA-Z0-9]+\s*\)\s*,\s*([^,)]+?)\s*\)\s*,\s*([^,)]+?)\s*\)\s*,",
	)
	.ok()?;
	let (seed_multiplier, seed_offset) = seed_re.captures_iter(js).find_map(|caps| {
		let multiplier = eval_int_expression(caps.get(1)?.as_str())?;
		let offset = eval_int_expression(caps.get(2)?.as_str())?;
		(multiplier > 0 && offset >= 0).then_some((multiplier, offset))
	})?;

	// LCG state update: `x = ...((x, <a>), <c>), <mod>);` — note the
	// repeated variable name; the regex captures it twice and we verify
	// the names match in code (rust regex has no backreferences).
	let lcg_re = Regex::new(
		r"([_$a-zA-Z0-9]+)\s*=\s*[^;]*?\(\s*([_$a-zA-Z0-9]+)\s*,\s*([^,)]+?)\s*\)\s*,\s*([^,)]+?)\s*\)\s*,\s*([^;)]+?)\s*\)\s*;",
	)
	.ok()?;
	let (a, c, modulus) = lcg_re.captures_iter(js).find_map(|caps| {
		let var = caps.get(1)?.as_str();
		if caps.get(2)?.as_str() != var {
			return None;
		}
		let a = eval_int_expression(caps.get(3)?.as_str())?;
		let c = eval_int_expression(caps.get(4)?.as_str())?;
		let modulus = eval_int_expression(caps.get(5)?.as_str())?;
		(a > 0 && c >= 0 && modulus > a && modulus > c).then_some((a, c, modulus))
	})?;

	Some(ShuffleParams {
		fixed_length: DEFAULT_FIXED_LENGTH,
		seed_multiplier,
		seed_offset,
		a,
		c,
		modulus,
	})
}

/// Compute the restore indices for `count` paragraphs.
///
/// Returns a permutation where `indices[i]` is the slot that the current
/// `i`-th paragraph belongs to in the restored order.
pub fn restore_indices(count: usize, chapter_id: i64, params: &ShuffleParams) -> Vec<usize> {
	let mut fixed: Vec<usize> = Vec::new();
	let mut shuffled: Vec<usize> = Vec::new();
	for i in 0..count {
		if (i as i64) < params.fixed_length {
			fixed.push(i);
		} else {
			shuffled.push(i);
		}
	}

	if count as i64 <= params.fixed_length {
		return fixed;
	}

	// Replay the LCG-driven Fisher-Yates shuffle on the tail. The
	// reference script computes `Math.floor(seed / modulus * (i + 1))`
	// in doubles; keep the float form so the port matches its rounding
	// exactly (the integer form agrees in practice but drifts on some
	// steps for other parameter sets).
	let mut seed = chapter_id
		.saturating_mul(params.seed_multiplier)
		.saturating_add(params.seed_offset);
	for i in (1..shuffled.len()).rev() {
		seed = seed.saturating_mul(params.a).saturating_add(params.c) % params.modulus;
		let j = ((seed as f64 / params.modulus as f64) * (i as f64 + 1.0)) as usize;
		shuffled.swap(i, j);
	}

	fixed.extend(shuffled);
	fixed
}

/// Extract the expression following `start_pattern` up to the first
/// top-level `terminator` character.
fn extract_trailing_expression(
	source: &str,
	start_pattern: &Regex,
	terminator: char,
) -> Option<String> {
	let m = start_pattern.find(source)?;
	let mut depth = 0i32;
	let tail = &source[m.end()..];
	for (idx, ch) in tail.char_indices() {
		match ch {
			'(' => depth += 1,
			')' => {
				if depth == 0 && terminator == ')' {
					return Some(tail[..idx].trim().to_string());
				}
				depth -= 1;
			}
			c if c == terminator && depth == 0 => {
				return Some(tail[..idx].trim().to_string());
			}
			_ => {}
		}
	}
	None
}

fn strip_outer_parentheses(expression: &str) -> Option<String> {
	let mut value = expression.trim().to_string();
	while value.starts_with('(') && value.ends_with(')') && wraps_whole(&value) {
		value = value[1..value.len() - 1].trim().to_string();
	}
	Some(value)
}

fn wraps_whole(value: &str) -> bool {
	let mut depth = 0i32;
	for (i, ch) in value.char_indices() {
		match ch {
			'(' => depth += 1,
			')' => {
				depth -= 1;
				if depth == 0 && i != value.len() - 1 {
					return false;
				}
			}
			_ => {}
		}
	}
	true
}

fn split_top_level(expression: &str, operator: char) -> Vec<String> {
	let mut parts = Vec::new();
	let mut depth = 0i32;
	let mut start = 0usize;
	for (i, ch) in expression.char_indices() {
		match ch {
			'(' => depth += 1,
			')' => depth -= 1,
			c if c == operator && depth == 0 => {
				parts.push(expression[start..i].trim().to_string());
				start = i + ch.len_utf8();
			}
			_ => {}
		}
	}
	parts.push(expression[start..].trim().to_string());
	parts
}

fn first_identifier(expression: &str) -> Option<String> {
	let re = Regex::new(r"[_$a-zA-Z][_$a-zA-Z0-9]*").ok()?;
	let m = re.find(expression)?;
	Some(m.as_str().to_string())
}

/// Replace every occurrence of `Number(variable)` / bare `variable` with the
/// given value, then evaluate.
fn eval_with_variable(expression: &str, variable: &str, value: i64) -> Option<i64> {
	let number_re = Regex::new(&format!(r"Number\s*\(\s*{}\s*\)", regex::escape(variable))).ok()?;
	let substituted = number_re.replace_all(expression, value.to_string());
	let bare_re = Regex::new(&format!(r"\b{}\b", regex::escape(variable))).ok()?;
	let substituted = bare_re.replace_all(&substituted, value.to_string());
	eval_int_expression(&substituted)
}

/// Evaluate an integer expression of hex/decimal literals and the operators
/// `+ - * / % << >> >>> ^ ~` with parentheses. Mirrors the reference
/// implementation's truncating division.
fn eval_int_expression(expression: &str) -> Option<i64> {
	ExpressionParser::new(expression).parse().ok()
}

struct ExpressionParser<'a> {
	source: &'a [u8],
	index: usize,
}

impl<'a> ExpressionParser<'a> {
	fn new(source: &'a str) -> Self {
		Self {
			source: source.as_bytes(),
			index: 0,
		}
	}

	fn parse(mut self) -> Result<i64, ()> {
		let value = self.parse_bitwise_xor()?;
		self.skip_whitespace();
		if self.index != self.source.len() {
			return Err(());
		}
		Ok(value)
	}

	fn parse_bitwise_xor(&mut self) -> Result<i64, ()> {
		let mut value = self.parse_shift()?;
		loop {
			self.skip_whitespace();
			if !self.match_str(b"^") {
				return Ok(value);
			}
			value ^= self.parse_shift()?;
		}
	}

	fn parse_shift(&mut self) -> Result<i64, ()> {
		let mut value = self.parse_add_sub()?;
		loop {
			self.skip_whitespace();
			if self.match_str(b"<<") {
				let shift = self.parse_add_sub()?;
				value = value.checked_shl(shift as u32).unwrap_or(0);
				continue;
			}
			if self.match_str(b">>>") {
				let shift = self.parse_add_sub()?;
				value = ((value as u64).checked_shr(shift as u32).unwrap_or(0)) as i64;
				continue;
			}
			if self.match_str(b">>") {
				let shift = self.parse_add_sub()?;
				value = value.checked_shr(shift as u32).unwrap_or(0);
				continue;
			}
			return Ok(value);
		}
	}

	fn parse_add_sub(&mut self) -> Result<i64, ()> {
		let mut value = self.parse_mul_div_mod()?;
		loop {
			self.skip_whitespace();
			if self.match_str(b"+") {
				value = value.saturating_add(self.parse_mul_div_mod()?);
				continue;
			}
			if self.match_str(b"-") {
				value = value.saturating_sub(self.parse_mul_div_mod()?);
				continue;
			}
			return Ok(value);
		}
	}

	fn parse_mul_div_mod(&mut self) -> Result<i64, ()> {
		let mut value = self.parse_unary()?;
		loop {
			self.skip_whitespace();
			if self.match_str(b"*") {
				value = value.saturating_mul(self.parse_unary()?);
				continue;
			}
			if self.match_str(b"/") {
				let rhs = self.parse_unary()?;
				if rhs == 0 {
					return Err(());
				}
				value /= rhs;
				continue;
			}
			if self.match_str(b"%") {
				let rhs = self.parse_unary()?;
				if rhs == 0 {
					return Err(());
				}
				value %= rhs;
				continue;
			}
			return Ok(value);
		}
	}

	fn parse_unary(&mut self) -> Result<i64, ()> {
		self.skip_whitespace();
		if self.match_str(b"+") {
			return self.parse_unary();
		}
		if self.match_str(b"-") {
			return self.parse_unary()?.checked_neg().ok_or(());
		}
		if self.match_str(b"~") {
			return Ok(!self.parse_unary()?);
		}
		self.parse_primary()
	}

	fn parse_primary(&mut self) -> Result<i64, ()> {
		self.skip_whitespace();
		if self.match_str(b"(") {
			let value = self.parse_bitwise_xor()?;
			self.skip_whitespace();
			if !self.match_str(b")") {
				return Err(());
			}
			return Ok(value);
		}

		let start = self.index;
		while self.index < self.source.len() {
			let b = self.source[self.index];
			let is_number_char = b.is_ascii_digit()
				|| (b'a'..=b'f').contains(&b)
				|| (b'A'..=b'F').contains(&b)
				|| b == b'x' || b == b'X';
			if !is_number_char {
				break;
			}
			self.index += 1;
		}
		if self.index == start {
			return Err(());
		}
		let token = core::str::from_utf8(&self.source[start..self.index]).map_err(|_| ())?;
		if token.len() > 2 && (token.starts_with("0x") || token.starts_with("0X")) {
			i64::from_str_radix(&token[2..], 16).map_err(|_| ())
		} else {
			token.parse::<i64>().map_err(|_| ())
		}
	}

	fn match_str(&mut self, value: &[u8]) -> bool {
		if self.source[self.index..].starts_with(value) {
			self.index += value.len();
			true
		} else {
			false
		}
	}

	fn skip_whitespace(&mut self) {
		while self.index < self.source.len() {
			match self.source[self.index] {
				b' ' | b'\t' | b'\n' | b'\r' => self.index += 1,
				_ => break,
			}
		}
	}
}
