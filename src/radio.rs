use crate::{source, Definitions, Metadata, Scheduler, Source, Time};

use rand::seq::SliceRandom;
use std::path::PathBuf;

pub struct Radio {
    definition_paths: Vec<PathBuf>,
    definitions: Definitions,
    scheduler: TalkScheduler,
}

#[derive(Debug)]
enum NowPlaying<'a> {
    Ad,
    News,
    Mono,
    Id,
    Song(&'a Metadata),
}

struct TalkScheduler {
    name: String,
    padding: f32,
    over_volume: f32,
    soft: Time,
    hard: Time,
    root: Scheduler,
    music: Scheduler,
    talk: Scheduler,
}

impl Radio {
    pub fn new<P>(mut scheduler: Scheduler, paths: Vec<P>) -> anyhow::Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        if paths.len() == 0 {
            anyhow::bail!("no definitions to load");
        }
        let scheduler = TalkScheduler {
            name: "".to_owned(),
            padding: 0.5,
            over_volume: 0.5,
            soft: Time::seconds(0.0),
            hard: Time::seconds(0.0),
            music: scheduler.subscheduler(),
            talk: scheduler.subscheduler(),
            root: scheduler,
        };
        let mut radio = Self {
            definition_paths: paths.iter().map(|p| p.as_ref().to_owned()).collect(),
            definitions: Definitions::empty(&paths[0]),
            scheduler,
        };
        radio.reload()?;
        Ok(radio)
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.definitions = Definitions::empty(&self.definition_paths[0]);
        for path in self.definition_paths.iter() {
            self.definitions.merge(Definitions::open(path)?);
        }
        self.scheduler.name = self.definitions.name.clone();
        Ok(())
    }

    pub async fn play_music(&mut self) -> anyhow::Result<()> {
        let song = self
            .definitions
            .music
            .choose(&mut rand::thread_rng())
            .ok_or(anyhow::anyhow!("no songs to play"))?;
        let over = self.definitions.general.choose(&mut rand::thread_rng());

        self.scheduler
            .add(
                &song.path,
                over,
                NowPlaying::Song(&song.metadata),
                song.pre,
                Some(song.post),
                false,
            )
            .await
    }

    pub async fn play_ad(&mut self) -> anyhow::Result<()> {
        if let Some(ad) = self.definitions.ad.choose(&mut rand::thread_rng()) {
            let over = self.definitions.to_ad.choose(&mut rand::thread_rng());
            self.scheduler
                .add(&ad, over, NowPlaying::Ad, 0.0, None, true)
                .await
        } else {
            Ok(())
        }
    }

    pub async fn play_news(&mut self) -> anyhow::Result<()> {
        if let Some(news) = self.definitions.news.choose(&mut rand::thread_rng()) {
            let over = self.definitions.to_news.choose(&mut rand::thread_rng());
            self.scheduler
                .add(&news, over, NowPlaying::News, 0.0, None, true)
                .await
        } else {
            Ok(())
        }
    }

    pub async fn play_id(&mut self) -> anyhow::Result<()> {
        if let Some(id) = self.definitions.id.choose(&mut rand::thread_rng()) {
            self.scheduler
                .add(&id, None, NowPlaying::Id, 0.0, None, false)
                .await
        } else {
            Ok(())
        }
    }

    pub async fn play_mono(&mut self) -> anyhow::Result<()> {
        if let Some(solo) = self.definitions.solo.choose(&mut rand::thread_rng()) {
            self.scheduler
                .add(&solo, None, NowPlaying::Mono, 0.0, None, false)
                .await
        } else {
            Ok(())
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            for ad_or_news in &[true, false] {
                for _ in 0..1 {
                    self.play_music().await?;
                }
                if *ad_or_news {
                    self.play_ad().await?;
                } else {
                    self.play_news().await?;
                }
                self.play_id().await?;
                self.play_mono().await?;
            }
        }
    }
}

impl TalkScheduler {
    pub async fn add<'a>(
        &mut self,
        mainpath: &PathBuf,
        overpath: Option<&PathBuf>,
        meta: NowPlaying<'a>,
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
            let soft_amt = (soft_end - self.soft).to_seconds(self.root.samplerate());
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
                    self.music
                        .set_volume(over_start, self.over_volume, self.padding);
                    self.music
                        .set_volume(soft_end - self.padding, 1.0, self.padding);
                    self.talk.add(over_start + self.padding, over);
                }
            }
        }

        // schedule the main showpiece
        let end = self
            .music
            .add(start, main)
            .ok_or(anyhow::anyhow!("unknown sound file length"))?;

        // wait until the start, then set the metadata
        self.root.wait(start).await?;
        match meta {
            NowPlaying::Ad => println!("{} - Advertisement", self.name),
            NowPlaying::News => println!("{} - News", self.name),
            NowPlaying::Mono => println!("{} - Monologue", self.name),
            NowPlaying::Id => println!("{} - Identification", self.name),
            NowPlaying::Song(m) => println!("{} - {} - {}", self.name, m.artist, m.title),
        }

        // update our soft and hard start times
        self.soft = if let Some(p) = post { start + p } else { end };
        self.hard = end + self.padding;
        Ok(())
    }
}
