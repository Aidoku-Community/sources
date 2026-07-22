import json
import os

import requests

BASE_URL = 'https://mangadot.net'


def resolve_ptr_table_json(table: list, index: int):
    value = table[index]

    if isinstance(value, dict):
        result = {}
        k: str
        v: int
        for k, v in value.items():
            key_index = int(k.lstrip('_'))
            key = table[key_index]
            value_index = v
            resolved_value = resolve_ptr_table_json(table, value_index) if value_index >= 0 else None
            result[key] = resolved_value
        return result
    elif isinstance(value, list):
        result = []
        for v in value:
            if v >= 0:
                result.append(resolve_ptr_table_json(table, v))
            else:
                result.append(None)
        return result
    else:
        return value


with requests.Session() as mangadot_session:
    response = mangadot_session.get(f'{BASE_URL}/search.data', params={
        '_routes': 'pages/SearchPage',
        'adult': 'both'
    })
    response.raise_for_status()
    json_ptr = response.json()
    search_json = resolve_ptr_table_json(json_ptr, 0)

    genres = search_json['pages/SearchPage']['data']['allGenres']

    with open('../res/filters.json', 'rt+') as f:
        filters_json = json.load(f)

        for obj in filters_json:
            if obj['id'] == 'genre':
                obj['options'] = genres
                break

        f.seek(0, os.SEEK_SET)
        json.dump(filters_json, f, ensure_ascii=False, indent='\t')
