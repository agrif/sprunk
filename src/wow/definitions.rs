use std::collections::HashMap;
use std::path::{Path, PathBuf};

use strict_yaml_rust::{StrictYaml, StrictYamlLoader};

use crate::normalize::normalize;

#[derive(Debug, Clone)]
pub struct Definitions {
    pub paths: Vec<PathBuf>,
    pub name: Option<String>,
    pub archives: Vec<PathBuf>,

    pub endpoints: Vec<String>,
    pub zones: HashMap<String, Zone>,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub name: String,
    pub parent: Option<String>,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub destination: String,
    pub via: Option<String>,
}

impl Connection {
    fn flip(&self, source: &Zone) -> Self {
        Self {
            destination: source.name.clone(),
            via: self.via.clone(),
        }
    }
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
            endpoints: vec![],
            zones: HashMap::new(),
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

        crate::Definitions::check_keys(
            data,
            &["name", "include", "archives", "endpoints", "zones"],
        )?;

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
        new.endpoints.extend(Self::get_str_vec(data, "endpoints")?);

        // read zones
        if let Some(zones) = crate::Definitions::get_vec(data, "zones")? {
            new.zones.reserve(zones.len());
            for zone in zones.iter() {
                crate::Definitions::check_keys(zone, &["name", "parent", "direct", "via"])?;

                let name = crate::Definitions::get_str(zone, "name")?
                    .ok_or_else(|| anyhow::anyhow!("zone requires name"))?;

                // read the parent
                let parent = crate::Definitions::get_str(zone, "parent")?;

                let mut connections = vec![];
                let direct = Self::get_str_vec(zone, "direct")?;
                for conn in direct {
                    connections.push(Connection {
                        destination: conn.to_owned(),
                        via: None,
                    });
                }

                if let Some(via) = Self::get_dict(zone, "via")? {
                    for middle in via.keys() {
                        let destinations = Self::get_str_vec(&zone["via"], middle)?;
                        for destination in destinations {
                            connections.push(Connection {
                                destination: destination.to_owned(),
                                via: Some(middle.clone()),
                            });
                        }
                    }
                }

                new.zones.insert(
                    name.to_owned(),
                    Zone {
                        name: name.to_owned(),
                        parent: parent.map(|s| s.to_owned()),
                        connections,
                    },
                );
            }
        }

        new.verify()?;

        Ok(new)
    }

    fn verify(&self) -> anyhow::Result<()> {
        // are all endpoints defined as zones?
        for endpoint in &self.endpoints {
            if !self.zones.contains_key(endpoint) {
                anyhow::bail!("endpoint zone not found: {:?}", endpoint);
            }
        }

        // are all connections and vias defined as zones?
        for zone in self.zones.values() {
            for conn in &zone.connections {
                if !self.zones.contains_key(&conn.destination) {
                    anyhow::bail!("connection endpoint not found: {:?}", conn.destination);
                }
                if let Some(via) = &conn.via {
                    if !self.zones.contains_key(via) {
                        anyhow::bail!("connection via not found: {:?}", via);
                    }
                }
            }
        }

        // are all connections reciprocated?
        for zone in self.zones.values() {
            for conn in &zone.connections {
                let recip = conn.flip(&zone);
                let dest = self
                    .zones
                    .get(&conn.destination)
                    .expect("could not find zone");
                if !dest.connections.contains(&recip) {
                    anyhow::bail!(
                        "flip connection {:?} -> {:?} via {:?} missing",
                        dest.name,
                        recip.destination,
                        recip.via
                    );
                }
            }
        }

        Ok(())
    }

    pub fn merge(&mut self, other: Self) {
        if self.name.is_none() {
            self.name = other.name;
        }

        self.archives.extend(other.archives);
        self.endpoints.extend(other.endpoints);
        // hmm, should I merge connections? no, for now
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

    fn get_dict<'a>(
        data: &'a StrictYaml,
        k: &str,
    ) -> anyhow::Result<Option<HashMap<String, &'a StrictYaml>>> {
        let v = if data[k].is_badvalue() {
            Some(None)
        } else {
            data[k].as_hash().map(|hash| {
                hash.iter()
                    .map(|(k, v)| k.as_str().map(|s| (s.to_owned(), v)))
                    .collect()
            })
        };
        v.ok_or_else(|| anyhow::anyhow!("bad value for {:?}, expected dict", k))
    }
}
