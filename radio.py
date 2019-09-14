#!/usr/bin/env python3

import os.path
import sys
import random

import click
import sprunk

class Radio:
    def __init__(self, defs):
        self.defs = defs
        self.padding = 1
        self.over_volume = 0.5

    def go_soft(self, soft_time, mainpath, overpath, pre=0, post=None):
        main = sprunk.FileSource(mainpath)
        over = sprunk.FileSource(overpath)

        # find the over start time, relative to music start time
        over_start_time = pre - (over.size / over.samplerate + 2 * self.padding)

        # find out when the music starts
        if soft_time >= -over_start_time:
            # seamless music, nice
            main_start = soft_time
        else:
            # there must be a break to fit this in
            main_start = -over_start_time
        over_start_time += main_start

        # ok, now we can do this
        md = self.music.add_source(main_start, main)
        if post is None:
            post = md
        self.music.set_volume(over_start_time, self.over_volume, duration=self.padding)
        od = self.talk.add_source(over_start_time + self.padding, over)
        yield over_start_time + self.padding + od
        self.music.set_volume(0, 1.0, duration=self.padding)
        yield main_start + post - (over_start_time + self.padding + od)
        return md - post

    def go_ad(self, sched, soft_time):
        ad = random.choice(self.defs['id']) # FIXME
        p = random.choice(self.defs['to-ad'])

        return self.go_soft(soft_time, ad, p)
        

    def go_music(self, sched, soft_time):
        # select a song randomly (FIXME: no repeats)
        m = random.choice(self.defs['music'])

        # select a preroll randomly
        p = random.choice(self.defs['time-morning'] + self.defs['time-evening'])

        print(m['title'])
        print('by', m['artist'])

        return self.go_soft(soft_time, m['path'], p, pre=m['pre'], post=m['post'])

    @sprunk.coroutine_method
    def go(self, sched):
        self.music = sched.subscheduler()
        self.talk = sched.subscheduler()

        soft_time = 0
        soft_time = yield from self.go_music(sched, soft_time)
        soft_time = yield from self.go_ad(sched, soft_time)
        soft_time = yield from self.go_music(sched, soft_time)

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

@cli.command()
@click.argument('DEFINITIONS', nargs=-1)
@click.option('-e', '--extension', default='ogg')
def lint(definitions, extension):
    defs = sprunk.load_definitions(definitions, extension)
    return sprunk.definitions.lint(defs)

@cli.command()
@output_option
@click.argument('DEFINITIONS', nargs=-1)
@click.option('-e', '--extension', default='ogg')
def radio(output, definitions, extension):
    defs = sprunk.load_definitions(definitions, extension)
    r = Radio(defs)
    sched = sprunk.Scheduler(output.samplerate, output.channels)
    r.go(sched)
    run(sched, output)

if __name__ == '__main__':
    cli()
