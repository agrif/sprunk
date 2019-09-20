import os.path

import strictyaml

__all__ = [
    'load_definitions',
]

DEF_KEYS = [
    'name',
    'prefix',
    'include',
    'id',
    'solo',
    'to-ad',
    'to-news',
    'time-morning',
    'time-evening',
    'general',
    'intro',
    'ad',
    'news',
    'music',
]

SPECIAL_KEYS = ['name', 'prefix', 'include', 'intro', 'music']

MUSIC_KEYS = [
    'path',
    'title',
    'artist',
    'album',
    'pre',
    'post',
]

MUSIC_OPTIONAL_KEYS = [
    'album',
]

def load_definitions(files, extension):
    whole = {'music': [], 'name': 'Sprunk', 'intro': []}
    for k in DEF_KEYS:
        if k in SPECIAL_KEYS:
            continue
        whole[k] = list()

    def locate_file(base, fname, ext=None):
        if ext is None:
            ext = '.' + extension
        return os.path.abspath(os.path.join(base, fname) + ext)

    def locate_files(base, data, key):
        l = data.get(key, [])
        for n in l:
            path = locate_file(base, n)
            if not path in whole[key]:
                whole[key].append(path)

    def parse_timestamp(s):
        a, b = s.split(':', 1)
        return int(a, base=10) * 60 + float(b)

    def handle_file(fname):
        with open(fname) as f:
            data = strictyaml.load(f.read()).data
        base = os.path.split(fname)[0]
        prefix = data.get('prefix')
        if prefix:
            base = os.path.join(base, prefix)

        for sub_fname in data.get('include', []):
            full_fname = locate_file(base, sub_fname, ext='')
            handle_file(full_fname)

        if 'name' in data:
            whole['name'] = data['name']

        for k in data:
            if k not in DEF_KEYS:
                raise RuntimeError('unknown key {} in file {}'.format(k, fname))
        for k in DEF_KEYS:
            if k in SPECIAL_KEYS:
                continue
            locate_files(base, data, k)
        for m in data.get('music', []):
            for k in m:
                if k not in MUSIC_KEYS:
                    raise RuntimeError('unknown key {} in file {}'.format(k, fname))
            for k in MUSIC_KEYS:
                if not k in m and k not in MUSIC_OPTIONAL_KEYS:
                    raise RuntimeError('missing key {} in file {}'.format(k, fname))
            m['path'] = locate_file(base, m['path'])
            for k in MUSIC_OPTIONAL_KEYS:
                if not k in m:
                    m[k] = None
            m['pre'] = parse_timestamp(m['pre'])
            m['post'] = parse_timestamp(m['post'])
            if not [n for n in whole['music'] if n['path'] == m['path']]:
                whole['music'].append(m)
        for i in data.get('intro', []):
            for k in i:
                if k not in MUSIC_KEYS:
                    raise RuntimeError('unknown key {} in file {}'.format(k, fname))
            if 'path' not in i:
                raise RuntimeError('missing key {} in file {}'.format('path', fname))

            i['path'] = locate_file(base, i['path'])
            if not [n for n in whole['intro'] if n['path'] == i['path']]:
                whole['intro'].append(i)

    for fname in files:
        handle_file(fname)

    # resolve intros
    def lookup_song(meta):
        found = None
        for m in whole['music']:
            for k in meta:
                if k == 'path':
                    continue
                if meta[k] != m[k]:
                    break
            else:
                # found a match!
                if found is not None:
                    raise RuntimeError('ambiguous intro match for {}'.format(meta))
                found = m
        if found is None:
            raise RuntimeError('no music matches intro for {}'.format(meta))
        return found
    for i in whole['intro']:
        m = lookup_song(i)
        if not 'intro' in m:
            m['intro'] = list()
        m['intro'].append(i['path'])

    return whole

def lint(defs):
    def check_all(fs):
        for f in fs:
            check(f)

    def check(f):
        if not (os.path.exists(f) and os.path.isfile(f)):
            print('NOT FOUND: {}'.format(f))
            return 1

    for k in DEF_KEYS:
        if k in SPECIAL_KEYS:
            continue
        check_all(defs[k])

    for m in defs['music']:
        check(m['path'])
        for i in m.get('intro', []):
            check(i)
    for i in defs['intro']:
        check(i['path'])

    print('ok!')

    return 0

        
