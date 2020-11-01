fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    /*let sink = sprunk::sink::System::new(1024)?;
    let mut sched = sprunk::Manager::new(sink, 1024);

    for (i, arg) in args[1..].iter().enumerate() {
        let file = std::fs::File::open(arg)?;
        let source = sprunk::source::Media::new(file)?;
        let mut subsched = sched.subscheduler();
        if let Some(end) = subsched.add(48000 * i as u64, source) {
            subsched.set_volume(0, 0.0, None);
            subsched.set_volume(24000, 1.0, Some(24000));
            subsched.set_volume(end - 24000, 0.0, Some(24000));
            sched.advance(end)?;
        }
    }

    sched.advance_to_end()?;*/

    for arg in args[1..].iter() {
        let def = sprunk::Definitions::open(arg)?;
        def.verify()?;
        println!("{:?}", def);
    }

    Ok(())
}
