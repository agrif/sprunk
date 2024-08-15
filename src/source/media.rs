use std::io::{Read, Seek, SeekFrom};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};

pub struct Media {
    track_id: u32,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    buffer: Option<SampleBuffer<f32>>,
    used: usize,
    reset_next: bool,
    end_of_stream: bool,
}

struct MediaReader<R> {
    inner: R,
    len: Option<u64>,
}

impl<R> MediaReader<R>
where
    R: Seek,
{
    fn new(mut inner: R) -> Self {
        let len = if let Ok(cur) = inner.stream_position() {
            let end = inner.seek(SeekFrom::End(0));
            let _ = inner.seek(SeekFrom::Start(cur));
            end.ok()
        } else {
            None
        };
        Self { inner, len }
    }
}

impl<R> Read for MediaReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<R> Seek for MediaReader<R>
where
    R: Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl<R> MediaSource for MediaReader<R>
where
    R: Read + Seek + Send + Sync,
{
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.len
    }
}

impl Media {
    pub fn new<R>(data: R) -> anyhow::Result<Self>
    where
        R: Read + Seek + Send + Sync + 'static,
    {
        let mss = MediaSourceStream::new(Box::new(MediaReader::new(data)), Default::default());

        let hint = Default::default();
        let meta_opts = Default::default();
        let fmt_opts = Default::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|_| anyhow::anyhow!("unsupported format"))?;

        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow::anyhow!("no supported tracks"))?;

        let dec_opts = Default::default();

        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .map_err(|_| anyhow::anyhow!("unsupported codec"))?;

        let track_id = track.id;

        Ok(Self {
            track_id,
            format,
            decoder,
            buffer: None,
            used: 0,
            reset_next: true,
            end_of_stream: false,
        })
    }

    fn get_packet(&mut self) -> anyhow::Result<bool> {
        if self.end_of_stream {
            return Ok(false);
        }

        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(Error::ResetRequired) => {
                    // as far as we care, this is end of stream
                    self.end_of_stream = true;
                    return Ok(false);
                }
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // this is end of stream, I'm pretty sure.
                    // symphonia doesn't document this
                    self.end_of_stream = true;
                    return Ok(false);
                }
                Err(e) => {
                    anyhow::bail!("error getting packet: {:?}", e);
                }
            };

            while !self.format.metadata().is_latest() {
                self.format.metadata().pop();
            }

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let current = self.buffer.as_ref().map(|b| {
                        b.capacity() >= decoded.capacity() * decoded.spec().channels.count()
                    });

                    if current != Some(true) {
                        self.buffer = Some(SampleBuffer::new(
                            (decoded.capacity() as u64) * 2,
                            *decoded.spec(),
                        ));
                    }

                    if let Some(buffer) = self.buffer.as_mut() {
                        buffer.copy_interleaved_ref(decoded);

                        if buffer.samples().is_empty() {
                            continue;
                        }

                        return Ok(true);
                    } else {
                        anyhow::bail!("no buffer to write to");
                    }
                }
                Err(Error::IoError(_)) => {
                    continue;
                }
                Err(Error::DecodeError(_)) => {
                    continue;
                }
                Err(e) => {
                    anyhow::bail!("decode error: {}", e);
                }
            }
        }
    }

    fn get_data(&mut self, mut f: impl FnMut(&[f32]) -> usize) -> anyhow::Result<bool> {
        if self.end_of_stream {
            return Ok(false);
        }

        if self.buffer.is_none() || self.reset_next {
            self.used = 0;
            self.reset_next = false;
            if !self.get_packet()? {
                return Ok(false);
            }
        }

        if let Some(buffer) = self.buffer.as_ref() {
            let samples = buffer.samples();
            assert!(self.used < samples.len());
            self.used += f(&samples[self.used..]);
            if self.used >= samples.len() {
                self.reset_next = true;
            }
            Ok(true)
        } else {
            anyhow::bail!("failed to read buffer");
        }
    }
}

impl super::Source for Media {
    fn samplerate(&self) -> f32 {
        self.decoder.codec_params().sample_rate.unwrap_or(0) as f32
    }

    fn channels(&self) -> u16 {
        self.decoder
            .codec_params()
            .channels
            .map(|c| c.count())
            .unwrap_or(0) as u16
    }

    fn len(&self) -> Option<u64> {
        self.decoder.codec_params().n_frames
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        let channels = self.channels();
        let ourlen = (buffer.len() / channels as usize) * channels as usize;

        let buffer = &mut buffer[..ourlen];
        let mut written = 0;
        while buffer.len() > written {
            let more = self.get_data(|samples| {
                let amt = samples.len().min(buffer.len() - written);
                buffer[written..written + amt].copy_from_slice(&samples[..amt]);
                written += amt;
                amt
            });

            match more {
                Ok(true) => continue,
                Ok(false) => break,
                Err(_) => {
                    // this is a legitimate error, but we have no way to
                    // propogate it
                    return 0;
                }
            }
        }

        written
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        // FIXME precision seeking, also, is TimeStamp correct?
        self.format.seek(
            SeekMode::Coarse,
            SeekTo::TimeStamp {
                ts: frame,
                track_id: self.track_id,
            },
        )?;
        self.decoder.reset();
        self.reset_next = true;
        self.end_of_stream = false;
        Ok(())
    }
}
