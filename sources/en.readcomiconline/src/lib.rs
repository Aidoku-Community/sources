#![no_std]
use aidoku::{
	Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Home, HomeComponent, HomeComponentValue,
	HomeLayout, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent,
	Result, Source, Viewer,
	alloc::{String, Vec, string::ToString, vec},
	helpers::{
		string::StripPrefixOrSelf,
		uri::{QueryParameters, encode_uri_component},
	},
	imports::{
		html::Document,
		js::JsContext,
		net::Request,
		std::{parse_date, send_partial_result},
	},
	prelude::*,
};

const BASE_URL: &str = "https://readcomiconline.li";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

struct ReadComicOnline;

impl Source for ReadComicOnline {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = if let Some(ref query) = query {
			let mut qs = QueryParameters::new();
			qs.push("page", Some(&page.to_string()));
			qs.push("comicName", Some(query));

			for filter in &filters {
				match filter {
					FilterValue::Select { id, value } => {
						qs.push(id, Some(value));
					}
					FilterValue::MultiSelect {
						included, excluded, ..
					} => {
						fn genre_id(genre: &str) -> &'static str {
							// [...document.querySelectorAll("ul#genres > li")]
							// 	.map((el) => `"${el.querySelector("label").textContent.trim()}" => "${el.querySelector("select").getAttribute("gid")}"`)
							// 	.join(",")
							// on https://readcomiconline.li/AdvanceSearch
							match genre {
								"Action" => "1",
								"Adventure" => "2",
								"Anthology" => "38",
								"Anthropomorphic" => "46",
								"Biography" => "41",
								"Children" => "49",
								"Comedy" => "3",
								"Crime" => "17",
								"Drama" => "19",
								"Family" => "25",
								"Fantasy" => "20",
								"Fighting" => "31",
								"Graphic Novels" => "5",
								"Historical" => "28",
								"Horror" => "15",
								"Leading Ladies" => "35",
								"LGBTQ" => "51",
								"Literature" => "44",
								"Manga" => "40",
								"Martial Arts" => "4",
								"Mature" => "8",
								"Military" => "33",
								"Mini-Series" => "56",
								"Movies & TV" => "47",
								"Music" => "55",
								"Mystery" => "23",
								"Mythology" => "21",
								"Personal" => "48",
								"Political" => "42",
								"Post-Apocalyptic" => "43",
								"Psychological" => "27",
								"Pulp" => "39",
								"Religious" => "53",
								"Robots" => "9",
								"Romance" => "32",
								"School Life" => "52",
								"Sci-Fi" => "16",
								"Slice of Life" => "50",
								"Sport" => "54",
								"Spy" => "30",
								"Superhero" => "22",
								"Supernatural" => "24",
								"Suspense" => "29",
								"Teen" => "57",
								"Thriller" => "18",
								"Vampires" => "34",
								"Video Games" => "37",
								"War" => "26",
								"Western" => "45",
								"Zombies" => "36",
								_ => "",
							}
						}
						qs.push(
							"ig",
							Some(
								&included
									.iter()
									.map(|s| genre_id(s))
									.collect::<Vec<_>>()
									.join(","),
							),
						);
						qs.push(
							"eg",
							Some(
								&excluded
									.iter()
									.map(|s| genre_id(s))
									.collect::<Vec<_>>()
									.join(","),
							),
						);
					}
					_ => {}
				}
			}

			format!("{BASE_URL}/AdvanceSearch?{qs}")
		} else {
			let mut path = "ComicList".to_string();
			let mut sort = "MostPopular";

			for filter in &filters {
				match filter {
					FilterValue::Text { id, value } => {
						let value = value.replace(" ", "-");
						if id == "author" {
							path = format!("Writer/{}", encode_uri_component(value));
						} else if id == "artist" {
							path = format!("Artist/{}", encode_uri_component(value));
						}
					}
					FilterValue::Sort { index, .. } => {
						sort = match index {
							0 => "",
							1 => "MostPopular",
							2 => "LatestUpdate",
							3 => "Newest",
							_ => "",
						}
					}
					FilterValue::MultiSelect { included, .. } => {
						if let Some(genre) = included.first() {
							let encoded = genre.replace(" & ", "-").replace(" ", "-");
							path = format!("Genre/{encoded}");
						}
					}
					_ => {}
				}
			}

			format!("{BASE_URL}/{path}/{sort}?page={page}")
		};

		let html = Request::get(&url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;
		Ok(parse_comic_list(html))
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;

		if needs_details {
			let info_element = html
				.select_first("div.barContent")
				.ok_or(error!("missing info element"))?;

			manga.title = info_element
				.select_first("a.bigChar")
				.and_then(|el| el.text())
				.unwrap_or(manga.title);
			manga.cover = html
				.select_first(".rightBox:eq(0) img")
				.and_then(|el| el.attr("abs:src"));
			manga.authors = info_element
				.select_first("p:has(span:contains(Writer:)) > a")
				.and_then(|el| el.text())
				.map(|str| vec![str]);
			manga.artists = info_element
				.select_first("p:has(span:contains(Artist:)) > a")
				.and_then(|el| el.text())
				.map(|str| vec![str]);
			manga.description = info_element
				.select_first("p:has(span:contains(Summary:)) ~ p")
				.and_then(|el| el.text());
			manga.tags = info_element
				.select("p:has(span:contains(Genres:)) > a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<_>>());
			manga.status = info_element
				.select_first("p:has(span:contains(Status:))")
				.and_then(|el| el.text())
				.map(|str| {
					if str.contains("Ongoing") {
						MangaStatus::Ongoing
					} else if str.contains("Completed") {
						MangaStatus::Completed
					} else {
						MangaStatus::Unknown
					}
				})
				.unwrap_or_default();
			manga.viewer = Viewer::LeftToRight;

			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = html.select("table.listing tr:gt(1)").map(|els| {
				els.filter_map(|el| {
					let url_element = el.select_first("a")?;
					let url = url_element.attr("abs:href")?;

					let mut chapter_number = None;
					let title = url_element.text().map(|text| {
						// remove series title prefix from chapter title
						let text = text.strip_prefix_or_self(&manga.title).trim();
						// parse chapter number after '#' (e.g. Issue #10)
						if let Some(idx) = text.find('#') {
							chapter_number = text[idx + 1..].parse::<f32>().ok();
						}
						text.into()
					});

					Some(Chapter {
						key: url.strip_prefix(BASE_URL)?.into(),
						title,
						chapter_number,
						date_uploaded: el
							.select_first("td:eq(1)")
							.and_then(|el| el.text())
							.and_then(|str| parse_date(str, "MM/dd/yyyy")),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;

		// todo: if the site changes often, this may need to be put in a separate file to request so that it can be updated without users updating the source
		// (this is what the mihon source does)
		const IMG_DECRYPT_EVAL: &str = "const assignRegex = /(_[^\\s=]*xnz)\\s*=\\s*['\"]([^'\"]+)['\"]/g;const matches = [..._encryptedString.matchAll(assignRegex)];const pageLinks = matches.map(m => decryptLink(m[2]));function atob(t){const e=\"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=\";let s=String(t).replace(/=+$/,\"\");if(s.length%4===1)throw new Error(\"'atob' failed: The string to be decoded is not correctly encoded.\");let n=\"\";for(let t=0,r,c,i=0;c=s.charAt(i++);~c&&(r=t%4?r*64+c:c,t++%4)?n+=String.fromCharCode(255&r>>(-2*t&6)):0)c=e.indexOf(c);return n}function decryptLink(t){let e=t.replace(/\\w{5}__\\w{3}__/g,\"g\").replace(/\\w{2}__\\w{6}_/g,\"g\").replace(/b/g,\"pw_.g28x\").replace(/h/g,\"d2pr.x_27\").replace(/pw_.g28x/g,\"b\").replace(/d2pr.x_27/g,\"h\");if(!e.startsWith(\"https\")){const t=e.indexOf(\"?\");const s=e.substring(t);const n=e.includes(\"=s0?\");const r=n?e.indexOf(\"=s0?\"):e.indexOf(\"=s1600?\");let c=e.substring(0,r);c=c.substring(15,33)+c.substring(50);const i=c.length;c=c.substring(0,i-11)+c[i-2]+c[i-1];const g=atob(c);let o=decodeURIComponent(g);o=o.substring(0,13)+o.substring(17);o=o.substring(0,o.length-2)+(n?\"=s0\":\"=s1600\");const a=!_useServer2?\"https://2.bp.blogspot.com\":\"https://img1.whatsnew247.net/pic\";e=`${a}/${o}${s}${_useServer2?\"&t=10\":\"\"}`}return e}JSON.stringify(pageLinks);";

		let scripts = html
			.select("script")
			.ok_or(error!("html select `script` failed"))?;

		let mut links = Vec::new();

		for script in scripts {
			let Some(data) = script.data().and_then(|s| {
				let s = s.trim();
				if s.is_empty() {
					return None;
				}
				serde_json::to_string(&s).ok()
			}) else {
				continue;
			};

			let js_string =
				format!("let _encryptedString = {data};let _useServer2 = false;{IMG_DECRYPT_EVAL}");
			let result = JsContext::new().eval(&js_string)?;

			if result.starts_with('[') && result.ends_with(']') {
				let new_links: Vec<String> = result[1..result.len() - 1]
					.split(',')
					.map(|s| s.trim_matches(|c| c == '"' || c == '\''))
					.filter(|s| !s.is_empty())
					.map(|s| s.to_string())
					.collect();
				links.extend(new_links);
			}
		}

		Ok(links
			.into_iter()
			.map(|link| Page {
				content: PageContent::url(link),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for ReadComicOnline {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let url = format!("{BASE_URL}/{}?page={page}", listing.id);
		let html = Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("User-Agent", USER_AGENT)
			.html()?;
		Ok(parse_comic_list(html))
	}
}

fn parse_comic_list(html: Document) -> MangaPageResult {
	let entries = html
		.select(".list-comic > .item > a:not(.hot-label)")
		.map(|elements| {
			elements
				.filter_map(|element| {
					let url = element.attr("abs:href")?;
					let key = url.strip_prefix(BASE_URL).map(String::from)?;
					let title = element.text().unwrap_or_default();
					let cover = element.select_first("img")?.attr("abs:src");
					Some(Manga {
						key,
						title,
						cover,
						url: Some(url),
						..Default::default()
					})
				})
				.collect::<Vec<Manga>>()
		})
		.unwrap_or_default();

	let has_next_page = html.select("ul.pager > li > a:contains(Next)").is_some();

	MangaPageResult {
		entries,
		has_next_page,
	}
}

impl Home for ReadComicOnline {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = Request::get(BASE_URL)?
			.header("User-Agent", USER_AGENT)
			.html()?;

		let mut components = Vec::new();

		if let Some(banner_element) = html.select_first(".banner > .details") {
			let url = banner_element
				.select_first("a")
				.and_then(|el| el.attr("abs:href"))
				.ok_or(error!("missing"))?;
			let key = url.strip_prefix_or_self(BASE_URL).into();
			let title = banner_element
				.select_first(".bigChar")
				.and_then(|el| el.text())
				.unwrap_or_default();
			let cover = banner_element
				.select_first("img")
				.and_then(|el| el.attr("abs:src"));
			let description = banner_element
				.select("p")
				.and_then(|mut els| els.next_back())
				.and_then(|el| el.text());
			let tags = banner_element
				.select("p:has(span:contains(Genres:)) > a")
				.map(|els| els.filter_map(|el| el.text()).collect::<Vec<_>>());
			components.push(HomeComponent {
				value: HomeComponentValue::BigScroller {
					entries: vec![Manga {
						key,
						title,
						cover,
						description,
						url: Some(url),
						tags,
						..Default::default()
					}],
					auto_scroll_interval: None,
				},
				..Default::default()
			});
		}

		let updates = html
			.select(".bigBarContainer > .barContent > .scrollable > .items a")
			.map(|els| {
				els.filter_map(|el| {
					let url = el.attr("abs:href").unwrap_or_default();
					let key = url.strip_prefix(BASE_URL)?.into();
					let title = el.own_text()?;
					let cover = el.select_first("img").and_then(|el| el.attr("abs:src"));
					Some(
						Manga {
							key,
							title,
							cover,
							url: Some(url),
							..Default::default()
						}
						.into(),
					)
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		if !updates.is_empty() {
			components.push(HomeComponent {
				title: Some("Latest update".into()),
				value: HomeComponentValue::Scroller {
					entries: updates,
					listing: None,
				},
				..Default::default()
			});
		}

		let new = html
			.select("#tab-newest > div")
			.map(|els| {
				els.filter_map(|el| {
					let url = el.select_first("a")?.attr("abs:href").unwrap_or_default();
					let key = url.strip_prefix(BASE_URL)?.into();
					let title = el.select_first(".title > span")?.text()?;
					let cover = el.select_first("img").and_then(|el| el.attr("abs:src"));
					Some(
						Manga {
							key,
							title,
							cover,
							url: Some(url),
							..Default::default()
						}
						.into(),
					)
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		if !new.is_empty() {
			components.push(HomeComponent {
				title: Some("New comic".into()),
				value: HomeComponentValue::MangaList {
					ranking: true,
					page_size: None,
					entries: new,
					listing: None,
				},
				..Default::default()
			});
		}

		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for ReadComicOnline {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let Some(path) = url.strip_prefix(BASE_URL) else {
			return Ok(None);
		};

		const COMIC_PATH: &str = "/Comic";

		if !path.starts_with(COMIC_PATH) {
			return Ok(None);
		}

		let mut segments = path.split('/').filter(|s| !s.is_empty());

		let first = segments.next();
		let second = segments.next();

		if let (Some(first), Some(second)) = (first, second) {
			let mut key = String::with_capacity(first.len() + second.len() + 2);
			key.push('/');
			key.push_str(first);
			key.push('/');
			key.push_str(second);
			Ok(Some(DeepLinkResult::Manga { key }))
		} else {
			Ok(None)
		}
	}
}

register_source!(ReadComicOnline, ListingProvider, Home, DeepLinkHandler);
