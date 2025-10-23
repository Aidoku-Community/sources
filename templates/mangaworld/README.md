# MangaWorld Template

## Updating Genres

On the source archive page (e.g. `https://www.mangaworld.cx/archive`), open the genre dropdown and paste into the console:

```js
(() => {
	const container = document.querySelector("div.dropdown.show > select");
	const inputs = container.querySelectorAll("option");
	const options = Array.from(inputs).map(i => i.textContent.trim());
	const ids = Array.from(inputs).map(i => i.getAttribute("data-name"));
	console.log("options:", JSON.stringify(options));
	console.log("ids:", JSON.stringify(ids));
})();
```
