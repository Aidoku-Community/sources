use aidoku::{
	Result,
	alloc::{String, collections::BTreeMap},
	imports::net::Request,
};
use spin::Lazy;

use crate::{
	endpoints::Url,
	models::responses::ConstantsResponse,
	settings::{get_api_url, get_image_server_url},
};

static IMAGE_SERVERS: Lazy<BTreeMap<u8, BTreeMap<String, String>>> =
	Lazy::new(|| load_image_servers().unwrap_or_default());

fn load_image_servers() -> Result<BTreeMap<u8, BTreeMap<String, String>>> {
	let api_url = get_api_url();
	let constants_url = Url::constants_with_fields(&api_url, &["imageServers"]);

	let response = Request::get(constants_url)?
		.send()?
		.get_json::<ConstantsResponse>()?;

	let mut servers_by_site: BTreeMap<u8, BTreeMap<String, String>> = BTreeMap::new();

	for server in response.data.image_servers.unwrap_or_default() {
		for &site_id in &server.site_ids {
			servers_by_site
				.entry(site_id as u8)
				.or_default()
				.insert(server.id.clone(), server.url.clone());
		}
	}

	Ok(servers_by_site)
}

pub fn get_selected_image_server_url(site_id: &u8) -> String {
	let selected_id = get_image_server_url();

	IMAGE_SERVERS
		.get(&(*site_id))
		.and_then(|site_servers| site_servers.get(&selected_id))
		.cloned()
		.unwrap_or_else(|| get_api_url()) // Fallback to API URL if server not found
}
