mod media;
mod mix;
mod resample;
mod sine;
mod volume;

pub use media::Media;
pub use mix::Mix;
pub use resample::Resample;
pub use sine::Sine;
pub use volume::Volume;

pub trait Source {
    fn samplerate(&self) -> f32;
    fn channels(&self) -> u16;
    fn len(&self) -> Option<u64>;

    fn fill(&mut self, buffer: &mut [f32]) -> usize;
    fn seek(&mut self, frame: u64) -> anyhow::Result<()>;

    fn force_fill(&mut self, mut buffer: &mut [f32]) -> usize {
        let mut filled = 0;
        while buffer.len() > 0 {
            let f = self.fill(buffer);
            if f == 0 {
                break;
            }
            buffer = &mut buffer[f..];
            filled += f;
        }
        filled
    }

    fn resample(self, samplerate: f32) -> Resample<Self>
    where
        Self: Sized,
    {
        Resample::new(self, samplerate)
    }

    fn remix(self, channels: u16) -> Mix<Self>
    where
        Self: Sized,
    {
        Mix::new_channels(self, channels)
    }

    fn remix_with(self, mix: Vec<Vec<f32>>) -> Mix<Self>
    where
        Self: Sized,
    {
        Mix::new(self, mix)
    }

    fn reformat(self, samplerate: f32, channels: u16) -> Resample<Mix<Self>>
    where
        Self: Sized,
    {
        self.remix(channels).resample(samplerate)
    }

    fn reformat_like<S>(self, other: &S) -> Resample<Mix<Self>>
    where
        Self: Sized,
        S: Source,
    {
        self.reformat(other.samplerate(), other.channels())
    }

    fn reformat_for<S>(self, other: &S) -> Resample<Mix<Self>>
    where
        Self: Sized,
        S: crate::Sink + ?Sized,
    {
        self.reformat(other.samplerate(), other.channels())
    }

    fn volume(self, volume: f32) -> Volume<Self>
    where
        Self: Sized,
    {
        Volume::new(self, volume)
    }

    fn normalize(self, lufs: f32) -> Volume<Self>
    where
        Self: Sized + Send + 'static,
    {
        Volume::new_lufs(self, lufs)
    }
}
