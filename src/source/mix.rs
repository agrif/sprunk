pub struct Mix<S> {
    source: S,
    mix: Vec<Vec<f32>>,
    buffer: Vec<f32>,
}

impl<S> Mix<S>
where
    S: super::Source,
{
    pub fn new(source: S, mix: Vec<Vec<f32>>) -> Self {
        for col in mix.iter() {
            if col.len() != source.channels() as usize {
                panic!(
                    "bad matrix shape for mix: {:?} x {:?}",
                    mix.len(),
                    col.len()
                );
            }
        }
        Self {
            source,
            mix,
            buffer: vec![],
        }
    }

    pub fn new_channels(source: S, channels: u16) -> Self {
        let old = source.channels();
        Self::new(source, find_mix(channels, old))
    }
}

impl<S> super::Source for Mix<S>
where
    S: super::Source,
{
    fn samplerate(&self) -> f32 {
        self.source.samplerate()
    }

    fn channels(&self) -> u16 {
        self.mix.len() as u16
    }

    fn len(&self) -> Option<u64> {
        self.source.len()
    }

    fn fill(&mut self, buffer: &mut [f32]) -> usize {
        let outchannels = self.mix.len();
        let inchannels = self.source.channels() as usize;
        self.buffer
            .resize(buffer.len() * inchannels / outchannels, 0.0);
        let samples = self.source.fill(&mut self.buffer);
        let frames = samples / inchannels;
        for f in 0..frames {
            let base_in = f * inchannels;
            let base_out = f * outchannels;
            for i in 0..outchannels {
                buffer[base_out + i] = 0.0;
                for j in 0..inchannels {
                    buffer[base_out + i] += self.mix[i][j] * self.buffer[base_in + j];
                }
            }
        }
        frames * outchannels
    }

    fn seek(&mut self, frame: u64) -> anyhow::Result<()> {
        self.source.seek(frame)
    }
}

// helper to create a mix from standard channel maps
fn find_mix(new: u16, old: u16) -> Vec<Vec<f32>> {
    match (new, old) {
        // stereo to mono
        (1, 2) => vec![vec![0.5, 0.5]],
        // pseudoinverse of (stereo to mono)
        (2, 1) => vec![vec![1.0], vec![1.0]],

        // ATSC mix for 5.1, surround 5.1 to stereo
        // http://www.atsc.org/wp-content/uploads/2015/03/A52-201212-17.pdf
        (2, 6) => vec![
            //   L    R    C      LFE  Ls     Rs
            vec![1.0, 0.0, 0.707, 0.0, 0.707, 0.0],
            vec![0.0, 1.0, 0.707, 0.0, 0.0, 0.707],
        ],
        // pseudoinverse of the above
        (6, 2) => vec![
            vec![0.53340314, -0.13333065],
            vec![-0.13333065, 0.53340314],
            vec![0.28285125, 0.28285125],
            vec![0.0, 0.0],
            vec![0.37711602, -0.09426477],
            vec![-0.09426477, 0.37711602],
        ],

        // composition of the (2, 1) and (6, 2) matrices
        (1, 6) => vec![vec![0.5, 0.5, 0.707, 0., 0.3535, 0.3535]],
        // pseudoinverse of the above
        (6, 1) => vec![
            vec![0.40007249],
            vec![0.40007249],
            vec![0.56570251],
            vec![0.],
            vec![0.28285125],
            vec![0.28285125],
        ],

        // default case
        _ => {
            // pseudo-identity as a last resort
            let mut mix = vec![vec![0.0; old as usize]; new as usize];
            let max = old.min(new);
            for i in 0..max as usize {
                mix[i][i] = 1.0;
            }
            mix
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn shapes() {
        // make sure find_mix(i, j) is (i, j) shaped
        for i in 0..10 {
            for j in 0..10 {
                let mix = super::find_mix(i, j);

                assert_eq!(mix.len(), i as usize);
                for col in mix.iter() {
                    assert_eq!(col.len(), j as usize);
                }
            }
        }
    }
}
