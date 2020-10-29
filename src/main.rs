use sprunk::Sink;

fn main() -> anyhow::Result<()> {
    //let file = std::fs::File::open(r"H:\local\gtav-radio\RADIO_02_POP\tape_loop\TAPE_LOOP.ogg")?;
    //let file = std::fs::File::open(r"H:\local\gtav-radio\RADIO_02_POP\six_underground\SIX_UNDERGROUND.ogg")?;
    let file = std::fs::File::open(r"C:\Users\agrif\Desktop\dance_6.ogg")?;

    let source = sprunk::source::Media::new(file)?;
    //let source = sprunk::source::Sine::new(44100.0, 1, 440.0);

    let mut sink = sprunk::sink::System::new(1024)?;
    sink.play(source, 1024)?;

    Ok(())
}
