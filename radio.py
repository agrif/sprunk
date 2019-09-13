#!/usr/bin/env python3

import click
import sprunk

def run(src, sink):
    src = src.reformat_like(sink)
    src.allocate(int(src.samplerate * 0.1))
    filled = src.buffer
    while len(filled) > 0:
        filled = src.fill()
        sink.write(filled)

def output_option(f):
    def open_sink(ctx, param, value):
        if value:
            return sprunk.FileSink(value, 48000, 2)
        return sprunk.PyAudioSink(48000, 2)
    return click.option('-o', '--output', type=str, callback=open_sink)(f)

@click.group()
def cli():
    pass

@cli.command()
@output_option
@click.argument('PATH')
def play(output, path):
    src = sprunk.FileSource(path)
    run(src, output)

if __name__ == '__main__':
    cli()
