import sys
import json
import urllib.request


def fetch_genres(api_base_url):
    # ensure no trailing slash
    api_base_url = api_base_url.rstrip("/")
    url = f"{api_base_url}/api/genres"
    req = urllib.request.Request(url, headers={"User-Agent": "Aidoku"})
    with urllib.request.urlopen(req) as response:
        result = response.read().decode("utf-8")
    return json.loads(result)


def update_filters(filters_path, api_base_url):
    # open filters.json
    with open(filters_path, "r") as f:
        filters = json.load(f)

    # fetch genres from api
    genres = fetch_genres(api_base_url)
    genre_names = [g["name"].strip() for g in genres]
    genre_ids = [str(g["id"]) for g in genres]

    # update genre filter
    for filter in filters:
        if filter.get("isGenre"):
            filter["options"] = genre_names
            filter["ids"] = genre_ids

    # write back to the file
    with open(filters_path, "w") as f:
        json.dump(filters, f, indent="\t", ensure_ascii=False)
        f.write("\n")

    print("Genres updated successfully.")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python update_filters.py /path/to/filters.json api_url")
        sys.exit(1)
    update_filters(sys.argv[1], sys.argv[2])
