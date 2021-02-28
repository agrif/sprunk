mod system;

pub use system::System;

pub trait Sink {
    fn samplerate(&self) -> f32;
    fn channels(&self) -> u16;

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()>;
}

impl Sink for Box<dyn Sink> {
    fn samplerate(&self) -> f32 {
        (**self).samplerate()
    }

    fn channels(&self) -> u16 {
        (**self).channels()
    }

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()> {
        (**self).write(buffer)
    }
}
