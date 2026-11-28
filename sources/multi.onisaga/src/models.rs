use crate::{
	api::{BASE_URL, CDN_URL},
	helpers::{
		clean_chapter_title, dedup_keep_order, demographic_name, form_name, media_url, non_empty,
		origin_name, parse_chapter_number, parse_content_rating, parse_iso_date, parse_status,
		plain_synopsis, status_name, viewer,
	},
};
use aidoku::{
	AidokuError, Chapter, Manga, MangaStatus, Result,
	alloc::{String, Vec, format, vec},
	prelude::*,
};
use serde::{Deserialize, Serialize};

const MAX_TERMS: usize = 20;

#[derive(Serialize, Deserialize)]
pub struct Rpc<T> {
	pub json: T,
}

#[derive(Deserialize)]
pub struct ErrorResponse {
	pub error: ErrorBody,
}

#[derive(Deserialize)]
pub struct ErrorBody {
	pub message: Option<String>,
}

#[derive(Default, Serialize)]
pub struct Terms {
	pub include: Vec<String>,
	pub exclude: Vec<String>,
}

impl Terms {
	pub fn is_empty(&self) -> bool {
		self.include.is_empty() && self.exclude.is_empty()
	}

	pub fn finalize(&mut self) {
		dedup_keep_order(&mut self.include);
		self.include.truncate(MAX_TERMS);
		let include = &self.include;
		self.exclude.retain(|slug| !include.contains(slug));
		dedup_keep_order(&mut self.exclude);
		self.exclude.truncate(MAX_TERMS);
	}
}

#[derive(Default, Serialize)]
pub struct Years {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub from: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub to: Option<i32>,
}

#[derive(Clone, Deserialize)]
pub struct TermOption {
	pub slug: String,
	pub name: String,
	pub group: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
	pub items: Vec<Work>,
	pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Work {
	slug: String,
	title: Option<String>,
	cover_url: Option<String>,
	origin: Option<String>,
	form: Option<String>,
	layout: Option<String>,
	demographic: Option<String>,
	publication_status: Option<String>,
	content_rating: Option<String>,
	synopsis: Option<String>,
	#[serde(default)]
	languages: Vec<WorkLanguage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkLanguage {
	language: String,
	readable_units: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeData {
	pub spotlight: Section<Vec<SpotlightItem>>,
	pub latest: Section<Vec<LatestEntry>>,
	pub new_on_site: Section<Vec<Work>>,
	pub finished: Section<Vec<Work>>,
	pub newly_published: Section<Vec<Work>>,
	#[serde(default)]
	pub trending: Vec<Work>,
}

#[derive(Deserialize)]
pub struct Section<T> {
	pub value: Option<T>,
}

#[derive(Deserialize)]
pub struct SpotlightItem {
	pub card: Option<Work>,
}

#[derive(Deserialize)]
pub struct LatestEntry {
	pub card: Option<Work>,
	#[serde(default)]
	pub units: Vec<LatestUnit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestUnit {
	release_id: Option<String>,
	label: String,
	title: Option<String>,
	language: String,
	at: Option<String>,
}

#[derive(Default, Deserialize)]
pub struct WorkDetail {
	pub slug: String,
	canonical_title: String,
	origin: Option<String>,
	layout: Option<String>,
	demographic: Option<String>,
	publication_status: String,
	content_rating: String,
	synopsis_md: Option<String>,
	metadata_version: String,
	#[serde(default)]
	credits: Vec<Credit>,
	#[serde(default)]
	terms: Vec<WorkTerm>,
	pub reading: Reading,
}

#[derive(Deserialize)]
struct Credit {
	name: String,
	role: String,
}

#[derive(Deserialize)]
struct WorkTerm {
	group: String,
	name: String,
	#[serde(default)]
	spoiler: bool,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
	pub work_id: String,
	#[serde(default)]
	streams: Vec<Stream>,
	#[serde(default)]
	languages: Vec<Scope>,
	#[serde(default)]
	pub groups: Vec<Group>,
	default_scope: Option<Scope>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stream {
	id: String,
	#[serde(default)]
	is_default: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
	pub stream_id: String,
	pub edition_id: Option<String>,
	pub language: String,
	#[serde(default)]
	readable_units: i32,
}

#[derive(Deserialize)]
pub struct Group {
	id: String,
	name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPage {
	pub items: Vec<ChapterUnit>,
	pub next_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterUnit {
	pub id: String,
	number: Option<String>,
	label: String,
	title: Option<String>,
	volume: Option<String>,
	#[serde(default)]
	pub readable_candidate_count: Option<i32>,
	pub selection: Option<Selection>,
}

#[derive(Deserialize)]
pub struct SelectPage {
	pub items: Vec<Selection>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
	#[serde(default)]
	pub chapter_id: String,
	selected_release_id: Option<String>,
	#[serde(default)]
	candidates: Vec<Release>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Release {
	id: String,
	#[serde(default)]
	group_ids: Vec<String>,
	published_at: String,
	readable_at: Option<String>,
	is_readable: bool,
	title: Option<String>,
	label: String,
}

#[derive(Deserialize)]
pub struct Bootstrap {
	manifest: Option<Manifest>,
}

#[derive(Deserialize)]
struct Manifest {
	pages: Vec<PageImage>,
}

#[derive(Clone, Deserialize)]
pub struct PageImage {
	pub ord: i32,
	pub url: String,
}

fn build_work(
	slug: String,
	title: Option<String>,
	cover: Option<String>,
	status: Option<&str>,
	rating: Option<&str>,
) -> Result<Manga> {
	let title = title
		.and_then(non_empty)
		.ok_or_else(|| error!("OniSaga returned a title without a name"))?;
	if slug.is_empty() {
		bail!("OniSaga returned a title without a slug");
	}
	// Adult covers come as a path on the main host, which carries the age cookie; the media host
	// would blur them.
	let cover = match cover {
		Some(path) if path.starts_with('/') => format!("{BASE_URL}{path}"),
		Some(url) => url,
		None => format!("{CDN_URL}/api/covers/{slug}"),
	};
	let cover = if matches!(rating, Some("mature" | "explicit")) {
		cover.replacen(CDN_URL, BASE_URL, 1)
	} else {
		cover
	};
	// The site's largest card width; the full original is several times heavier.
	let separator = if cover.contains('?') { '&' } else { '?' };
	Ok(Manga {
		key: slug,
		title,
		cover: Some(format!("{cover}{separator}w=480")),
		status: status.map_or(MangaStatus::Unknown, parse_status),
		content_rating: parse_content_rating(rating),
		..Default::default()
	})
}

impl Work {
	pub fn allowed(
		&self,
		ratings: &[String],
		origins: Option<&[String]>,
		languages: &[String],
	) -> bool {
		self.content_rating
			.as_ref()
			.is_none_or(|rating| ratings.contains(rating))
			&& origins.is_none_or(|origins| {
				self.origin
					.as_ref()
					.is_some_and(|origin| origins.contains(origin))
			}) && (self.languages.is_empty()
			|| self
				.languages
				.iter()
				.any(|entry| languages.contains(&entry.language)))
	}

	pub fn into_details(self, languages: &[String]) -> Result<Manga> {
		let Self {
			slug,
			title,
			cover_url,
			origin,
			form,
			layout,
			demographic,
			publication_status,
			content_rating,
			synopsis,
			languages: work_languages,
		} = self;
		// The app's Spotlight shows no status or chapter count of its own, so they follow the type
		// as on the site's slide; the count is the reader's best language, each one numbering alike.
		let chapters = work_languages
			.into_iter()
			.filter(|entry| languages.contains(&entry.language))
			.filter_map(|entry| entry.readable_units)
			.max()
			.filter(|&count| count > 0)
			.map(|count| match count {
				1 => "1 chapter".into(),
				_ => format!("{count} chapters"),
			});
		let pills = [
			origin.as_deref().and_then(origin_name).map(Into::into),
			publication_status
				.as_deref()
				.and_then(status_name)
				.map(Into::into),
			chapters,
			demographic
				.as_deref()
				.and_then(demographic_name)
				.map(Into::into),
			form.as_deref().and_then(form_name).map(Into::into),
		]
		.into_iter()
		.flatten()
		.collect::<Vec<String>>();
		let tags = Some(if pills.is_empty() {
			vec!["Comic".into()]
		} else {
			pills
		});
		let description = synopsis.as_deref().and_then(plain_synopsis);
		let viewer = viewer(layout.as_deref(), origin.as_deref(), &[]);
		Ok(Manga {
			description,
			tags,
			viewer,
			..build_work(
				slug,
				title,
				cover_url,
				publication_status.as_deref(),
				content_rating.as_deref(),
			)?
		})
	}
}

impl TryFrom<Work> for Manga {
	type Error = AidokuError;

	fn try_from(work: Work) -> Result<Self> {
		build_work(
			work.slug,
			work.title,
			work.cover_url,
			work.publication_status.as_deref(),
			work.content_rating.as_deref(),
		)
	}
}

impl LatestUnit {
	pub fn into_chapter(self, languages: &[String]) -> Option<Chapter> {
		if !languages.contains(&self.language) {
			return None;
		}
		let key = self.release_id.filter(|id| !id.is_empty())?;
		let chapter_number = parse_chapter_number(&self.label);
		let title = clean_chapter_title(self.title, &self.label, chapter_number);
		Some(Chapter {
			key,
			title,
			chapter_number,
			date_uploaded: self.at.as_deref().and_then(parse_iso_date),
			language: Some(self.language),
			..Default::default()
		})
	}
}

impl WorkDetail {
	pub fn into_basic_manga(self) -> Result<Manga> {
		let cover = (!self.slug.is_empty()).then(|| {
			if self.metadata_version.is_empty() {
				format!("{CDN_URL}/api/covers/{}", self.slug)
			} else {
				format!(
					"{CDN_URL}/api/covers/{}?mv={}",
					self.slug, self.metadata_version
				)
			}
		});
		build_work(
			self.slug,
			Some(self.canonical_title),
			cover,
			Some(&self.publication_status),
			Some(&self.content_rating),
		)
	}
}

impl TryFrom<WorkDetail> for Manga {
	type Error = AidokuError;

	fn try_from(detail: WorkDetail) -> Result<Self> {
		let WorkDetail {
			slug,
			canonical_title,
			origin,
			layout,
			demographic,
			publication_status,
			content_rating,
			synopsis_md,
			metadata_version,
			credits,
			terms,
			..
		} = detail;

		let mut authors = Vec::new();
		let mut artists = Vec::new();
		for (name, role) in credits
			.into_iter()
			.filter_map(|c| non_empty(c.name).map(|name| (name, c.role)))
		{
			if matches!(
				role.as_str(),
				"story" | "author" | "story_and_art" | "original_creator"
			) && !authors.contains(&name)
			{
				authors.push(name.clone());
			}
			if matches!(
				role.as_str(),
				"art" | "illustrator" | "story_and_art" | "character_design"
			) && !artists.contains(&name)
			{
				artists.push(name);
			}
		}

		let mut tags = terms
			.into_iter()
			.filter(|t| {
				!t.spoiler && matches!(t.group.as_str(), "genre" | "theme" | "demographic" | "tag")
			})
			.filter_map(|t| non_empty(t.name))
			.collect::<Vec<_>>();
		if let Some(demo) = demographic.as_deref().and_then(demographic_name) {
			tags.push(demo.into());
		}
		tags.sort_unstable();
		tags.dedup();

		let viewer = viewer(layout.as_deref(), origin.as_deref(), &tags);
		let description = synopsis_md.and_then(non_empty);
		let basic = WorkDetail {
			slug,
			canonical_title,
			publication_status,
			content_rating,
			metadata_version,
			..Default::default()
		}
		.into_basic_manga()?;
		let url = Some(format!("{BASE_URL}/title/{}", basic.key));
		Ok(Manga {
			authors: (!authors.is_empty()).then_some(authors),
			artists: (!artists.is_empty()).then_some(artists),
			description,
			url,
			tags: (!tags.is_empty()).then_some(tags),
			viewer,
			..basic
		})
	}
}

impl Reading {
	// Prefer the title's default stream when a language appears in more than one numbering.
	pub fn language_scopes(&self, languages: &[String]) -> Vec<Scope> {
		let default_stream = self
			.default_scope
			.as_ref()
			.map(|s| s.stream_id.as_str())
			.or_else(|| {
				self.streams
					.iter()
					.find(|s| s.is_default)
					.map(|s| s.id.as_str())
			});
		languages
			.iter()
			.filter_map(|lang| {
				self.languages
					.iter()
					.filter(|s| s.language == *lang && s.readable_units > 0)
					.max_by_key(|s| {
						let preferred = default_stream == Some(s.stream_id.as_str())
							|| self.default_scope.as_ref().is_some_and(|def| {
								def.stream_id == s.stream_id
									&& def.edition_id == s.edition_id
									&& def.language == s.language
							});
						(preferred, s.readable_units)
					})
					.cloned()
			})
			.collect()
	}
}

impl ChapterUnit {
	pub fn into_chapters(
		self,
		slug: &str,
		language: &str,
		groups: &[Group],
		deduplicate: bool,
		fallback: Option<Selection>,
	) -> Vec<Chapter> {
		let Some(selection) = self.selection.or(fallback) else {
			return Vec::new();
		};
		let chapter_number = self
			.number
			.as_deref()
			.and_then(|n| n.parse::<f32>().ok())
			.or_else(|| parse_chapter_number(&self.label));
		let volume_number = self.volume.as_deref().and_then(|v| v.parse::<f32>().ok());
		let unit_label = self.label;
		let unit_title = self.title;

		let mut releases = selection
			.candidates
			.into_iter()
			.filter(|r| r.is_readable && !r.id.is_empty())
			.collect::<Vec<_>>();
		if let Some(selected_id) = selection.selected_release_id.as_deref()
			&& let Some(pos) = releases.iter().position(|r| r.id == selected_id)
			&& pos > 0
		{
			let chosen = releases.remove(pos);
			releases.insert(0, chosen);
		}
		if deduplicate {
			releases.truncate(1);
		}

		releases
			.into_iter()
			.map(|r| {
				let scanlators = r
					.group_ids
					.iter()
					.filter_map(|id| {
						let name = groups.iter().find(|g| g.id == *id)?.name.trim();
						(!name.is_empty()).then(|| name.into())
					})
					.collect::<Vec<String>>();
				let label = if r.label.is_empty() {
					&unit_label
				} else {
					&r.label
				};
				let title = clean_chapter_title(
					r.title.or_else(|| unit_title.clone()),
					label,
					chapter_number,
				);
				let date_uploaded = r
					.readable_at
					.as_deref()
					.or(Some(r.published_at.as_str()))
					.and_then(parse_iso_date);
				let url = Some(format!("{BASE_URL}/read/{slug}/{}", r.id));
				Chapter {
					key: r.id,
					title,
					chapter_number,
					volume_number,
					date_uploaded,
					scanlators: (!scanlators.is_empty()).then_some(scanlators),
					url,
					language: Some(language.into()),
					..Default::default()
				}
			})
			.collect()
	}
}

impl Bootstrap {
	pub fn into_pages(self) -> Result<Vec<PageImage>> {
		let mut pages = self.manifest.map(|m| m.pages).unwrap_or_default();
		if pages.is_empty() || pages.iter().any(|p| p.url.is_empty()) {
			bail!("OniSaga has no pages for this chapter");
		}
		pages.sort_unstable_by_key(|p| p.ord);
		for page in &mut pages {
			page.url = media_url(core::mem::take(&mut page.url));
		}
		Ok(pages)
	}
}
