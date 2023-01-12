use anyhow::Context;
use clap::{ArgAction, Parser};
use parallel_stream::{IntoParallelStream, ParallelStream};
use rand::prelude::IteratorRandom;
use simplelog::__private::log;
use simplelog::{debug, info, ColorChoice, ConfigBuilder, TermLogger, TerminalMode};
use std::borrow::Borrow;
use std::ops::Index;

const TYPES: [&str; 5] = ["apnic", "arin", "ripencc", "afrinic", "lacnic"];

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, short = 'c')]
    country: String,

    /// Not implemented yet
    #[arg(long, action = ArgAction::SetTrue, hide = true)]
    ipv6: bool,

    #[arg(long, short = 'v', action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let data = tokio::spawn(async { grab_data().await });

    init_logging(args.borrow().verbose)?;

    let uppercase_raw = args.borrow().country.to_uppercase();
    let country = match &uppercase_raw.len() {
        2 => rust_iso3166::from_alpha2(&uppercase_raw),
        3 => rust_iso3166::from_alpha3(&uppercase_raw),
        _ => None,
    }
    .expect(&*format!("Invalid Country Code: {}", &uppercase_raw));

    debug!("Country: {:?}", country);

    // TODO: Implement IPv6
    // TODO: Possibly buffer data and return once a match is found instead of waiting for all data to be fetched.
    let bytes = data.await??;
    let data = std::str::from_utf8(&bytes)?;
    let ip_str = if args.ipv6 { "ipv6" } else { "ipv4" };

    let line = data
        .lines()
        .filter(|line| !line.starts_with('#')) // Remove comments
        .map(|line| line.split('|').collect::<Vec<&str>>()) // Split on pipe
        .filter(|parts| parts.len() == 7) // Remove invalid lines
        .filter(|parts| parts.index(1) == &country.alpha2) // Filter by country
        .filter(|parts| parts.index(2) == &ip_str) // Filter by IP version
        .filter(|parts| parts.index(6) == &"allocated") // Make sure it's allocated
        .choose(&mut rand::thread_rng()) // Choose a random one
        .context("Select random element.")?;

    let ip = line.index(3);
    let final_string = if args.ipv6 {
        return Err(anyhow::anyhow!("IPv6 not implemented yet."));
    } else {
        format!(
            "{}{}",
            ip.strip_suffix('0').context("Strip last 0")?,
            rand::random::<u8>()
        )
    };

    info!(
        "Random `{}` {} => {}",
        &country.name, &ip_str, &final_string
    );

    return Ok(());
}

fn init_logging(verbosity: u8) -> anyhow::Result<()> {
    let level = match verbosity {
        0 => log::LevelFilter::Info,
        1 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    let config = ConfigBuilder::new()
        .set_time_level(log::LevelFilter::Off)
        .build();

    TermLogger::init(level, config, TerminalMode::Mixed, ColorChoice::Auto)
        .context("Initializing logger")?;

    debug!("Logging initialized at level: {:?}", level);

    return Ok(());
}

async fn grab_data() -> anyhow::Result<Vec<u8>> {
    let out: Vec<Vec<u8>> = TYPES
        .to_vec()
        .into_par_stream()
        .map(|name| async move {
            reqwest::get(get_url(&name))
                .await
                .expect("Failed to get data")
                .bytes()
                .await
                .expect("Failed to get bytes")
                .to_vec()
        })
        .collect()
        .await;

    return Ok(out.concat());
}

fn get_url(name: &str) -> String {
    let target = if name == "arin" {
        "arin-extended"
    } else {
        name
    };

    return format!(
        "http://ftp.apnic.net/stats/{}/delegated-{}-latest",
        name, target
    );
}
