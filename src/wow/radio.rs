use std::collections::HashMap;

use crate::{Scheduler, source, Time, RandomMixer};
use super::{AmbientScheduler, Definitions, Data, Area, Soundscape, Sound};

pub struct Radio<F> {
    definitions: Definitions,
    scheduler: AmbientScheduler,
    metadata_callback: F,

    data: Data,
    areacache: HashMap<String, Area>,
    last_played: Option<String>,
    r_zones: RandomMixer<String>,
}

macro_rules! set_metadata {
    ($self:expr, $fmt:expr) =>
        (($self.metadata_callback)(
            format!(concat!("{} - ", $fmt),
                    $self.definitions.name.as_deref().unwrap_or("Sprunk"))
        ));
    ($self:expr, $fmt:expr, $($arg:tt)*) =>
        (($self.metadata_callback)(
            format!(concat!("{} - ", $fmt),
                    $self.definitions.name.as_deref().unwrap_or("Sprunk"),
                    $($arg)*))
        );
}

impl<F> Radio<F> where F: FnMut(String) {
    pub fn new<PI, P>(mut scheduler: Scheduler, paths: PI, metadata_callback: F) -> anyhow::Result<Self> where PI: Iterator<Item = P>, P: AsRef<std::path::Path> {
        Ok(Self {
            definitions: Definitions::open(paths)?,
            scheduler: AmbientScheduler::new(scheduler, Time::seconds(3.0)),
            metadata_callback,

            data: Data::new(),
            areacache: HashMap::new(),
            last_played: None,
            r_zones: RandomMixer::new(),
        })
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        self.definitions.reload()?;
        self.data.set_paths(self.definitions.archives.iter())?;

        self.areacache.clear();
        for zone in self.definitions.zones.values() {
            let area = self.data.get_zone(&zone.name, zone.parent.as_ref()).ok_or_else(|| anyhow::anyhow!("could not get zone: {:?}", zone.name))?;
            self.areacache.insert(zone.name.clone(), area);
        }

        Ok(())
    }

    pub async fn play_sound_block(&mut self, zone: &Area, sound: &Sound) -> anyhow::Result<()> {
        for path in &sound.items {
            // some intros are duplicated in the music block, so avoid that
            if self.last_played.as_ref() == Some(path) {
                continue;
            }

            let data = self.data.read_file(path)?;
            self.scheduler.add_music(sound.volume, data).await?;
            self.last_played = Some(path.clone());

            let file_name = path.rsplit_once('\\').map(|t| t.1);
            let file_stem = file_name.and_then(|name| name.rsplit_once('.').map(|t| t.0));

            if let Some(name) = file_stem {
                set_metadata!(self, "{} - {}", zone.name, name);
            } else {
                set_metadata!(self, "{}", zone.name);
            }
        }

        Ok(())
    }

    pub async fn play_zone_soundscape(&mut self, zone: &Area, soundscape: &Soundscape) -> anyhow::Result<()> {
        // set up the ambience
        if soundscape.ambience.items.is_empty() {
            self.scheduler.add_ambience(1.0, None).await?;
        } else {
            let ambience_path = &soundscape.ambience.items[0];
            let ambience_data = self.data.read_file(ambience_path)?;
            self.scheduler.add_ambience(soundscape.ambience.volume, Some(ambience_data)).await?;
        }

        // play intro music (if any)
        self.play_sound_block(zone, &zone.intro).await?;

        // play normal music
        self.play_sound_block(zone, &soundscape.music).await?;

        Ok(())
    }

    pub async fn play_zone(&mut self, name: &str) -> anyhow::Result<()> {
        let zone = self.areacache.get(name).ok_or_else(|| anyhow::anyhow!("could not get zone: {:?}", name))?.clone();
        //println!("{:#?}", zone);

        // FIXME day/night
        self.play_zone_soundscape(&zone, &zone.day).await?;

        Ok(())

    }

    pub async fn path(&mut self, start: &String, end: &String) -> anyhow::Result<()> {
        let path_outline = pathfinding::directed::dijkstra::dijkstra(&start, |name| {
            self.definitions.zones.get(name.as_str()).expect("could not get zone").connections.iter().map(|conn| (&conn.destination, if conn.via.is_some() { 2 } else { 1 }))
        }, |name| *name == end);

        let Some((path_outline, _)) = path_outline else {
            // no path found, fudge it
            self.play_zone(end).await?;
            return Ok(());
        };

        // discard start, which has already played, and clone
        let path_outline: Vec<String> = path_outline[1..].into_iter().map(|n| n.as_str().to_owned()).collect();

        let mut current = self.definitions.zones.get(start).expect("could not get zone");
        for zone in &path_outline {
            // look for a via connection and route through the via if found
            for conn in &current.connections {
                if &conn.destination == zone {
                    if let Some(via) = &conn.via {
                        self.play_zone(&via.clone()).await?;
                        break;
                    }
                }
            }

            // play the zone itself
            self.play_zone(zone).await?;

            // update the current zone
            current = self.definitions.zones.get(zone).expect("could not get zone");
        }

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.reload()?;
        //println!("{:#?}", self.definitions);

        // make sure we don't loop forever looking for a unique pair
        assert!(self.definitions.endpoints.len() >= 2);

        // start in a random zone
        let mut start = self.r_zones.choose(self.definitions.endpoints.iter(), |z| z).expect("could not get start zone").clone();
        self.play_zone(&start).await?;

        loop {
            // where are we going?

            // loop until we find an end that differs from the start
            let end = loop {
                let end = self.r_zones.choose(self.definitions.endpoints.iter(), |z| z).expect("could not get end zone").clone();
                if end != start {
                    break end;
                }
            };

            self.path(&start, &end).await?;
            start = end;

            // it is... mostly safe to ignore this error
            // it's possible to reload definitions but then fail to
            // find zones, but that should be an Err from play_zone later
            let _ = self.reload();
        }
    }
}
