use crate::state::Event;
use anyhow::{Context, Result};
use chrono::prelude::*;
use exposurelib::config::RefreshPeriod;
use exposurelib::logger;
use exposurelib::rpcs::{self, DownloadParams};
use std::net::SocketAddr;
use std::time::Duration;
use tarpc::{client, context, tokio_serde::formats};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use tokio::time;

pub struct Updater {
    diagnosis_server: rpcs::DiagnosisServerClient,
    refresh_period: RefreshPeriod,
    client_state: Sender<Event>,
    from: DateTime<Utc>,
}

impl Updater {
    pub async fn new(
        diagnosis_server_endpoint: SocketAddr,
        refresh_period: RefreshPeriod,
        from: DateTime<Utc>,
        client_state: Sender<Event>,
    ) -> Result<Self> {
        let mut transport = tarpc::serde_transport::tcp::connect(
            diagnosis_server_endpoint,
            formats::Bincode::default,
        );
        transport.config_mut().max_frame_length(usize::MAX);
        let transport = transport.await.context(format!(
            "Error creating TCP Bincode connect with diagnosis server at {:?}",
            diagnosis_server_endpoint
        ))?;
        let diagnosis_server =
            rpcs::DiagnosisServerClient::new(client::Config::default(), transport)
                .spawn()
                .context("Error spawning diagnosis server client")?;
        Ok(Self {
            diagnosis_server,
            refresh_period,
            from,
            client_state,
        })
    }
    pub async fn run(mut self) -> ! {
        let refresh_period = Duration::from(self.refresh_period);
        let mut interval = time::interval(refresh_period);
        loop {
            interval.tick().await;
            logger::info!(
                "Downloading latest chunks from {:?} from diagnosis server",
                self.from
            );
            let (tx, rx) = oneshot::channel();
            let new_chunks_event = match self
                .diagnosis_server
                .download(context::current(), DownloadParams { from: self.from })
                .await
            {
                Ok(updates) => {
                    if updates.len() == 0 {
                        logger::debug!("No new chunks");
                        continue;
                    }
                    logger::debug!("New chunks: {:?}", updates);
                    Event::NewChunks {
                        last_from: self.from,
                        chunks: updates,
                        resp: tx,
                    }
                }
                Err(e) => {
                    logger::error!("Errow while downloading DKs from diagnosis server: {}", e);
                    continue;
                }
            };
            self.client_state.send(new_chunks_event).await.unwrap();
            self.from = rx.await.unwrap();
        }
    }
}
