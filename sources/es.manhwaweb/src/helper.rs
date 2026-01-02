use aidoku::{
    imports::{net::Request, std::sleep},
    Result,
};

/// Enforces a rate limit by sleeping before returning the request object.
/// This prevents spamming the server and getting IP banned.
pub fn request_with_limits(url: &str, method: &str) -> Result<Request> {
    // Sleep for 1 second to respect rate limits.
    sleep(1);
    match method {
        "POST" => Request::post(url).map_err(|e| e.into()),
        _ => Request::get(url).map_err(|e| e.into()),
    }
}
