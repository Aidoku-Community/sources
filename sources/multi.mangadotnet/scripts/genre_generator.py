import json

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
    mangadot_session.headers.update({
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:159.0) Gecko/20100101 Firefox/159.0',
    })
    cookies: dict[str, str] = {
        'cf_clearance': '',
        'ory_kratos_session': ''
    }
    mangadot_session.cookies.update(cookies)

    response = mangadot_session.get(f'{BASE_URL}/api/search', params={
        'facets': 1,
        'limit': 1,
        'lang': 'all'
    })
    response.raise_for_status()
    search_json = response.json()

    languages = [l['key'] for l in search_json['facets']['languages']]
    genres = [g['key'] for g in search_json['facets']['genres']]

    with open('../res/source.json', 'rt') as f:
        source_json = json.load(f)
        source_json['info']['languages'] = sorted(languages)

    with open('../res/source.json', 'wt') as f:
        json.dump(source_json, f, ensure_ascii=False, indent='\t')
        f.write('\n')
        f.flush()

    with open('../res/filters.json', 'rt') as f:
        filters_json = json.load(f)

        for obj in filters_json:
            if obj['id'] == 'genre':
                obj['options'] = sorted(genres)
                break

    with open('../res/filters.json', 'wt') as f:
        json.dump(filters_json, f, ensure_ascii=False, indent='\t')
        f.write('\n')
        f.flush()
