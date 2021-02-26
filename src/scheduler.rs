use std::cell::RefCell;
use std::rc::Rc;

use async_oneshot::{oneshot, Sender};
use async_executor::{LocalExecutor, Task};

use crate::Source;

#[derive(Clone, Copy, Debug)]
pub struct Time {
    frames: u64,
    seconds: f32,
}

pub struct Scheduler {
    data: Rc<RefCell<SchedulerData>>,
    executor: Rc<RefCell<LocalExecutor<'static>>>,
    samplerate: f32,
    channels: u16,
}

pub struct SchedulerSource {
    data: Rc<RefCell<SchedulerData>>,
    executor: Rc<RefCell<LocalExecutor<'static>>>,
    buffer: Vec<f32>,
    samplerate: f32,
    channels: u16,
}

struct SchedulerData {
    offset: u64,
    timers: Vec<(u64, Sender<()>)>,
    scheduled: Vec<(u64, Box<dyn Source>)>,
    active: Vec<Box<dyn Source>>,
    volume: f32,
    ramps: Vec<(u64, f32)>,
}

impl Time {
    pub fn frames(frames: u64) -> Self {
        Self {
            frames,
            seconds: 0.0,
        }
    }

    pub fn seconds(seconds: f32) -> Self {
        Self { frames: 0, seconds }
    }

    pub fn to_frames(&self, samplerate: f32) -> u64 {
        if self.seconds >= 0.0 {
            self.frames + (self.seconds * samplerate).round() as u64
        } else {
            self.frames - (-self.seconds * samplerate).round() as u64
        }
    }

    pub fn to_seconds(&self, samplerate: f32) -> f32 {
        self.seconds + self.frames as f32 / samplerate
    }
}

impl std::ops::Add<Time> for Time {
    type Output = Time;
    fn add(self, rhs: Time) -> Time {
        Time {
            seconds: self.seconds + rhs.seconds,
            frames: self.frames + rhs.frames,
        }
    }
}

impl std::ops::Sub<Time> for Time {
    type Output = Time;
    fn sub(self, rhs: Time) -> Time {
        Time {
            seconds: self.seconds - rhs.seconds,
            frames: self.frames - rhs.frames,
        }
    }
}

impl std::ops::Add<f32> for Time {
    type Output = Time;
    fn add(self, rhs: f32) -> Time {
        Time {
            seconds: self.seconds + rhs,
            frames: self.frames,
        }
    }
}

impl std::ops::Sub<f32> for Time {
    type Output = Time;
    fn sub(self, rhs: f32) -> Time {
        Time {
            seconds: self.seconds - rhs,
            frames: self.frames,
        }
    }
}

impl From<f32> for Time {
    fn from(seconds: f32) -> Self {
        Self::seconds(seconds)
    }
}

impl Scheduler {
    pub fn new(samplerate: f32, channels: u16) -> (Scheduler, SchedulerSource) {
        let data = Rc::new(RefCell::new(SchedulerData {
            offset: 0,
            timers: Vec::with_capacity(10),
            scheduled: Vec::with_capacity(10),
            active: Vec::with_capacity(10),
            volume: 1.0,
            ramps: Vec::with_capacity(2),
        }));
        let executor = Rc::new(RefCell::new(LocalExecutor::new()));
        let scheduler = Scheduler {
            data: data.clone(),
            executor: executor.clone(),
            samplerate,
            channels,
        };
        let source = SchedulerSource {
            data,
            executor,
            buffer: Vec::new(),
            samplerate,
            channels,
        };
        (scheduler, source)
    }

    pub fn subscheduler(&mut self) -> Scheduler {
        let (sched, src) = Scheduler::new(self.samplerate, self.channels);
        let mut subdata = sched.data.borrow_mut();
        let mut data = self.data.borrow_mut();
        subdata.offset = data.offset;
        data.active.push(Box::new(src));
        drop(subdata);
        sched
    }

    pub fn add<T, S>(&mut self, start: T, src: S) -> Option<Time>
    where
        T: Into<Time>,
        S: Source + 'static,
    {
        let start = start.into().to_frames(self.samplerate);
        let src = src.reformat(self.samplerate, self.channels);
        let end = src.len().map(|l| Time::frames(l + start));
        let mut data = self.data.borrow_mut();
        data.scheduled.push((start, Box::new(src)));
        end
    }

    pub fn run<F, Fut, T>(self, f: F) -> Task<T>
    where
        F: FnOnce(Scheduler) -> Fut,
        Fut: std::future::Future<Output = T> + 'static,
        T: 'static,
    {
        let execcell = self.executor.clone();
        let exec = execcell.borrow();
        exec.spawn(f(self))
    }

    pub async fn wait<T>(&mut self, time: T) -> anyhow::Result<Time>
    where
        T: Into<Time>,
    {
        let time = time.into();
        let mut data = self.data.borrow_mut();
        let (send, recv) = oneshot();
        data.timers
            .push((time.to_frames(self.samplerate), send));
        drop(data);
        if let Err(_) = recv.await {
            anyhow::bail!("scheduler source dropped");
        }
        Ok(time)
    }

    fn add_ramp_point(&mut self, start: Time, volume: f32) {
        let start = start.to_frames(self.samplerate);
        let mut data = self.data.borrow_mut();
        let idx = data
            .ramps
            .binary_search_by_key(&start, |r| r.0)
            .unwrap_or_else(|x| x);
        data.ramps.insert(idx, (start, volume));
    }

    fn get_volume(&mut self, time: Time) -> f32 {
        let time = time.to_frames(self.samplerate);
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

    pub fn set_volume<T, U>(&mut self, start: T, volume: f32, duration: U) -> Time
    where
        T: Into<Time>,
        U: Into<Time>,
    {
        let start = start.into();
        let mut duration = duration.into();
        if duration.to_seconds(self.samplerate) < 0.005 {
            duration = Time::seconds(0.005);
        }
        let orig = self.get_volume(start);
        self.add_ramp_point(start, orig);
        self.add_ramp_point(start + duration, volume);
        start + duration
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

        let data = self.data.borrow();
        let offset = data.offset;
        let end = offset + buffer.len() as u64 / self.channels as u64;
        drop(data); // so we can borrow it in try_tick()

        loop {
            let mut go_again = false;

            let exec = self.executor.borrow();
            while exec.try_tick() {
                go_again = true;
            }

            let mut data = self.data.borrow_mut();
            let mut i = 0;
            while i != data.timers.len() {
                let (ref mut start, _) = data.timers[i];
                if *start < end {
                    let (_, send) = data.timers.remove(i);
                    let _ = send.send(());
                    go_again = true;
                } else {
                    i += 1;
                }
            }

            if !go_again {
                break;
            }
        }

        let mut data = self.data.borrow_mut();

        // do we have anything to do, even?
        if data.active.len() == 0 && data.scheduled.len() == 0 && data.timers.len() == 0 {
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
            if *start < offset {
                data.scheduled.remove(i);
                continue;
            }

            if *start < end as u64 {
                let len = (end - *start) as usize * self.channels as usize;
                let avail = src.force_fill(&mut self.buffer[..len]);
                let dest = (*start - offset) as usize * self.channels as usize;
                for j in 0..avail {
                    buffer[dest + j] = self.buffer[j];
                }

                let (_, src) = data.scheduled.remove(i);
                if avail == len {
                    data.active.push(src);
                }
            } else {
                i += 1;
            }
        }

        // apply the volume ramp
        let mut volume = data.volume;
        let mut delta;
        let mut i = 0;
        while let Some((time, vol)) = data.ramps.get(0) {
            if *time < offset {
                data.ramps.remove(0);
                continue;
            }

            delta = (vol - volume) / (time - offset - i as u64) as f32;
            while i < buffer.len() && offset as usize + i < *time as usize {
                buffer[i] *= volume;
                volume += delta;
                i += 1;
            }
            if offset as usize + i == *time as usize {
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

        data.offset = end;
        buffer.len()
    }

    fn seek(&mut self, _frame: u64) -> anyhow::Result<()> {
        anyhow::bail!("cannot seek a scheduler")
    }
}
