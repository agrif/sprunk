import sys
import random
import urllib.parse
import collections
import traceback
import datetime

import requests
import sprunk.scheduler
import sprunk.sources
import sprunk.definitions

__all__ = [
    'Radio',
]

class Radio:
    def __init__(self, definitions, extensions=None, meta_url=None, loudness=-14.0):
        self.definition_files = definitions
        self.extensions = extensions
        self.defs = None
        self.meta_url = meta_url
        self.padding = 0.5
        self.over_volume = 0.5
        self.no_repeat_percent = 0.5
        self.intro_chance = 0.5
        self.loudness = loudness
        self.random_lasts = collections.defaultdict(lambda: collections.deque())

        self.reload()

    def reload(self):
        if self.defs is None:
            self.defs = sprunk.definitions.load_definitions(self.definition_files, self.extensions)
        else:
            # we already have definitions, don't fail if this fails
            try:
                self.defs = sprunk.definitions.load_definitions(self.definition_files, self.extensions)
            except Exception as e:
                print('Error while reloading definitions:', file=sys.stderr)
                traceback.print_exc(file=sys.stderr)

    def set_metadata(self, meta):
        parts = [self.defs.get('name'), meta.get('artist'), meta.get('title')]
        parts = [p for p in parts if p is not None]
        song = ' - '.join(parts)
        if not song:
            song = 'NO INFORMATION'

        print('###', song)

        if self.meta_url:
            parts = list(urllib.parse.urlparse(self.meta_url))
            query = dict(urllib.parse.parse_qsl(parts[4]))
            query['song'] = song
            parts[4] = urllib.parse.urlencode(query)
            our_meta_url = urllib.parse.urlunparse(parts)
            try:
                requests.get(our_meta_url)
            except Exception:
                print('### (failed to set metadata via url)')
                pass

    def choice(self, key):
        def key_of(m):
            if isinstance(m, dict) and 'path' in m:
                return m['path']
            return m

        choices = self.defs.get(key, [])
        if len(choices) == 0:
            return None

        no_repeat = int(len(choices) * self.no_repeat_percent)
        used = self.random_lasts[key]
        while len(used) > no_repeat:
            used.pop()

        choices_left = [m for m in choices if not key_of(m) in used]
        while not choices_left:
            k = used.pop()
            choices_left = [m for m in choices if key_of(m) == k]
        m = random.choice(choices_left)
        used.appendleft(key_of(m))
        return m

    def go_soft(self, soft_time, mainpath, overpath, meta, pre=0, post=None, force=False):
        if mainpath is None:
            return soft_time
        main = sprunk.sources.FileSource(mainpath).reformat_like(self.music).normalize(self.loudness)
        if overpath:
            over = sprunk.sources.FileSource(overpath).reformat_like(self.talk).normalize(self.loudness)
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
        self.music.add_callback(main_start, lambda _: self.set_metadata(meta))
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
        return self.go_break(sched, soft_time, 'ad', 'to-ad', 'Advertisement')

    def go_news(self, sched, soft_time):
        return self.go_break(sched, soft_time, 'news', 'to-news', 'News')

    def go_break(self, sched, soft_time, main_set, over_set, title):
        ad = self.choice(main_set)
        p = self.choice(over_set)

        admeta = {
            'title': title,
        }

        if ad is not None:
            soft_time = yield from self.go_soft(soft_time, ad, p, admeta, force=True)
        return soft_time

    def go_id(self, sched, soft_time):
        idpath = self.choice('id')

        idmeta = {
            'title': 'Identification',
        }

        return self.go_soft(soft_time, idpath, None, idmeta)

    def go_solo(self, sched, soft_time):
        solo = self.choice('solo')

        solometa = {
            'title': 'Monologue',
        }

        return self.go_soft(soft_time, solo, None, solometa)

    def go_music(self, sched, soft_time):
        # reload before each song!
        self.reload()

        # select a song randomly
        m = self.choice('music')
        if random.random() < self.intro_chance:
            # we want an intro, what are our choices...
            p_choices = [self.choice('general')]

            # time-based
            hour = datetime.datetime.now().hour
            if hour >= 4 and hour < 12:
                p_choices.append(self.choice('time-morning'))
            if hour >= 17 and hour < 24:
                p_choices.append(self.choice('time-evening'))

            # song-based
            if m.get('intro', []):
                p_choices.append(random.choice(m.get('intro')))

            # filter and choose one randomly
            p_choices = [p for p in p_choices if p is not None]
            if p_choices:
                p = random.choice(p_choices)
            else:
                p = None
        else:
            p = None

        return self.go_soft(soft_time, m['path'], p, m, pre=m['pre'], post=m['post'],)

    @sprunk.scheduler.coroutine_method
    def go(self, sched):
        self.music = sched.subscheduler()
        self.talk = sched.subscheduler()

        soft_time = 0
        while True:
            for go_break in [self.go_ad, self.go_news]:
                for _ in range(12):
                    soft_time = yield from self.go_music(sched, soft_time)
                    yield self.padding
                soft_time = yield from go_break(sched, soft_time)
                yield self.padding
                soft_time = yield from self.go_id(sched, soft_time)
                yield self.padding
                soft_time = yield from self.go_solo(sched, soft_time)
                yield self.padding
