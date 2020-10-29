use std::io::BufReader;

use rodio::decoder::Decoder;
use rodio::source::SamplesConverter;
use rodio::Source;

pub struct Media<R: std::io::Read + std::io::Seek> {
    source: SamplesConverter<Decoder<BufReader<R>>, f32>,
}

impl<R> Media<R>
where
    R: std::io::Read + std::io::Seek + std::marker::Send + 'static,
{
    pub fn new(data: R) -> anyhow::Result<Self> {
        Ok(Media {
            source: Decoder::new(BufReader::new(data))?.convert_samples(),
        })
    }
}

impl<R> super::Source for Media<R>
where
    R: std::io::Read + std::io::Seek + std::marker::Send + 'static,
{
    fn samplerate(&self) -> f32 {
        self.source.sample_rate() as f32
    }

    fn channels(&self) -> u16 {
        self.source.channels()
    }

    fn len(&self) -> Option<u64> {
        self.source
            .total_duration()
            .map(|d| (d.as_secs_f32() * self.samplerate()).round() as u64)
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        let mut i = 0;
        if buffer.len() == 0 {
            return 0;
        }
        while let Some(v) = self.source.next() {
            buffer[i] = v;
            i += 1;
            if i >= buffer.len() {
                break;
            }
        }
        i
    }

    fn seek(&mut self, _frame: u64) -> anyhow::Result<()> {
        anyhow::bail!("cannot seek rodio sources")
    }
}
