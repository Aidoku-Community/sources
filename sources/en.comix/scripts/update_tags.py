import dataclasses
import json
from pathlib import Path
from urllib.request import Request, urlopen


class EnhancedJSONEncoder(json.JSONEncoder):
    def default(self, o):
        if dataclasses.is_dataclass(o):
            return dataclasses.asdict(o)
        return super().default(o)


def fetch_genres(url: str):
    req = Request(url)
    with urlopen(req) as response:
        data = response.read().decode("utf-8")
    return json.loads(data)["result"]["items"]


def update_filters_file(filters_path: Path, genres):
    with filters_path.open("r") as f:
        filters = json.load(f)

    for filter_obj in filters:
        if filter_obj.get("title") == "Genres":
            filter_obj["options"] = [genre["title"] for genre in genres]
            filter_obj["ids"] = [str(genre["term_id"]) for genre in genres]

    with filters_path.open("w") as f:
        json.dump(filters, f, indent="\t", cls=EnhancedJSONEncoder)
        f.write("\n")


def update_settings_file(path: Path, genres, themes):
    with path.open("r") as f:
        settings = json.load(f)

    def update_multi_select(items, key, new_titles, new_values):
        for item in items:
            if item.get("type") == "multi-select" and item.get("key") == key:
                item["titles"] = new_titles
                item["values"] = new_values
            if "items" in item:
                update_multi_select(item["items"], key, new_titles, new_values)

    genre_titles = [genre["title"] for genre in genres]
    genre_values = [str(genre["term_id"]) for genre in genres]
    theme_titles = [theme["title"] for theme in themes]
    theme_values = [str(theme["term_id"]) for theme in themes]

    update_multi_select(settings, "hiddenGenres", genre_titles, genre_values)
    update_multi_select(settings, "hiddenThemes", theme_titles, theme_values)

    with path.open("w") as f:
        json.dump(settings, f, indent="\t")
        f.write("\n")


def main():
    genres = fetch_genres("https://comix.to/api/v2/terms?type=genre")
    themes = fetch_genres("https://comix.to/api/v2/terms?type=theme")
    formats = fetch_genres("https://comix.to/api/v2/terms?type=format")

    all_genres = []
    all_genres += genres
    all_genres += themes
    all_genres += formats
    filters_path = Path(__file__).resolve().parent.parent / "res" / "filters.json"
    update_filters_file(filters_path, all_genres)

    settings_path = Path(__file__).resolve().parent.parent / "res" / "settings.json"
    update_settings_file(settings_path, genres, themes)


if __name__ == "__main__":
    main()
