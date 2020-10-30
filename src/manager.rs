use crate::{Scheduler, SchedulerSource, Sink, Source};

pub struct Manager<S> {
    sink: S,
    buffer: Vec<f32>,
    buffersize: u64,
    offset: u64,
    scheduler: Scheduler,
    source: SchedulerSource,
}

impl<S> Manager<S>
where
    S: Sink,
{
    pub fn new(sink: S, buffersize: usize) -> Self {
        let (scheduler, source) = Scheduler::new(sink.samplerate(), sink.channels());
        Self {
            buffer: vec![0.0; buffersize * sink.channels() as usize],
            buffersize: buffersize as u64,
            offset: 0,
            sink,
            scheduler,
            source,
        }
    }

    pub fn advance(&mut self, frames: u64) -> anyhow::Result<()> {
        self.offset += frames;
        while self.offset > self.buffersize {
            // emit a chunk
            let avail = self.source.force_fill(&mut self.buffer);
            self.buffer[avail..].iter_mut().for_each(|v| *v = 0.0);
            self.sink.write(&self.buffer)?;

            self.offset -= self.buffersize;
        }
        Ok(())
    }

    pub fn advance_to_end(&mut self) -> anyhow::Result<()> {
        loop {
            let avail = self.source.fill(&mut self.buffer);
            if avail == 0 {
                break Ok(());
            }
            self.sink.write(&self.buffer[..avail])?;
        }
    }
}

impl<S> std::ops::Deref for Manager<S> {
    type Target = Scheduler;
    fn deref(&self) -> &Self::Target {
        &self.scheduler
    }
}

impl<S> std::ops::DerefMut for Manager<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scheduler
    }
}
