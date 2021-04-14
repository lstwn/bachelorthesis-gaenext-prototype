mod listener;
mod state;
mod updater;
use anyhow::{Context, Result};
use chrono::prelude::*;
use exposurelib::args::{crate_authors, crate_description, crate_name, crate_version, Args};
use exposurelib::config::ClientConfig;
use exposurelib::logger;
use listener::Listener;
use serde_yaml;
use state::ClientState;
use std::fs;
use tokio::sync::mpsc;
use tokio::task;
use updater::Updater;

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

    // TODO: globally shared ds client, change per key limit to 1 in ds code afterward

    let (state_tx, state_rx) = mpsc::channel::<state::Event>(100);
    let (listener_tx, listener_rx) = mpsc::channel::<std::time::Duration>(100);

    let initial_from = Utc::now()
        - config
            .params
            .infection_period
            .as_duration(config.params.tek_rolling_period);
    let updater = Updater::new(
        config.diagnosis_server_endpoint,
        config.params.refresh_period,
        initial_from,
        state_tx.clone(),
    )
    .await
    .context("Error creating updater")?;

    let listener = Listener::new(config.client_endpoint, listener_rx, state_tx);

    let state = ClientState::new(config, state_rx, listener_tx)
        .await
        .context("Error creating client state")?;

    let state_handle = task::spawn(async move { state.run().await });
    let updater_handle = task::spawn(async move { updater.run().await });
    let listener_handle = task::spawn(async move { listener.run().await });

    state_handle.await.context("State panicked")?;
    updater_handle.await.context("Updater panicked")?;
    listener_handle.await.context("Listener panicked")?;

    Ok(())
}
