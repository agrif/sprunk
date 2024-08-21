use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use wow_dbc::wrath_tables as tables;
use wow_dbc::{DbcTable, Indexable};

pub struct Data {
    paths: Vec<PathBuf>,
    archives: HashMap<PathBuf, mpq::Archive>,
    areas: tables::area_table::AreaTable,
    sounds: tables::sound_entries::SoundEntries,
    ambiences: tables::sound_ambience::SoundAmbience,
    musics: tables::zone_music::ZoneMusic,
    intro_musics: tables::zone_intro_music_table::ZoneIntroMusicTable,
}

#[derive(Clone, Debug)]
pub struct Sound<M = String> {
    pub volume: f32,
    pub items: Vec<M>,
}

impl<M> Sound<M> {
    pub fn map<F, N, E>(self, f: F) -> Result<Sound<N>, E>
    where
        F: FnMut(M) -> Result<N, E>,
    {
        let items: Result<Vec<N>, E> = self.items.into_iter().map(f).collect();
        Ok(Sound {
            volume: self.volume,
            items: items?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Soundscape<M = String> {
    pub ambience: Sound<M>,
    pub music: Sound<M>,
}

impl<M> Soundscape<M> {
    pub fn map<F, N, E>(self, mut f: F) -> Result<Soundscape<N>, E>
    where
        F: FnMut(M) -> Result<N, E>,
    {
        Ok(Soundscape {
            ambience: self.ambience.map(&mut f)?,
            music: self.music.map(&mut f)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Area<M = String> {
    pub name: String,
    pub parent: Option<Box<Area<M>>>,
    pub intro: Sound<M>,

    pub day: Soundscape<M>,
    pub night: Soundscape<M>,
}

impl<M> Area<M> {
    pub fn map<F, N, E>(self, mut f: F) -> Result<Area<N>, E>
    where
        F: FnMut(M) -> Result<N, E>,
    {
        Ok(Area {
            name: self.name,
            parent: self
                .parent
                .map(|p| p.map(&mut f).map(Box::new))
                .transpose()?,
            intro: self.intro.map(&mut f)?,
            day: self.day.map(&mut f)?,
            night: self.night.map(&mut f)?,
        })
    }
}

impl Data {
    pub fn new() -> Self {
        Self {
            paths: vec![],
            archives: HashMap::new(),
            areas: tables::area_table::AreaTable { rows: vec![] },
            sounds: tables::sound_entries::SoundEntries { rows: vec![] },
            ambiences: tables::sound_ambience::SoundAmbience { rows: vec![] },
            musics: tables::zone_music::ZoneMusic { rows: vec![] },
            intro_musics: tables::zone_intro_music_table::ZoneIntroMusicTable { rows: vec![] },
        }
    }
    pub fn set_paths_from_dir<P1, P2>(&mut self, path: P1, locale: P2) -> anyhow::Result<Self>
    where
        P1: AsRef<std::path::Path>,
        P2: AsRef<std::path::Path>,
    {
        let mut paths = vec![];

        // FIXME this does need to be in order, probably
        // but directory order is fine for now
        fn load_dir(paths: &mut Vec<PathBuf>, path: &std::path::Path) -> anyhow::Result<()> {
            for leaf in std::fs::read_dir(path)? {
                let leaf = leaf?.path();

                if leaf.extension() == Some(std::ffi::OsStr::new("MPQ")) {
                    paths.push(leaf);
                }
            }

            Ok(())
        }

        load_dir(&mut paths, path.as_ref())?;
        load_dir(&mut paths, &path.as_ref().join(locale))?;

        let mut new = Self::new();
        new.set_paths(paths.iter())?;
        Ok(new)
    }

    pub fn set_paths<P>(&mut self, paths: impl Iterator<Item = P>) -> anyhow::Result<()>
    where
        P: AsRef<Path>,
    {
        let paths: Vec<PathBuf> = paths.map(|p| p.as_ref().to_owned()).collect();
        let to_add: Vec<PathBuf> = paths
            .iter()
            .filter(|p| !self.archives.contains_key(p.as_path()))
            .cloned()
            .collect();

        let to_remove: Vec<PathBuf> = self
            .archives
            .keys()
            .filter(|p| !paths.contains(p))
            .cloned()
            .collect();

        let add_archives = to_add
            .as_slice()
            .into_par_iter()
            .map(|path| {
                println!("loading {}", path.display());
                let archive = mpq::Archive::open(path)?;
                Ok((path.clone(), archive))
            })
            .collect::<anyhow::Result<HashMap<PathBuf, mpq::Archive>>>()?;

        for remove in &to_remove {
            self.archives.remove(remove).expect("archive not removed");
        }

        self.archives.extend(add_archives);

        let current_paths: HashSet<&PathBuf> = self.archives.keys().collect();
        let expected_paths: HashSet<&PathBuf> = paths.iter().collect();
        assert_eq!(expected_paths, current_paths);

        self.paths = paths;
        self.reload()?;

        Ok(())
    }

    fn reload(&mut self) -> anyhow::Result<()> {
        self.areas = self.read_table()?;
        self.sounds = self.read_table()?;
        self.ambiences = self.read_table()?;
        self.musics = self.read_table()?;
        self.intro_musics = self.read_table()?;

        Ok(())
    }

    fn read_table<T>(&mut self) -> anyhow::Result<T>
    where
        T: DbcTable,
    {
        let path = format!("DBFilesClient/{}", T::FILENAME);
        let bytes = self.read_file(&path)?;
        let table = T::read(&mut bytes.as_slice())?;
        Ok(table)
    }

    fn parse_sound(
        &self,
        sound: Option<&tables::sound_entries::SoundEntriesRow>,
        parent: Option<&Sound<String>>,
    ) -> Sound<String> {
        if let Some(s) = sound {
            Sound {
                volume: s.volume_float,
                items: s
                    .file
                    .iter()
                    .filter(|f| f.len() > 0)
                    .map(|f| format!("{}\\{}", s.directory_base, f))
                    .collect(),
            }
        } else {
            if let Some(s) = parent {
                s.clone()
            } else {
                Sound {
                    volume: 0.0,
                    items: Vec::new(),
                }
            }
        }
    }

    fn parse_soundscape(
        &self,
        idx: usize,
        ambience_info: Option<&tables::sound_ambience::SoundAmbienceRow>,
        music_info: Option<&tables::zone_music::ZoneMusicRow>,
        parent: Option<&Soundscape<String>>,
    ) -> Soundscape<String> {
        let ambience = self.parse_sound(
            ambience_info.and_then(|i| self.sounds.get(i.ambience_id[idx])),
            parent.map(|s| &s.ambience),
        );

        let music = self.parse_sound(
            music_info.and_then(|i| self.sounds.get(i.sounds[idx])),
            parent.map(|s| &s.music),
        );

        Soundscape { ambience, music }
    }

    fn parse_area(&self, area: &tables::area_table::AreaTableRow) -> Area<String> {
        let parent = self.get_zone_by_id(area.parent_area_id);
        let name = area.area_name_lang.en_gb.clone();

        let intro = self.parse_sound(
            self.intro_musics
                .get(area.intro_sound)
                .and_then(|s| self.sounds.get(s.sound_id)),
            // intros do not get inherited
            None,
        );

        let ambience = self.ambiences.get(area.ambience_id);
        let music = self.musics.get(area.zone_music);

        let day = self.parse_soundscape(0, ambience, music, parent.as_ref().map(|a| &a.day));
        let night = self.parse_soundscape(1, ambience, music, parent.as_ref().map(|a| &a.night));

        Area {
            name,
            parent: parent.map(Box::new),
            intro,
            day,
            night,
        }
    }

    pub fn list_zones<'a>(&'a self) -> impl Iterator<Item = Area<String>> + 'a {
        self.areas.rows().iter().map(|area| self.parse_area(area))
    }

    pub fn get_zone_by_id(
        &self,
        key: impl TryInto<tables::area_table::AreaTableKey>,
    ) -> Option<Area> {
        self.areas.get(key).map(|area| self.parse_area(area))
    }

    pub fn get_zone<S1, S2>(&self, name: S1, parent: Option<S2>) -> Option<Area>
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        for area in self.areas.rows() {
            if area.area_name_lang.en_gb == name.as_ref() {
                let area = self.parse_area(area);
                if area.parent.as_ref().map(|p| p.name.as_str())
                    == parent.as_ref().map(|s| s.as_ref())
                {
                    return Some(area);
                }
            }
        }

        None
    }

    pub fn read_file<S>(&mut self, path: S) -> anyhow::Result<Vec<u8>>
    where
        S: AsRef<str>,
    {
        for archive_path in self.paths.iter().rev() {
            let archive = self
                .archives
                .get_mut(archive_path)
                .expect("archive not loaded");

            if let Ok(file) = archive.open_file(path.as_ref()) {
                //println!("loaded {} from {}", path.as_ref(), archive_path.display());
                let mut buf = vec![0; file.size() as usize];
                file.read(archive, &mut buf)?;
                return Ok(buf);
            }
        }

        Err(anyhow::anyhow!("file not found in archives"))
    }
}
