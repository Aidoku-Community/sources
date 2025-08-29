#![no_std]
use aidoku::{
	alloc::{String, Vec},
	canvas::Rect,
	imports::{canvas::Canvas, defaults::defaults_get},
	prelude::*,
	Result, Source,
};
use mangareader::{Impl, MangaReader, Params};

const BASE_URL: &str = "https://mangareader.to";

struct MangaReaderTo;

impl Impl for MangaReaderTo {
	fn new() -> Self {
		Self
	}

	fn params(&self) -> Params {
		Params {
			base_url: BASE_URL.into(),
			page_selector: ".iv-card".into(),
			get_chapter_selector: || {
				defaults_get::<Vec<String>>("languages")
					.map(|langs| {
						langs
							.iter()
							.map(|lang| format!("#{lang}-chapters > li"))
							.collect::<Vec<String>>()
							.join(", ")
							.into()
					})
					.unwrap_or_else(|| "#en-chapters > li".into())
			},
			get_chapter_language: |element| {
				element
					.parent()
					.map(|p| match p.id().unwrap_or_default().as_str() {
						"en-chapters" => "en",
						"ja-chapters" => "ja",
						_ => "en",
					})
					.map(|lang| lang.into())
					.unwrap_or_else(|| "en".into())
			},
			get_page_url_path: |chapter_id| {
				format!("/ajax/image/list/chap/{chapter_id}?mode=vertical")
			},
			..Default::default()
		}
	}

	fn process_page_image(
		&self,
		_params: &Params,
		response: aidoku::ImageResponse,
		context: Option<aidoku::PageContext>,
	) -> Result<aidoku::imports::canvas::ImageRef> {
		if response.code == 404 {
			bail!("Missing image");
		}

		let shuffled = context.is_some_and(|c| c.get("shuffled").is_some());
		if !shuffled {
			return Ok(response.image);
		};

		let width = response.image.width();
		let height = response.image.height();

		let mut canvas = Canvas::new(width, height);

		let img_rect = Rect::new(0.0, 0.0, width, height);
		canvas.draw_image(&response.image, img_rect);

		Ok(canvas.get_image())
	}
}

register_source!(
	MangaReader<MangaReaderTo>,
	ListingProvider,
	Home,
	DeepLinkHandler,
	PageImageProcessor
);
