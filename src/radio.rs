use crate::{Definitions, RandomMixer, Scheduler, SoftScheduler};

use rand::Rng;
use std::path::PathBuf;

pub struct Radio {
    definitions: Definitions,
    scheduler: SoftScheduler,

    // some fun parameters
    intro_chance: f32,

    // our shufflers
    r_music: RandomMixer<PathBuf>,
    r_general: RandomMixer<PathBuf>,
    r_intro: RandomMixer<PathBuf>,
    r_time_morning: RandomMixer<PathBuf>,
    r_time_evening: RandomMixer<PathBuf>,
    r_ad: RandomMixer<PathBuf>,
    r_to_ad: RandomMixer<PathBuf>,
    r_news: RandomMixer<PathBuf>,
    r_to_news: RandomMixer<PathBuf>,
    r_id: RandomMixer<PathBuf>,
    r_solo: RandomMixer<PathBuf>,
}

impl Radio {
    pub fn new<PI, P>(mut scheduler: Scheduler, paths: PI) -> anyhow::Result<Self>
    where
        PI: Iterator<Item = P>,
        P: AsRef<std::path::Path>,
    {
        // parameters: padding and over_volume
        let scheduler = SoftScheduler::new(&mut scheduler, 0.5, 0.5);

        Ok(Self {
            definitions: Definitions::open(paths)?,
            scheduler,

            // parameters
            intro_chance: 0.3,

            r_music: RandomMixer::new(),
            r_general: RandomMixer::new(),
            r_intro: RandomMixer::new(),
            r_time_morning: RandomMixer::new(),
            r_time_evening: RandomMixer::new(),
            r_ad: RandomMixer::new(),
            r_to_ad: RandomMixer::new(),
            r_news: RandomMixer::new(),
            r_to_news: RandomMixer::new(),
            r_id: RandomMixer::new(),
            r_solo: RandomMixer::new(),
        })
    }

    pub async fn play_music(&mut self) -> anyhow::Result<()> {
        let song = self
            .r_music
            .choose(self.definitions.music.iter(), |s| &s.path)
            .ok_or_else(|| anyhow::anyhow!("no songs to play"))?;

        let mut rng = rand::thread_rng();
        let mut over = None;
        if rng.gen::<f32>() < self.intro_chance {
            // we *will* have an intro, but which one!
            let mut choices = Vec::with_capacity(3);

            // general choices are always available
            if let Some(p) = self.r_general.possibility(self.definitions.general.iter()) {
                choices.push(p);
            }

            // what about time-based?
            use chrono::Timelike;
            let now = chrono::offset::Local::now();
            if now.hour() >= 4 && now.hour() < 12 {
                if let Some(p) = self
                    .r_time_morning
                    .possibility(self.definitions.time_morning.iter())
                {
                    choices.push(p);
                }
            } else if now.hour() >= 17 && now.hour() < 24 {
                if let Some(p) = self
                    .r_time_evening
                    .possibility(self.definitions.time_evening.iter())
                {
                    choices.push(p);
                }
            }

            // what about a song-specific intro?
            if let Some(p) = self
                .r_intro
                .possibility(self.definitions.get_intros(&song.metadata))
            {
                choices.push(p.map(|i| &i.path));
            }

            // choose one of these!
            if choices.len() > 0 {
                let possibility = choices.remove(rng.gen_range(0..choices.len()));
                over = Some(possibility.accept(|p| p));
            }
        }

        self.scheduler
            .add(&song.path, over, song.pre, Some(song.post), false)
            .await?;
        println!(
            "{} - {} - {}",
            self.definitions.name, song.metadata.artist, song.metadata.title
        );
        Ok(())
    }

    pub async fn play_ad(&mut self) -> anyhow::Result<()> {
        if let Some(ad) = self.r_ad.choose(self.definitions.ad.iter(), |p| p) {
            let over = self.r_to_ad.choose(self.definitions.to_ad.iter(), |p| p);
            self.scheduler.add(&ad, over, 0.0, None, true).await?;
            println!("{} - Advertisement", self.definitions.name);
        }
        Ok(())
    }

    pub async fn play_news(&mut self) -> anyhow::Result<()> {
        if let Some(news) = self.r_news.choose(self.definitions.news.iter(), |p| p) {
            let over = self
                .r_to_news
                .choose(self.definitions.to_news.iter(), |p| p);
            self.scheduler.add(&news, over, 0.0, None, true).await?;
            println!("{} - News", self.definitions.name);
        }
        Ok(())
    }

    pub async fn play_id(&mut self) -> anyhow::Result<()> {
        if let Some(id) = self.r_id.choose(self.definitions.id.iter(), |p| p) {
            self.scheduler.add(&id, None, 0.0, None, false).await?;
            println!("{} - Identification", self.definitions.name);
        }
        Ok(())
    }

    pub async fn play_mono(&mut self) -> anyhow::Result<()> {
        if let Some(solo) = self.r_solo.choose(self.definitions.solo.iter(), |p| p) {
            self.scheduler.add(&solo, None, 0.0, None, false).await?;
            println!("{} - Monologue", self.definitions.name);
        }
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            for ad_or_news in &[true, false] {
                // reload failures can be ignored safely
                // maaaaybe it should be logged. but it's fine.
                let _ = self.definitions.reload();

                for _ in 0..12 {
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
