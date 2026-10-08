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

## Ranobe support

- Catalog and metadata: `GET /ranobe/` and `GET /ranobe/{slug}.{book_id}/`
  (HTML)
- Chapter list: `GET /api/ranobe/{book_id}/chapters` (JSON)
- Chapter content: `GET /api/ranobe/{book_id}/chapters/{chapter_id}` (JSON)

Ranobe chapter content is an ordered sequence of text and image blocks. When a
chapter includes text, the source combines its text blocks into one Aidoku text
page, preserving paragraph/block boundaries, line breaks and basic bold/italic
formatting, and omits illustrations. This lets Aidoku's dedicated text reader
handle long chapters. If the API returns only images, the source keeps those
image pages in their original order. Chapter IDs come from the API; older
URL-based chapter keys are resolved against the API list for compatibility.

## Updating filters

Manga filters are generated from the current `/manga/` DOM. Status values come
from `data-status`, kinds from `data-kind`, and genres from both
`data-genre-id` and `data-genre-slug`. Genre values must use the current
`id-slug` pairs, for example `90-Dementia`, rather than numeric IDs alone.

Ranobe has a separate status/genre filter set from `/ranobe/`:
`ranobe_status` and `ranobe_genres`. Their current values are maintained in
`res/filters.json`; genre IDs use the `id-slug` form. Genre exclusions are
intentionally disabled for Ranobe: direct probes with excluded values were
canonicalized by the site to inclusion-only results, so the exclusion syntax
has not been confirmed. Keep Ranobe filters separate from Manga because their
option IDs differ. The UI presents both sets in the shared filter sheet; each
catalog handler ignores the other section's controls. The site's Ranobe `kind`
checkboxes did not yield a confirmed URL parameter during probing, so they are
deliberately not exposed by the source.

The source intentionally does not register `Home` or `ListingProvider`.

## Legacy filter generator

The script below was used with the previous desu.uno catalog DOM. It is kept
as a historical reference for maintainers and for investigating older source
versions.

It is **not compatible with the current source** and must not be run unchanged
to regenerate `res/filters.json`. In particular, the current genre filters use
`data-genre-id` together with `data-genre-slug` and `id-slug` values, while this
script emits the older genre representation.

### To update filters use following JS code in browser at [this page](https://desu.uno/manga/)
#### Note: this code will automatically copy a new filters JSON

```js
let result = [{
    "id": "order",
    "type": "sort",
    "title": "Упорядочить",
    "canAscend": false,
    "options": ["По добавлению", "По алфавиту", "По популярности", "По обновлению"],
    "default": {
        "index": 3
    }
}];

let getRoot = function (cls) {
    return document.querySelectorAll(`ul[class="${cls}"] > li > div`);
}

var temp = Array.from(getRoot('catalog-status')).map(x => {
    let id = x.querySelector('input[type="checkbox"]')?.dataset.status;
    let name = x.querySelector('span[class="filter-control-text"]')?.innerText;
    return { id, name };
});
result.push({
    id: 'status',
    type: 'multi-select',
    title: 'Статус',
    options: temp.map(x => x.name),
    ids: temp.map(x => x.id)
});

temp = Array.from(getRoot('catalog-kinds')).map(x => {
    let id = x.querySelector('input[type="checkbox"]')?.dataset.kind;
    let name = x.querySelector('span[class="filter-control-text"]')?.innerText;
    return { id, name };
});
result.push({
    id: 'kinds',
    type: 'multi-select',
    title: 'Тип',
    options: temp.map(x => x.name),
    ids: temp.map(x => x.id)
});

temp = Array.from(getRoot('catalog-genres')).map(x => {
    let checkBox = x.querySelector('input[type="checkbox"]');
    let isTag = x.querySelector('span[class="filter-control-text"] > span')?.innerText == '#';
    let id = checkBox.dataset.genreId;
    let name = checkBox.dataset.genreName;
    return { id, name, isTag };
});
result.push({
    id: 'genres',
    type: 'multi-select',
    title: 'Жанры',
    isGenre: true,
    canExclude: false,
    options: temp.filter(x => !x.isTag).map(x => x.name),
    ids: temp.filter(x => !x.isTag).map(x => x.id)
});
result.push({
    id: 'tags',
    type: 'multi-select',
    title: 'Теги',
    isGenre: true,
    canExclude: false,
    options: temp.filter(x => x.isTag).map(x => x.name),
    ids: temp.filter(x => x.isTag).map(x => x.id)
});

copy(JSON.stringify(result, null, 4) + '\n');
```
