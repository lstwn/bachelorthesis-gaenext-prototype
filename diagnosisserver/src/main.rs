use anyhow::Result;
use exposurelib::args::{crate_authors, crate_description, crate_name, crate_version, Args};
use exposurelib::config::DiagnosisServerConfig;
use exposurelib::logger;
use std::fs;
use tokio::net::TcpListener;

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
    logger::trace!("Diagnosis Server started");

    let listener = TcpListener::bind(&config.endpoint).await?;

    loop {
        match listener.accept().await {
            Ok((socket, peer_addr)) => {
                logger::info!("Accepted new client {}", peer_addr);
            }
            Err(e) => logger::warn!("Could not accept client {:?}", e),
        }
    }
}
