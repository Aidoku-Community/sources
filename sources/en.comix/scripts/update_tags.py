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


def main():
    genres = fetch_genres("https://comix.to/api/v2/terms?type=genre")
    genres += fetch_genres("https://comix.to/api/v2/terms?type=theme")
    genres += fetch_genres("https://comix.to/api/v2/terms?type=format")
    filters_path = Path(__file__).resolve().parent.parent / "res" / "filters.json"
    update_filters_file(filters_path, genres)


if __name__ == "__main__":
    main()
