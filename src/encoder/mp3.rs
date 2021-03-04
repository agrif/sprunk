pub struct Mp3 {
    samplerate: u32,
    lame: lame::Lame,
    left: Vec<i16>,
    right: Vec<i16>,
    out: Vec<u8>,
}

impl Mp3 {
    pub fn new(samplerate: u32, kbitrate: Option<i32>, quality: Option<u8>) -> anyhow::Result<Mp3> {
        let mut lame =
            lame::Lame::new().ok_or_else(|| anyhow::anyhow!("out of memory in mp3 encoder"))?;
        lame.set_sample_rate(samplerate)
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        lame.set_channels(2)
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        lame.set_quality(quality.unwrap_or(5))
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        lame.set_kilobitrate(kbitrate.unwrap_or(300))
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;
        lame.init_params()
            .map_err(|_| anyhow::anyhow!("could not create mp3 encoder"))?;

        Ok(Mp3 {
            samplerate,
            lame,
            left: Vec::new(),
            right: Vec::new(),
            out: Vec::new(),
        })
    }
}

impl super::Encoder for Mp3 {
    fn samplerate(&self) -> f32 {
        self.samplerate as f32
    }

    fn channels(&self) -> u16 {
        2
    }

    fn encode(&mut self, buffer: &[f32]) -> anyhow::Result<&[u8]> {
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
            .lame
            .encode(&self.left, &self.right, &mut self.out)
            .map_err(|_| anyhow::anyhow!("mp3 encoding error"))?;
        Ok(&self.out[..amt])
    }
}
