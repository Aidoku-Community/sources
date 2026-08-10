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
