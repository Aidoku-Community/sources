# Asura Scans

## Updating Genres

On https://asurascans.com/browse, run:

```js
(() => {
  const island = document.querySelector('astro-island[component-url*="BrowseFilters"]');
  if (!island) {
    console.error('BrowseFilters astro-island not found');
    return;
  }

  const rawProps = island.getAttribute('props');
  if (!rawProps) {
    console.error('No props attribute found on astro-island');
    return;
  }

  // decode html entities
  const textarea = document.createElement('textarea');
  textarea.innerHTML = rawProps;
  const decodedProps = textarea.value;
  const props = JSON.parse(decodedProps);

  const genreEntries = props.availableGenres?.[1] || [];
  const genres = genreEntries.map((entry) => {
    const g = entry?.[1] || {};
    return {
      id: g.id?.[1],
      name: g.name?.[1],
      slug: g.slug?.[1],
    };
  }).filter(g => g.id != null && g.name && g.slug);

  const options = genres.map(g => g.name);
  const ids = genres.map(g => g.slug);

  console.log('options:', JSON.stringify(options));
  console.log('ids:', JSON.stringify(ids));
})();
```
