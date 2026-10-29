use aidoku::{
	Chapter,
	alloc::{String, Vec},
};

pub fn comic<'a>(entries: impl Iterator<Item = (&'a str, &'a str)>) -> Vec<Chapter> {
	let (mut chapters, mut annotated): (Vec<_>, Vec<_>) = entries
		.map(|(title, name)| parse_entry(title, name))
		.unzip();
	let has_chapters = chapters
		.iter()
		.any(|chapter| chapter.chapter_number.is_some());
	for label in &mut annotated {
		label.take_if(|label| label.is_extra(&chapters, has_chapters));
	}
	confirm_in_order(
		&mut chapters,
		&mut annotated,
		|chapter| &mut chapter.chapter_number,
		|label| label.chapter,
	);
	confirm_in_order(
		&mut chapters,
		&mut annotated,
		|chapter| &mut chapter.volume_number,
		|label| label.volume,
	);
	chapters
}

fn parse_entry(title: &str, name: &str) -> (Chapter, Option<VolumeChapter>) {
	let chapter = parse_plain_title(title, name);
	if let Some(number) = chapter.chapter_number.or(chapter.volume_number) {
		return (unsquash(chapter, number, name), None);
	}
	match VolumeChapter::parse(title) {
		Some(labels) if labels.is_pair() => (labels.into_chapter(), None),
		Some(labels) if labels.rest.is_some() => (chapter, Some(labels)),
		_ => (chapter, None),
	}
}

fn parse_plain_title(title: &str, name: &str) -> Chapter {
	let full_name = Some(name).filter(|name| !name.is_empty());
	let unnumbered = || Chapter {
		title: full_name.map(Into::into),
		..Default::default()
	};
	let Some(label) = Label::parse(title).filter(Label::is_plain) else {
		return unnumbered();
	};
	let named = Label::parse(name);
	let source = named
		.as_ref()
		.filter(|named| fixes_title(named, &label))
		.unwrap_or(&label);
	let (number, is_volume) = (source.number, source.is_volume());
	let volume_extra = is_volume && !label.is_volume() && !source.rest.is_empty();
	let restated = named
		.as_ref()
		.filter(|named| named.restates(number, is_volume));
	// the site also titles extras "0话"
	if (number == 0.0 && restated.is_none()) || volume_extra {
		return unnumbered();
	}

	let title = match restated {
		Some(named) => Some(named.title()).filter(|rest| rest.contains(|c: char| !is_digit(c))),
		None => full_name,
	};
	Chapter {
		title: title.map(Into::into),
		chapter_number: (!is_volume).then_some(number),
		volume_number: is_volume.then_some(number),
		..Default::default()
	}
}

fn fixes_title(named: &Label, title: &Label) -> bool {
	let followed_by_number = named
		.rest
		.trim_start_matches(|c: char| c.is_whitespace() || c == '第')
		.starts_with(is_digit);
	if named.unit.is_none() || followed_by_number {
		return false;
	}
	let other_unit = named.is_volume() != title.is_volume();
	let other_number = named.number as i32 != title.number as i32;
	let adds_decimal = is_integer(title.number) && !is_integer(named.number);
	other_unit || other_number || adds_decimal
}

// titles squash some pairs into a decimal: "2.1话" for "02卷01话"
fn unsquash(chapter: Chapter, number: f32, name: &str) -> Chapter {
	if is_integer(number) {
		return chapter;
	}
	let squashed_volume = Some(number as i32);
	VolumeChapter::parse(name)
		.filter(|pair| pair.is_pair() && pair.volume.map(|volume| volume as i32) == squashed_volume)
		.map_or(chapter, VolumeChapter::into_chapter)
}

fn is_integer(number: f32) -> bool {
	number as i32 as f32 == number
}

fn confirm_in_order(
	chapters: &mut [Chapter],
	annotated: &mut [Option<VolumeChapter>],
	number: fn(&mut Chapter) -> &mut Option<f32>,
	label_number: fn(&VolumeChapter) -> Option<f32>,
) {
	let mut run = Vec::new();
	let mut newer = None;
	for (chapter, label) in chapters.iter_mut().zip(annotated) {
		let known = *number(chapter);
		if known.is_none() {
			if let Some(value) = label.as_ref().and_then(label_number) {
				run.push((chapter, label, value));
			}
			continue;
		}
		confirm_run(&mut run, newer, known, number);
		newer = known;
	}
	confirm_run(&mut run, newer, None, number);
}

fn confirm_run(
	run: &mut Vec<(&mut Chapter, &mut Option<VolumeChapter>, f32)>,
	newer: Option<f32>,
	older: Option<f32>,
	number: fn(&mut Chapter) -> &mut Option<f32>,
) {
	let counts_down = newer
		.into_iter()
		.chain(run.iter().map(|&(.., value)| value))
		.chain(older)
		.is_sorted_by(|a, b| a > b);
	for (chapter, label, value) in run.drain(..) {
		if counts_down && let Some(label) = label.take() {
			*number(chapter) = Some(value);
			chapter.title = label.rest;
		}
	}
}

struct VolumeChapter {
	chapter: Option<f32>,
	volume: Option<f32>,
	rest: Option<String>,
}

impl VolumeChapter {
	fn parse(text: &str) -> Option<Self> {
		let mut labels = text
			.char_indices()
			.filter_map(|(start, _)| label_at(text, start));
		let first = labels.next()?;
		let second = labels.next();
		let (chapter, volume) = match &second {
			None if first.is_volume => (None, Some(first.number)),
			None => (Some(first.number), None),
			Some(second) if first.is_volume && !second.is_volume => {
				(Some(second.number), Some(first.number))
			}
			Some(_) => return None,
		};
		let last = second.as_ref().unwrap_or(&first);
		let before = trim_leftover(text.get(..first.start)?);
		let between = match &second {
			Some(second) => trim_leftover(text.get(first.end..second.start)?),
			None => "",
		};
		let after = trim_leftover(text.get(last.end..)?);
		let rest: Vec<_> = [before, between, after]
			.into_iter()
			.filter(|piece| !piece.is_empty())
			.collect();
		Some(Self {
			chapter,
			volume,
			rest: (!rest.is_empty()).then(|| rest.join(" ")),
		})
	}

	fn is_pair(&self) -> bool {
		self.chapter.is_some() && self.volume.is_some()
	}

	fn is_extra(&self, chapters: &[Chapter], has_chapters: bool) -> bool {
		let volume_among_chapters = self.volume.is_some() && has_chapters;
		let repeats = |chapter: &Chapter| {
			let same_chapter = self.chapter.is_some() && self.chapter == chapter.chapter_number;
			let same_volume = self.volume.is_some() && self.volume == chapter.volume_number;
			same_chapter || same_volume
		};
		volume_among_chapters || chapters.iter().any(repeats)
	}

	fn into_chapter(self) -> Chapter {
		Chapter {
			chapter_number: self.chapter,
			volume_number: self.volume,
			title: self.rest,
			..Default::default()
		}
	}
}

struct Span {
	start: usize,
	end: usize,
	number: f32,
	is_volume: bool,
}

fn label_at(text: &str, start: usize) -> Option<Span> {
	let (before, rest) = text.split_at_checked(start)?;
	if !rest.starts_with(is_digit) || continues_number(before) {
		return None;
	}
	let label = Label::parse(rest).filter(|label| label.unit.is_some())?;
	Some(Span {
		start,
		end: text.len() - label.rest.len(),
		number: label.number,
		is_volume: label.is_volume(),
	})
}

fn continues_number(before: &str) -> bool {
	let before = before.strip_suffix(is_range_dash).unwrap_or(before);
	before.ends_with(|c| is_digit(c) || c == '.')
}

fn trim_leftover(piece: &str) -> &str {
	piece
		.strip_suffix('第')
		.unwrap_or(piece)
		.trim_matches(|c| is_separator(c) || is_bracket(c))
}

struct Label<'a> {
	ordinal: bool,
	number: f32,
	range: bool,
	unit: Option<char>,
	rest: &'a str,
}

impl<'a> Label<'a> {
	fn parse(text: &'a str) -> Option<Self> {
		let text = text.strip_prefix('#').unwrap_or(text);
		let (ordinal, text) = match text.strip_prefix('第') {
			Some(rest) => (true, rest),
			None => (false, text),
		};
		let (number, rest) = read_number(text)?;
		let (range, rest) = match rest.strip_prefix(is_range_dash).and_then(read_number) {
			Some((end, rest)) => (end > number, rest),
			None => (false, rest),
		};
		let mut chars = rest.chars();
		let (unit, rest) = match chars.next() {
			Some(c @ ('卷' | '话')) => (Some(c), chars.as_str()),
			_ => (None, rest),
		};
		Some(Self {
			ordinal,
			number,
			range,
			unit,
			rest,
		})
	}

	fn is_volume(&self) -> bool {
		self.unit == Some('卷')
	}

	fn is_plain(&self) -> bool {
		self.unit.is_some() && self.rest.is_empty()
	}

	fn restates(&self, number: f32, is_volume: bool) -> bool {
		let same_unit = self.unit.is_none() || self.is_volume() == is_volume;
		same_unit && !self.range && self.number as i32 == number as i32 && self.ends_label()
	}

	fn after_counter(&self) -> &'a str {
		let counter =
			|c: char| self.unit.is_none() && self.ordinal && c.is_alphabetic() && !c.is_ascii();
		self.rest.strip_prefix(counter).unwrap_or(self.rest)
	}

	fn ends_label(&self) -> bool {
		let rest = self.after_counter();
		self.unit.is_some()
			|| !self.ordinal
			|| rest.is_empty()
			|| rest.starts_with(|c| is_separator(c) || is_bracket(c))
	}

	fn title(&self) -> &'a str {
		self.after_counter().trim_start_matches(is_separator)
	}
}

fn read_number(text: &str) -> Option<(f32, &str)> {
	let integer = text.strip_prefix(is_digit)?.trim_start_matches(is_digit);
	let rest = match integer.strip_prefix('.') {
		Some(fraction) if fraction.starts_with(is_digit) => fraction.trim_start_matches(is_digit),
		_ => integer,
	};
	let number = text.get(..text.len() - rest.len())?.parse().ok()?;
	Some((number, rest))
}

fn is_separator(c: char) -> bool {
	c.is_whitespace() || matches!(halfwidth(c), '-' | '—' | '.' | ':' | '·' | ',' | '/')
}

fn is_range_dash(c: char) -> bool {
	matches!(halfwidth(c), '-' | '~')
}

fn is_digit(c: char) -> bool {
	c.is_ascii_digit()
}

fn halfwidth(c: char) -> char {
	match c {
		'！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
		_ => c,
	}
}

fn is_bracket(c: char) -> bool {
	matches!(
		halfwidth(c),
		'(' | ')' | '【' | '】' | '《' | '》' | '「' | '」'
	)
}
