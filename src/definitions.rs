use std::path::{Path, PathBuf};

use strict_yaml_rust::{StrictYaml, StrictYamlLoader};

use crate::normalize::normalize;

#[derive(Debug, Clone)]
pub struct Definitions {
    pub paths: Vec<PathBuf>,
    pub name: Option<String>,
    pub solo: Vec<PathBuf>,
    pub general: Vec<PathBuf>,
    pub to_ad: Vec<PathBuf>,
    pub to_news: Vec<PathBuf>,
    pub time_evening: Vec<PathBuf>,
    pub time_morning: Vec<PathBuf>,
    pub id: Vec<PathBuf>,
    pub ad: Vec<PathBuf>,
    pub news: Vec<PathBuf>,
    pub intro: Vec<Intro>,
    pub music: Vec<Song>,
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Intro {
    pub path: PathBuf,
    pub metadata: Metadata,
}

#[derive(Debug, Clone)]
pub struct Song {
    pub path: PathBuf,
    pub metadata: Metadata,
    pub pre: f32,
    pub post: f32,
}

impl Definitions {
    pub fn open<PI, P>(paths: PI) -> anyhow::Result<Self>
    where
        PI: Iterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut defs = Definitions::empty();
        defs.paths = paths.map(|p| p.as_ref().to_owned()).collect();
        defs.reload()?;
        Ok(defs)
    }

    pub fn empty() -> Self {
        Definitions {
            paths: vec![],
            name: None,
            solo: vec![],
            general: vec![],
            to_ad: vec![],
            to_news: vec![],
            time_evening: vec![],
            time_morning: vec![],
            id: vec![],
            ad: vec![],
            news: vec![],
            intro: vec![],
            music: vec![],
        }
    }

    pub fn reload(&mut self) -> anyhow::Result<()> {
        let mut new = Definitions::empty();
        for path in self.paths.iter() {
            new.merge(Definitions::load_one(path)?);
        }
        std::mem::swap(&mut new.paths, &mut self.paths);
        *self = new;
        Ok(())
    }

    fn load_one(path: &PathBuf) -> anyhow::Result<Self> {
        let mut new = Definitions::empty();
        let base = path.parent().unwrap_or(Path::new("."));
        let contents = std::fs::read_to_string(&path)?;
        let datawhole = StrictYamlLoader::load_from_str(&contents)?;
        let data = datawhole
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("could not get definition document"))?;
        Self::check_keys(
            data,
            &[
                "name",
                "prefix",
                "include",
                "solo",
                "general",
                "to-ad",
                "to-news",
                "time-evening",
                "time-morning",
                "id",
                "ad",
                "news",
                "intro",
                "music",
            ],
        )?;

        let prefix = base.join(Self::get_str(data, "prefix")?.unwrap_or("."));

        // read and merge includes first
        if let Some(includes) = Self::get_vec(data, "include")? {
            for inc in includes.iter() {
                let inc = inc
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("includes must be strings"))?;
                let inc = normalize(&prefix.join(inc));
                new.merge(Definitions::load_one(&inc)?);
            }
        }

        // read the radio name
        if let Some(name) = Self::get_str(data, "name")? {
            new.name = Some(name.to_owned());
        }

        // read in simple path lists
        new.solo.extend(Self::get_path_vec(data, "solo", &prefix)?);
        new.general
            .extend(Self::get_path_vec(data, "general", &prefix)?);
        new.to_ad
            .extend(Self::get_path_vec(data, "to-ad", &prefix)?);
        new.to_news
            .extend(Self::get_path_vec(data, "to-news", &prefix)?);
        new.time_evening
            .extend(Self::get_path_vec(data, "time-evening", &prefix)?);
        new.time_morning
            .extend(Self::get_path_vec(data, "time-morning", &prefix)?);
        new.id.extend(Self::get_path_vec(data, "id", &prefix)?);
        new.ad.extend(Self::get_path_vec(data, "ad", &prefix)?);
        new.news.extend(Self::get_path_vec(data, "news", &prefix)?);

        // read intros
        if let Some(intros) = Self::get_vec(data, "intro")? {
            new.intro.reserve(intros.len());
            for intro in intros.iter() {
                Self::check_keys(intro, &["path", "title", "artist", "album"])?;
                let path = Self::get_str(intro, "path")?
                    .ok_or_else(|| anyhow::anyhow!("song requires path"))?;
                let path = Self::verify_media(&prefix.join(path))?;
                let title = Self::get_str(intro, "title")?
                    .ok_or_else(|| anyhow::anyhow!("song requires title"))?
                    .to_owned();
                let artist = Self::get_str(intro, "artist")?
                    .ok_or_else(|| anyhow::anyhow!("song requires artist"))?
                    .to_owned();
                let album = Self::get_str(intro, "album")?.map(|s| s.to_owned());
                new.intro.push(Intro {
                    path,
                    metadata: Metadata {
                        title,
                        artist,
                        album,
                    },
                })
            }
        }

        // read music
        if let Some(songs) = Self::get_vec(data, "music")? {
            new.music.reserve(songs.len());
            for song in songs.iter() {
                Self::check_keys(song, &["path", "title", "artist", "album", "pre", "post"])?;
                let path = Self::get_str(song, "path")?
                    .ok_or_else(|| anyhow::anyhow!("song requires path"))?;
                let path = Self::verify_media(&prefix.join(path))?;
                let title = Self::get_str(song, "title")?
                    .ok_or_else(|| anyhow::anyhow!("song requires title"))?
                    .to_owned();
                let artist = Self::get_str(song, "artist")?
                    .ok_or_else(|| anyhow::anyhow!("song requires artist"))?
                    .to_owned();
                let album = Self::get_str(song, "album")?.map(|s| s.to_owned());
                let pre = Self::get_str(song, "pre")?
                    .ok_or_else(|| anyhow::anyhow!("song requires pre"))?;
                let post = Self::get_str(song, "post")?
                    .ok_or_else(|| anyhow::anyhow!("song requires post"))?;
                new.music.push(Song {
                    path,
                    metadata: Metadata {
                        title,
                        artist,
                        album,
                    },
                    pre: Self::parse_time(pre)?,
                    post: Self::parse_time(post)?,
                })
            }
        }

        Ok(new)
    }

    pub fn merge(&mut self, other: Definitions) {
        if other.name.is_some() {
            self.name = other.name;
        }
        self.solo.extend(other.solo);
        self.general.extend(other.general);
        self.to_ad.extend(other.to_ad);
        self.to_news.extend(other.to_news);
        self.time_evening.extend(other.time_evening);
        self.time_morning.extend(other.time_morning);
        self.id.extend(other.id);
        self.ad.extend(other.ad);
        self.news.extend(other.news);
        self.intro.extend(other.intro);
        self.music.extend(other.music);
    }

    pub fn verify(&self) -> anyhow::Result<()> {
        // make sure each intro matches a song
        'intro: for intro in self.intro.iter() {
            for song in self.music.iter() {
                if Self::meta_match(&intro.metadata, &song.metadata) {
                    continue 'intro;
                }
            }
            anyhow::bail!("intro does not match any song: {:?}", intro.path);
        }
        Ok(())
    }

    fn meta_match(a: &Metadata, b: &Metadata) -> bool {
        if let Some(ref aa) = a.album {
            if let Some(ref ab) = b.album {
                if aa != ab {
                    return false;
                }
            }
        }
        a.artist == b.artist && a.title == b.title
    }

    pub fn get_intros<'a>(&'a self, meta: &'a Metadata) -> impl Iterator<Item = &'a Intro> {
        self.intro
            .iter()
            .filter(move |i| Self::meta_match(&i.metadata, meta))
    }

    fn get_str<'a>(data: &'a StrictYaml, k: &str) -> anyhow::Result<Option<&'a str>> {
        let v = if data[k].is_badvalue() {
            Some(None)
        } else {
            data[k].as_str().map(Some)
        };
        v.ok_or_else(|| anyhow::anyhow!("bad value for {:?}, expected string", k))
    }

    fn get_vec<'a>(data: &'a StrictYaml, k: &str) -> anyhow::Result<Option<&'a Vec<StrictYaml>>> {
        let v = if data[k].is_badvalue() {
            Some(None)
        } else {
            data[k].as_vec().map(Some)
        };
        v.ok_or_else(|| anyhow::anyhow!("bad value for {:?}, expected list", k))
    }

    fn get_path_vec(data: &StrictYaml, k: &str, prefix: &Path) -> anyhow::Result<Vec<PathBuf>> {
        if let Some(paths) = Self::get_vec(data, k)? {
            let mut ret = Vec::with_capacity(paths.len());
            for p in paths.iter() {
                let p = p
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("bad path in {:?}, expected string", k))?;
                ret.push(Self::verify_media(&prefix.join(Path::new(p)))?);
            }
            Ok(ret)
        } else {
            Ok(vec![])
        }
    }

    fn verify_media(path: &Path) -> anyhow::Result<PathBuf> {
        let mut mutpath = normalize(path);
        let exts = &["flac", "wav", "ogg"];
        for ext in exts {
            mutpath.set_extension(ext);
            if mutpath.exists() {
                return Ok(mutpath);
            }
        }
        anyhow::bail!("file does not exist: {:?} (tried: {:?})", path, exts);
    }

    fn parse_time(time: &str) -> anyhow::Result<f32> {
        let mut r = 0.0;
        for part in time.split(":") {
            r *= 60.0;
            r += part
                .parse::<f32>()
                .map_err(|_| anyhow::anyhow!("bad timestamp: {:?}", time))?;
        }
        Ok(r)
    }

    fn check_keys(data: &StrictYaml, keys: &[&str]) -> anyhow::Result<()> {
        let hash = data
            .as_hash()
            .ok_or_else(|| anyhow::anyhow!("expected yaml dictionary"))?;
        for k in hash.keys() {
            if let Some(k) = k.as_str() {
                if !keys.contains(&k) {
                    anyhow::bail!("unknown key {:?}", k);
                }
            } else {
                anyhow::bail!("unknown key {:?}", k);
            }
        }
        Ok(())
    }
}
