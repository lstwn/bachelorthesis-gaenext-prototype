use anyhow::{Context, Result};
use chrono::prelude::*;
use exposurelib::client_state::{BluetoothLayer, Keys, Match};
use exposurelib::config::{ClientConfig, Participant, SystemParams};
use exposurelib::diagnosis_server_state::Chunk;
use exposurelib::logger;
use exposurelib::primitives::*;
use exposurelib::rpcs;
use exposurelib::rpcs::{DownloadParams, ForwardParams};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tarpc::{client, context, tokio_serde::formats};
use tokio::sync::Mutex;
use tokio::task;
use tokio::time;
use std::convert::TryInto;

pub struct ClientState {
    participant: Participant,
    system_params: SystemParams,
    last_download: Arc<Mutex<DateTime<Utc>>>,
    computations: Arc<Mutex<HashMap<ComputationId, Computation>>>,
    keys: Arc<Mutex<Keys>>,
    bluetooth_layer: Arc<Mutex<BluetoothLayer>>,
    diagnosis_server_client: Arc<rpcs::DiagnosisServerClient>,
}

impl ClientState {
    pub async fn new(config: ClientConfig) -> Result<Self> {
        let keys = Arc::new(Mutex::new(config.state.keys));
        let bluetooth_layer = Arc::new(Mutex::new(config.state.bluetooth_layer));
        let last_download = Arc::new(Mutex::new(
            Utc::now()
                - config
                    .params
                    .infection_period
                    .as_duration(config.params.tek_rolling_period),
        ));

        let mut transport = tarpc::serde_transport::tcp::connect(
            &config.diagnosis_server_endpoint,
            formats::Json::default,
        );
        transport.config_mut().max_frame_length(usize::MAX);
        let transport = transport.await.context("Error creating TCP transport")?;

        let diagnosis_server_client = Arc::new(
            rpcs::DiagnosisServerClient::new(client::Config::default(), transport)
                .spawn()
                .context("Error spawning diagnosis server client")?,
        );

        let client_state = Self {
            participant: config.participant,
            system_params: config.params,
            last_download,
            computations: Arc::new(Mutex::new(HashMap::new())),
            keys,
            bluetooth_layer,
            diagnosis_server_client,
        };
        loop {
            match client_state.init().await {
                Ok(_) => break,
                Err(err) => {
                    logger::error!("Error while announcing DKs to blacklist: {}", err);
                    let sleep = 3;
                    logger::info!(
                        "Reattempting to announce DKs to blacklist in {} seconds",
                        sleep
                    );
                    time::sleep(std::time::Duration::from_secs(sleep)).await;
                }
            }
        }
        client_state.update().await;
        Ok(client_state)
    }
    async fn init(&self) -> Result<()> {
        if self.participant.positively_tested {
            logger::warn!("Client is positively tested and announcing its TEKs to the blacklist");
            let diagnosis_keys = {
                let keys = self.keys.lock().await;
                keys.all_teks()
            };
            let computation_id = self
                .diagnosis_server_client
                .blacklist_upload(
                    context::current(),
                    rpcs::BlacklistUploadParams { diagnosis_keys },
                )
                .await?;
            self.add_computation(computation_id, Computation::default())
                .await;
        }
        Ok(())
    }
    async fn update(&self) -> () {
        let diagnosis_server_client = Arc::clone(&self.diagnosis_server_client);
        let last_download = Arc::clone(&self.last_download);
        let bluetooth_layer = Arc::clone(&self.bluetooth_layer);
        let refresh_period = std::time::Duration::from(self.system_params.refresh_period);
        let mut interval = time::interval(refresh_period);
        task::spawn(async move {
            loop {
                interval.tick().await;
                let from = {
                    let last_download = last_download.lock().await;
                    *last_download
                };
                // TODO: proper retry strategy
                let updates: Vec<Chunk> = match diagnosis_server_client
                    .download(context::current(), DownloadParams { from })
                    .await
                {
                    Ok(updates) => updates,
                    Err(e) => {
                        logger::error!("Errow while downloading DKs from Diagnosis Server: {}", e);
                        break;
                    }
                };
                let mut next_last_download = from;
                for chunk in updates {
                    if *chunk.covers().to_excluding() > next_last_download {
                        next_last_download = chunk.covers().to_excluding().clone();
                    }
                    let bluetooth_layer = bluetooth_layer.lock().await;
                    for (computation_id, computation_state) in chunk.data().iter() {
                        for blacklist_tek in computation_state.blacklist().iter() {
                            // TODO: skip tek if conversion fails
                            let option = bluetooth_layer.match_with(blacklist_tek.clone().try_into().unwrap());
                        }
                        for greylist_tek in computation_state.greylist().iter() {
                            let option = bluetooth_layer.match_with(greylist_tek.clone().try_into().unwrap());
                        }
                    }
                    // TODO: Did I meet ?
                }
                let mut last_download = last_download.lock().await;
                *last_download = next_last_download;
            }
        });
    }
    async fn add_computation(&self, computation_id: ComputationId, computation: Computation) -> () {
        logger::info!("Adding new computation with {:?}", computation_id);
        let mut computations = self.computations.lock().await;
        match computations.insert(computation_id, computation) {
            Some(old_computation) => logger::error!(
                "Computation with {:?} already present with old values: {:?}",
                computation_id,
                old_computation
            ),
            None => {}
        };
    }
    pub async fn on_tek_forward(&self, params: &ForwardParams) -> () {}
    async fn on_tek_match(&self, tek: Validity<TemporaryExposureKey>) -> () {}
}

#[derive(Default, Debug)]
pub struct Computation {
    successors: HashSet<Match>,
    redlist: HashSet<Validity<TemporaryExposureKey>>,
}

impl Computation {
    pub fn is_own(&self) -> bool {
        self.successors.is_empty()
    }
}
