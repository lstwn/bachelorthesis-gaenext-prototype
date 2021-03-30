use crate::{handler::ConnectionHandler, state::ClientState};
use anyhow::{Context, Result};
use exposurelib::logger;
use exposurelib::{config::ComputationPeriod, rpcs::Forwarder};
use futures::{future, prelude::*};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tarpc::server::{self, Channel, Incoming};
use tarpc::tokio_serde::formats;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time;

pub struct ForwarderServer {
    address: SocketAddr,
    timeout: ComputationPeriod,
    listening: Mutex<Option<mpsc::Sender<()>>>,
}

impl ForwarderServer {
    pub fn new(
        address: SocketAddr,
        timeout: ComputationPeriod,
    ) -> Arc<Self> {
        Arc::new(Self {
            address,
            timeout,
            listening: Mutex::new(None),
        })
    }
    pub async fn request(self: Arc<Self>, client_state: Arc<ClientState>) -> () {
        let listening = self.listening.lock().await.is_some();
        if !listening {
            self.start(client_state).await;
        } else {
            self.extend().await;
        }
    }
    async fn start(self: Arc<Self>, client_state: Arc<ClientState>) -> () {
        let (tx, rx) = mpsc::channel(100);
        {
            let mut listening = self.listening.lock().await;
            *listening = Some(tx);
        };
        let task_self = Arc::clone(&self);
        task::spawn(async move {
            let result = tokio::select! {
                result1 = Arc::clone(&task_self).timeout(rx) => result1,
                result2 = Arc::clone(&task_self).listen(client_state) => result2,
            };
            match result {
                Ok(_) => {
                    let mut listening = task_self.listening.lock().await;
                    *listening = None;
                }
                Err(e) => {
                    logger::error!("{}", e);
                }
            }
        });
    }
    async fn extend(self: Arc<Self>) -> () {
        match self
            .listening
            .lock()
            .await
            .as_mut()
            .expect("ForwarderServer should already be listening when extend() is called")
            .send(())
            .await
        {
            Ok(_) => logger::info!("Issueing new request to restart computation timeout"),
            Err(_) => logger::info!("Too late to issue a request to restart computation timeout"),
        }
    }
    async fn timeout(self: Arc<Self>, mut rx: mpsc::Receiver<()>) -> Result<()> {
        loop {
            time::sleep(Duration::from(self.timeout)).await;
            match futures::future::poll_fn(|cx| rx.poll_recv(cx)).await {
                Some(_) => {
                    // debug hint: hopefully poll_recv() *does* consume
                    // the value from the channel..
                    logger::info!("Restarting computation timeout due to new request");
                    continue;
                }
                None => {
                    logger::info!("Computation timeout expired, shutting down ForwardServer");
                    return Ok(());
                }
            };
        }
    }
    pub async fn listen(self: Arc<Self>, client_state: Arc<ClientState>) -> Result<()> {
        let mut listener =
            tarpc::serde_transport::tcp::listen(&self.address, formats::Bincode::default)
                .await
                .context("Error creating TCP Bincode listener")?;
        listener.config_mut().max_frame_length(usize::MAX);

        logger::info!("Starting to listen for forwardable TEKs at {:?} for at least {:?}", self.address, self.timeout);

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
                    Arc::clone(&client_state),
                );
                channel.requests().execute(server.serve())
            })
            // max 100 channels (i.e. clients)
            .buffer_unordered(100)
            .for_each(|_| async {})
            .await;

        Ok(())
    }
}
