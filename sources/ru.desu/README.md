### To update filters use following JS code in browser at [this page](https://desu.work/manga/):  
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
    let id = checkBox.dataset.genreSlug;
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
