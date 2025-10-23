# MadTheme Template

## Updating Genres

On the search page, paste into the console:

```js
(() => {
	const inputs = document.querySelectorAll('input[type="checkbox"][name="genre[]"]');
	const options = Array.from(inputs).map(i => i.parentElement.nextElementSibling.textContent.trim());
	const ids = Array.from(inputs).map(i => i.value);
	console.log("options:", JSON.stringify(options));
	console.log("ids:", JSON.stringify(ids));
})();
```
