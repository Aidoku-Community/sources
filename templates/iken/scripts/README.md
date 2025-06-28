## Updating Source Genres

For fetching/updating an iken source's genres, use the following python script:
```sh
python update_genres.py /path/to/filters.json api_url
```

For example, for updating Hive Scans from this directory;
```sh
python update_genres.py ../../../sources/en.hivescans/res/filters.json https://api.hivetoons.org
```
