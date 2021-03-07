mod realtime;
mod shout;
mod stream;
mod system;

pub use realtime::Realtime;
pub use self::shout::Shout;
pub use stream::Stream;
pub use system::System;

pub trait Sink {
    fn samplerate(&self) -> f32;
    fn channels(&self) -> u16;

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()>;

    fn realtime(self) -> Realtime<Self>
    where
        Self: Sized,
    {
        Realtime::new(self)
    }
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
