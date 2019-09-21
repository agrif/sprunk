import concurrent.futures

import numpy
import samplerate
import soundfile
import pyloudnorm

import sprunk.channels

__all__ = [
    'Source',
    'FileSource',
]

class Source:
    def __init__(self, samplerate, channels, size=None):
        self.samplerate = samplerate
        self.channels = channels
        self.size = size
        self.buffer = None

    def allocate(self, frames):
        buf = numpy.zeros((frames, self.channels), dtype=numpy.float32)
        self.buffer = buf
        return buf

    def fill(self, max=None):
        raise NotImplementedError('Source.fill')

    def seek(self, frame):
        raise NotImplementedError('Source.seek')

    def read_all(self):
        if self.size is not None:
            buf = numpy.zeros((self.size, self.channels))
        else:
            buf = numpy.zeros((1024 * 1024, self.channels))
        if self.buffer is None:
            self.allocate(1024 * 1024) # arbitrary!

        i = 0
        while True:
            filled = self.fill()
            l = len(filled)
            if l == 0:
                break
            if i + l > len(buf):
                buf.resize((len(buf), self.channels))
            buf[i:i+l] = filled[:]
            i += l
        return buf[0:i]

    def remix(self, mix):
        return Mix(self, numpy.asarray(mix))
    
    def resample(self, newrate):
        if newrate == self.samplerate:
            return self
        return Resample(self, newrate)
    
    def reformat(self, samplerate=None, channels=None):
        src = self
        if channels and channels < src.channels:
            src = src.remix(sprunk.channels.find_mix(channels, self.channels))
        if samplerate and samplerate != src.samplerate:
            src = src.resample(samplerate)
        if channels and channels > src.channels:
            src = src.remix(sprunk.channels.find_mix(channels, self.channels))
        return src
    
    def reformat_like(self, other):
        return self.reformat(other.samplerate, other.channels)

    def normalize(self, loudness=-14.0):
        return Normalize(self, loudness)

class Mix(Source):
    # mix has shape (new_channels, old_channels)
    def __init__(self, inner, mix):
        assert mix.shape[1] == inner.channels
        super().__init__(inner.samplerate, mix.shape[0], size=inner.size)
        self.inner = inner
        self.mix = mix
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self, max=None):
        filled = self.inner.fill(max=max)
        self.buffer[:] = self.inner.buffer @ self.mix.T
        return self.buffer[:len(filled)]

    def seek(self, frame):
        self.inner.seek(frame)
        
class Resample(Source):
    def __init__(self, inner, newrate):
        newsize = None
        if inner.size:
            newsize = int(numpy.ceil(inner.size * newrate / inner.samplerate))
        super().__init__(newrate, inner.channels, size=newsize)
        self.resampler = samplerate.converters.Resampler(channels=self.channels)
        self.ratio = newrate / inner.samplerate
        self.inner = inner
    
    def allocate(self, frames):
        innerframes = int(frames * self.inner.samplerate / self.samplerate)
        self.inner.allocate(innerframes)
        # make sure our assumption works with libsamplerate's assumption
        assert frames >= int(innerframes * self.ratio)
        return super().allocate(frames)
    
    def fill(self, max=None):
        if max is None:
            max = len(self.buffer)
        filled = self.inner.fill(max=int(numpy.ceil(max * self.inner.samplerate / self.samplerate)))
        if len(filled) > 0:
            proc = self.resampler.process(filled, self.ratio, end_of_input=False)
        else:
            proc = self.resampler.process(filled, self.ratio, end_of_input=True)
        self.buffer[:len(proc)] = proc
        return self.buffer[:len(proc)]

    def seek(self, frame):
        self.inner.seek(frame * self.inner.samplerate / self.samplerate)

class Normalize(Source):
    executor = concurrent.futures.ThreadPoolExecutor()

    def __init__(self, inner, loudness):
        super().__init__(inner.samplerate, inner.channels, size=inner.size)
        self.inner = inner
        self.loudness = loudness
        self.worker = self.executor.submit(self._calculate_loudness)

    def _calculate_loudness(self):
        meter = pyloudnorm.Meter(self.samplerate)
        measured = meter.integrated_loudness(self.inner.read_all()[:, :5])
        self.inner.seek(0)
        return measured

    def _ensure_done(self):
        if self.worker:
            self.measured = self.worker.result()
            self.worker = None

    def allocate(self, frames):
        self._ensure_done()
        self.inner.allocate(frames)
        return super().allocate(frames)

    def fill(self, max=None):
        self._ensure_done()
        filled = self.inner.fill(max=max)
        self.buffer[:] = pyloudnorm.normalize.loudness(self.inner.buffer, self.measured, self.loudness)
        return self.buffer[:len(filled)]

    def seek(self, frame):
        self._ensure_done()
        self.inner.seek(frame)

class FileSource(Source):
    def __init__(self, path):
        data = soundfile.SoundFile(path)
        if data.seekable():
            data.seek(0, soundfile.SEEK_END)
            end = data.tell()
            data.seek(0, soundfile.SEEK_SET)
        else:
            end = None

        super().__init__(data.samplerate, data.channels, size=end)
        self.data = data

    def fill(self, max=None):
        if max is None:
            max = len(self.buffer)
        return self.data.read(out=self.buffer[:max])

    def seek(self, frame):
        self.data.seek(frame, soundfile.SEEK_SET)
