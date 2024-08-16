use crate::{source, Scheduler, Source, Time};

use std::path::PathBuf;

pub struct AmbientScheduler {
    crossfade: Time,

    root: Scheduler,
    music: Scheduler,
    ambience: [Scheduler; 2],
    active_ambience: usize,
    ambience_source: Option<(f32, Vec<u8>)>,

    music_end: Time,
    ambience_end: Time,
}

impl AmbientScheduler {
    pub fn new(mut root: Scheduler, crossfade: Time) -> Self {
        Self {
            crossfade,

            music: root.subscheduler(),
            ambience: array_init::array_init(|_| root.subscheduler_with_volume(0.0)),
            root: root,
            active_ambience: 0,
            ambience_source: None,

            music_end: Time::seconds(0.0),
            ambience_end: Time::seconds(0.0),
        }
    }

    fn load_media(&self, volume: f32, data: Vec<u8>) -> anyhow::Result<source::Volume<source::Media>> {
        Ok(source::Media::new(std::io::Cursor::new(data))?.volume(volume))
    }

    fn load_ambience(&self) -> anyhow::Result<Option<source::Volume<source::Media>>> {
        if let Some((volume, data)) = &self.ambience_source {
            Ok(Some(self.load_media(*volume, data.clone())?))
        } else {
            Ok(None)
        }
    }

    pub async fn add_music(&mut self, volume: f32, data: Vec<u8>) -> anyhow::Result<()> {
        let source = self.load_media(volume, data)?;
        let start = self.music_end;
        let end = self.music.add(start, source).ok_or_else(|| anyhow::anyhow!("unknown sound file length"))?;

        self.music.wait(start).await?;
        self.music_end = end;

        if self.ambience_source.is_some() {
            while self.ambience_end.to_frames(self.root.samplerate()) < self.music_end.to_frames(self.root.samplerate()) {
                let ambience_source = self.load_ambience()?.unwrap();
                self.ambience_end = self.ambience[self.active_ambience].add(self.ambience_end, ambience_source).ok_or_else(|| anyhow::anyhow!("unknown sound file length"))?;
            }
        }
        
        Ok(())
    }

    pub async fn add_ambience(&mut self, volume: f32, data: Option<Vec<u8>>) -> anyhow::Result<()> {
        let source = if let Some(data) = &data {
            Some(self.load_media(volume, data.clone())?)
        } else {
            None
        };

        let new_active_ambience = (self.active_ambience + 1) % self.ambience.len();
        let mut old_ambience = &mut self.ambience[self.active_ambience];
        let mut new_ambience = self.root.subscheduler_with_volume(0.0);

        old_ambience.set_volume(self.music_end, 0.0, self.crossfade);
        new_ambience.set_volume(self.music_end, 1.0, self.crossfade);

        let end = if let Some(source) = source {
            new_ambience.add(self.music_end, source).ok_or_else(|| anyhow::anyhow!("unknown sound file length"))?
        } else {
            self.ambience_end
        };

        self.ambience[new_active_ambience] = new_ambience;
        self.active_ambience = new_active_ambience;
        self.ambience_end = end;
        self.ambience_source = data.map(|d| (volume, d));

        Ok(())
    }
}
