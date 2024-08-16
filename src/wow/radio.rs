use crate::{Scheduler, source, Time};
use super::{Definitions, Data};

pub struct Radio<F> {
    definitions: Definitions,
    scheduler: Scheduler,
    metadata_callback: F,

    data: Data,
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
    pub fn new<PI, P>(scheduler: Scheduler, paths: PI, metadata_callback: F) -> anyhow::Result<Self> where PI: Iterator<Item = P>, P: AsRef<std::path::Path> {
        let definitions = Definitions::open(paths)?;
        let mut chain = mpq::Chain::new();

        for archive in definitions.archives.clone() {
            println!("loading {:?}", archive);
            chain.add(archive)?;
        }

        Ok(Self {
            definitions,
            scheduler,
            metadata_callback,

            data: Data::new_from_chain(chain)?,
        })
    }

    pub async fn play_zone(&mut self, start: Time, name: &str) -> anyhow::Result<Time> {
        let zone = self.data.get_zone(name).ok_or_else(|| anyhow::anyhow!("could not get zone: {:?}", name))?;
        let path = &zone.day.music.items[0];
        let data = self.data.read_file(path)?;
        let cursor = std::io::Cursor::new(data);
        let media = source::Media::new(cursor)?;

        let end = self.scheduler.add(start, media).ok_or_else(|| anyhow::anyhow!("unknown sound file length"))?;

        Ok(end)

    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut start = Time::seconds(0.0);
        loop {
            for name in self.definitions.zones.clone() {
                let new_start = self.play_zone(start, &name).await?;
                self.scheduler.wait(start).await?;
                set_metadata!(self, "{}", name);
                start = new_start;
            }
        }
    }
}
