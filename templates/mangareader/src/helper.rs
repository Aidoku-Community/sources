use aidoku::{alloc::String, imports::html::Element};

pub trait ElementImageAttr {
	fn img_attr(&self) -> Option<String>;
}

fn preferred_image_attr(mut attr: impl FnMut(&str) -> Option<String>) -> Option<String> {
	[
		"abs:data-lazy-src",
		"abs:data-src",
		"abs:data-url",
		"abs:src",
		"data-url",
	]
	.into_iter()
	.find_map(&mut attr)
}

impl ElementImageAttr for Element {
	fn img_attr(&self) -> Option<String> {
		preferred_image_attr(|name| self.attr(name))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lazy_cover_precedes_placeholder_src() {
		let cover = preferred_image_attr(|name| match name {
			"abs:data-src" => Some("https://cdn.example/cover.jpg".into()),
			"abs:src" => Some("data:image/gif;base64,placeholder".into()),
			_ => None,
		});

		assert_eq!(cover.as_deref(), Some("https://cdn.example/cover.jpg"));
	}
}
