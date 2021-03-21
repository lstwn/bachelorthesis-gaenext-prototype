use anyhow::Result;
use exposurelib::args::{crate_authors, crate_description, crate_name, crate_version, Args};
use exposurelib::config::ClientConfig;
use exposurelib::logger;
use serde_yaml;
use std::fs;
use tarpc::{client, context, tokio_serde::formats};
use exposurelib::rpcs;

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

    let keys = config.state.keys();
    // If exposed, notify and forward
    // First: Periodically fetch from DS
    // Listen for connections for timer duration
    // Store forwarding information

    let mut transport =
        tarpc::serde_transport::tcp::connect(&config.diagnosis_server_endpoint, formats::Json::default);
    transport.config_mut().max_frame_length(usize::MAX);

    // WorldClient is generated by the service attribute. It has a constructor `new` that takes a
    // config and any Transport as input.
    let client = rpcs::DiagnosisServerClient::new(client::Config::default(), transport.await?).spawn()?;

    // The client has an RPC method for each RPC defined in the annotated trait. It takes the same
    // args as defined, with the addition of a Context, which is always the first arg. The Context
    // specifies a deadline and trace information which can be helpful in debugging requests.
    let hello = client.hello(context::current(), String::from(config.name())).await?;

    logger::warn!("{}", hello);

    Ok(())
    // let listener = TcpListener::bind(&config.client_endpoint).await?;

    // loop {
    //     match listener.accept().await {
    //         Ok((socket, peer_addr)) => {
    //             logger::info!("Accepted new client {}", peer_addr);
    //         }
    //         Err(e) => logger::warn!("Could not accept client {:?}", e),
    //     }
    // }
}
