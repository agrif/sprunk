use crate::{Scheduler, SchedulerSource, Sink, Source, Time};

pub struct Manager<S, T> {
    sink: S,
    buffer: Vec<f32>,
    buffersize: u64,
    offset: u64,
    source: SchedulerSource,
    task: async_executor::Task<anyhow::Result<T>>,
}

impl<S, T> Manager<S, T>
where
    S: Sink,
    T: 'static,
{
    pub fn new<F, Fut>(sink: S, buffersize: usize, f: F) -> Self
    where
        F: FnOnce(Scheduler) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>> + 'static,
    {
        let (scheduler, source) = Scheduler::new(sink.samplerate(), sink.channels());
        Self {
            buffer: vec![0.0; buffersize * sink.channels() as usize],
            buffersize: buffersize as u64,
            offset: 0,
            sink,
            source,
            task: scheduler.run(f),
        }
    }

    pub fn advance<Ti>(&mut self, frames: Ti) -> anyhow::Result<()>
    where
        Ti: Into<Time>,
    {
        self.offset += frames.into().to_frames(self.source.samplerate());
        while self.offset > self.buffersize {
            // emit a chunk
            let avail = self.source.force_fill(&mut self.buffer);
            self.buffer[avail..].iter_mut().for_each(|v| *v = 0.0);
            self.sink.write(&self.buffer)?;

            self.offset -= self.buffersize;
        }
        Ok(())
    }

    pub fn skip<Ti>(&mut self, frames: Ti)
    where
        Ti: Into<Time>,
    {
        self.offset += frames.into().to_frames(self.source.samplerate());
        while self.offset > self.buffersize {
            // this *could* be done more efficiently, but this is fine
            self.source.force_fill(&mut self.buffer);
            self.offset -= self.buffersize;
        }
    }

    pub fn advance_to_end(mut self) -> anyhow::Result<T> {
        loop {
            let avail = self.source.fill(&mut self.buffer);
            if avail == 0 {
                return futures_lite::future::block_on(self.task);
            }
            self.sink.write(&self.buffer[..avail])?;
        }
    }
}
