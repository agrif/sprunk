use wow_dbc::wrath_tables as tables;
use wow_dbc::{DbcTable, Indexable};

trait DbcTableExt: wow_dbc::DbcTable {
    fn read_from(data: &mut mpq::Chain) -> anyhow::Result<Self> {
        let path = format!("DBFilesClient/{}", Self::FILENAME);
        let bytes = data.read(&path)?;
        let table = Self::read(&mut bytes.as_slice())?;
        Ok(table)
    }
}

impl<T> DbcTableExt for T where T: wow_dbc::DbcTable {}

pub struct Data {
    chain: mpq::Chain,
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
    pub fn new<P1, P2>(path: P1, locale: P2) -> anyhow::Result<Self>
    where
        P1: AsRef<std::path::Path>,
        P2: AsRef<std::path::Path>,
    {
        let mut chain = mpq::Chain::new();

        // FIXME this does need to be in order, probably
        // but directory order is fine for now
        fn load_dir(chain: &mut mpq::Chain, path: &std::path::Path) -> anyhow::Result<()> {
            for leaf in std::fs::read_dir(path)? {
                let leaf = leaf?.path();

                if leaf.extension() == Some(std::ffi::OsStr::new("MPQ")) {
                    chain.add(leaf)?;
                }
            }

            Ok(())
        }

        load_dir(&mut chain, path.as_ref())?;
        load_dir(&mut chain, &path.as_ref().join(locale))?;

        Self::new_from_chain(chain)
    }

    pub fn new_from_chain(mut chain: mpq::Chain) -> anyhow::Result<Self> {
        Ok(Self {
            areas: DbcTableExt::read_from(&mut chain)?,
            sounds: DbcTableExt::read_from(&mut chain)?,
            ambiences: DbcTableExt::read_from(&mut chain)?,
            musics: DbcTableExt::read_from(&mut chain)?,
            intro_musics: DbcTableExt::read_from(&mut chain)?,
            chain,
        })
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

    pub fn get_zone<S>(&self, name: S) -> Option<Area>
    where
        S: AsRef<str>,
    {
        for area in self.areas.rows() {
            if area.area_name_lang.en_gb == name.as_ref() {
                return Some(self.parse_area(area));
            }
        }

        None
    }

    pub fn read_file<S>(&mut self, path: S) -> anyhow::Result<Vec<u8>>
    where
        S: AsRef<str>,
    {
        Ok(self.chain.read(path.as_ref())?)
    }
}
