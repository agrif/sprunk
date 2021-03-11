pub struct Volume<S> {
    state: VolumeState<S>,
    samplerate: f32,
    channels: u16,
    len: Option<u64>,
}

enum VolumeState<S> {
    Calculating(std::thread::JoinHandle<anyhow::Result<(S, f32)>>, f32),
    Failed,
    Ready(S, f32),
}

impl<S> VolumeState<S> {
    fn get(&mut self) -> anyhow::Result<(&mut S, f32)> {
        if let VolumeState::Ready(ref mut s, vol) = self {
            return Ok((s, *vol));
        }

        let m = std::mem::replace(self, VolumeState::Failed);
        match m {
            VolumeState::Calculating(handle, target) => {
                if let Ok(Ok((src, measured))) = handle.join() {
                    let volume = f32::powf(10.0, (target - measured) / 20.0);
                    *self = VolumeState::Ready(src, volume);
                }
                return self.get();
            }
            _ => anyhow::bail!("volume normalization failed"),
        }
    }
}

impl<S> Volume<S>
where
    S: super::Source,
{
    pub fn new(source: S, volume: f32) -> Self {
        Self {
            samplerate: source.samplerate(),
            channels: source.channels(),
            len: source.len(),
            state: VolumeState::Ready(source, volume),
        }
    }

    pub fn new_lufs(mut source: S, lufs: f32) -> Self
    where
        S: Send + 'static,
    {
        let samplerate = source.samplerate();
        let channels = source.channels();
        let len = source.len();

        let handle = std::thread::spawn(move || {
            let mut ebu = ebur128::EbuR128::new(
                source.channels() as u32,
                source.samplerate() as u32,
                ebur128::Mode::I,
            )?;
            let mut buffer = vec![0.0; (ebu.rate() * ebu.channels()) as usize];
            loop {
                let amt = source.fill(&mut buffer);
                if amt == 0 {
                    break;
                }
                ebu.add_frames_f32(&mut buffer[..amt])?;
            }
            let loudness = ebu.loudness_global()? as f32;
            source.seek(0)?;
            Ok((source, loudness))
        });
        Self {
            samplerate,
            channels,
            len,
            state: VolumeState::Calculating(handle, lufs),
        }
    }
}

impl<S> super::Source for Volume<S>
where
    S: super::Source,
{
    fn samplerate(&self) -> f32 {
        self.samplerate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn len(&self) -> Option<u64> {
        self.len
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        let (source, _) = self.state.get()?;
        source.seek(frame)
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        if let Ok((source, volume)) = self.state.get() {
            let size = source.fill(buffer);
            for i in 0..size {
                buffer[i] *= volume;
            }
            size
        } else {
            0
        }
    }
}
