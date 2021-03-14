use anyhow::Result;
use exposurelib::args::{crate_authors, crate_description, crate_name, crate_version, Args};
use exposurelib::config::ClientConfig;
use exposurelib::logger;
use std::fs;

fn main() -> Result<()> {
    let args = Args::new(
        crate_name!(),
        crate_version!(),
        crate_authors!(),
        crate_description!(),
    );
    let config = fs::read_to_string(&args.config_file_path)?;
    let config: ClientConfig = serde_yaml::from_str(&config)?;
    logger::setup_logger(
        &args.log_file_path,
        args.log_level,
        String::from(config.name()),
    );
    logger::info!("Hello from client {}", config.name());
    Ok(())
}
