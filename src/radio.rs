use crate::{source, Definitions, Scheduler, Time};

use rand::seq::SliceRandom;
use std::path::PathBuf;

pub struct Radio {
    definition_paths: Vec<PathBuf>,
    definitions: Definitions,
    scheduler: Scheduler,
    soft_time: Time,
    hard_time: Time,
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
        let mut radio = Self {
            definition_paths: paths.iter().map(|p| p.as_ref().to_owned()).collect(),
            definitions: Definitions::empty(&paths[0]),
            music: scheduler.subscheduler(),
            talk: scheduler.subscheduler(),
            scheduler,
            soft_time: Time::seconds(0.0),
            hard_time: Time::seconds(0.0),
        };
        radio.reload()?;
        Ok(radio)
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.definitions = Definitions::empty(&self.definition_paths[0]);
        for path in self.definition_paths.iter() {
            self.definitions.merge(Definitions::open(path)?);
        }
        Ok(())
    }

    pub async fn schedule_soft(
        &mut self,
        mainpath: &PathBuf,
        overpath: Option<&PathBuf>,
        pre: f32,
        post: Option<f32>,
        force: bool,
    ) -> anyhow::Result<()> {
        // FIXME
        let main = source::Media::new(std::fs::File::open(&mainpath)?)?;
        let end = self
            .music
            .add(self.hard_time, main)
            .ok_or(anyhow::anyhow!("unknown sound file length"))?;
        self.hard_time = end;
        self.soft_time = end;
        self.scheduler.wait(self.hard_time).await?;
        Ok(())
    }

    pub async fn play_music(&mut self) -> anyhow::Result<()> {
        let song = self
            .definitions
            .music
            .choose(&mut rand::thread_rng())
            .map(|x| x.clone())
            .ok_or(anyhow::anyhow!("no songs to play"))?;

        println!("{} - {}", song.metadata.artist, song.metadata.title);
        self.schedule_soft(&song.path, None, 0.0, None, false).await
    }

    pub async fn play_ad(&mut self) -> anyhow::Result<()> {
        if let Some(ad) = self
            .definitions
            .ad
            .choose(&mut rand::thread_rng())
            .map(|x| x.clone())
        {
            self.schedule_soft(&ad, None, 0.0, None, true).await
        } else {
            Ok(())
        }
    }

    pub async fn play_news(&mut self) -> anyhow::Result<()> {
        if let Some(news) = self
            .definitions
            .news
            .choose(&mut rand::thread_rng())
            .map(|x| x.clone())
        {
            self.schedule_soft(&news, None, 0.0, None, true).await
        } else {
            Ok(())
        }
    }

    pub async fn play_id(&mut self) -> anyhow::Result<()> {
        if let Some(id) = self
            .definitions
            .id
            .choose(&mut rand::thread_rng())
            .map(|x| x.clone())
        {
            self.schedule_soft(&id, None, 0.0, None, false).await
        } else {
            Ok(())
        }
    }

    pub async fn play_solo(&mut self) -> anyhow::Result<()> {
        if let Some(solo) = self
            .definitions
            .solo
            .choose(&mut rand::thread_rng())
            .map(|x| x.clone())
        {
            self.schedule_soft(&solo, None, 0.0, None, false).await
        } else {
            Ok(())
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            for ad_or_news in &[true, false] {
                for _ in 0..1 {
                    self.play_music().await?;
                    if *ad_or_news {
                        self.play_ad().await?;
                    } else {
                        self.play_news().await?;
                    }
                    self.play_id().await?;
                    self.play_solo().await?;
                }
            }
        }
    }
}
