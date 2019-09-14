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

@cli.command()
@output_option
@input_argument('SONG')
@input_argument('OVER')
def over(output, song, over):
    sched = sprunk.Scheduler(output.samplerate, output.channels)
    sched.set_volume(0, 0.0)
    sched.schedule_source(0, song)
    def more(s):
        s.schedule_source(0, over)
        s.set_volume(0, 1.0, duration=10)
    sched.schedule_callback(1, more)
    run(sched, output)

if __name__ == '__main__':
    cli()
