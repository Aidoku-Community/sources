# Desu

Aidoku source for [desu.uno](https://desu.uno).

## Current API

- Catalog: `GET /manga/?page=...&order_by=...`
- Search: `POST /manga/search/` with the form field `q` and the
  `X-Requested-With: XMLHttpRequest` header
- Manga details: `GET /api/manga/{manga_id}`
- Chapters: `GET /api/manga/{manga_id}/chapters`
- Chapter pages: `GET /api/manga/{manga_id}/chapters/{chapter_id}`

The root `/api/manga` list endpoint is obsolete. Catalog and search use the
HTML endpoints above instead.

## Updating filters

Generate `res/filters.json` from the current `/manga/` DOM. Status values come
from `data-status`, kinds from `data-kind`, and genres from both
`data-genre-id` and `data-genre-slug`. Genre values must use the current
`id-slug` pairs, for example `90-Dementia`, rather than numeric IDs alone.

The source intentionally does not register `Home` or `ListingProvider`.
