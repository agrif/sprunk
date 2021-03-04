use std::thread::sleep;
use std::time::{Duration, Instant};

pub struct Realtime<S> {
    inner: S,
    runout: Option<Instant>,
}

impl<S> Realtime<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            runout: None,
        }
    }
}

impl<S> super::Sink for Realtime<S>
where
    S: super::Sink,
{
    fn samplerate(&self) -> f32 {
        self.inner.samplerate()
    }

    fn channels(&self) -> u16 {
        self.inner.channels()
    }

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()> {
        let duration = Duration::from_secs_f32(
            buffer.len() as f32 / (self.channels() as f32 * self.samplerate()),
        );
        let now = Instant::now();

        // wait until last chunk of this size is starting to go out
        if let Some(ref end) = self.runout {
            if *end > now + duration {
                sleep(*end - now - duration);
            }
        }

        self.inner.write(buffer)?;
        *self.runout.get_or_insert(now) += duration;
        Ok(())
    }
}
