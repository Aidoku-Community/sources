use aidoku::{
	Result,
	alloc::{String, Vec, collections::btree_map::BTreeMap, string::ToString},
	imports::{net::Request, std::current_date},
};
use spin::{Once, RwLock};

use crate::{
	auth::AuthRequest,
	context::Context,
	endpoints::Url,
	models::{chapter::LibGroupChapterListItem, responses::ChaptersResponse},
};

/// Timestamped entry
struct TimedVec {
	data: Vec<LibGroupChapterListItem>,
	created_at: i64,
}

impl TimedVec {
	fn new(data: Vec<LibGroupChapterListItem>, now: i64) -> Self {
		Self {
			data,
			created_at: now,
		}
	}

	fn is_expired(&self, now: i64, ttl_seconds: Option<i64>) -> bool {
		match ttl_seconds {
			Some(ttl) if ttl > 0 => now - self.created_at > ttl,
			_ => false,
		}
	}
}

/// Cache that maps manga_key -> chapters
pub struct ChaptersCache {
	cache: RwLock<BTreeMap<String, TimedVec>>,
	ttl_seconds: Option<i64>,
	now_fn: fn() -> i64,
}

impl ChaptersCache {
	pub fn new_with_ttl(ttl_seconds: Option<i64>, now_fn: fn() -> i64) -> Self {
		Self {
			cache: RwLock::new(BTreeMap::new()),
			ttl_seconds,
			now_fn,
		}
	}

	/// Get chapters
	pub fn get_chapters(
		&self,
		manga_key: &str,
		ctx: &Context,
	) -> Result<Vec<LibGroupChapterListItem>> {
		let now = (self.now_fn)();

		// 1. Check cache
		{
			let guard = self.cache.read();
			if let Some(entry) = guard.get(manga_key)
				&& !entry.is_expired(now, self.ttl_seconds)
			{
				return Ok(entry.data.clone());
			}
		}

		// 2. Fetch and update
		let mut guard = self.cache.write();

		let chapters_url = Url::manga_chapters(&ctx.api_url, manga_key);
		let chapters = Request::get(chapters_url)?
			.authed(ctx)?
			.get_json::<ChaptersResponse>()?
			.data;

		guard.insert(manga_key.to_string(), TimedVec::new(chapters.clone(), now));

		Ok(chapters)
	}

	/// Clear all cache entries.
	pub fn clear(&self) {
		let mut guard = self.cache.write();
		guard.clear();
	}
}

static CHAPTERS_CACHE: Once<ChaptersCache> = Once::new();

/// Global accessor — lazy init
pub fn get_chapters_cache(ttl_seconds: Option<i64>) -> &'static ChaptersCache {
	CHAPTERS_CACHE.call_once(|| ChaptersCache::new_with_ttl(ttl_seconds, current_date))
}

#[cfg(test)]
mod test;
