use crate::{source, Definitions, Metadata, Scheduler, SoftScheduler, Source, Time};

use rand::seq::SliceRandom;

pub struct Radio {
    definition_paths: Vec<std::path::PathBuf>,
    definitions: Definitions,
    scheduler: SoftScheduler,
}

impl Radio {
    pub fn new<P>(mut scheduler: Scheduler, paths: Vec<P>) -> anyhow::Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        if paths.len() == 0 {
            anyhow::bail!("no definitions to load");
        }
        let scheduler = SoftScheduler::new(&mut scheduler, 0.5, 0.5);
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
            .add(&song.path, over, song.pre, Some(song.post), false)
            .await?;
        println!(
            "{} - {} - {}",
            self.definitions.name, song.metadata.artist, song.metadata.title
        );
        Ok(())
    }

    pub async fn play_ad(&mut self) -> anyhow::Result<()> {
        if let Some(ad) = self.definitions.ad.choose(&mut rand::thread_rng()) {
            let over = self.definitions.to_ad.choose(&mut rand::thread_rng());
            self.scheduler.add(&ad, over, 0.0, None, true).await?;
            println!("{} - Advertisement", self.definitions.name);
        }
        Ok(())
    }

    pub async fn play_news(&mut self) -> anyhow::Result<()> {
        if let Some(news) = self.definitions.news.choose(&mut rand::thread_rng()) {
            let over = self.definitions.to_news.choose(&mut rand::thread_rng());
            self.scheduler.add(&news, over, 0.0, None, true).await?;
            println!("{} - News", self.definitions.name);
        }
        Ok(())
    }

    pub async fn play_id(&mut self) -> anyhow::Result<()> {
        if let Some(id) = self.definitions.id.choose(&mut rand::thread_rng()) {
            self.scheduler.add(&id, None, 0.0, None, false).await?;
            println!("{} - Identification", self.definitions.name);
        }
        Ok(())
    }

    pub async fn play_mono(&mut self) -> anyhow::Result<()> {
        if let Some(solo) = self.definitions.solo.choose(&mut rand::thread_rng()) {
            self.scheduler.add(&solo, None, 0.0, None, false).await?;
            println!("{} - Monologue", self.definitions.name);
        }
        Ok(())
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
