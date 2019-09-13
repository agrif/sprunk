import sys

import click
import numpy
try:
    import pyaudio
except ImportError:
    pass
import samplerate
import soundfile

# pip install numpy pyaudio samplerate SoundFile

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
    
    def mono_to_many(self, mix):
        assert self.channels == 1
        return MonoToMany(self, numpy.asarray(mix))
        
    def many_to_mono(self, mix=None):
        if mix is None:
            mix = numpy.ones(self.channels) / self.channels
        assert self.channels == len(mix)
        return ManyToMono(self, numpy.asarray(mix))
    
    def resample(self, newrate):
        if newrate == self.samplerate:
            return self
        return Resample(self, newrate)
    
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

class MonoToMany(Source):
    def __init__(self, inner, mix):
        super().__init__(inner.samplerate, len(mix), size=inner.size)
        self.inner = inner
        self.mix = mix
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self, max=None):
        filled = self.inner.fill(max=max)
        self.buffer[:] = self.inner.buffer * self.mix[numpy.newaxis, :]
        return self.buffer[:len(filled)]
        
class ManyToMono(Source):
    def __init__(self, inner, mix):
        super().__init__(inner.samplerate, 1, size=inner.size)
        self.inner = inner
        self.mix = mix
    
    def allocate(self, frames):
        self.inner.allocate(frames)
        return super().allocate(frames)
    
    def fill(self, max=None):
        filled = self.inner.fill(max=max)
        self.buffer[:, 0] = self.inner.buffer @ self.mix
        return self.buffer[:len(filled)]

class Resample(Source):
    def __init__(self, inner, newrate):
        newsize = None
        if self.size:
            newsize = int(numpy.ceil(self.size * newrate / self.samplerate))
        super().__init__(newrate, inner.channels, size=newsize)
        self.resampler = samplerate.converters.Resampler(channels=self.channels)
        self.ratio = newrate / self.samplerate
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
        filled = self.inner.fill(max=int(max * self.inner.samplerate / self.samplerate))
        if len(filled) > 0:
            proc = self.resampler.process(filled, self.ratio, end_of_input=False)
        else:
            proc = self.resampler.process(filled, self.ratio, end_of_input=True)
        self.buffer[:len(proc)] = proc
        return self.buffer[:len(proc)]

class Sink:
    def __init__(self, samplerate, channels):
        self.samplerate = samplerate
        self.channels = channels
    
    def write(self, buf):
        raise NotImplementedError('Sink.write')

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

class PyAudioSink(Sink):
    def __init__(self, samplerate, channels):
        p = pyaudio.PyAudio()
        stream = p.open(
            format=pyaudio.paFloat32,
            channels=channels,
            rate=samplerate,
            output=True,
        )
        super().__init__(samplerate, channels)
        self.stream = stream
    
    def write(self, buf):
        self.stream.write(buf, num_frames=len(buf))

class FileSink(Sink):
    def __init__(self, path, samplerate, channels, **kwargs):
        data = soundfile.SoundFile(path, mode='w', samplerate=samplerate, channels=channels, **kwargs)
        super().__init__(samplerate, channels)
        self.data = data
    
    def write(self, buf):
        self.data.write(buf)

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
            return FileSink(value, 48000, 2)
        return PyAudioSink(48000, 2)
    return click.option('-o', '--output', type=str, callback=open_sink)(f)

@click.group()
def cli():
    pass

@cli.command()
@output_option
@click.argument('PATH')
def play(output, path):
    src = FileSource(path)
    run(src, output)

if __name__ == '__main__':
    cli()
