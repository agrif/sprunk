use clap::clap_app;

fn main() -> anyhow::Result<()> {
    let matches = clap_app!(sprunk =>
                            (@arg OUTPUT: -o --output +takes_value "set output")
                            (@arg RADIOYAML: +required "radio definitions list")
                            (@arg MOUNT: +required "radio mount point")
    )
    .get_matches();

    let radioyaml = matches.value_of("RADIOYAML").unwrap();
    let mount = matches.value_of("MOUNT").unwrap();
    let output = matches
        .value_of("OUTPUT")
        .map(sprunk::Output::from_str)
        .transpose()?;
    let index = sprunk::RadioIndex::open(&radioyaml)?;
    index.play(mount, output)?;

    Ok(())
}
