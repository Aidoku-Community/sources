use aidoku::{
	alloc::{String, Vec, vec},
	prelude::*,
};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

const TARGET_SPAN: u32 = 222;
const MAX_SPAN: u32 = 259;
const MIN_SPAN: u32 = 185;

pub struct Tile {
	pub x: u32,
	pub y: u32,
	pub width: u32,
	pub height: u32,
}

// what the site draws into a tile: the scrambled tile `source`, flipped and then turned `rot`
// quarters counterclockwise (the site rotates by -rot * PI / 2)
#[derive(Clone, Default)]
pub struct Move {
	pub source: usize,
	pub rot: u8,
	pub flip_h: bool,
	pub flip_v: bool,
}

pub struct Placement {
	pub turns: u8,
	pub mirror: bool,
	pub src_x: u32,
	pub src_y: u32,
	pub width: u32,
	pub height: u32,
	pub dst_x: i64,
	pub dst_y: i64,
}

// every seed string below has to match the site's character for character
pub fn plan(width: u32, height: u32, chapter: &str, token: &str) -> (Vec<Tile>, Vec<Move>) {
	let seed = hmac_sha256_hex(token, chapter);
	let mut random = SeedRandom::new(&seed);
	let cols = spans(width, &mut random);
	let rows = spans(height, &mut random);

	let mut tiles = Vec::with_capacity(cols.len() * rows.len());
	let mut y = 0;
	for &height in &rows {
		let mut x = 0;
		for &width in &cols {
			tiles.push(Tile {
				x,
				y,
				width,
				height,
			});
			x += width;
		}
		y += height;
	}

	// in order of first appearance, as the site iterates a Map
	let mut groups: Vec<((u32, u32), Vec<usize>)> = Vec::new();
	for (index, tile) in tiles.iter().enumerate() {
		let size = (tile.width, tile.height);
		match groups.iter_mut().find(|(key, _)| *key == size) {
			Some((_, members)) => members.push(index),
			None => groups.push((size, vec![index])),
		}
	}

	let mut moves = vec![Move::default(); tiles.len()];
	let mut done = Vec::new();
	for &((w, h), ref members) in &groups {
		if done.contains(&(w, h)) {
			continue;
		}
		let key = format!("{w}x{h}");
		let turns: &[u8] = if w == h { &[0, 1, 2, 3] } else { &[0, 2] };
		let others = groups
			.iter()
			.find(|(size, _)| w != h && *size == (h, w))
			.map(|(_, others)| others);
		let Some(others) = others else {
			assign(
				&mut moves,
				members,
				&format!("{seed}|perm|{key}"),
				&format!("{seed}|tf|self|{key}"),
				turns,
			);
			continue;
		};

		// tiles of w x h and h x w trade places, turned by a quarter to fit
		done.push((h, w));
		let other = format!("{h}x{w}");
		let mut pair = SeedRandom::new(&format!("{seed}|pair|{key}|{other}"));
		let mut a = members.to_vec();
		let mut b = others.to_vec();
		shuffle(&mut a, &mut pair);
		shuffle(&mut b, &mut pair);
		let n = a.len().min(b.len());
		for i in 0..n {
			moves[a[i]] = transform(&format!("{seed}|tf|AtoB|{key}|{i}"), b[i], &[1, 3]);
			moves[b[i]] = transform(&format!("{seed}|tf|BtoA|{other}|{i}"), a[i], &[1, 3]);
		}
		assign(
			&mut moves,
			&a[n..],
			&format!("{seed}|perm|A|{key}"),
			&format!("{seed}|tf|A|{key}|rest"),
			turns,
		);
		assign(
			&mut moves,
			&b[n..],
			&format!("{seed}|perm|B|{other}"),
			&format!("{seed}|tf|B|{other}|rest"),
			turns,
		);
	}
	(tiles, moves)
}

// the sdk's transform can't hold a reflection, so flips are read from a mirrored copy instead.
// the site draws a w x h tile turned into an h x w slot 1px off; here it fits the slot exactly
pub fn placements(width: u32, tiles: &[Tile], moves: &[Move]) -> Vec<Placement> {
	tiles
		.iter()
		.zip(moves)
		.map(|(target, step)| {
			let source = &tiles[step.source];
			let turns = (step.rot + if step.flip_v { 2 } else { 0 }) % 4;
			let mirror = step.flip_h != step.flip_v;
			let (x, y) = (i64::from(target.x), i64::from(target.y));
			let (w, h) = (i64::from(target.width), i64::from(target.height));
			let (dst_x, dst_y) = match turns {
				1 => (-(y + h), x),
				2 => (-(x + w), -(y + h)),
				3 => (y, -(x + w)),
				_ => (x, y),
			};
			Placement {
				turns,
				mirror,
				src_x: if mirror {
					width - source.x - source.width
				} else {
					source.x
				},
				src_y: source.y,
				width: source.width,
				height: source.height,
				dst_x,
				dst_y,
			}
		})
		.collect()
}

fn assign(
	moves: &mut [Move],
	members: &[usize],
	perm_seed: &str,
	transform_seed: &str,
	turns: &[u8],
) {
	let mut sources = members.to_vec();
	shuffle(&mut sources, &mut SeedRandom::new(perm_seed));
	for (i, (&target, source)) in members.iter().zip(sources).enumerate() {
		moves[target] = transform(&format!("{transform_seed}|{i}"), source, turns);
	}
}

fn transform(seed: &str, source: usize, turns: &[u8]) -> Move {
	let mut random = SeedRandom::new(seed);
	let rot = turns[(random.next() * turns.len() as f64) as usize];
	let flip_h = random.next() < 0.5;
	let flip_v = random.next() < 0.5;
	Move {
		source,
		rot,
		flip_h,
		flip_v,
	}
}

// the site rounds with Math.round, which takes a half up
fn spans(length: u32, random: &mut SeedRandom) -> Vec<u32> {
	let mut count = ((length + TARGET_SPAN / 2) / TARGET_SPAN).max(1);
	while length / count > MAX_SPAN {
		count += 1;
	}
	while length / count < MIN_SPAN && count > 1 {
		count -= 1;
	}
	let mut spans = vec![length / count; count as usize];
	for span in spans.iter_mut().take((length % count) as usize) {
		*span += 1;
	}
	shuffle(&mut spans, random);
	spans
}

fn shuffle<T>(items: &mut [T], random: &mut SeedRandom) {
	for i in (1..items.len()).rev() {
		let j = (random.next() * (i + 1) as f64) as usize;
		items.swap(i, j);
	}
}

fn hmac_sha256_hex(key: &str, message: &str) -> String {
	let mut mac =
		Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("hmac accepts any key length");
	mac.update(message.as_bytes());
	mac.finalize()
		.into_bytes()
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect()
}

// seedrandom 3's default ARC4 generator
struct SeedRandom {
	s: [u8; 256],
	i: u8,
	j: u8,
}

impl SeedRandom {
	fn new(seed: &str) -> Self {
		let mut key = Vec::new();
		let mut smear = 0u32;
		for (j, byte) in seed.bytes().enumerate() {
			let index = j & 255;
			if index == key.len() {
				key.push(0);
			}
			smear ^= u32::from(key[index]) * 19;
			key[index] = smear.wrapping_add(u32::from(byte)) as u8;
		}
		if key.is_empty() {
			key.push(0);
		}

		let mut s = [0u8; 256];
		for (i, value) in s.iter_mut().enumerate() {
			*value = i as u8;
		}
		let mut j = 0u8;
		for i in 0..s.len() {
			j = j.wrapping_add(key[i % key.len()]).wrapping_add(s[i]);
			s.swap(i, usize::from(j));
		}
		let mut random = Self { s, i: 0, j: 0 };
		for _ in 0..256 {
			random.byte();
		}
		random
	}

	fn byte(&mut self) -> u8 {
		self.i = self.i.wrapping_add(1);
		self.j = self.j.wrapping_add(self.s[usize::from(self.i)]);
		self.s.swap(usize::from(self.i), usize::from(self.j));
		self.s[usize::from(self.s[usize::from(self.i)].wrapping_add(self.s[usize::from(self.j)]))]
	}

	fn next(&mut self) -> f64 {
		const SIGNIFICANCE: f64 = (1u64 << 52) as f64;
		const OVERFLOW: f64 = (1u64 << 53) as f64;
		let mut n = 0.0;
		for _ in 0..6 {
			n = n * 256.0 + f64::from(self.byte());
		}
		let mut d = (1u64 << 48) as f64;
		let mut x = 0u32;
		while n < SIGNIFICANCE {
			n = (n + f64::from(x)) * 256.0;
			d *= 256.0;
			x = u32::from(self.byte());
		}
		while n >= OVERFLOW {
			n /= 2.0;
			d /= 2.0;
			x >>= 1;
		}
		(n + f64::from(x)) / d
	}
}
