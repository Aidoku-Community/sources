use aidoku::imports::defaults::defaults_get;

const HIDE_WATERMARK_KEY: &str = "hideWatermark";

pub fn hide_watermark() -> bool {
	defaults_get::<bool>(HIDE_WATERMARK_KEY).unwrap_or(false)
}
