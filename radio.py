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

def input_argument(*args, **kwargs):
    def open_file(ctx, param, value):
        return sprunk.FileSource(value)
    return click.argument(*args, **kwargs, callback=open_file)

@click.group()
def cli():
    pass

@cli.command()
@output_option
@input_argument('SRC')
def play(output, src):
    run(src, output)

@sprunk.coroutine
def over_coroutine(sched, song, over):
    padding = 1
    start_over = 3
    oversched = sched.subscheduler()
    songsched = sched.subscheduler()

    songsched.add_source(0, song)
    over_length = oversched.add_source(start_over, over)
    yield start_over - padding
    full_volume = songsched.get_volume(0)
    songsched.set_volume(0, 0.5, duration=padding)
    yield padding + over_length
    songsched.set_volume(0, full_volume, duration=padding)

@cli.command()
@output_option
@input_argument('SONG')
@input_argument('OVER')
def over(output, song, over):
    sched = sprunk.Scheduler(output.samplerate, output.channels)
    over_coroutine(sched, song, over)
    run(sched, output)

if __name__ == '__main__':
    cli()
