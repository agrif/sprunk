use crate::{source, Scheduler, Time, Source};

use std::path::PathBuf;

pub struct SoftScheduler {
    padding: f32,
    over_volume: f32,
    soft: Time,
    hard: Time,
    main: Scheduler,
    over: Scheduler,
}

impl SoftScheduler {
    pub fn new(root: &mut Scheduler, padding: f32, over_volume: f32) -> Self {
        Self {
            padding,
            over_volume,
            soft: Time::seconds(0.0),
            hard: Time::seconds(0.0),
            main: root.subscheduler(),
            over: root.subscheduler(),
        }
    }
    pub async fn add<'a>(
        &mut self,
        mainpath: &PathBuf,
        overpath: Option<&PathBuf>,
        pre: f32,
        post: Option<f32>,
        force: bool,
    ) -> anyhow::Result<()> {
        let main = source::Media::new(std::fs::File::open(&mainpath)?)?;
        let mut start = self.hard;

        // do we have a voiceover to do?
        if let Some(overpath) = overpath {
            let over = source::Media::new(std::fs::File::open(overpath)?)?;
            // figure out when our soft time ends, and how long it is
            let mut soft_end = start + pre;
            let soft_amt = (soft_end - self.soft).to_seconds(self.over.samplerate());
            if let Some(over_frames) = over.len() {
                // we have a voiceover with a known length. will it fit?
                let over_amt = over_frames as f32 / over.samplerate() + 2.0 * self.padding;
                if over_amt < soft_amt || force {
                    // it either fits, or we'll make it fit
                    let bonus = over_amt - soft_amt;
                    if bonus > 0.0 {
                        // adjust both the hard music start and soft end
                        start = start + bonus;
                        soft_end = soft_end + bonus;
                    }
                    // schedule the voiceover and volume ramps
                    let over_start = soft_end - over_amt;
                    self.main
                        .set_volume(over_start, self.over_volume, self.padding);
                    self.main
                        .set_volume(soft_end - self.padding, 1.0, self.padding);
                    self.over.add(over_start + self.padding, over);
                }
            }
        }

        // schedule the main showpiece
        let end = self
            .main
            .add(start, main)
            .ok_or(anyhow::anyhow!("unknown sound file length"))?;

        // wait until the start
        self.main.wait(start).await?;

        // update our soft and hard start times
        self.soft = if let Some(p) = post { start + p } else { end };
        self.hard = end + self.padding;
        Ok(())
    }
}
