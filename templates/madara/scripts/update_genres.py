import sys
import json
import urllib.request
from html.parser import HTMLParser

class GenreHTMLParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.in_checkbox = False
        self.in_label = False
        self.in_checkbox_group = False
        self.checkbox_group_found = False
        self.current_genre_id = None
        self.current_genre_name = None
        self.genre_ids = []
        self.genre_names = []

    def handle_starttag(self, tag, attrs):
        attrs_dict = dict(attrs)
        classes = attrs_dict.get("class", "")
        if tag == "div" and "checkbox-group" in classes and not self.checkbox_group_found:
            self.in_checkbox_group = True
            self.checkbox_group_found = True
        elif tag == "div" and "checkbox" in classes and self.in_checkbox_group:
            self.in_checkbox = True
            self.current_genre_id = None
            self.current_genre_name = None
        elif self.in_checkbox and tag == "label":
            self.in_label = True
        elif self.in_checkbox and tag == "input":
            if attrs_dict.get("type") == "checkbox" and "value" in attrs_dict:
                self.current_genre_id = attrs_dict["value"]

    def handle_endtag(self, tag):
        if tag == "div" and self.in_checkbox:
            if self.current_genre_id and self.current_genre_name:
                self.genre_ids.append(self.current_genre_id)
                self.genre_names.append(self.current_genre_name.strip())
            self.in_checkbox = False
            self.current_genre_id = None
            self.current_genre_name = None
        elif tag == "div" and self.in_checkbox_group:
            self.in_checkbox_group = False
        elif tag == "label" and self.in_label:
            self.in_label = False

    def handle_data(self, data):
        if self.in_label:
            self.current_genre_name = (self.current_genre_name or "") + data

def fetch_genres_from_html(base_url, genre_endpoint="/?s=genre&post_type=wp-manga"):
    url = base_url.rstrip("/") + genre_endpoint
    req = urllib.request.Request(
        url,
        headers={"Referer": base_url.rstrip("/") + "/", "User-Agent": "Aidoku"}
    )
    with urllib.request.urlopen(req) as response:
        html = response.read().decode("utf-8")
    parser = GenreHTMLParser()
    parser.feed(html)
    return parser.genre_names, parser.genre_ids

def update_filters(filters_path, base_url):
    with open(filters_path, "r") as f:
        filters = json.load(f)
    genre_names, genre_ids = fetch_genres_from_html(base_url)
    for filter in filters:
        if filter.get("isGenre"):
            filter["options"] = genre_names
            filter["ids"] = genre_ids
    with open(filters_path, "w") as f:
        json.dump(filters, f, indent="\t", ensure_ascii=False)
        f.write("\n")
    print("Genres updated successfully.")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python update_genres.py /path/to/filters.json base_url")
        sys.exit(1)
    update_filters(sys.argv[1], sys.argv[2])
