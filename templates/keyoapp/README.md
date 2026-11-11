# Keyoapp Template

- `Keyoapp<T: Impl>`: catalogue at `/series/`.
- `KeyoappV2<T: ImplV2>`: catalogue at `?browse=1`. Set `"supportsTagSearch": true` in
  the consumer's `source.json`.

```rust
register_source!(KeyoappV2<Site>, Home, DeepLinkHandler);
```

## Updating Filters

Use the site's type, genre and status values in the consumer's `res/filters.json`.

For `Keyoapp`, run this in the console on `/series/`:

```js
(() => {
	const script = [...document.scripts].map(s => s.textContent).find(t => t.includes("initializeDropdownMenu"));
	for (const kind of ["type", "genre", "status"]) {
		const part = script.slice(script.indexOf(`type: "${kind}"`));
		const items = part.slice(part.indexOf("items:"), part.indexOf("]"));
		const options = [...items.matchAll(/displayName:\s*"([^"]+)"/g)].map(m => m[1]);
		const ids = [...items.matchAll(/value:\s*"([^"]+)"/g)].map(m => m[1]);
		console.log(kind, "options:", JSON.stringify(options));
		console.log(kind, "ids:", JSON.stringify(ids));
	}
})();
```

For `KeyoappV2`, run this on `?browse=1`:

```js
(() => {
	for (const [id, key] of [["fsel-sort", "sort"], ["fsel-type", "type"], ["fsel-status", "status"], ["fsel-genres", "tag[]"]]) {
		const options = [...document.querySelectorAll(`#${id} [role=option]`)];
		console.log(id, "options:", JSON.stringify(options.map(o => o.textContent.trim())));
		console.log(id, "ids:", JSON.stringify(options.map(o => new URL(o.href).searchParams.get(key) ?? "")));
	}
})();
```

Keep V2 sorts in the `SORTS` order from `src/v2.rs`. Use the site's `tag[]` values as genre IDs.

## Home

Set `Params::hidden_rows` to omit home rows by heading.
