use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub config_files_path: PathBuf,
    pub log_files_path: PathBuf,
    pub log_level: String,
}

impl Args {
    const CONFIG_FILES_PATH: &'static str = "CONFIG_FILES_PATH";
    const CONFIG_FILES_PATH_DEFAULT: &'static str = "configs";
    const LOG_FILES_PATH: &'static str = "LOG_FILES_PATH";
    const LOG_FILES_PATH_DEFAULT: &'static str = "logs";
    const LOG_LEVEL: &'static str = "verbosity";
    const LOG_LEVEL_DEFAULT: &'static str = "vvvv";

    pub fn new() -> Self {
        let matches = App::new(crate_name!())
            .version(crate_version!())
            .author(crate_authors!())
            .about(crate_description!())
            .arg(
                Arg::with_name(Self::CONFIG_FILES_PATH)
                    .short("c")
                    .long("config-files")
                    .value_name("DIRECTORY")
                    .default_value(Self::CONFIG_FILES_PATH_DEFAULT)
                    .help("Sets the directory in which all generated config files from the configurator reside"),
            )
            .arg(
                Arg::with_name(Self::LOG_FILES_PATH)
                    .short("l")
                    .long("log-files")
                    .value_name("DIRECTORY")
                    .default_value(Self::LOG_FILES_PATH_DEFAULT)
                    .help("Sets the directory in which the log files will appear."),
            )
            .arg(
                Arg::with_name(Self::LOG_LEVEL)
                    .short("v")
                    .multiple(true)
                    .default_value(Self::LOG_LEVEL_DEFAULT)
                    .help("Sets the log level for both diagnosis server and clients"),
            )
            .get_matches();

        Self {
            config_files_path: PathBuf::from(matches.value_of(Self::CONFIG_FILES_PATH).unwrap()),
            log_files_path: PathBuf::from(matches.value_of(Self::LOG_FILES_PATH).unwrap()),
            log_level: matches.value_of(Self::LOG_LEVEL).unwrap().into(),
        }
    }
}
