import click
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
def radio(output, definitions, extensions, meta_url, buffer_size):
    if extensions:
        extensions = extensions.split(',')
    else:
        extensions = None
    r = sprunk.Radio(definitions, extensions, meta_url)
    sched = sprunk.Scheduler(output.samplerate, output.channels)
    r.go(sched)
    sched.run(output, buffer_size)

if __name__ == '__main__':
    cli()
