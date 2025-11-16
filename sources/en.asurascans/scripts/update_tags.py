import dataclasses
import json
from pathlib import Path
from urllib.request import urlopen, Request


class EnhancedJSONEncoder(json.JSONEncoder):
    def default(self, o):
        if dataclasses.is_dataclass(o):
            return dataclasses.asdict(o)
        return super().default(o)


def fetch_genres(url: str):
    req = Request(url)
    with urlopen(req) as response:
        data = response.read().decode("utf-8")
    return json.loads(data)["genres"]


def update_filters_file(filters_path: Path, genres):
    with filters_path.open("r") as f:
        filters = json.load(f)

    for filter_obj in filters:
        if filter_obj.get("title") == "Genre":
            filter_obj["options"] = [genre["name"] for genre in genres]
            filter_obj["ids"] = [str(genre["id"]) for genre in genres]

    with filters_path.open("w") as f:
        json.dump(filters, f, indent="\t", cls=EnhancedJSONEncoder)
        f.write("\n")


def main():
    genres = fetch_genres("https://gg.asuracomic.net/api/series/filters")
    filters_path = Path(__file__).resolve().parent.parent / "res" / "filters.json"
    update_filters_file(filters_path, genres)


if __name__ == "__main__":
    main()
