use aidoku::{alloc::String, imports::html::Element};

pub trait ElementImageAttr {
	fn img_attr(&self) -> Option<String>;
}

impl ElementImageAttr for Element {
	fn img_attr(&self) -> Option<String> {
		self.attr("abs:data-lazy-src")
			.or_else(|| self.attr("abs:data-src"))
			.or_else(|| self.attr("abs:data-url"))
			.or_else(|| self.attr("abs:src"))
			.or_else(|| self.attr("data-url"))
	}
}
