use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{OutputCallbackInfo, Sample, SampleFormat, Stream, StreamConfig};
use rb::{RbConsumer, RbProducer, RB};

pub struct System {
    config: StreamConfig,
    stream: Option<Stream>,
    _buffer: rb::SpscRb<f32>,
    tx: rb::Producer<f32>,
}

struct AudioThread {
    rx: rb::Consumer<f32>,
    buffer: Vec<f32>,
}

impl Drop for System {
    fn drop(&mut self) {
        // FIXME this is a MASSIVE HACK
        // this will totally leak the output stream
        // however, on my systems, dropping it normally hangs forever
        // is this better? I don't know.
        std::mem::forget(self.stream.take());
    }
}

impl System {
    pub fn new(buffersize: usize) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("could not find default output device"))?;
        let supported = device.supported_output_configs()?;
        let config = supported
            .max_by_key(|c| c.max_sample_rate())
            .ok_or_else(|| anyhow::anyhow!("no supported audio configurations"))?
            .with_max_sample_rate();
        let err_fn = |err| eprintln!("audio stream error: {}", err);
        let sample_format = config.sample_format();
        let config: StreamConfig = config.into();

        let buffer = rb::SpscRb::new(buffersize * config.channels as usize);
        let tx = buffer.producer();
        let rx = buffer.consumer();
        let mut thread = AudioThread { rx, buffer: vec![] };

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |d, cb| thread.callback::<f32>(d, cb),
                err_fn,
            ),
            SampleFormat::I16 => device.build_output_stream(
                &config,
                move |d, cb| thread.callback::<i16>(d, cb),
                err_fn,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config,
                move |d, cb| thread.callback::<u16>(d, cb),
                err_fn,
            ),
        }?;
        stream.play()?;
        Ok(Self {
            config,
            stream: Some(stream),
            _buffer: buffer,
            tx,
        })
    }
}

impl AudioThread {
    pub fn callback<T>(&mut self, mut data: &mut [T], _: &OutputCallbackInfo)
    where
        T: Sample,
    {
        if self.buffer.len() < data.len() {
            self.buffer.resize(data.len(), 0.0);
        }
        while data.len() > 0 {
            if let Some(cnt) = self.rx.read_blocking(&mut self.buffer[..data.len()]) {
                for (i, sample) in self.buffer[..cnt].iter().enumerate() {
                    data[i] = Sample::from(sample);
                }
                data = &mut data[cnt..];
            } else {
                break;
            }
        }
    }
}

impl super::Sink for System {
    fn samplerate(&self) -> f32 {
        self.config.sample_rate.0 as f32
    }

    fn channels(&self) -> u16 {
        self.config.channels
    }

    fn write(&mut self, mut buffer: &[f32]) -> anyhow::Result<()> {
        while buffer.len() > 0 {
            if let Some(cnt) = self.tx.write_blocking(buffer) {
                buffer = &buffer[cnt..];
            } else {
                break;
            }
        }
        Ok(())
    }
}
