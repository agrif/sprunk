import functools

import numpy

import sprunk.sources

__all__ = [
    'Scheduler',
    'coroutine',
    'coroutine_method',
]

class Scheduler(sprunk.sources.Source):
    def __init__(self, samplerate, channels):
        super().__init__(samplerate, channels)
        self.sources = [] # [frame, src]
        self.callbacks = [] # [frame, cb]
        self.active = [] # src

        # this is modified when calling callbacks, to get frame-perfect
        # schedules
        self.frame_offset = 0

        # we can only schedule one volume ramp at a time
        # wait until one is finished to set the next
        # [startframe, endframe, m, startvol, endvol]
        self.volume_ramp = [0, 1, 0, 1.0, 1.0]

    def subscheduler(self):
        s = Scheduler(self.samplerate, self.channels)
        self.active.append(s)
        if self.buffer is not None:
            s.allocate(len(self.buffer))
        return s

    def add_source(self, start, src):
        src = src.reformat_like(self)
        startframe = int(self.samplerate * start)
        startframe += self.frame_offset
        if startframe < 0:
            startframe = 0
        self.sources.append([startframe, src])
        if src.size:
            return src.size / self.samplerate

    def add_callback(self, start, src):
        startframe = int(self.samplerate * start)
        startframe += self.frame_offset
        if startframe < 0:
            startframe = 0
        self.callbacks.append([startframe, src])

    def add_agent(self, start, agent):
        agent.scheduler = self
        self.add_callback(start, lambda _: agent.run())

    def get_volume(self, time):
        timeframe = int(self.samplerate * time) + self.frame_offset
        start, end, m, vol1, vol2 = self.volume_ramp
        if timeframe < start:
            return vol1
        elif timeframe >= end:
            return vol2
        else:
            return (timeframe - start) * m + vol1

    def set_volume(self, start, volume, duration=0.005):
        startframe = int(self.samplerate * start) + self.frame_offset
        endframe = int(self.samplerate * (start + duration)) + self.frame_offset
        if startframe < 0:
            startframe = 0
        if endframe < 0:
            endframe = 0
        if startframe == endframe:
            endframe += 1

        # figure out existing volume
        oldvolume = self.get_volume(start)

        m = (volume - oldvolume) / (endframe - startframe)
        self.volume_ramp = [startframe, endframe, m, oldvolume, volume]

    def allocate(self, frames):
        for src in self.active:
            src.allocate(frames)
        self.bufferframes = numpy.arange(0, frames)
        self.buffervolume = numpy.zeros(frames)
        return super().allocate(frames)

    def _process_schedule(self, scheduled, window):
        to_remove = []
        for schedule in scheduled:
            start, x = schedule
            if start < window:
                yield start, x
                to_remove.append(schedule)
            else:
                schedule[0] -= window
        for t in to_remove:
            scheduled.remove(t)

    def fill(self, max=None):
        if max is None:
            max = len(self.buffer)

        # a helper function to ensure a buffer is fully filled
        # returns False if src is over, True if still has data
        def force_fill(buf, src):
            filled = src.fill(max=len(buf))
            amount = len(filled)
            buf[:amount] += filled
            if amount == 0:
                return False
            if amount < len(buf):
                return force_fill(buf[amount:], src)
            return True

        # zero our buffer
        self.buffer[:max] = 0

        # run any scheduled callbacks, which might add stuff to play
        # this block
        for start, cb in self._process_schedule(self.callbacks, max):
            try:
                self.frame_offset = start
                cb(self)
            finally:
                self.frame_offset = 0

        # render all our active sources
        to_remove = []
        scheduler_states = []
        for src in self.active:
            alive = force_fill(self.buffer[:max], src)
            if isinstance(src, Scheduler):
                scheduler_states.append(alive)
            else:
                if not alive:
                    to_remove.append(src)
        for src in to_remove:
            self.active.remove(src)

        # check if there is nothing left to do
        if not ([a for a in self.active if not isinstance(a, Scheduler)] or self.sources or self.callbacks or any(scheduler_states) or to_remove):
            return self.buffer[0:0]

        # figure out which scheduled sources are now active,
        # and partially render them
        for start, src in self._process_schedule(self.sources, max):
            src.allocate(len(self.buffer))
            if force_fill(self.buffer[start:max], src):
                self.active.append(src)

        # apply volume ramp
        startframe, endframe, m, volstart, volend = self.volume_ramp
        self.buffervolume[:max] = (self.bufferframes[:max] - startframe) * m + volstart
        if startframe > 0:
            i = startframe if startframe < max else max
            self.buffervolume[:i] = volstart
        if endframe <= max:
            i = endframe if endframe > 0 else 0
            self.buffervolume[i:max] = volend
        self.buffer[:max] *= self.buffervolume[:max, numpy.newaxis]
        self.volume_ramp[0] -= max
        self.volume_ramp[1] -= max

        return self.buffer[:max]

def coroutine(f):
    @functools.wraps(f)
    def inner(scheduler, *args, **kwargs):
        runner = f(scheduler, *args, **kwargs)
        try:
            runner = iter(runner)
        except TypeError:
            runner = iter([])
        def callback(s):
            try:
                delay = next(runner)
            except StopIteration:
                return
            s.add_callback(delay, callback)
        scheduler.add_callback(0, callback)
        # FIXME allow yield from to be used to block on subcalls
    return inner

def coroutine_method(f):
    @functools.wraps(f)
    def inner(self, scheduler, *args, **kwargs):
        runner = f(self, scheduler, *args, **kwargs)
        try:
            runner = iter(runner)
        except TypeError:
            runner = iter([])
        def callback(s):
            try:
                delay = next(runner)
            except StopIteration:
                return
            s.add_callback(delay, callback)
        scheduler.add_callback(0, callback)
        # FIXME allow yield from to be used to block on subcalls
    return inner
