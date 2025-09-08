pub enum SortOptions {
    RatingScore,
    MostFollows,
    MostReviews,
    MostComments,
    MostChapters,
    NewChapters,
    RecentlyCreated,
    NameAtoZ,
    Views24hours,
    Views7days,
    Views30days,
    Views360days,
    ViewsTotal,
}

impl From<i32> for SortOptions {
	fn from(value: i32) -> Self {
		match value {
			0 => SortOptions::RatingScore,
			1 => SortOptions::MostFollows,
			2 => SortOptions::MostReviews,
			3 => SortOptions::MostComments,
			4 => SortOptions::MostChapters,
			5 => SortOptions::NewChapters,
            6 => SortOptions::RecentlyCreated,
			7 => SortOptions::NameAtoZ,
			8 => SortOptions::Views24hours,
			9 => SortOptions::Views7days,
		    10 => SortOptions::Views30days,
            11 => SortOptions::Views360days,
		    12 => SortOptions::ViewsTotal,
			_ => SortOptions::RatingScore,
		}
	}
}

impl From<SortOptions> for &str {
	fn from(val: SortOptions) -> Self {
		match val {
			SortOptions::RatingScore => "field_score",
			SortOptions::MostFollows => "field_follow",
			SortOptions::MostReviews => "field_review",
			SortOptions::MostComments => "field_comment",
			SortOptions::MostChapters => "field_chapter",
			SortOptions::NewChapters => "field_update",
            SortOptions::RecentlyCreated => "field_create",
			SortOptions::NameAtoZ => "field_name",
			SortOptions::Views24hours => "views_h024",
			SortOptions::Views7days => "views_d007",
		    SortOptions::Views30days => "views_d030",
            SortOptions::Views360days => "views_d360",
		    SortOptions::ViewsTotal => "views_d00",
		}
	}
}