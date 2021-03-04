use crate::Encoder;

pub struct Stream<F, E> {
    inner: F,
    encoder: E,
}

impl<F, E> Stream<F, E>
where
    F: std::io::Write,
    E: Encoder,
{
    pub fn new(inner: F, encoder: E) -> Self {
        Stream { inner, encoder }
    }
}

impl<F, E> super::Sink for Stream<F, E>
where
    F: std::io::Write,
    E: Encoder,
{
    fn samplerate(&self) -> f32 {
        self.encoder.samplerate()
    }

    fn channels(&self) -> u16 {
        self.encoder.channels()
    }

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()> {
        let encoded = self.encoder.encode(buffer)?;
        self.inner.write_all(encoded)?;
        Ok(())
    }
}
