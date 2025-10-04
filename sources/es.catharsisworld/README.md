# CatharsisWorld

## Updating Genres

This source uses a different style for the home page, so instead of running the genre update script, you can go to the site search page (`https://catharsisworld.dig-it.info/?s=&post_type=wp-manga`) and paste this in the console:

```js
(() => {
  const container = document.querySelector(".flex.flex-wrap");
  const inputs = container.querySelectorAll('input[type="checkbox"][name="genre[]"]');
  const options = Array.from(inputs).map(i => i.nextElementSibling.textContent.trim());
  const ids = Array.from(inputs).map(i => i.value);
  console.log("options:", JSON.stringify(options));
  console.log("ids:", JSON.stringify(ids));
})();
```
