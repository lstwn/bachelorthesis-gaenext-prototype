mod handler;
mod state;
use anyhow::Result;
use exposurelib::args::{crate_authors, crate_description, crate_name, crate_version, Args};
use exposurelib::config::DiagnosisServerConfig;
use exposurelib::logger;
use exposurelib::rpcs::DiagnosisServer;
use futures::{future, prelude::*};
use handler::ConnectionHandler;
use state::DiagnosisServerState;
use std::fs;
use std::sync::Arc;
use tarpc::server::{self, Channel, Incoming};
use tarpc::tokio_serde::formats;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::new(
        crate_name!(),
        crate_version!(),
        crate_authors!(),
        crate_description!(),
    );

    let config = fs::read_to_string(&args.config_file_path)?;
    let config: DiagnosisServerConfig = serde_yaml::from_str(&config)?;

    logger::setup_logger(&args.log_file_path, args.log_level, String::from("ds"));

    let state = Arc::new(DiagnosisServerState::new(&config));

    logger::trace!("Diagnosis Server listening on {}", config.endpoint);

    let mut listener =
        tarpc::serde_transport::tcp::listen(&config.endpoint, formats::Bincode::default).await?;
    listener.config_mut().max_frame_length(usize::MAX);
    listener
        // ignore accept errors
        .filter_map(|r| future::ready(r.ok()))
        .map(server::BaseChannel::with_defaults)
        // just one channel per *ip/port combo* (instead of per ip) in our simulation case
        .max_channels_per_key(1, |t| t.as_ref().peer_addr().unwrap())
        // function serve() is generated by the service attribute
        // it takes as input any type implementing the generated service trait
        .map(|channel| {
            let server = ConnectionHandler::new(
                channel.as_ref().as_ref().peer_addr().unwrap(),
                Arc::clone(&state),
            );
            channel.requests().execute(server.serve())
        })
        // max 100 channels (i.e. clients)
        .buffer_unordered(100)
        .for_each(|_| async {})
        .await;

    Ok(())
}
