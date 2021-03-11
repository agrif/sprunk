use clap::clap_app;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let matches = clap_app!(
        sprunk =>
            (@subcommand play =>
             (@arg OUTPUT: -o --output +takes_value "set output")
             (@arg RADIOYAML: +required "radio definitions list")
             (@arg MOUNT: +required "radio mount point")
            )
            (@subcommand serve =>
             (@arg BIND: -b --bind +takes_value "set server bind")
             (@arg RADIOYAML: +required "radio definitions list")
             (@arg STATICFILES: +required "static files to serve also")
            )
    )
    .get_matches();

    if let Some(matches) = matches.subcommand_matches("play") {
        let radioyaml = matches.value_of("RADIOYAML").unwrap();
        let mount = matches.value_of("MOUNT").unwrap();
        let output = matches
            .value_of("OUTPUT")
            .map(|s| sprunk::Output::from_str(s))
            .transpose()?
            .map(|s| s.to_sink(24000))
            .transpose()?;
        let index = sprunk::RadioIndex::open(&radioyaml)?;
        index.play(mount, output, false)?;
    }

    if let Some(matches) = matches.subcommand_matches("serve") {
        let radioyaml = matches.value_of("RADIOYAML").unwrap();
        let staticfiles = matches.value_of("STATICFILES").unwrap();
        let index = sprunk::RadioIndex::open(&radioyaml)?;
        let addr = if let Some(b) = matches.value_of("BIND") {
            b.parse()?
        } else {
            ([127, 0, 0, 1], 8000).into()
        };
        println!("now serving radio at http://{}/", addr);
        sprunk::server_run(&addr, index, staticfiles).await?;
    }

    Ok(())
}
