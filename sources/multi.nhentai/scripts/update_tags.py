import json
import os
import re
from urllib.request import urlopen, Request

# nhentai requires User-Agent
user_agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604"


# Parse from https://nhentai.net/tags all pages
def extract_tags(html: str) -> list[tuple[str, int]]:
    tags: list[tuple[str, int]] = []
    # Find all <a href="/tag/...">...</a>
    for m in re.finditer(r'<a[^>]+href="(/tag/[^"]+)"[^>]*>(.*?)</a>', html, re.DOTALL):
        a_html: str = m.group(0)
        # Extract tag name
        name_match = re.search(r'<span[^>]*class="name"[^>]*>(.*?)</span>', a_html)
        if name_match:
            name: str = name_match.group(1).strip()
        else:
            # Fallback: get text between <a>...</a> minus any <span>
            name = re.sub(r"<.*?>", "", m.group(2)).strip()
        # Extract count
        count_match = re.search(r'<span[^>]*class="count"[^>]*>(.*?)</span>', a_html)
        if count_match:
            count_text: str = count_match.group(1).strip().replace(",", "")
            if count_text.endswith("K"):
                count: int = int(float(count_text[:-1]) * 1000)
            elif count_text.endswith("M"):
                count = int(float(count_text[:-1]) * 1000000)
            else:
                try:
                    count = int(count_text)
                except ValueError:
                    count = 0
        else:
            count = 0
        if count >= 10:
            tags.append((name, count))
    return tags


tags: list[tuple[str, int]] = []
page = 1
while True:
    url = f"https://nhentai.net/tags/popular?page={page}"
    req: Request = Request(url, headers={"User-Agent": user_agent})
    try:
        with urlopen(req) as response:
            html = response.read().decode("utf-8")
    except Exception:
        break
    page_tags = extract_tags(html)
    if not page_tags:
        break
    tags.extend(page_tags)
    page += 1
    if page > 100:
        break

tags.sort(key=lambda x: x[0].lower())
popular_tags = [name for name, _ in tags]

filters_json = os.path.join(
    os.path.dirname(os.path.realpath(__file__)), "..", "res", "filters.json"
)
with open(filters_json, "r") as f:
    filters = json.load(f)
    for filter in filters:
        if filter.get("id") == "tags":
            filter["options"] = popular_tags

with open(filters_json, "w") as f:
    json.dump(filters, f, indent="\t")
    _ = f.write("\n")
