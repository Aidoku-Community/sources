# ニコニコ静画 private api

the api has more functions that could be implemented into the source in the future.

home:
- data: https://api.nicomanga.jp/api/v1/app/manga/aggregate/homescreen?primary_thumbnail_type=thumbnail&recommend_type=t1
- data.result.pickup.primary: featured banners and regular manga
- data.result.pickup_recommends: contains objects with title and contents for scroller
- data.result.features: see above
- data.result.recommends: see above

listings:
- ranking: https://api.nicomanga.jp/api/v1/app/manga/contents/ranking?limit=20&category=all&span=hourly&offset=0
  - category can be shonen, josei, etc.
  - span can be hourly, daily, weekly, monthly, or total
- recommended: https://api.nicomanga.jp/api/v1/app/manga/contents/recommend?offset=0&limit=5
  - only returns a small number of items

search:
- genres:
  - genre ids: https://api.nicomanga.jp/api/v1/app/manga/genres
  - manga matching genre: https://api.nicomanga.jp/api/v1/app/manga/genres/74/contents?limit=20&offset=0&sort=contents_updated
- can set category: https://api.nicomanga.jp/api/v1/app/manga/contents?limit=20&offset=0&sort=contents_updated&category=shonen
- can search by tag instead of keyword: https://api.nicomanga.jp/api/v1/app/manga/contents?limit=20&mode=tag&offset=0&q=%E8%8A%B1%E8%A6%8B&sort=contents_updated
- can't filter by both keyword and other filters, besides sort, so `hidesFiltersWhileSearching` needs to be set but would disable sorting when searching.
