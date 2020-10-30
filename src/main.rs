use sprunk::Sink;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (mut sched, schedsrc) = sprunk::Scheduler::new(48000.0, 2);

    for (i, arg) in args[1..].iter().enumerate() {
        let file = std::fs::File::open(arg)?;
        let source = sprunk::source::Media::new(file)?;
        let mut subsched = sched.subscheduler();
        subsched.add(48000 * i as u64, source);
    }

    let mut sink = sprunk::sink::System::new(1024)?;
    sink.play(schedsrc, 1024)?;

    Ok(())
}
