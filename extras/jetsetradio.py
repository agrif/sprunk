#!/usr/bin/env python3
import ast
import os
import os.path
import re
import collections
import urllib.parse
import tempfile
import subprocess

import requests
import click
import strictyaml

base = 'https://jetsetradio.live/'

listnames = [
    'classic',
    'future',
    'ggs',
    'poisonjam',
    'noisetanks',
    'loveshockers',
    'rapid99',
    'immortals',
    'doomriders',
    'goldenrhinos',
    'bumps',
    'summer',
    'christmas',
]

def do_list(name, output, pretend, force):
    if not pretend:
        os.makedirs(output, exist_ok=True)
    listbase = urllib.parse.urljoin(base, 'audioplayer/stations/{}/'.format(name))
    listindex = urllib.parse.urljoin(listbase, '~list.js')

    matcher = re.compile(r'^{0}Array\[{0}Array.length\]\s*=\s*([^;]+);\s*$'.format(re.escape(name)), re.I)

    stub = collections.OrderedDict()
    stub['prefix'] = '../{}'.format(name)
    stub['music'] = []
    
    with requests.get(listindex) as f:
        f.raise_for_status()
        if not f.encoding:
            f.encoding = f.apparent_encoding
        for line in f.iter_lines():
            if not line:
                continue
            line = line.decode(f.encoding)
            m = matcher.match(line)
            if not m:
                raise RuntimeError('line failed to match: {!r}'.format(line))
            fname = ast.literal_eval(m.group(1))
            furl = urllib.parse.urljoin(listbase, urllib.parse.quote(fname + '.mp3'))
            fullfname = os.path.join(output, fname + '.ogg')
            print(fullfname)

            if ' - ' in fname:
                artist, title = fname.split(' - ', 2)
                meta = collections.OrderedDict()
                meta['path'] = fname
                meta['title'] = title.strip()
                meta['artist'] = artist.strip()
                meta['pre'] = '0:00'
                meta['post'] = '0:00'
                stub['music'].append(meta)
            
            if not pretend and (force or not os.path.exists(fullfname)):
                with requests.get(furl, stream=True) as r:
                    r.raise_for_status()
                    with tempfile.TemporaryDirectory() as tmpdir:
                        dest = os.path.join(tmpdir, 'input')
                        with open(dest, 'wb') as w:
                            for chunk in r.iter_content(chunk_size=8192):
                                if chunk:
                                    w.write(chunk)
                        # now, convert dest to fname
                        subprocess.check_output(['ffmpeg', '-i', dest, '-c:a', 'libvorbis', '-vn', '-q:a', '4', fullfname], stderr=subprocess.PIPE)

    if stub['music']:
        y = strictyaml.as_document(stub)
        with open(os.path.join(output, 'stub.yaml'), 'w') as f:
            f.write(y.as_yaml())

@click.command()
@click.option('-l', '--lists', metavar='LISTGLOB', default='classic,future,bumps')
@click.option('-p', '--pretend', is_flag=True)
@click.option('-f', '--force', is_flag=True)
@click.argument('output')
def main(lists, output, pretend, force):
    if lists == '*':
        lists = listnames
    else:
        lists = [l.strip().lower() for l in lists.split(',')]
        for l in lists:
            if not l in listnames:
                raise RuntimeError('unknown list name: {}'.format(l))

    for l in lists:
        out = os.path.join(output, l)
        do_list(l, out, pretend, force)

if __name__ == '__main__':
    main()
