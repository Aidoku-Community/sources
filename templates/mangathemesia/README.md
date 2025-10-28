# MangaThemesia Template

## Updating Genres

On the search page, paste into the console:

```js
(() => {
	const inputs = document.querySelectorAll('ul.genrez > li');
	const options = Array.from(inputs).map(i => i.lastElementChild.textContent.trim());
	const ids = Array.from(inputs).map(i => i.firstElementChild.value);
	console.log("options:", JSON.stringify(options));
	console.log("ids:", JSON.stringify(ids));
})();
```
