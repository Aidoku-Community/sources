import json
import os
import subprocess
import shutil
from urllib.request import urlopen, Request
from bs4 import BeautifulSoup

# nhentai requires User-Agent
user_agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) GSA/300.0.598994205 Mobile/15E148 Safari/604"

# Parse from https://nhentai.net/tags all pages
tags = []
page = 1
while True:
    url = f"https://nhentai.net/tags/popular?page={page}"
    req = Request(url, headers={"User-Agent": user_agent})
    try:
        with urlopen(req) as response:
            html = response.read().decode('utf-8')
    except:
        break
    soup = BeautifulSoup(html, 'html.parser')
    page_tags = []
    for a in soup.select('a[href*="/tag/"]'):
        href = a['href']
        name = a.select_one('.name')
        if name:
            name = name.text.strip()
        else:
            name = a.text.strip()
        count_span = a.select_one('.count')
        if count_span:
            count_text = count_span.text.strip().replace(',', '')
            if count_text.endswith('K'):
                count = int(float(count_text[:-1]) * 1000)
            elif count_text.endswith('M'):
                count = int(float(count_text[:-1]) * 1000000)
            else:
                count = int(count_text)
        else:
            count = 0
        tag_id = href.strip('/').split('/')[-1]
        if count >= 10:  # include tags with popularity >= 10
            page_tags.append((name, tag_id, count))
    if not page_tags:
        break
    tags.extend(page_tags)
    page += 1
    if page > 100:  # safety break to avoid infinite loop
        break

# Sort by name
tags.sort(key=lambda x: x[0].lower())
popular_tags = [(name, id) for name, id, count in tags]

filters_json = os.path.join(
    os.path.dirname(os.path.realpath(__file__)), "..", "res", "filters.json"
)
with open(filters_json, "r") as f:
    filters = json.load(f)
    for filter in filters:
        if filter.get("id") == "tags":
            filter["options"] = [name for name, id in popular_tags]
            filter["ids"] = [id for name, id in popular_tags]

with open(filters_json, "w") as f:
    json.dump(filters, f, indent="\t")
    f.write("\n")
