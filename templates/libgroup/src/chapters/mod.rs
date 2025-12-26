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

	/// Get chapters with Double-Checked Locking
	pub fn get_chapters(
		&self,
		manga_key: &str,
		ctx: &Context,
	) -> Result<Vec<LibGroupChapterListItem>> {
		let now = (self.now_fn)();

		// 1. Fast path: read lock
		// Allow multiple threads to read simultaneously
		{
			let guard = self.cache.read();
			if let Some(entry) = guard.get(manga_key)
				&& !entry.is_expired(now, self.ttl_seconds)
			{
				return Ok(entry.data.clone());
			}
		}

		// 2. Slow path: write lock
		// Only one thread enters here
		let mut guard = self.cache.write();

		// Double-check: another thread might have inserted it while we waited for the write lock
		if let Some(entry) = guard.get(manga_key) {
			if !entry.is_expired(now, self.ttl_seconds) {
				return Ok(entry.data.clone());
			}
			// It is actually expired/missing, proceed to remove/overwrite
			guard.remove(manga_key);
		}

		// 3. Fetch from network
		// We still hold the write lock, preventing others from duplicating this request
		let chapters_url = Url::manga_chapters(&ctx.api_url, manga_key);
		let chapters = Request::get(chapters_url)?
			.authed(ctx)?
			.get_json::<ChaptersResponse>()?
			.data;

		// 4. Update cache
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
