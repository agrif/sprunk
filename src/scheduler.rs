use std::cell::RefCell;
use std::rc::Rc;

use crate::Source;

pub struct Scheduler {
    data: Rc<RefCell<SchedulerData>>,
    samplerate: f32,
    channels: u16,
}

pub struct SchedulerSource {
    data: Rc<RefCell<SchedulerData>>,
    buffer: Vec<f32>,
    samplerate: f32,
    channels: u16,
}

struct SchedulerData {
    scheduled: Vec<(u64, Box<dyn Source>)>,
    active: Vec<Box<dyn Source>>,
}

impl Scheduler {
    pub fn new(samplerate: f32, channels: u16) -> (Scheduler, SchedulerSource) {
        let data = Rc::new(RefCell::new(SchedulerData {
            scheduled: Vec::with_capacity(10),
            active: Vec::with_capacity(10),
        }));
        let scheduler = Scheduler {
            data: data.clone(),
            samplerate,
            channels,
        };
        let source = SchedulerSource {
            data,
            buffer: Vec::new(),
            samplerate,
            channels,
        };
        (scheduler, source)
    }

    pub fn subscheduler(&mut self) -> Scheduler {
        let (sched, src) = Scheduler::new(self.samplerate, self.channels);
        let mut data = self.data.borrow_mut();
        data.active.push(Box::new(src));
        sched
    }

    pub fn add<S>(&mut self, start: u64, src: S)
    where
        S: Source + 'static,
    {
        let src = src.reformat(self.samplerate, self.channels);
        let mut data = self.data.borrow_mut();
        data.scheduled.push((start, Box::new(src)));
    }
}

impl Source for SchedulerSource {
    fn samplerate(&self) -> f32 {
        self.samplerate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn len(&self) -> Option<u64> {
        None
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        self.buffer.resize(buffer.len(), 0.0);
        buffer.iter_mut().for_each(|m| *m = 0.0);

        let mut data = self.data.borrow_mut();

        // do we have anything to do, even?
        if data.active.len() == 0 && data.scheduled.len() == 0 {
            return 0;
        }

        // render our active sources
        let mut i = 0;
        while i != data.active.len() {
            let avail = data.active[i].force_fill(&mut self.buffer);
            for j in 0..avail {
                buffer[j] += self.buffer[j];
            }
            if avail < self.buffer.len() {
                data.active.remove(i);
            } else {
                i += 1;
            }
        }

        // deal with our scheduled sources
        let mut i = 0;
        let window = buffer.len() as u64 / self.channels as u64;
        while i != data.scheduled.len() {
            let (ref mut start, ref mut src) = data.scheduled[i];
            if *start < window as u64 {
                let len = (window - *start) as usize * self.channels as usize;
                let avail = src.force_fill(&mut self.buffer[..len]);
                let offset = *start as usize * self.channels as usize;
                for j in 0..avail {
                    buffer[offset + j] = self.buffer[j];
                }

                let (_, src) = data.scheduled.remove(i);
                if avail == len {
                    data.active.push(src);
                }
            } else {
                *start -= window;
                i += 1;
            }
        }

        buffer.len()
    }

    fn seek(&mut self, _frame: u64) -> anyhow::Result<()> {
        anyhow::bail!("cannot seek a scheduler")
    }
}
