pub struct Lame<F> {
    sample_rate: u32,
    inner: F,
    encoder: lame::Lame,
    left: Vec<i16>,
    right: Vec<i16>,
    out: Vec<u8>,
}

impl<F> Lame<F>
where
    F: std::io::Write,
{
    pub fn new(
        inner: F,
        sample_rate: u32,
        kbitrate: Option<i32>,
        quality: Option<u8>,
    ) -> anyhow::Result<Lame<F>> {
        let mut encoder =
            lame::Lame::new().ok_or_else(|| anyhow::anyhow!("out of memory in mp3 encoder"))?;
        encoder
            .set_sample_rate(sample_rate)
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        encoder
            .set_channels(2)
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        encoder
            .set_quality(quality.unwrap_or(5))
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        encoder
            .set_kilobitrate(kbitrate.unwrap_or(300))
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        encoder
            .init_params()
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;

        Ok(Lame {
            sample_rate,
            inner,
            encoder,
            left: Vec::new(),
            right: Vec::new(),
            out: Vec::new(),
        })
    }
}

impl<F> super::Sink for Lame<F>
where
    F: std::io::Write,
{
    fn samplerate(&self) -> f32 {
        self.sample_rate as f32
    }

    fn channels(&self) -> u16 {
        2
    }

    fn write(&mut self, buffer: &[f32]) -> anyhow::Result<()> {
        let samples = (buffer.len() + 1) / 2;
        self.left.resize(samples, 0);
        self.right.resize(samples, 0);

        // from lame.h, worst case
        let mp3size = ((samples * 5 + 3) / 4) + 7200;
        self.out.resize(mp3size, 0);

        for (i, v) in buffer.iter().enumerate() {
            if i % 2 > 0 {
                self.right[i / 2] = cpal::Sample::from(v);
            } else {
                self.left[i / 2] = cpal::Sample::from(v);
            }
        }

        let amt = self
            .encoder
            .encode(&self.left, &self.right, &mut self.out)
            .map_err(|_| anyhow::anyhow!("mp3 encoding error"))?;
        self.inner.write_all(&self.out[..amt])?;
        Ok(())
    }
}
