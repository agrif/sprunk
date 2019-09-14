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
        self.random_lasts = {}

    def choice(self, key):
        # FIXME better random
        if len(self.defs.get(key, [])) == 0:
            return None
        i = self.random_lasts.get(key, None)
        if i is None:
            i = 0
            random.shuffle(self.defs.get(key))
        if i >= len(self.defs.get(key)):
            i = 0
        self.random_lasts[key] = i + 1
        return self.defs.get(key)[i]

    def go_soft(self, soft_time, mainpath, overpath, pre=0, post=None, force=False):
        if mainpath is None:
            return soft_time
        main = sprunk.FileSource(mainpath)
        if overpath:
            over = sprunk.FileSource(overpath)
        else:
            over = None

        # find the over start time, relative to music start time
        if over:
            over_start_time = pre - (over.size / over.samplerate + 2 * self.padding)
            skip_over = False
        else:
            over_start_time = 0
            skip_over = True

        # find out when the music starts
        if soft_time >= -over_start_time:
            # seamless music, nice
            main_start = soft_time
        elif force:
            # there must be a break to fit this in
            main_start = -over_start_time
        else:
            # we need a break but we can't force it
            main_start = soft_time
            skip_over = True
        over_start_time += main_start

        # ok, now we can do this
        md = self.music.add_source(main_start, main)
        if post is None:
            post = md
        if skip_over:
            yield main_start + post
        else:
            self.music.set_volume(over_start_time, self.over_volume, duration=self.padding)
            od = self.talk.add_source(over_start_time + self.padding, over)
            yield over_start_time + self.padding + od
            self.music.set_volume(0, 1.0, duration=self.padding)
            yield main_start + post - (over_start_time + self.padding + od)
        return md - post

    def go_ad(self, sched, soft_time):
        idpath = self.choice('id')
        ad = self.choice('ad')
        p = self.choice('to-ad')

        idsrc = sprunk.FileSource(idpath)

        print('### AD')

        if ad is not None:
            soft_time = yield from self.go_soft(soft_time, ad, p, force=True)
        duration = self.music.add_source(soft_time, idsrc)
        yield soft_time + duration
        return 0

    def go_solo(self, sched, soft_time):
        solo = self.choice('solo')

        print('### SOLO')

        return self.go_soft(soft_time, solo, None)

    def go_music(self, sched, soft_time):
        # select a song randomly
        m = self.choice('music')
        p = self.choice('general')

        print('###', m['title'])
        print('   ', 'by', m['artist'])

        return self.go_soft(soft_time, m['path'], p, pre=m['pre'], post=m['post'],)

    @sprunk.coroutine_method
    def go(self, sched):
        self.music = sched.subscheduler()
        self.talk = sched.subscheduler()

        soft_time = 0
        while True:
            for _ in range(3):
                soft_time = yield from self.go_music(sched, soft_time)
                yield self.padding
            soft_time = yield from self.go_ad(sched, soft_time)
            yield self.padding
            soft_time = yield from self.go_solo(sched, soft_time)
            yield self.padding

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
