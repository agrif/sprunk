use std::path::{Path, PathBuf};

use strict_yaml_rust::{StrictYaml, StrictYamlLoader};

use crate::normalize::normalize;

// in seconds
const HOTSTART_WINDOW: f32 = 60.0 * 2.0;

#[derive(Debug, Clone)]
pub struct RadioIndex {
    info: std::collections::HashMap<String, RadioInfo>,
}

#[derive(Debug, Clone)]
pub struct RadioInfo {
    files: Vec<PathBuf>,
    output: Output,
}

#[derive(Debug, Clone)]
pub enum Output {
    System,
    File(PathBuf),
    Icecast {
        mount: String,
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
            station.update(&mount, data)?;
            station.update(&mount, k.1)?;
            info.insert(mount, station);
        }
        Ok(Self { info: info })
    }

    pub fn contains_key<S>(&self, station: S) -> bool
    where
        S: AsRef<str>,
    {
        self.info.get(station.as_ref()).is_some()
    }

    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, String, RadioInfo> {
        self.info.keys()
    }

    pub fn load<S>(&self, station: S) -> anyhow::Result<crate::Definitions>
    where
        S: AsRef<str>,
    {
        let stationdef = self
            .info
            .get(station.as_ref())
            .ok_or_else(|| anyhow::anyhow!("could not find station"))?;
        crate::Definitions::open(stationdef.files.iter())
    }

    pub fn play<S>(&self, station: S, output: Option<Box<dyn crate::Sink>>, hotstart: bool) -> anyhow::Result<()>
    where
        S: AsRef<str>,
    {
        let stationdef = self
            .info
            .get(station.as_ref())
            .ok_or_else(|| anyhow::anyhow!("could not find station"))?;
        let bufsize = 24000;
        let sink = output
            .map(Ok)
            .unwrap_or_else(|| stationdef.output.to_sink(bufsize))?;
        let files = stationdef.files.clone();
        let mut manager = crate::Manager::new(sink, bufsize, move |sched| async move {
            let mut radio = crate::Radio::new(sched, files.iter())?;
            radio.run().await
        });

        if hotstart {
            use rand::Rng;
            // advance a random amount
            let amt = HOTSTART_WINDOW * rand::thread_rng().gen::<f32>();
            manager.skip(amt);
        }

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

    fn update(&mut self, mount: &String, data: &StrictYaml) -> anyhow::Result<()> {
        self.output.update_icecast(mount, &data["icecast"])?;
        self.output.update(&data["output"])?;
        Ok(())
    }
}

impl Output {
    pub fn from_str(spec: &str) -> anyhow::Result<Self> {
        if let Some(pos) = spec.find(":") {
            let (p1, p2) = spec.split_at(pos);
            Self::from_type_arg(p1, Some(&p2[1..]))
        } else {
            Self::from_type_arg(spec, None).or_else(|_| Self::from_type_arg("file", Some(spec)))
        }
    }

    fn from_type_arg(typ: &str, arg: Option<&str>) -> anyhow::Result<Self> {
        Ok(match typ {
            "play" => Output::System,
            "system" => Output::System,
            "file" => {
                let fname = arg.ok_or_else(|| anyhow::anyhow!("file output expects value"))?;
                Output::File(fname.into())
            }
            _ => anyhow::bail!("bad output value"),
        })
    }

    fn update(&mut self, data: &StrictYaml) -> anyhow::Result<()> {
        if data.is_badvalue() {
            return Ok(());
        }
        let val = data
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("output should be a string"))?;

        *self = Self::from_str(val)?;
        Ok(())
    }

    fn update_icecast(&mut self, mount: &String, data: &StrictYaml) -> anyhow::Result<()> {
        if data.is_badvalue() {
            return Ok(());
        }

        let mut fullmount = "/".to_owned();
        fullmount += mount;

        let host = data["host"].as_str().unwrap_or("localhost:8000").to_owned();
        let schema = data["schema"].as_str().unwrap_or("http").to_owned();
        let user = data["user"].as_str().unwrap_or("source").to_owned();
        let password = data["password"].as_str().map(|s| s.to_owned());
        *self = Output::Icecast {
            mount: fullmount,
            host,
            schema,
            user,
            password,
        };
        Ok(())
    }

    pub fn to_sink(&self, bufsize: usize) -> anyhow::Result<Box<dyn crate::Sink>> {
        match *self {
            Output::System => Ok(Box::new(crate::sink::System::new(bufsize)?)),
            Output::File(ref fname) => {
                // all files are mp3 I guess
                let encoder = crate::encoder::Mp3::new(48000, None, None)?;
                let file = std::fs::File::create(fname)?;
                Ok(Box::new(crate::sink::Stream::new(file, encoder)))
            }
            Output::Icecast {
                ref mount,
                ref host,
                ref user,
                ref password,
                // schema unused. maybe we should use it eventually...
                ..
            } => {
                use std::net::ToSocketAddrs;
                for addr in host.to_socket_addrs()? {
                    let ip = format!("{}", addr.ip());
                    let port = addr.port();
                    let encoder = crate::encoder::Mp3::new(48000, None, None)?;
                    if let Ok(sink) = crate::sink::Shout::new(
                        encoder,
                        &ip,
                        port,
                        &mount,
                        &user,
                        password.as_ref().map(|t| t.as_str()),
                    ) {
                        return Ok(Box::new(sink));
                    }
                }
                anyhow::bail!("bad hostname for icecast");
            }
        }
    }
}
