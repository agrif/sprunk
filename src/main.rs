fn main() -> anyhow::Result<()> {
    let paths = std::env::args().skip(1);
    let sink = sprunk::sink::System::new(24000)?;
    let manager = sprunk::Manager::new(sink, 24000, move |sched| async move {
        let mut radio = sprunk::Radio::new(sched, paths)?;
        radio.run().await
    });

    manager.advance_to_end()?;

    Ok(())
}
