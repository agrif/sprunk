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

    def fill(self, max=None):
        raise NotImplementedError('Source.fill')
    
    def mono_to_many(self, mix):
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
            src = src.mono_to_many(numpy.ones(channels))
        return src
    
    def reformat_like(self, other):
        return self.reformat(other.samplerate, other.channels)

@attr.s
class MonoToMany(Source):
    mix = attr.ib()
    inner = attr.ib(repr=False)
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self, max=None):
        filled = self.inner.fill(max=max)
        self.buffer[:] = self.inner.buffer * self.mix[numpy.newaxis, :]
        return self.buffer[:len(filled)]
        
@attr.s
class ManyToMono(Source):
    mix = attr.ib()
    inner = attr.ib(repr=False)
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self, max=None):
        filled = self.inner.fill(max=max)
        self.buffer[:, 0] = self.inner.buffer @ self.mix
        return self.buffer[:len(filled)]

@attr.s
class Resample(Source):
    inner = attr.ib(repr=False)
    resampler = attr.ib(repr=False)
    ratio = attr.ib()
    
    def allocate(self, frames):
        innerframes = int(frames * self.inner.samplerate / self.samplerate)
        self.inner.allocate(innerframes)
        # make sure our assumption works with libsamplerate's assumption
        assert frames >= int(innerframes * self.ratio)
        return super().allocate(frames)
    
    def fill(self, max=None):
        if max is None:
            max = len(self.buffer)
        filled = self.inner.fill(max=int(max * self.inner.samplerate / self.samplerate))
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

    def fill(self, max=None):
        if max is None:
            max = len(self.buffer)
        return self.data.read(out=self.buffer[:max])

@attr.s
class VolumeControl(Source):
    inner = attr.ib(repr=False)
    condlist = attr.ib(repr=False, default=attr.Factory(list))
    funclist = attr.ib(repr=False, default=attr.Factory(list))
    last_frame = attr.ib(default=0, repr=False)
    
    # volume keys must be added in order, or weird things happen
    
    @classmethod
    def new(cls, inner):
        return cls(
            samplerate=inner.samplerate,
            channels=inner.channels,
            size=inner.size,
            inner=inner,
        ).set_volume(0, 1.0)
    
    def set_volume(self, frame, volume):
        keyframe = int(numpy.round(frame))
        self.condlist.append(lambda f: f >= keyframe)
        self.funclist.append(volume)
        return self
    
    def ramp_volume(self, frame, end, duration):
        start = self.funclist[-1]
        variance = end - start
        keyframe = int(numpy.round(frame))
        keyduration = int(numpy.round(duration))
        keyframeend = keyframe + keyduration
        self.condlist.append(lambda f: (f >= keyframe) & (f <= keyframeend))
        self.funclist.append(lambda f: start + (f - keyframe) * variance / keyduration)
        return self.set_volume(frame + duration, end)
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        self.framebuffer = numpy.arange(self.last_frame, self.last_frame + frames, dtype=numpy.float)
        return super().allocate(frames)
    
    def fill(self, max=None):
        filled = self.inner.fill(max=max)
        self.buffer[:] = self.inner.buffer * numpy.piecewise(self.framebuffer, [f(self.framebuffer) for f in self.condlist], self.funclist)[:, numpy.newaxis]
        self.framebuffer += len(filled)
        self.last_frame += len(filled)
        return self.buffer[:len(filled)]

@attr.s
class Scheduler(Source):
    sources = attr.ib(default=attr.Factory(list))
    active = attr.ib(default=attr.Factory(list))
    last_frame = attr.ib(default=0)
    
    @classmethod
    def new(cls, samplerate=48000, channels=2):
        return cls(
            samplerate=samplerate,
            channels=channels,
            size=None,
        )
    
    def schedule(self, start, src):
        assert self.samplerate == src.samplerate
        assert self.channels == src.channels
        if start < self.last_frame:
            start = self.last_frame
        self.sources.append((int(start), src))
        if getattr(self, 'buffer', None):
            src.allocate(len(self.buffer))
        if src.size:
            return start + src.size
        return None
    
    def allocate(self, frames):
        for _, src in self.sources:
            src.allocate(frames)
        for src in self.active:
            src.allocate(frames)
        return super().allocate(frames)
    
    def fill(self, max=None):
        if max is None:
            max = len(self.buffer)
        
        # do not fill if there is nothing left
        if not self.sources and not self.active:
            return self.buffer[0:0]
        
        # zero our buffer
        self.buffer[:max] = 0
        
        # helper to keep trying to fill a buffer until it's filled
        def fill_into(buf, src):
            filled = src.fill(max=len(buf))
            buf[:len(filled)] += filled
            if len(filled) == 0:
                return False
            if len(filled) < len(buf):
                return fill_into(buf[len(filled):], src)
            return True
        
        # handle all our actives
        to_remove = []
        for src in self.active:
            if not fill_into(self.buffer[:max], src):
                to_remove.append(src)
        for src in to_remove:
            self.active.remove(src)
        
        # look for new actives
        to_remove = []
        for startframe, src in self.sources:
            if not startframe < self.last_frame + max:
                continue
            # this source starts in this block
            local_start = startframe - self.last_frame
            if local_start < 0:
                local_start = 0
            if fill_into(self.buffer[local_start:max], src):
                self.active.append(src)
            to_remove.append((startframe, src))
        for t in to_remove:
            self.sources.remove(t)
        
        self.last_frame += max
        return self.buffer[:max]

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

@attr.s
class FileSink(Sink):
    data = attr.ib(repr=False)
    
    @classmethod
    def open(cls, path, samplerate, channels, **kwargs):
        data = soundfile.SoundFile(path, mode='w', samplerate=samplerate, channels=channels, **kwargs)
        return cls(
            samplerate=samplerate,
            channels=channels,
            data=data,
        )
    
    def write(self, buf):
        self.data.write(buf)

if __name__ == '__main__':
    sched = Scheduler.new()
    src1 = FileSource.open(sys.argv[1]).reformat_like(sched)
    src2 = VolumeControl.new(FileSource.open(sys.argv[2]).reformat_like(sched))
    
    t = sched.schedule(sched.samplerate, src1)
    _ = sched.schedule(0, src2)
    
    src2.set_volume(0, 1.0)
    src2.ramp_volume(0, 0.3, src2.samplerate)
    src2.ramp_volume(t, 1.0, src2.samplerate)
    
    #sink = PyAudioSink.open(sched.samplerate, sched.channels)
    sink = FileSink.open('output.ogg', sched.samplerate, sched.channels)
    sched.allocate(int(sched.samplerate * 0.1))
    filled = sched.buffer
    while len(filled) > 0:
        filled = sched.fill()
        sink.write(filled)
