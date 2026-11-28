use aidoku::alloc::{String, Vec, format, string::ToString};
use base64::{
	Engine,
	alphabet::URL_SAFE,
	engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

pub const SEED: &str = "di183bafh964gce5072";

const POSITION_MULTIPLIER: u32 = 795_580_989;
const SEED_MULTIPLIER: u32 = 2_048_675_417;
const DECLARATION_WINDOW: usize = 600;
const MODULE_WINDOW: usize = 8 * 1024;
// Tokens are unpadded base64url.
const TOKENS: GeneralPurpose = GeneralPurpose::new(
	&URL_SAFE,
	GeneralPurposeConfig::new()
		.with_decode_allow_trailing_bits(true)
		.with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

pub fn decode(token: &str, index: usize, seed: &str) -> Option<String> {
	let mut state = initial_state(index, seed)?;
	let bytes: Vec<u8> = TOKENS
		.decode(token)
		.ok()?
		.into_iter()
		.map(|byte| {
			state = xorshift(state);
			byte ^ (state as u8)
		})
		.collect();
	String::from_utf8(bytes).ok()
}

fn initial_state(index: usize, seed: &str) -> Option<u32> {
	let position = u32::try_from(index).ok()?.wrapping_add(1);
	let length = u32::try_from(seed.len()).ok()?;
	let mut state = position.wrapping_mul(POSITION_MULTIPLIER) ^ length;
	for byte in seed.bytes() {
		state = (state ^ u32::from(byte)).wrapping_mul(SEED_MULTIPLIER);
		state ^= state >> 15;
	}
	Some(if state == 0 { 1 } else { state })
}

fn xorshift(mut state: u32) -> u32 {
	state ^= state << 13;
	state ^= state >> 17;
	state ^= state << 5;
	state
}

pub fn seed_from_chunk(chunk: &str, module: u64) -> Option<String> {
	let marker = format!("{module}:function(");
	let start = chunk
		.match_indices(&marker)
		.find(|&(at, _)| at == 0 || matches!(chunk.as_bytes()[at - 1], b'{' | b','))?
		.0;
	let body = prefix(&chunk[start..], MODULE_WINDOW);
	if !body.contains(&POSITION_MULTIPLIER.to_string())
		|| !body.contains(&SEED_MULTIPLIER.to_string())
	{
		return None;
	}
	let head = prefix(body, DECLARATION_WINDOW);
	let decoder = ["new TextEncoder", "new TextDecoder"]
		.iter()
		.filter_map(|name| head.find(name))
		.min()?;
	head[..decoder]
		.split('"')
		.skip(1)
		.step_by(2)
		.find(|literal| plausible_seed(literal))
		.map(String::from)
}

fn plausible_seed(literal: &str) -> bool {
	(8..=64).contains(&literal.len()) && literal.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn prefix(text: &str, max: usize) -> &str {
	let mut end = max.min(text.len());
	while !text.is_char_boundary(end) {
		end -= 1;
	}
	&text[..end]
}
