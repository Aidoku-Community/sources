#!/usr/bin/env python3
"""Regenerates the tags filter in res/filters.json from the Hentai2Read site.

The site is the source of truth for its tags: content tags come from its tag picker's JSON
endpoint and categories from its category index. Run this when the site adds or renames tags,
or with --check to fail if the filter is out of date.

    python3 scripts/update_filters.py          # rewrite res/filters.json
    python3 scripts/update_filters.py --check  # exit 1 if it would change
"""

import argparse
import json
import re
import sys
import urllib.request
from pathlib import Path

BASE_URL = "https://hentai2read.com"
FILTERS_PATH = Path(__file__).resolve().parent.parent / "res" / "filters.json"
USER_AGENT = "Mozilla/5.0 (compatible; aidoku-sources update_filters.py)"
PAGE_DATA = re.compile(r'<script data-page="app" type="application/json">(.*?)</script>', re.S)


def fetch(url: str, accept: str) -> str:
	request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT, "Accept": accept})
	with urllib.request.urlopen(request, timeout=30) as response:
		return response.read().decode("utf-8")


def content_tags() -> list[tuple[int, str]]:
	tags = json.loads(fetch(f"{BASE_URL}/search/tags", "application/json"))
	return [(tag["id"], tag["name"]) for tag in tags]


def categories() -> list[tuple[int, str]]:
	found = []
	page, last_page = 1, 1
	while page <= last_page:
		html = fetch(f"{BASE_URL}/tags/category?page={page}", "text/html")
		match = PAGE_DATA.search(html)
		if not match:
			sys.exit(f"no page data at /tags/category?page={page} (blocked by Cloudflare?)")
		tags = json.loads(match.group(1))["props"]["tags"]
		found += [(tag["id"], tag["name"]) for tag in tags["data"]]
		last_page = tags.get("lastPage", 1)
		page += 1
	return found


def main() -> None:
	parser = argparse.ArgumentParser(description="Regenerate the tags filter from Hentai2Read.")
	parser.add_argument("--check", action="store_true", help="exit 1 if filters.json is out of date")
	args = parser.parse_args()

	tags = {tag_id: name for tag_id, name in content_tags() + categories()}
	names = list(tags.values())
	duplicates = {name for name in names if names.count(name) > 1}
	if duplicates:
		sys.exit(f"duplicate tag names would be ambiguous in the filter: {sorted(duplicates)}")
	ordered = sorted(tags.items(), key=lambda tag: tag[1].lower())

	filters = json.loads(FILTERS_PATH.read_text())
	tag_filter = next(filter for filter in filters if filter.get("id") == "tags")
	old = dict(zip(tag_filter["ids"], tag_filter["options"]))
	tag_filter["options"] = [name for _, name in ordered]
	tag_filter["ids"] = [str(tag_id) for tag_id, _ in ordered]
	new = dict(zip(tag_filter["ids"], tag_filter["options"]))

	added = sorted(new.keys() - old.keys(), key=int)
	removed = sorted(old.keys() - new.keys(), key=int)
	renamed = sorted((key for key in new.keys() & old.keys() if new[key] != old[key]), key=int)
	for key in added:
		print(f"+ {key} {new[key]}")
	for key in removed:
		print(f"- {key} {old[key]}")
	for key in renamed:
		print(f"~ {key} {old[key]} -> {new[key]}")
	print(f"{len(ordered)} tags: {len(added)} added, {len(removed)} removed, {len(renamed)} renamed")

	output = json.dumps(filters, indent="\t", ensure_ascii=False) + "\n"
	if args.check:
		sys.exit(0 if output == FILTERS_PATH.read_text() else 1)
	FILTERS_PATH.write_text(output)


if __name__ == "__main__":
	main()
