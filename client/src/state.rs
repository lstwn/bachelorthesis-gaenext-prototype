use anyhow::{Context, Result};
use chrono::prelude::*;
use exposurelib::config::{ClientConfig, Participant, SystemParams};
use exposurelib::diagnosis_server_state::Chunk;
use exposurelib::logger;
use exposurelib::primitives::*;
use exposurelib::rpcs;
use exposurelib::rpcs::{DownloadParams, ForwardParams};
use exposurelib::{
    client_state::{BluetoothLayer, Keys, Match},
    diagnosis_server_state::ListType,
};
use std::convert::TryFrom;
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};
use tarpc::{client, context, tokio_serde::formats};
use tokio::task;
use tokio::time;
use tokio::{sync::Mutex, task::JoinHandle};

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
    pub async fn new(config: ClientConfig) -> Result<JoinHandle<()>> {
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
            formats::Bincode::default,
        );
        transport.config_mut().max_frame_length(usize::MAX);
        let transport = transport.await.context("Error creating TCP transport")?;
        let diagnosis_server_client = Arc::new(
            rpcs::DiagnosisServerClient::new(client::Config::default(), transport)
                .spawn()
                .context("Error spawning diagnosis server client")?,
        );

        let client_state = Arc::new(Self {
            participant: config.participant,
            system_params: config.params,
            last_download,
            computations: Arc::new(Mutex::new(HashMap::new())),
            keys,
            bluetooth_layer,
            diagnosis_server_client,
        });
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
        let handle = client_state.update().await;
        Ok(handle)
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
        // TODO: LISTEN ON PORTS!!!
        Ok(())
    }
    async fn update(self: Arc<Self>) -> JoinHandle<()> {
        let refresh_period = std::time::Duration::from(self.system_params.refresh_period);
        let mut interval = time::interval(refresh_period);
        task::spawn(async move {
            loop {
                interval.tick().await;
                let from = {
                    let last_download = self.last_download.lock().await;
                    *last_download
                };
                logger::info!(
                    "Downloading latest chunks from {:?} from Diagnosis Server",
                    from
                );
                // TODO: proper retry strategy
                let updates: Vec<Chunk> = match self
                    .diagnosis_server_client
                    .download(context::current(), DownloadParams { from })
                    .await
                {
                    Ok(updates) => updates,
                    Err(e) => {
                        logger::error!("Errow while downloading DKs from Diagnosis Server: {}", e);
                        continue;
                    }
                };
                logger::debug!("Chunks: {:?}", updates);
                let mut next_last_download = from;
                for chunk in updates {
                    if *chunk.covers().to_excluding() > next_last_download {
                        next_last_download = chunk.covers().to_excluding().clone();
                    }
                    self.process_chunk(chunk).await;
                }
                let mut last_download = self.last_download.lock().await;
                *last_download = next_last_download;
            }
        })
    }
    async fn process_chunk(&self, chunk: Chunk) -> () {
        let bluetooth_layer = self.bluetooth_layer.lock().await;
        let filter_match = |tek| match Validity::<TekKeyring>::try_from(tek) {
            Ok(tek) => bluetooth_layer.match_with(tek),
            Err(e) => {
                logger::warn!("Could not derive RPIK and AEMK from TEK: {}", e);
                None
            }
        };
        for (computation_id, computation_state) in chunk.to_data().into_iter() {
            let (blacklist, greylist) = computation_state.to_data();
            {
                // in new scope to release lock after check here in order to enable
                // use of computations in on_tek_match()
                let computations = self.computations.lock().await;
                let keys = self.keys.lock().await;
                if computations.contains_key(&computation_id) {
                    if let Some(_) = greylist.iter().find(|tek| keys.is_own_tek(tek)) {
                        logger::warn!("WARNING: SSEV alert: client had a high-risk transitive contact with an infected participant.")
                    }
                }
            };
            for matched in blacklist.into_iter().filter_map(filter_match) {
                self.on_tek_match(matched, ListType::Blacklist, computation_id)
                    .await;
            }
            for matched in greylist.into_iter().filter_map(filter_match) {
                self.on_tek_match(matched, ListType::Greylist, computation_id)
                    .await;
            }
        }
    }
    async fn add_computation(&self, computation_id: ComputationId, computation: Computation) -> () {
        logger::info!("Adding new computation with {:?}", computation_id);
        let mut computations = self.computations.lock().await;
        match computations.insert(computation_id, computation) {
            Some(old_computation) => logger::error!(
                "Computation with {:?} already present with old: {:?}",
                computation_id,
                old_computation
            ),
            None => {}
        };
    }
    async fn on_tek_match(
        &self,
        matched: Match,
        from: ListType,
        computation_id: ComputationId,
    ) -> () {
        logger::info!(
            "TODO: on TEK match: Matched {:?} from {:?} with comp id {:?}",
            matched,
            from,
            computation_id
        );
        let computations = self.computations.lock().await;
        if from == ListType::Blacklist {
            if !matched.high_risk().is_empty() {
                logger::warn!(
                    "WARNING: client had a high-risk traced contact with an infected participant"
                ); // TODO: when? From<ExposureTime> for DateTime<Utc>
            } else {
                logger::warn!(
                    "WARNING: client had a low-risk traced contact with an infected participant"
                );
            }
        }
        if matched.high_risk().is_empty() {
            return;
        }
        if let Some(computation) = computations.get(&computation_id) {
            if from == ListType::Greylist && computation.redlist.contains(matched.tek()) {
                return;
            }
        }
        // TODO: get own tek of match !
        // TODO: add to computations, listen on ports and initiate forwarding of own tek
    }
    pub async fn on_tek_forward(&self, params: &ForwardParams) -> () {
        // upload, if I'm the pooling node!
    }
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
