use crate::samplerate::{ConverterType, SampleRate};

pub struct Resample<S> {
    source: S,
    samplerate: f32,
    inrate: f32,
    converter: SampleRate,
    buffer: Vec<f32>,
    bufferstart: usize,
}

impl<S> Resample<S>
where
    S: super::Source,
{
    pub fn new(source: S, samplerate: f32) -> Self {
        Self {
            converter: SampleRate::new(ConverterType::SincFastest, source.channels())
                .unwrap(),
            inrate: source.samplerate(),
            buffer: vec![],
            bufferstart: 0,
            source,
            samplerate,
        }
    }
}

impl<S> super::Source for Resample<S>
where
    S: super::Source,
{
    fn samplerate(&self) -> f32 {
        self.samplerate
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn len(&self) -> Option<u64> {
        self.source
            .len()
            .map(|s| (s as f32 * self.samplerate / self.source.samplerate()).round() as u64)
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        let ratio = self.samplerate / self.inrate;
        let mut innerlen = buffer.len() as f32 / ratio;
        innerlen += self.source.channels() as f32 * 2.0;
        if innerlen > self.bufferstart as f32 {
            self.buffer.resize(innerlen.round() as usize, 0.0);
        }

        let avail = self.source.fill(&mut self.buffer[self.bufferstart..]);
        let (input_used, output_gen) = self
            .converter
            .process(
                ratio as f64,
                &self.buffer[..avail + self.bufferstart],
                buffer,
            )
            .unwrap();
        self.buffer.copy_within(input_used.., 0);
        self.bufferstart = avail + self.bufferstart - input_used;
        output_gen
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        self.converter.reset().unwrap();
        self.bufferstart = 0;
        self.source
            .seek((frame as f32 * self.source.samplerate() / self.samplerate).round() as u64)
    }
}
