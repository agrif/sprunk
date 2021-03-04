mod mp3;

pub use mp3::Mp3;

pub trait Encoder {
    fn samplerate(&self) -> f32;
    fn channels(&self) -> u16;

    fn encode(&mut self, buffer: &[f32]) -> anyhow::Result<&[u8]>;
}
