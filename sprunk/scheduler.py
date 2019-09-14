import numpy

import sprunk.sources

__all__ = [
    'Scheduler',
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

    def schedule_source(self, start, src):
        startframe = int(self.samplerate * start)
        startframe += self.frame_offset
        if startframe < 0:
            startframe = 0
        if self.buffer is not None:
            src.allocate(len(self.buffer))
        self.sources.append([startframe, src])

    def schedule_callback(self, start, src):
        startframe = int(self.samplerate * start)
        startframe += self.frame_offset
        if startframe < 0:
            startframe = 0
        self.callbacks.append([startframe, src])

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
        oldstart, oldend, oldm, oldvol1, oldvol2 = self.volume_ramp
        if self.frame_offset < oldstart:
            oldvolume = oldvol1
        elif self.frame_offset >= oldend:
            oldvolume = oldvol2
        else:
            oldvolume = (self.frame_offset - oldstart) * oldm + oldvol1

        m = (volume - oldvolume) / (endframe - startframe)
        self.volume_ramp = [startframe, endframe, m, oldvolume, volume]

    def allocate(self, frames):
        for _, src in self.sources:
            src.allocate(frames)
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

        # check if there is nothing left to do
        if not (self.active or self.sources or self.callbacks):
            return self.buffer[0:0]

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
        for src in self.active:
            if not force_fill(self.buffer[:max], src):
                to_remove.append(src)
        for src in to_remove:
            self.active.remove(src)

        # figure out which scheduled sources are now active,
        # and partially render them
        for start, src in self._process_schedule(self.sources, max):
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
