import sys
import os
import subprocess

import click
import strictyaml
import sprunk

@click.group()
def cli():
    pass

@cli.command()
@click.argument('DEFINITIONS', nargs=-1)
@click.option('-e', '--extensions', default=None)
def lint(definitions, extensions):
    if extensions:
        extensions = extensions.split(',')
    else:
        extensions = None
    defs = sprunk.load_definitions(definitions, extensions)
    return sprunk.definitions.lint(defs)

@cli.command()
@click.option('-o', '--output', type=sprunk.open_sink, default=sprunk.open_sink)
@click.argument('DEFINITIONS', nargs=-1)
@click.option('-e', '--extensions', default=None)
@click.option('-m', '--meta-url')
@click.option('-s', '--buffer-size', default=0.5, type=float)
def play(output, definitions, extensions, meta_url, buffer_size):
    if extensions:
        extensions = extensions.split(',')
    else:
        extensions = None
    while True:
        # do this now: an error here is unrecoverable
        _ = sprunk.load_definitions(definitions, extensions=extensions)
        try:
            r = sprunk.Radio(definitions, extensions, meta_url)
            sched = sprunk.Scheduler(output.samplerate, output.channels)
            r.go(sched)
            sched.run(output, buffer_size)
        except Exception as e:
            print(e)

def read_radio_choices(path):
    with open(path) as f:
        data = strictyaml.load(f.read()).data
    data = data.get('stations', {})
    return data.keys()
    
def read_radio_definitions(path, mount):
    if mount.startswith('/'):
        mount = mount[1:]

    base = os.path.split(os.path.abspath(path))[0]

    with open(path) as f:
        data = strictyaml.load(f.read()).data

    defs = data.get('stations', {}).get(mount, {'files': []})
    for k in data:
        if k == 'stations':
            continue
        defs[k] = data[k]

    output = None
    meta_url = None
    if 'icecast' in defs:
        ic = defs['icecast']
        host = ic.get('host', 'localhost:8000')
        schema = ic.get('schema', 'http')
        user = ic.get('user', 'source')
        password = ic.get('password', 'hackme')
        tmpl = dict(host=host, schema=schema, user=user, password=password)

        output = 'ffmpegre:-acodec libmp3lame -ab 300k -content_type audio/mpeg -f mp3 icecast://source:{password}@{host}/{{mount}}'.format(**tmpl)
        meta_url = '{schema}://source:{password}@{host}/admin/metadata?mount=%2F{{mount}}&mode=updinfo'.format(**tmpl)

    if 'output' in defs:
        output = defs['output']

    files = []
    for fname in defs['files']:
        fname = os.path.abspath(os.path.join(base, os.path.expanduser(fname)))
        files.append(fname)

    return dict(
        mount=mount,
        key='sprunk-' + mount,
        files=files,
        output=output.format(mount=mount) if output else None,
        meta_url=meta_url.format(mount=mount) if meta_url else None,
        extensions=defs.get('extensions', None),
        buffersize=defs.get('buffersize', None),
    )

@cli.command()
@click.option('-o', '--output', type=str, default=None) # yes really
@click.argument('DEFINITION')
@click.argument('MOUNT')
@click.option('-d', '--detach', is_flag=True)
def start(output, definition, mount, detach):
    start_inner(output, definition, mount, detach)

def start_inner(output, definition, mount, detach):
    defs = read_radio_definitions(definition, mount)
    if output is None:
        output = defs['output']

    args = ['play'] + defs['files']
    if output:
        args += ['-o', output]
    if defs['extensions']:
        args += ['-e', ','.join(defs['extensions'])]
    if defs['meta_url']:
        args += ['-m', defs['meta_url']]
    if defs['buffersize']:
        args += ['-s', str(defs['buffersize'])]

    # construct our final command line
    args = [sys.executable, sys.argv[0]] + args

    # does this screen session exist?
    if subprocess.call(['screen', '-S', defs['key'], '-X', 'select', '.'], stdout=subprocess.DEVNULL) == 0:
        # session exists, just attach if needed
        if not detach:
            os.execlp('screen', '-r', defs['key'])
        return

    # we need to create a screen
    if detach:
        args = ['screen', '-dmS', defs['key'], '--'] + args
    else:
        args = ['screen', '-S', defs['key'], '--'] + args

    # modify env to include us in the path
    env = os.environ.copy()
    ourpath = os.path.split(os.path.split(__file__)[0])[0]
    if 'PYTHONPATH' in env:
        ourpath += ':' + env['PYTHONPATH']
    env['PYTHONPATH'] = ourpath

    # go!
    if detach:
        subprocess.check_call(args, env=env)
    else:
        os.execvpe(args[0], args, env)

@cli.command()
@click.option('-o', '--output', type=str, default=None) # yes really
@click.argument('DEFINITION')
def start_all(output, definition):
    stations = read_radio_choices(definition)
    for name in stations:
        start_inner(output, definition, name, True)

@cli.command()
@click.argument('DEFINITION')
@click.argument('MOUNT')
def stop(definition, mount):
    stop_inner(definition, mount)

def stop_inner(definition, mount):
    defs = read_radio_definitions(definition, mount)

    # does this screen session exist?
    if subprocess.call(['screen', '-S', defs['key'], '-X', 'select', '.'], stdout=subprocess.DEVNULL) == 0:
        # session exists, remove it
        subprocess.check_call(['screen', '-S', defs['key'], '-p', '0', '-X', 'stuff', '\x03'])

@cli.command()
@click.argument('DEFINITION')
def stop_all(definition):
    stations = read_radio_choices(definition)
    for name in stations:
        stop_inner(definition, name)

if __name__ == '__main__':
    cli()
