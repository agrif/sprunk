mod mp3;

pub use mp3::Mp3;

#[derive(Clone, Debug)]
pub enum Format {
    Mp3,
    Other(String),
}

pub trait Encoder {
    fn samplerate(&self) -> f32;
    fn channels(&self) -> u16;
    fn format(&self) -> Format;

    fn encode(&mut self, buffer: &[f32]) -> anyhow::Result<&[u8]>;
}
