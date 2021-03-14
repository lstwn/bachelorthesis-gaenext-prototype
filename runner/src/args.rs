use clap::{crate_authors, crate_description, crate_name, crate_version, App, Arg};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub config_files_path: PathBuf,
}

impl Args {
    const CONFIG_FILES_PATH: &'static str = "CONFIG_FILES_PATH";
    const CONFIG_FILES_PATH_DEFAULT: &'static str = "configs";

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
            .get_matches();

        Self {
            config_files_path: PathBuf::from(matches.value_of(Self::CONFIG_FILES_PATH).unwrap()),
        }
    }
}
