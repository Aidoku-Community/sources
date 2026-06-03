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

const BASE_URL: &str = "https://rcostation.xyz";
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
		const IMG_DECRYPT_EVAL: &str = r#"const urlPattern=/^https?:\/\/(?:www\.)?[a-z0-9-]+(?:\.[a-z0-9-]+)+\b(?:[\/a-z0-9-._~:?#@!$&'()*+,;=%]*)$/i,reverseOrder=!1,replacePatternRegex=/\.replace\(\s*\/(\w+__\w+_)\/g\s*,\s*['"](\w)['"]\s*\)/,replaceMatch=_encryptedString.match(replacePatternRegex),obfuscationPattern=replaceMatch?new RegExp(replaceMatch[1],"g"):/\w{2}__\w{6}_/g,replacementChar=replaceMatch?replaceMatch[2]:"e",baseUrlMatch=_encryptedString.match(/baeu\(\w+,\s*["'](https?:\/\/[^"']+)["']\)/),detectedBaseUrl=baseUrlMatch?baseUrlMatch[1]:null,assignRegex=/(_[^\s=]*xnz)\s*=\s*['"]([^'"]+)['"]/g,matches=[..._encryptedString.matchAll(assignRegex)],pageLinks=matches.map(t=>decryptLink(t[2]));function atob(t){let e=String(t).replace(/=+$/,"");if(e.length%4==1)throw new Error("'atob' failed: The string to be decoded is not correctly encoded.");let s="";for(let t,r,n=0,p=0;r=e.charAt(p++);~r&&(t=n%4?64*t+r:r,n++%4)?s+=String.fromCharCode(255&t>>(-2*n&6)):0)r="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=".indexOf(r);return s}function decryptLink(t,e=0){let s=t.replace(obfuscationPattern,replacementChar).replace(/pw_.g28x/g,"b").replace(/d2pr.x_27/g,"h");if(0!=e&&(s=s.substr(e,s.length-e)),(s.endsWith("=s0")||s.endsWith("=s1600"))&&(s=s.replace("https://2.bp.blogspot.com/","")+"?"),!s.startsWith("https")){const t=s.indexOf("?"),e=s.substring(t),r=s.includes("=s0?"),n=r?s.indexOf("=s0?"):s.indexOf("=s1600?");let p=s.substring(0,n);p=p.substring(15,33)+p.substring(50);const o=p.length;p=p.substring(0,o-11)+p[o-2]+p[o-1];const c=atob(p);let a=decodeURIComponent(c);a=a.substring(0,13)+a.substring(17),a=a.substring(0,a.length-2)+(r?"=s0":"=s1600");s=`${detectedBaseUrl??(_useServer2?"https://ano1.rconet.biz/pic":"https://2.bp.blogspot.com")}/${a}${e}${_useServer2?"&t=10":""}`}return s}const blocklist=["https://2.bp.blogspot.com/pw/AP1GczP6zCVVfdmN6OoVnm7CLvEfmHMUawyEwJWouX9C6SHwsiuYfLkUr9FsM6Zo34qNzPKeQeahBx9ckBZJQckiJmX1UwKD7uh900yz5rKyG4zT2rfIrqFviEJIev1Pg_pGRuSG57rIH6BDwGCTmiE4MjA","https://2.bp.blogspot.com/pw/AP1GczP48thKMga7cud0tjtHtYqsvZzhYY0HyAxVzM3O1D6tkLbi0fT9NDZFFFH69hNnoGsnqJSEIh4mmpEoU1BJSfNXIz1f5aLXl41RM9os7ePn7ipbrYbIuqiQxAV0hhJZrNLl7FmauwLQ01paCrP6KAE","https://2.bp.blogspot.com/pw/AP1GczNXprTMfAP2AHFFWvCbKq6qReXrqSohz87KeBjV0nh6XoLsE1NpzL7Rp9llxoY208IPARiIDON_TO6dZB0ZMNeB8J7xzUzbS9h6To7aGpOZshFofw-wFQ0KJ3y3wolSwzLrduZZ_0w8_6gGuTEB-98","https://2.bp.blogspot.com/pw/AP1GczMVY_zWeag2n981CRX7jaZ73Sr0NtidtJhnvJ3-Rmh2fIo-PoQRI0ZksQEbpTjDHgBeNYbQ2hQodsY-Dv0FXUhiU_mus5z5L5lMVAH82kXYqOd2IEw","https://2.bp.blogspot.com/pw/AP1GczOKY-6EDGVvlQGB2wj0xxB5JgcyiujFJC3CHgwqBOLIidwmoP6DLiMpX__Fw6MMPvLezN6soeV0A8pKSHUrC4rxZyO5vov40g1g4ipZdkFlzUouAFA","https://2.bp.blogspot.com/pw/AP1GczO8AETT3k19nhJwxHm0sHCSy0tXyhSOYxnq3EUrmlvgY5yPqDaxcd1XZ7reQKH-lKgpGK4o3sW_9Yu6feqii79riXN3Ghi8Xs1S5Z4wi-aeHrq5PzOX"];function getCleanedLinks(){const t=pageLinks.filter((t,e)=>{if(!t)return!1;const s=t.split("?")[0].split("=")[0],r=pageLinks.findIndex(t=>t.split("?")[0].split("=")[0]===s)===e,n=-1===blocklist.indexOf(s),p=urlPattern.test(s);return r&&n&&p});return t}JSON.stringify(getCleanedLinks());"#;

		let scripts = html
			.select("script")
			.ok_or(error!("html select `script` failed"))?;

		let combined_scripts = scripts
			.filter_map(|script| {
				script.data().and_then(|s| {
					let s = s.trim();
					if s.is_empty() {
						return None;
					}
					Some(s.into())
				})
			})
			.collect::<Vec<String>>()
			.join("\n");
		let data = serde_json::to_string(&combined_scripts)
			.map_err(|_| error!("Failed to encode extracted JS"))?;
		let js_string =
			format!("let _encryptedString = {data};let _useServer2 = false;{IMG_DECRYPT_EVAL}");
		let result = JsContext::new().eval(&js_string)?;

		let links = serde_json::from_str::<Vec<String>>(&result)
			.map_err(|_| error!("Failed to decode JS result"))?;

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
