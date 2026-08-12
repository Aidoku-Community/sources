use aidoku::alloc::{String, Vec};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Localized {
	pub(crate) lang: String,
	pub(crate) content: String,
}

#[derive(Deserialize)]
pub(crate) struct Cover {
	pub(crate) original: Option<ImageSize>,
}

#[derive(Deserialize)]
pub(crate) struct ImageSize {
	pub(crate) url: String,
}

#[derive(Deserialize)]
pub(crate) struct SearchData {
	pub(crate) search: SearchConnection<SearchManga>,
}

#[derive(Deserialize)]
pub(crate) struct MangaData {
	pub(crate) manga: MangaInfo,
}

#[derive(Deserialize)]
pub(crate) struct HomeMangasData {
	pub(crate) mangas: HomeMangaConnection,
}

#[derive(Deserialize)]
pub(crate) struct HomePopularData {
	#[serde(rename = "mangaPopularByPeriod")]
	pub(crate) manga_popular_by_period: Vec<HomeManga>,
}

#[derive(Deserialize)]
pub(crate) struct HomeRecommendationsData {
	#[serde(rename = "mangaRecommendations")]
	pub(crate) manga_recommendations: Vec<HomeManga>,
}

#[derive(Deserialize)]
pub(crate) struct ChaptersData {
	#[serde(rename = "mangaChapters")]
	pub(crate) manga_chapters: ChapterConnection,
}

#[derive(Deserialize)]
pub(crate) struct ReaderData {
	#[serde(rename = "mangaChapter")]
	pub(crate) manga_chapter: ReaderChapter,
}

#[derive(Deserialize)]
pub(crate) struct SearchConnection<T> {
	pub(crate) edges: Vec<Edge<T>>,
}

#[derive(Deserialize)]
pub(crate) struct HomeMangaConnection {
	pub(crate) edges: Vec<Edge<HomeManga>>,
	#[serde(rename = "pageInfo")]
	pub(crate) page_info: PageInfo,
}

#[derive(Deserialize)]
pub(crate) struct ChapterConnection {
	pub(crate) edges: Vec<Edge<RemoteChapter>>,
	#[serde(rename = "pageInfo")]
	pub(crate) page_info: PageInfo,
}

#[derive(Deserialize)]
pub(crate) struct PageInfo {
	#[serde(rename = "hasNextPage")]
	pub(crate) has_next_page: bool,
	#[serde(rename = "endCursor")]
	pub(crate) end_cursor: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Edge<T> {
	pub(crate) node: T,
}

#[derive(Deserialize)]
pub(crate) struct SearchManga {
	pub(crate) slug: String,
	pub(crate) original_name: String,
	pub(crate) titles: Vec<Localized>,
	pub(crate) manga_status: String,
	pub(crate) manga_rating: String,
	pub(crate) cover: Option<Cover>,
}

#[derive(Deserialize)]
pub(crate) struct HomeManga {
	pub(crate) slug: String,
	#[serde(rename = "originalName")]
	pub(crate) original_name: Localized,
	pub(crate) titles: Vec<Localized>,
	pub(crate) status: String,
	pub(crate) rating: String,
	pub(crate) cover: Option<Cover>,
}

#[derive(Deserialize)]
pub(crate) struct Branch {
	pub(crate) id: String,
	#[serde(rename = "primaryBranch")]
	pub(crate) primary_branch: bool,
	#[serde(rename = "teamActivities")]
	pub(crate) team_activities: Vec<TeamActivity>,
}

#[derive(Deserialize)]
pub(crate) struct TeamActivity {
	pub(crate) team: Team,
}

#[derive(Deserialize)]
pub(crate) struct Team {
	pub(crate) id: String,
	pub(crate) name: String,
	#[allow(dead_code)]
	pub(crate) slug: String,
}

#[derive(Deserialize)]
pub(crate) struct Label {
	pub(crate) slug: String,
	pub(crate) titles: Vec<Localized>,
}

#[derive(Deserialize)]
pub(crate) struct MangaInfo {
	pub(crate) slug: String,
	pub(crate) original_name: Localized,
	pub(crate) titles: Vec<Localized>,
	pub(crate) manga_status: String,
	pub(crate) rating: String,
	#[serde(rename = "mainStaff")]
	pub(crate) main_staff: Vec<StaffMember>,
	pub(crate) branches: Vec<Branch>,
	pub(crate) cover: Option<Cover>,
	pub(crate) labels: Vec<Label>,
}

#[derive(Deserialize)]
pub(crate) struct StaffMember {
	pub(crate) person: Person,
	pub(crate) roles: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct Person {
	pub(crate) name: String,
}

#[derive(Deserialize)]
pub(crate) struct RemoteChapter {
	pub(crate) slug: String,
	pub(crate) team_ids: Vec<String>,
	pub(crate) name: Option<String>,
	pub(crate) number: String,
	pub(crate) volume: String,
}

#[derive(Deserialize)]
pub(crate) struct ReaderChapter {
	pub(crate) pages: Vec<ReaderPage>,
}

#[derive(Deserialize)]
pub(crate) struct ReaderPage {
	pub(crate) image: Option<Cover>,
}
