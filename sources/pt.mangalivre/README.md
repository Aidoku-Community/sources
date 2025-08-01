# MangaLivre

## Updating Genres

This source uses a different style for the home page, so instead of running the genre update script, you can go to the site search page (`https://mangalivre.tv/?s=&post_type=wp-manga`) and paste this in the console:

```js
(() => {
	const links = document.querySelectorAll(".genres-filter a.btn, .genres-filter a.dropdown-item");
	const options = Array.from(links).map(a => a.textContent.trim());
	const ids = Array.from(links).map(a => {
		const m = a.href.match(/genre=([^&]+)/);
		return m ? decodeURIComponent(m[1]) : null;
	});
	console.log("options:", JSON.stringify(options));
	console.log("ids:", JSON.stringify(ids));
})();
```
