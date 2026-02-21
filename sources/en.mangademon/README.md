# MangaDemon

## Updating Genres

On the advanced search page, paste into the console:

```js
(() => {
	const inputs = document.querySelectorAll('#genres-container > li');
	const options = Array.from(inputs).map(i => i.lastChild.textContent.trim());
	const ids = Array.from(inputs).map(i => i.firstElementChild.value);
	console.log("options:", JSON.stringify(options));
	console.log("ids:", JSON.stringify(ids));
})();
```
