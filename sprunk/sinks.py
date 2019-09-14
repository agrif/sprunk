import subprocess

import soundfile
try:
    import pyaudio
except ImportError:
    import warnings
    warnings.warn('could not import PyAudio, live playback is not possible')

__all__ = [
    'Sink',
    'PyAudioSink',
    'FileSink',
    'FFmpegSink',
]

class Sink:
    def __init__(self, samplerate, channels):
        self.samplerate = samplerate
        self.channels = channels
    
    def write(self, buf):
        raise NotImplementedError('Sink.write')

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

class FFmpegSink(Sink):
    def __init__(self, samplerate, channels, realtime, args):
        self.process = subprocess.Popen(['ffmpeg', '-f', 's16le', '-ar', str(samplerate), '-ac', str(channels)] + (['-re'] if realtime else []) + ['-i', '-'] + args, stdin=subprocess.PIPE)
        data = soundfile.SoundFile(self.process.stdin, mode='w', samplerate=samplerate, channels=channels, format='RAW', subtype='PCM_16', endian='LITTLE')
        super().__init__(samplerate, channels)
        self.data = data
    
    def write(self, buf):
        self.data.write(buf)
