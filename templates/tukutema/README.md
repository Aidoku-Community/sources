# Tukutema Template

## Updating Genres

On the source library page (e.g. `https://rawkuma.net/library/`), paste into the console:

```js
(() => {
	const genres = searchTerms.genre;
	const options = Object.values(genres).map(g => g.name);
	const ids = Object.values(genres).map(g => g.slug);
	console.log("options:", JSON.stringify(options));
	console.log("ids:", JSON.stringify(ids));
})();
```
