mod handler;
mod state;
mod listener;
use anyhow::{Context, Result};
use exposurelib::args::{crate_authors, crate_description, crate_name, crate_version, Args};
use exposurelib::config::ClientConfig;
use exposurelib::logger;
use serde_yaml;
use state::ClientState;
use std::fs;

#[tokio::main]
async fn main() -> Result<()> {
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

    logger::trace!("Client {} started", config.name());

    ClientState::new(config)
        .await
        .context("Error creating client state")?
        .await
        .context("Client panicked")?;

    Ok(())
}
