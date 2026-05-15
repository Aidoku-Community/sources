use aidoku::{
	Result,
	alloc::String,
	imports::net::{Request, TimeUnit, set_rate_limit},
};

/// Initializes the rate limiter for this source.
/// Call this once during source initialization to enforce rate limits globally.
pub fn setup_rate_limit() {
	// Allow 1 request every 2 seconds to avoid IP bans.
	set_rate_limit(1, 2, TimeUnit::Seconds);
}

/// Creates a request object for the given URL and method.
pub fn create_request(url: &str, method: &str) -> Result<Request> {
	match method {
		"POST" => Request::post(url).map_err(Into::into),
		_ => Request::get(url).map_err(Into::into),
	}
}

/// Helper function to map genre IDs to Spanish names.
pub fn get_genre_name(id: &str) -> String {
	match id {
		"3" => "Accion",
		"29" => "Aventura",
		"18" => "Comedia",
		"1" => "Drama",
		"42" => "Recuentos de la vida",
		"2" => "Romance",
		"5" => "Venganza",
		"6" => "Harem",
		"23" => "Fantasia",
		"31" => "Sobrenatural",
		"25" => "Tragedia",
		"43" => "Psicologico",
		"32" => "Horror",
		"44" => "Thriller",
		"28" => "Historias cortas",
		"30" => "Ecchi",
		"34" => "Gore",
		"37" => "Girls love",
		"27" => "Boys love",
		"45" => "Reencarnacion",
		"41" => "Sistema de niveles",
		"33" => "Ciencia ficcion",
		"38" => "Apocaliptico",
		"39" => "Artes marciales",
		"40" => "Superpoderes",
		"35" => "Cultivacion (cultivo)",
		"8" => "Milf",
		_ => "Desconocido",
	}
	.into()
}
