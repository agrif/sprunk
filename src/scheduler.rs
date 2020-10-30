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
    volume: f32,
    ramps: Vec<(u64, f32)>,
}

impl Scheduler {
    pub fn new(samplerate: f32, channels: u16) -> (Scheduler, SchedulerSource) {
        let data = Rc::new(RefCell::new(SchedulerData {
            scheduled: Vec::with_capacity(10),
            active: Vec::with_capacity(10),
            volume: 1.0,
            ramps: Vec::with_capacity(2),
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

    pub fn add<S>(&mut self, start: u64, src: S) -> Option<u64>
    where
        S: Source + 'static,
    {
        let src = src.reformat(self.samplerate, self.channels);
        let len = src.len();
        let mut data = self.data.borrow_mut();
        data.scheduled.push((start, Box::new(src)));
        len
    }

    fn add_ramp_point(&mut self, start: u64, volume: f32) {
        let mut data = self.data.borrow_mut();
        let idx = data
            .ramps
            .binary_search_by_key(&start, |r| r.0)
            .unwrap_or_else(|x| x);
        data.ramps.insert(idx, (start, volume));
    }

    fn get_volume(&mut self, time: u64) -> f32 {
        let data = self.data.borrow();
        let mut lastvol = data.volume;
        let mut lasttime = 0;
        for (t, vol) in data.ramps.iter() {
            if time < *t {
                let p = (time - lasttime) as f32 / (t - lasttime) as f32;
                return lastvol + (vol - lastvol) * p;
            }
            lastvol = *vol;
            lasttime = *t;
        }
        lastvol
    }

    pub fn set_volume(&mut self, start: u64, volume: f32, duration: Option<u64>) {
        let duration = duration.unwrap_or_else(|| (0.005 * self.samplerate).ceil() as u64);
        let orig = self.get_volume(start);
        self.add_ramp_point(start, orig);
        self.add_ramp_point(start + duration, volume);
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
        let window = buffer.len() as u64 / self.channels as u64;

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

        // apply the volume ramp
        let mut volume = data.volume;
        let mut delta;
        let mut i = 0;
        while let Some((time, vol)) = data.ramps.get(0) {
            delta = (vol - volume) / (time - i as u64) as f32;
            while i < buffer.len() && i < *time as usize {
                buffer[i] *= volume;
                volume += delta;
                i += 1;
            }
            if i == *time as usize {
                volume = *vol;
                data.ramps.remove(0);
            }
            if i == buffer.len() {
                break;
            }
        }
        data.volume = volume;
        while i < buffer.len() {
            buffer[i] *= volume;
            i += 1;
        }
        for (time, _) in data.ramps.iter_mut() {
            *time -= window;
        }

        buffer.len()
    }

    fn seek(&mut self, _frame: u64) -> anyhow::Result<()> {
        anyhow::bail!("cannot seek a scheduler")
    }
}
