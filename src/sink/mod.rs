mod system;

pub use system::System;

pub trait Sink {
    fn samplerate(&self) -> f32;
    fn channels(&self) -> u16;

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()>;

    fn play<S>(&mut self, source: S, buffersize: usize) -> anyhow::Result<()>
    where
        S: crate::Source,
    {
        use crate::Source;
        let mut source = source.reformat_for(self);
        let mut buffer = vec![0.0; self.channels() as usize * buffersize];
        loop {
            let avail = source.fill(&mut buffer);
            if avail == 0 {
                break;
            }
            self.write(&buffer[..avail])?;
        }
        Ok(())
    }
}
