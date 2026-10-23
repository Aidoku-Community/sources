# Olympus Scanlation

## Updating genres

The genre and status ids in `res/filters.json` come from `/api/genres-statuses`. To print the current genre options and ids:

```sh
curl -s https://olympusxyz.com/api/genres-statuses | python3 -c '
import json, sys
genres = json.load(sys.stdin)["genres"]
print(json.dumps(["Todos"] + [g["name"].strip() for g in genres], ensure_ascii=False))
print(json.dumps([""] + [str(g["id"]) for g in genres]))'
```

The site has two genres named "Superpoderes" (ids 33 and 70), so the second one is listed as "Superpoderes (2)".
