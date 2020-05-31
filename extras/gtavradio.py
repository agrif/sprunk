#!/usr/bin/env/python3

import subprocess
import glob
import os
import os.path

import click

@click.command()
@click.option('-f', '--format', default=None)
@click.option('-d', '--delete', is_flag=True)
@click.argument('source', type=click.Path(exists=True, file_okay=False))
@click.argument('output', type=click.Path(exists=True, file_okay=False))
def main(source, output, format, delete):
    name = None
    head = source
    while not name:
        head, name = os.path.split(head)

    def process_wav(path):
        base = os.path.splitext(path)[0]
        outpath = os.path.splitext(os.path.relpath(path, source))[0]
        print('adding', outpath, '...')
        if format is not None:
            subprocess.check_call(['ffmpeg', '-y', '-i', path, base + '.' + format], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            if delete:
                os.remove(path)
        return outpath

    processed_files = []

    def process_glob(*args):
        for n in sorted(glob.glob(os.path.join(source, *args))):
            if n not in processed_files:
                yield process_wav(n)
                processed_files.append(n)


    with open(os.path.join(output, name + '.yaml'), 'w') as f:
        f.write('name: {}\n\n'.format(name))
        f.write('include:\n')
        f.write('  - {}_id.yaml\n'.format(name))
        f.write('  - {}_host.yaml\n'.format(name))
        f.write('  - {}_music.yaml\n'.format(name))
        f.write('  - RADIO_ADVERTS.yaml\n')
        f.write('  - RADIO_NEWS.yaml\n')

    with open(os.path.join(output, name + '_id.yaml'), 'w') as f:
        f.write('prefix: ../{}\n\n'.format(name))
        f.write('id:\n')
        for n in process_glob('id_[0-9][0-9]', '*.wav'):
            f.write('  - {}\n'.format(n))

    with open(os.path.join(output, name + '_host.yaml'), 'w') as f:
        f.write('prefix: ../{}\n\n'.format(name))
        f.write('solo:\n')
        for n in process_glob('mono_solo_[0-9][0-9]', '*.wav'):
            f.write('  - {}\n'.format(n))
        f.write('\n')
        f.write('general:\n')
        for n in process_glob('general', '*.wav'):
            f.write('  - {}\n'.format(n))
        f.write('\n')
        f.write('to-ad:\n')
        for n in process_glob('to', 'TO_AD_*.wav'):
            f.write('  - {}\n'.format(n))
        f.write('\n')
        f.write('to-news:\n')
        for n in process_glob('to', 'TO_NEWS_*.wav'):
            f.write('  - {}\n'.format(n))
        f.write('\n')
        f.write('time-evening:\n')
        for n in process_glob('time', 'EVENING_*.wav'):
            f.write('  - {}\n'.format(n))
        f.write('\n')
        f.write('time-morning:\n')
        for n in process_glob('time', 'MORNING_*.wav'):
            f.write('  - {}\n'.format(n))
        f.write('\n')

        f.write('intro:\n')
        for n in process_glob('intro', '*.wav'):
            mname = n.split('/', 1)[1]
            if '_' in mname:
                mname = mname.rsplit('_', 1)[0]
            f.write('  - path: {}\n'.format(n))
            f.write('    title: {}\n'.format(mname))
            f.write('    artist: ??\n')
            f.write('\n')

    with open(os.path.join(output, name + '_music.yaml'), 'w') as f:
        f.write('prefix: ../{}\n\n'.format(name))
        f.write('music:\n')
        for n in process_glob('*', '*.wav'):
            mname = n.split('/', 1)[1]
            f.write('  - path: {}\n'.format(n))
            f.write('    title: {}\n'.format(mname))
            f.write('    artist: ??\n')
            f.write('    album: optional\n')
            f.write('    pre: 0:00\n')
            f.write('    post: 0:00\n')
            f.write('\n')

if __name__ == '__main__':
    main()
