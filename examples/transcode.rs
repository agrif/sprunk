use clap::clap_app;

use sprunk::{Sink, Source};

const SAMPLERATE: u32 = 48000;
const BUFFER_SIZE: usize = 1024;
const LOUDNESS: f32 = -14.0;

fn main() -> anyhow::Result<()> {
    let matches = clap_app!(transcode =>
        (@arg INPUT: +required "input file")
        (@arg OUTPUT: +required "output file")
    )
    .get_matches();

    let input = matches.value_of("INPUT").unwrap();
    let output = matches.value_of("OUTPUT").unwrap();

    let enc = sprunk::encoder::Mp3::new(SAMPLERATE, None, None)?;
    let mut sink = sprunk::sink::Stream::new(std::fs::File::create(output)?, enc);
    let mut source = sprunk::source::Media::new(std::fs::File::open(input)?)?
        .reformat_for(&sink)
        .normalize(LOUDNESS);

    let mut buf = [0.0; BUFFER_SIZE];
    loop {
        let amt = source.fill(&mut buf);
        if amt == 0 {
            break;
        }
        sink.write(&buf[..amt])?;
    }

    Ok(())
}
