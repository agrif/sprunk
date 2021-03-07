use crate::encoder::Format;
use crate::Encoder;

pub struct Shout<E> {
    conn: shout::ShoutConn,
    encoder: E,
}

impl<E> Shout<E>
where
    E: Encoder,
{
    pub fn new(
        encoder: E,
        host: &str,
        port: u16,
        mount: &str,
        user: &str,
        password: Option<&str>,
    ) -> anyhow::Result<Self> {
        let mut builder = shout::ShoutConnBuilder::new()
            .host(host.to_owned())
            .port(port)
            .mount(mount.to_owned())
            .user(user.to_owned());
        if let Some(pw) = password {
            builder = builder.password(pw.to_owned());
        }

        // FIXME check with encoder...
        builder = builder.format(match encoder.format() {
            Format::Mp3 => shout::ShoutFormat::MP3,
            _ => anyhow::bail!("cannot stream in this format"),
        });

        let conn = builder
            .build()
            .map_err(|_| anyhow::anyhow!("error connecting to streaming server"))?;
        Ok(Shout { conn, encoder })
    }
}

impl<E> super::Sink for Shout<E>
where
    E: Encoder,
{
    fn samplerate(&self) -> f32 {
        self.encoder.samplerate()
    }

    fn channels(&self) -> u16 {
        self.encoder.channels()
    }

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()> {
        let encoded = self.encoder.encode(buffer)?;
        self.conn
            .send(encoded)
            .map_err(|_| anyhow::anyhow!("error sending data to streaming server"))?;
        self.conn.sync();
        Ok(())
    }
}
