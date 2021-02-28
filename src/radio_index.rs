use std::path::{Path, PathBuf};

use strict_yaml_rust::{StrictYaml, StrictYamlLoader};

use crate::normalize::normalize;

#[derive(Debug, Clone)]
pub struct RadioIndex {
    info: std::collections::HashMap<String, RadioInfo>,
}

#[derive(Debug, Clone)]
struct RadioInfo {
    files: Vec<PathBuf>,
    output: Output,
}

#[derive(Debug, Clone)]
enum Output {
    System,
    Icecast {
        host: String,
        schema: String,
        user: String,
        password: Option<String>,
    },
}

impl RadioIndex {
    pub fn open<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let base = path.as_ref().parent().unwrap_or(Path::new("."));
        let contents = std::fs::read_to_string(&path)?;
        let datawhole = StrictYamlLoader::load_from_str(&contents)?;
        let data = datawhole
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("could not get radio document"))?;
        let mut info = std::collections::HashMap::new();
        let defs = data["stations"]
            .as_hash()
            .ok_or_else(|| anyhow::anyhow!("could not get stations dictionary"))?;
        for k in defs {
            let mount =
                k.0.as_str()
                    .ok_or_else(|| anyhow::anyhow!("mountpoint names should be strings"))?
                    .to_owned();
            let files = k.1["files"]
                .as_vec()
                .ok_or_else(|| anyhow::anyhow!("station files should be a list"))?;
            let mut station = RadioInfo::new();
            for file in files.iter() {
                let leaf = file
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("station files should be a list of strings"))?;
                let whole = normalize(&base.join(leaf));
                station.files.push(whole);
            }
            station.update(data)?;
            station.update(k.1)?;
            info.insert(mount, station);
        }
        Ok(Self { info: info })
    }

    pub fn play<S>(&self, station: S) -> anyhow::Result<()>
    where
        S: AsRef<str>,
    {
        let stationdef = self
            .info
            .get(station.as_ref())
            .ok_or_else(|| anyhow::anyhow!("could not find station"))?;
        // FIXME bufsize
        let bufsize = 24000;
        let sink = stationdef.output.to_sink(bufsize)?;
        let files = stationdef.files.clone();
        let manager = crate::Manager::new(sink, bufsize, move |sched| async move {
            let mut radio = crate::Radio::new(sched, files.iter())?;
            radio.run().await
        });

        manager.advance_to_end()
    }
}

impl RadioInfo {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            output: Output::System,
        }
    }

    fn update(&mut self, data: &StrictYaml) -> anyhow::Result<()> {
        self.output.update_icecast(&data["icecast"])?;
        self.output.update(&data["output"])?;
        Ok(())
    }
}

impl Output {
    fn update(&mut self, data: &StrictYaml) -> anyhow::Result<()> {
        if data.is_badvalue() {
            return Ok(());
        }
        let val = data
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("output should be a string"))?;
        if val == "system" {
            *self = Output::System;
        } else {
            anyhow::bail!("bad output value");
        }
        Ok(())
    }

    fn update_icecast(&mut self, data: &StrictYaml) -> anyhow::Result<()> {
        if data.is_badvalue() {
            return Ok(());
        }

        let host = data["host"].as_str().unwrap_or("localhost:8000").to_owned();
        let schema = data["schema"].as_str().unwrap_or("http").to_owned();
        let user = data["user"].as_str().unwrap_or("source").to_owned();
        let password = data["password"].as_str().map(|s| s.to_owned());
        *self = Output::Icecast {
            host,
            schema,
            user,
            password,
        };
        Ok(())
    }

    fn to_sink(&self, bufsize: usize) -> anyhow::Result<Box<dyn crate::Sink>> {
        // FIXME
        Ok(Box::new(crate::sink::System::new(bufsize)?))
    }
}
