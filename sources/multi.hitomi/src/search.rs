use aidoku::{
	Result,
	alloc::{String, Vec},
	imports::net::Request,
	prelude::*,
};
use sha2::{Digest, Sha256};

use crate::{LTN_URL, PAGE_SIZE, REFERER};

pub fn decode_nozomi(data: &[u8]) -> Vec<i64> {
	data.as_chunks::<4>()
		.0
		.iter()
		.map(|b| u32::from_be_bytes(*b) as i64)
		.collect()
}

pub fn nozomi_url_for_ns_tag(query: &str, language: &str) -> Option<String> {
	let q = query.replace('_', " ");
	let colon = q.find(':')?;
	let ns = q[..colon].trim();
	let tag = q[colon + 1..].trim();
	let url = match ns {
		"language" => format!("{LTN_URL}/index-{tag}.nozomi"),
		"female" | "male" => {
			let encoded = q.replace(' ', "%20");
			format!("{LTN_URL}/tag/{encoded}-{language}.nozomi")
		}
		"artist" | "group" | "series" | "character" | "tag" | "type" => {
			let encoded = tag.replace(' ', "%20");
			format!("{LTN_URL}/{ns}/{encoded}-{language}.nozomi")
		}
		_ => return None,
	};
	Some(url)
}

pub fn fetch_nozomi_page(url: &str, page: i32) -> Result<(Vec<i64>, bool)> {
	let offset = (page - 1) * PAGE_SIZE;
	let first_byte = offset * 4;
	let last_byte = first_byte + PAGE_SIZE * 4 - 1;
	let range = format!("bytes={first_byte}-{last_byte}");

	let data = Request::get(url)?
		.header("Range", &range)
		.header("Referer", REFERER)
		.data()?;

	let ids = decode_nozomi(&data);
	let has_next = ids.len() == PAGE_SIZE as usize;
	Ok((ids, has_next))
}

fn hash_term(term: &str) -> [u8; 4] {
	let mut hasher = Sha256::new();
	hasher.update(term.as_bytes());
	let result = hasher.finalize();
	[result[0], result[1], result[2], result[3]]
}

fn fetch_galleries_index_version() -> Option<String> {
	let url = format!("{LTN_URL}/galleriesindex/version?_=0");
	Request::get(&url)
		.ok()?
		.header("Referer", REFERER)
		.string()
		.ok()
		.map(|s| s.trim().into())
}

struct BNode {
	keys: Vec<Vec<u8>>,
	datas: Vec<(u64, u32)>,
	subnode_addresses: Vec<u64>,
}

#[inline]
fn read_u32_be(data: &[u8], pos: &mut usize) -> Option<u32> {
	if *pos + 4 > data.len() {
		return None;
	}
	let v = u32::from_be_bytes([data[*pos], data[*pos + 1], data[*pos + 2], data[*pos + 3]]);
	*pos += 4;
	Some(v)
}

#[inline]
fn read_u64_be(data: &[u8], pos: &mut usize) -> Option<u64> {
	if *pos + 8 > data.len() {
		return None;
	}
	let v = u64::from_be_bytes([
		data[*pos],
		data[*pos + 1],
		data[*pos + 2],
		data[*pos + 3],
		data[*pos + 4],
		data[*pos + 5],
		data[*pos + 6],
		data[*pos + 7],
	]);
	*pos += 8;
	Some(v)
}

fn decode_node(data: &[u8]) -> Option<BNode> {
	let mut pos = 0usize;

	let num_keys = read_u32_be(data, &mut pos)? as usize;
	let mut keys = Vec::new();
	for _ in 0..num_keys {
		let key_size = read_u32_be(data, &mut pos)? as usize;
		if key_size == 0 || key_size > 32 {
			return None;
		}
		if pos + key_size > data.len() {
			return None;
		}
		keys.push(data[pos..pos + key_size].to_vec());
		pos += key_size;
	}

	let num_datas = read_u32_be(data, &mut pos)? as usize;
	let mut datas = Vec::new();
	for _ in 0..num_datas {
		let offset = read_u64_be(data, &mut pos)?;
		let length = read_u32_be(data, &mut pos)?;
		datas.push((offset, length));
	}

	let mut subnode_addresses = Vec::new();
	for _ in 0..17 {
		subnode_addresses.push(read_u64_be(data, &mut pos)?);
	}

	Some(BNode {
		keys,
		datas,
		subnode_addresses,
	})
}

const MAX_NODE_SIZE: usize = 464;

fn fetch_node(version: &str, address: u64) -> Option<BNode> {
	let url = format!("{LTN_URL}/galleriesindex/galleries.{version}.index");
	let end = address + MAX_NODE_SIZE as u64 - 1;
	let range = format!("bytes={address}-{end}");
	let data = Request::get(&url)
		.ok()?
		.header("Referer", REFERER)
		.header("Range", &range)
		.data()
		.ok()?;
	decode_node(&data)
}

fn b_search(key: &[u8; 4], node: &BNode, version: &str) -> Option<(u64, u32)> {
	if node.keys.is_empty() {
		return None;
	}

	let mut found = false;
	let mut where_idx = node.keys.len();
	for (i, k) in node.keys.iter().enumerate() {
		let cmp = compare_keys(key, k);
		if cmp <= 0 {
			found = cmp == 0;
			where_idx = i;
			break;
		}
	}

	if found {
		return node.datas.get(where_idx).copied();
	}

	let is_leaf = node.subnode_addresses.iter().all(|&a| a == 0);
	if is_leaf {
		return None;
	}

	let child_addr = *node.subnode_addresses.get(where_idx)?;
	if child_addr == 0 {
		return None;
	}

	let child_node = fetch_node(version, child_addr)?;
	b_search(key, &child_node, version)
}

fn compare_keys(a: &[u8], b: &[u8]) -> i32 {
	let top = a.len().min(b.len());
	for i in 0..top {
		if a[i] < b[i] {
			return -1;
		}
		if a[i] > b[i] {
			return 1;
		}
	}
	0
}

fn fetch_galleryids_from_data(version: &str, offset: u64, length: u32) -> Option<Vec<i64>> {
	if length == 0 || length > 100_000_000 {
		return None;
	}
	let url = format!("{LTN_URL}/galleriesindex/galleries.{version}.data");
	let end = offset + length as u64 - 1;
	let range = format!("bytes={offset}-{end}");
	let data = Request::get(&url)
		.ok()?
		.header("Referer", REFERER)
		.header("Range", &range)
		.data()
		.ok()?;

	if data.len() < 4 {
		return None;
	}
	let num_ids = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
	let expected = num_ids * 4 + 4;
	if data.len() != expected {
		return None;
	}

	let ids = data[4..]
		.as_chunks::<4>()
		.0
		.iter()
		.map(|b| u32::from_be_bytes(*b) as i64)
		.collect();
	Some(ids)
}

pub fn search_plain_text(term: &str) -> Option<Vec<i64>> {
	let normalized = term.replace('_', " ").to_lowercase();
	let key = hash_term(&normalized);
	let version = fetch_galleries_index_version()?;
	let root = fetch_node(&version, 0)?;
	let (offset, length) = b_search(&key, &root, &version)?;
	fetch_galleryids_from_data(&version, offset, length)
}
