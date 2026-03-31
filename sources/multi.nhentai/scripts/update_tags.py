import json
import os
from urllib.request import urlopen, Request

# nhentai requires User-Agent
user_agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604"


def fetch_tags_from_api() -> list[tuple[str, int]]:
	"""Fetch popular tags from nhentai API v2."""
	tags: list[tuple[str, int]] = []
	page = 1
	while True:
		url = f"https://nhentai.net/api/v2/tags/tag?sort=popular&page={page}&per_page=100"
		req = Request(url, headers={"User-Agent": user_agent})
		try:
			with urlopen(req) as response:
				data = json.load(response)
		except Exception as exc:
			print(f"Failed to fetch tags from API: {exc}")
			break

		result = data.get("result", [])
		if not result:
			break

		for item in result:
			name = item.get("name", "").strip()
			count = item.get("count", 0)
			if not name:
				continue
			if count >= 10:
				tags.append((name, count))

		page += 1
		if page > data.get("num_pages", 0):
			break
		if page > 100:
			break

	return tags


if __name__ == "__main__":
	tags = fetch_tags_from_api()

	tags.sort(key=lambda x: x[0].lower())
	popular_tags = [name for name, _ in tags]

	filters_json = os.path.join(
		os.path.dirname(os.path.realpath(__file__)), "..", "res", "filters.json"
	)
	with open(filters_json, "r", encoding="utf-8") as f:
		filters = json.load(f)
		for filter in filters:
			if filter.get("id") == "tags":
				filter["options"] = popular_tags

	with open(filters_json, "w", encoding="utf-8") as f:
		json.dump(filters, f, indent="\t", ensure_ascii=False)
		_ = f.write("\n")
