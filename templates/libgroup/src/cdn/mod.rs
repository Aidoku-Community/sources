use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use aidoku::{
	Result,
	alloc::{String, collections::btree_map::BTreeMap},
	imports::{net::Request, std::sleep},
};
use spin::{Once, RwLock};

use crate::{
	endpoints::Url,
	models::responses::ConstantsResponse,
	settings::{get_api_url, get_image_server_url},
};

struct CacheEntry {
	data: BTreeMap<u8, BTreeMap<String, String>>,
	created_at: u64,
}

impl CacheEntry {
	fn new(data: BTreeMap<u8, BTreeMap<String, String>>) -> Self {
		Self {
			data,
			created_at: get_monotonic_counter(),
		}
	}

	fn is_expired(&self, ttl_ticks: u64) -> bool {
		get_monotonic_counter().saturating_sub(self.created_at) > ttl_ticks
	}
}

struct ImageServerCache {
	cache: RwLock<Option<CacheEntry>>,
	loading: AtomicBool,
}

impl ImageServerCache {
	fn new() -> Self {
		Self {
			cache: RwLock::new(None),
			loading: AtomicBool::new(false),
		}
	}

	fn get_base_url(&self, site_id: &u8) -> String {
		// Try cache first
		{
			let cache_guard = self.cache.read();
			if let Some(ref entry) = *cache_guard
				&& !entry.is_expired(36000)
			{
				// ~1 hour in ticks
				return self.extract_url(&entry.data, site_id);
			}
		}

		// Cache expired/missing, reload
		self.load_and_get(site_id)
	}

	fn load_and_get(&self, site_id: &u8) -> String {
		// Ensure only one thread loads
		if self
			.loading
			.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_ok()
		{
			let result = self.load_data();
			self.loading.store(false, Ordering::Release);

			match result {
				Ok(data) => {
					let entry = CacheEntry::new(data.clone());
					*self.cache.write() = Some(entry);
					self.extract_url(&data, site_id)
				}
				Err(_) => {
					// Use stale cache if available
					let cache_guard = self.cache.read();
					if let Some(ref entry) = *cache_guard {
						self.extract_url(&entry.data, site_id)
					} else {
						String::new()
					}
				}
			}
		} else {
			// Another thread is loading, wait
			while self.loading.load(Ordering::Acquire) {
				sleep(1); // 1 second
			}

			// Try cache again
			let cache_guard = self.cache.read();
			if let Some(ref entry) = *cache_guard {
				self.extract_url(&entry.data, site_id)
			} else {
				String::new()
			}
		}
	}

	fn load_data(&self) -> Result<BTreeMap<u8, BTreeMap<String, String>>> {
		let api_url = get_api_url();
		let constants_url = Url::constants_with_fields(&api_url, &["imageServers"]);

		let response = Request::get(constants_url)?
			.send()?
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

	fn extract_url(&self, data: &BTreeMap<u8, BTreeMap<String, String>>, site_id: &u8) -> String {
		let selected_id = get_image_server_url();

		data.get(site_id)
			.and_then(|site_servers| site_servers.get(&selected_id))
			.cloned()
			.unwrap_or_default()
	}
}

static CACHE: Once<ImageServerCache> = Once::new();
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn get_monotonic_counter() -> u64 {
	COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn get_cache() -> &'static ImageServerCache {
	CACHE.call_once(ImageServerCache::new)
}

pub fn get_selected_image_server_url(site_id: &u8) -> String {
	get_cache().get_base_url(site_id)
}

#[cfg(test)]
mod test;
