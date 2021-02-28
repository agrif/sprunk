use clap::clap_app;

fn main() -> anyhow::Result<()> {
    let matches = clap_app!(sprunk =>
                            (@arg RADIOYAML: +required "radio definitions list")
                            (@arg MOUNT: +required "radio mount point")
    )
    .get_matches();

    let radioyaml = matches.value_of("RADIOYAML").unwrap();
    let mount = matches.value_of("MOUNT").unwrap();
    let index = sprunk::RadioIndex::open(&radioyaml)?;
    index.play(mount)?;

    Ok(())
}
