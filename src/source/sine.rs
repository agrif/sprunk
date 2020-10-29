pub struct Sine {
    samplerate: f32,
    channels: u16,
    omega: f32,
    sample: u64,
    channel: u16,
}

impl Sine {
    pub fn new(samplerate: f32, channels: u16, frequency: f32) -> Self {
        Sine {
            samplerate,
            channels,
            omega: 2.0 * std::f32::consts::PI * frequency,
            sample: 0,
            channel: 0,
        }
    }
}

impl super::Source for Sine {
    fn samplerate(&self) -> f32 {
        self.samplerate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn len(&self) -> Option<u64> {
        None
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        for i in 0..buffer.len() {
            let v = (self.omega * self.sample as f32 / self.samplerate).sin();

            buffer[i] = v;

            self.channel += 1;
            if self.channel >= self.channels {
                self.sample += 1;
                self.channel = 0;
            }
        }
        buffer.len()
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        self.sample = frame;
        self.channel = 0;
        Ok(())
    }
}
