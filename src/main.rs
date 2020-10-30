use sprunk::Sink;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file = std::fs::File::open(&args[1])?;
    let source = sprunk::source::Media::new(file)?;
    let mut sink = sprunk::sink::System::new(1024)?;
    sink.play(source, 1024)?;

    Ok(())
}
