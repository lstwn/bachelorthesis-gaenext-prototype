pub use clap::{crate_authors, crate_description, crate_name, crate_version};
use clap::{App, Arg};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub config_file_path: PathBuf,
    pub log_file_path: PathBuf,
    pub log_level: log::LevelFilter,
}

impl Args {
    const CONFIG: &'static str = "config";
    const LOG: &'static str = "log";
    const VERBOSITY: &'static str = "verbosity";

    pub fn new(name: &str, version: &str, authors: &str, description: &str) -> Self {
        let matches = App::new(name)
            .version(version)
            .author(authors)
            .about(description)
            .arg(
                Arg::with_name(Self::CONFIG)
                    .short("c")
                    .long("config")
                    .value_name("FILE")
                    .required(true)
                    .help("Sets the yaml config file"),
            )
            .arg(
                Arg::with_name(Self::LOG)
                    .short("l")
                    .long("log")
                    .value_name("FILE")
                    .required(true)
                    .help("Sets the log output file"),
            )
            .arg(
                Arg::with_name(Self::VERBOSITY)
                    .short("v")
                    .multiple(true)
                    .help("Sets level of verbosity"),
            )
            .get_matches();
        let config_file_path = matches.value_of(Self::CONFIG).unwrap().into();
        let log_file_path = matches.value_of(Self::LOG).unwrap().into();
        let log_level = match matches.occurrences_of(Self::VERBOSITY) {
            0 => log::LevelFilter::Error,
            1 => log::LevelFilter::Warn,
            2 => log::LevelFilter::Info,
            3 => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        Args {
            config_file_path,
            log_file_path,
            log_level,
        }
    }
}
