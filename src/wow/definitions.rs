use std::path::{Path, PathBuf};

use strict_yaml_rust::{StrictYaml, StrictYamlLoader};

use crate::normalize::normalize;

#[derive(Debug, Clone)]
pub struct Definitions {
    pub paths: Vec<PathBuf>,
    pub name: Option<String>,
    pub archives: Vec<PathBuf>,
    pub zones: Vec<String>,
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
        Self {
            paths: vec![],
            name: None,
            archives: vec![],
            zones: vec![],
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

        crate::Definitions::check_keys(data, &["name", "include", "archives", "zones"])?;

        // read and merge includes first
        if let Some(includes) = crate::Definitions::get_vec(data, "include")? {
            for inc in includes.iter() {
                let inc = inc
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("includes must be strings"))?;
                let inc = normalize(&base.join(inc));
                new.merge(Definitions::load_one(&inc)?);
            }
        }

        // read the name
        if let Some(name) = crate::Definitions::get_str(data, "name")? {
            new.name = Some(name.to_owned());
        }

        // read in string lists
        new.archives.extend(
            Self::get_str_vec(data, "archives")?
                .into_iter()
                .map(|p| normalize(&base.join(p))),
        );
        new.zones.extend(Self::get_str_vec(data, "zones")?);

        Ok(new)
    }

    pub fn merge(&mut self, other: Self) {
        if self.name.is_none() {
            self.name = other.name;
        }

        self.archives.extend(other.archives);
        self.zones.extend(other.zones);
    }

    fn get_str_vec(data: &StrictYaml, k: &str) -> anyhow::Result<Vec<String>> {
        if let Some(strs) = crate::Definitions::get_vec(data, k)? {
            let mut ret = Vec::with_capacity(strs.len());
            for p in strs.iter() {
                let p = p
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("bad string in {:?}", k))?;
                ret.push(p.to_owned());
            }
            Ok(ret)
        } else {
            Ok(vec![])
        }
    }
}
