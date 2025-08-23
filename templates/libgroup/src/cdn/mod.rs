use aidoku::{
	Result,
	alloc::{String, collections::btree_map::BTreeMap},
	imports::{net::Request, std::current_date},
};
use spin::{Once, RwLock};

use crate::{
	auth::AuthRequest, context::Context, endpoints::Url, models::responses::ConstantsResponse,
	settings::get_image_server_url,
};

struct CacheEntry {
	data: BTreeMap<u8, BTreeMap<String, String>>,
	created_at: i64,
}

impl CacheEntry {
	fn new(data: BTreeMap<u8, BTreeMap<String, String>>, now: i64) -> Self {
		Self {
			data,
			created_at: now,
		}
	}

	fn is_expired(&self, now: i64, ttl_seconds: i64) -> bool {
		now - self.created_at > ttl_seconds
	}
}

/// Image server cache:
/// - On miss or expired: we synchronously load.
/// - If load fails and there is a stale entry, we return the stale entry.
/// - If load fails and no entry exists, return empty string.
pub struct ImageServerCache {
	cache: RwLock<Option<CacheEntry>>,
	// TTL for the image server list: default 1 hour
	ttl_seconds: i64,
	now_fn: fn() -> i64,
}

impl ImageServerCache {
	pub fn new_with_ttl(ttl_seconds: i64, now_fn: fn() -> i64) -> Self {
		Self {
			cache: RwLock::new(None),
			ttl_seconds,
			now_fn,
		}
	}

	pub fn new() -> Self {
		fn now() -> i64 {
			current_date()
		}
		Self::new_with_ttl(3600, now)
	}

	/// Public getter that returns the selected base URL (may be empty)
	pub fn get_base_url(&self, ctx: &Context) -> String {
		let now = (self.now_fn)();

		// Fast path: check cache under read lock
		{
			let guard = self.cache.read();
			if let Some(ref entry) = *guard
				&& !entry.is_expired(now, self.ttl_seconds)
			{
				let selected_id = get_image_server_url();
				return self.extract_url(&entry.data, &ctx.site_id, &selected_id);
			}
		}

		// Miss or expired: attempt reload synchronously.
		match self.load_data(ctx) {
			Ok(data) => {
				let entry = CacheEntry::new(data.clone(), now);
				*self.cache.write() = Some(entry);
				let selected_id = get_image_server_url();
				self.extract_url(&data, &ctx.site_id, &selected_id)
			}
			Err(_) => {
				// Load failed: return stale if present, else empty
				let guard = self.cache.read();
				if let Some(ref entry) = *guard {
					let selected_id = get_image_server_url();
					self.extract_url(&entry.data, &ctx.site_id, &selected_id)
				} else {
					String::new()
				}
			}
		}
	}

	fn load_data(&self, ctx: &Context) -> Result<BTreeMap<u8, BTreeMap<String, String>>> {
		let constants_url = Url::constants_with_fields(&ctx.api_url, &["imageServers"]);

		let response = Request::get(constants_url)?
			.authed(ctx)?
			.get_json::<ConstantsResponse>()?;

		let mut servers_by_site: BTreeMap<u8, BTreeMap<String, String>> = BTreeMap::new();

		for server in response.data.image_servers.unwrap_or_default() {
			for &site_id in &server.site_ids {
				servers_by_site
					.entry(site_id)
					.or_default()
					.insert(server.id.clone(), server.url.clone());
			}
		}

		Ok(servers_by_site)
	}

	fn extract_url(
		&self,
		data: &BTreeMap<u8, BTreeMap<String, String>>,
		site_id: &u8,
		server_id: &str,
	) -> String {
		data.get(site_id)
			.and_then(|site_servers| site_servers.get(server_id))
			.cloned()
			.unwrap_or_default()
	}
}

static IMAGE_SERVER_CACHE: Once<ImageServerCache> = Once::new();

pub fn get_image_server_cache() -> &'static ImageServerCache {
	IMAGE_SERVER_CACHE.call_once(ImageServerCache::new)
}

pub fn get_selected_image_server_url(ctx: &Context) -> String {
	get_image_server_cache().get_base_url(ctx)
}

#[cfg(test)]
mod test;
