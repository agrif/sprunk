fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let sink = sprunk::sink::System::new(1024)?;
    let manager = sprunk::Manager::new(sink, 1024, move |mut sched| async move {
        let mut now = sprunk::Time::frames(0);
        for (i, arg) in args[1..].iter().enumerate() {
            let file = std::fs::File::open(arg)?;
            let source = sprunk::source::Media::new(file)?;
            let mut subsched = sched.subscheduler();
            if let Some(end) = subsched.add(now, source) {
                subsched.set_volume(now, 0.0, 0.0);
                subsched.set_volume(now, 1.0, 0.5);
                subsched.set_volume(end - 0.5, 0.0, 0.5);
                sched.wait(end).await?;
                now = end;
            }
        }
        Ok(())
    });

    manager.advance_to_end()?;

    /*
    for arg in args[1..].iter() {
        let def = sprunk::Definitions::open(arg)?;
        def.verify()?;
        println!("{:?}", def);
    }*/

    Ok(())
}
