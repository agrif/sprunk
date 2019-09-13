import sys

import numpy
import attr
import pyaudio
import samplerate
import soundfile

# pip install numpy attrs pyaudio samplerate SoundFile

@attr.s
class Source:
    samplerate = attr.ib()
    channels = attr.ib()
    size = attr.ib() # in samples, None if unsized

    def allocate(self, frames):
        buf = numpy.zeros((frames, self.channels), dtype=numpy.float32)
        self.buffer = buf
        return buf

    def fill(self):
        raise NotImplementedError('Source.fill')
    
    def mono_to_many(self, mix=None):
        if mix is None:
            mix = numpy.ones(self.channels)
        assert self.channels == 1
        return MonoToMany(
            samplerate=self.samplerate,
            channels=len(mix),
            size=self.size,
            mix=numpy.asarray(mix),
            inner=self,
        )
        
    def many_to_mono(self, mix=None):
        if mix is None:
            mix = numpy.ones(self.channels) / self.channels
        assert self.channels == len(mix)
        return ManyToMono(
            samplerate=self.samplerate,
            channels=1,
            size=self.size,
            mix=numpy.asarray(mix),
            inner=self,
        )
    
    def resample(self, newrate):
        if newrate == self.samplerate:
            return self
        return Resample(
            samplerate=newrate,
            channels=self.channels,
            size=int(numpy.ceil(self.size * newrate / self.samplerate)) if self.size else None,
            resampler=samplerate.converters.Resampler(channels=self.channels),
            ratio=newrate / self.samplerate,
            inner=self,
        )
    
    def reformat(self, samplerate=None, channels=None):
        src = self
        if channels and channels < src.channels:
            if not channels == 1:
                raise RuntimeError("can only downmix {} channels to mono".format(src.channels))
            src = src.many_to_mono()
        if samplerate and samplerate != src.samplerate:
            src = src.resample(samplerate)
        if channels and channels > src.channels:
            if not src.channels == 1:
                raise RuntimeError("can only upmix mono to {} channels".format(channels))
            src = src.mono_to_many()
        return src

@attr.s
class MonoToMany(Source):
    mix = attr.ib()
    inner = attr.ib(repr=False)
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self):
        filled = self.inner.fill()
        self.buffer[:] = self.inner.buffer * self.mix[numpy.newaxis, :]
        return self.buffer[:len(filled)]
        
@attr.s
class ManyToMono(Source):
    mix = attr.ib()
    inner = attr.ib(repr=False)
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self):
        filled = self.inner.fill()
        self.buffer[:, 0] = self.inner.buffer @ self.mix
        return self.buffer[:len(filled)]

@attr.s
class Resample(Source):
    inner = attr.ib(repr=False)
    resampler = attr.ib(repr=False)
    ratio = attr.ib()
    
    def allocate(self, frames):
        innerframes = int(numpy.ceil(frames * self.inner.samplerate / self.samplerate))
        self.inner.allocate(innerframes)
        # make sure our assumption works with libsamplerate's assumption
        assert frames == int(innerframes * self.ratio)
        return super().allocate(frames)
    
    def fill(self):
        filled = self.inner.fill()
        if len(filled) > 0:
            proc = self.resampler.process(filled, self.ratio, end_of_input=False)
        else:
            proc = self.resampler.process(filled, self.ratio, end_of_input=True)
        self.buffer[:len(proc)] = proc
        return self.buffer[:len(proc)]

@attr.s
class Sink:
    samplerate = attr.ib()
    channels = attr.ib()
    
    def write(self, buf):
        raise NotImplementedError('Sink.write')

@attr.s
class FileSource(Source):
    data = attr.ib(repr=False)

    @classmethod
    def open(cls, path):
        data = soundfile.SoundFile(path)
        if data.seekable():
            data.seek(0, soundfile.SEEK_END)
            end = data.tell()
            data.seek(0, soundfile.SEEK_SET)
        else:
            end = None
        return cls(
            samplerate=data.samplerate,
            channels=data.channels,
            size=end,
            data=data,
        )

    def fill(self):
        return self.data.read(out=self.buffer)

@attr.s
class PyAudioSink(Sink):
    stream = attr.ib(repr=False)
    
    @classmethod
    def open(cls, samplerate, channels):
        p = pyaudio.PyAudio()
        stream = p.open(
            format=pyaudio.paFloat32,
            channels=channels,
            rate=samplerate,
            output=True,
        )
        return cls(
            samplerate=samplerate,
            channels=channels,
            stream=stream,
        )
    
    def write(self, buf):
        self.stream.write(buf, num_frames=len(buf))

if __name__ == '__main__':
    src = FileSource.open(sys.argv[1]).reformat(44100 // 16, 1)
    print(src)
    sink = PyAudioSink.open(src.samplerate, src.channels)
    src.allocate(1024 * 8)
    filled = src.buffer
    while len(filled) > 0:
        filled = src.fill()
        print('got {} frames'.format(len(filled)))
        sink.write(filled)
