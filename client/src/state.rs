use crate::listener::ForwarderServer;
use anyhow::{Context, Result};
use chrono::prelude::*;
use exposurelib::config::{ClientConfig, Participant, SystemParams};
use exposurelib::diagnosis_server_state::Chunk;
use exposurelib::logger;
use exposurelib::primitives::*;
use exposurelib::rpcs;
use exposurelib::rpcs::{
    BlacklistUploadParams, DownloadParams, ForwardParams, GreylistUploadParams,
};
use exposurelib::time::ExposureTimeSet;
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
    forwarder_server: Arc<ForwarderServer>,
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
        let transport = transport.await.context(format!(
            "Error creating TCP Bincode connect with diagnosis server at {:?}",
            config.diagnosis_server_endpoint
        ))?;
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
            forwarder_server: ForwarderServer::new(
                config.client_endpoint,
                config.params.computation_period,
            ),
        });
        loop {
            match Arc::clone(&client_state).init().await {
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
    async fn init(self: Arc<Self>) -> Result<()> {
        if self.participant.positively_tested {
            logger::warn!("Client is positively tested and announcing its TEKs to the blacklist");
            let diagnosis_keys = {
                let keys = self.keys.lock().await;
                keys.all_teks()
            };
            Arc::clone(&self.forwarder_server)
                .request(Arc::clone(&self))
                .await;
            let computation_id = self
                .diagnosis_server_client
                .blacklist_upload(context::current(), BlacklistUploadParams { diagnosis_keys })
                .await?;
            self.add_computation(computation_id, Computation::default())
                .await;
        }
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
                logger::debug!("New chunks: {:?}", updates);
                let mut next_last_download = from;
                for chunk in updates {
                    if *chunk.covers().to_excluding() > next_last_download {
                        next_last_download = chunk.covers().from_including().clone();
                    }
                    Arc::clone(&self).process_chunk(chunk).await;
                }
                let mut last_download = self.last_download.lock().await;
                *last_download = next_last_download;
            }
        })
    }
    async fn process_chunk(self: Arc<Self>, chunk: Chunk) -> () {
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
                if let Err(e) = Arc::clone(&self)
                    .on_tek_match(matched, ListType::Blacklist, computation_id)
                    .await
                {
                    logger::error!("Error during blacklist TEK match event: {}", e);
                }
            }
            for matched in greylist.into_iter().filter_map(filter_match) {
                if let Err(e) = Arc::clone(&self)
                    .on_tek_match(matched, ListType::Greylist, computation_id)
                    .await
                {
                    logger::error!("Error during greylist TEK match event: {}", e);
                }
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
        self: Arc<Self>,
        matched: Match,
        from: ListType,
        computation_id: ComputationId,
    ) -> Result<()> {
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
            logger::info!("Skipping TEK match due missing high risk encounter");
            return Ok(());
        }
        let mut computations = self.computations.lock().await;
        if let Some(computation) = computations.get(&computation_id) {
            if from == ListType::Greylist && computation.redlist().contains(matched.tek()) {
                logger::info!("Skipping TEK match due to redlist and greylist presence");
                return Ok(());
            }
        }
        let keys = self.keys.lock().await;
        let tekrp = self.system_params.tek_rolling_period;
        let valid_from = matched.tek().valid_from();
        let own_tek = match keys.exposure_keyring(valid_from, tekrp) {
            Some(exposure_keyring) => Validity::new(
                valid_from,
                tekrp,
                TemporaryExposureKey::from(exposure_keyring.clone()),
            ),
            None => unreachable!("There should *always* be an own TEK for a tekrp during which a foreign TEK was matched"),
        };
        logger::info!("New TEK forwarding chain from origin");
        Arc::clone(&self.forwarder_server)
            .request(Arc::clone(&self))
            .await;
        let client = Self::get_forwarder_client(matched.connection_identifier()).await?;
        client
            .forward(
                context::current(),
                ForwardParams::new(
                    computation_id,
                    valid_from,
                    tekrp,
                    own_tek.to_keyring(),
                    matched.high_risk().clone(),
                ),
            )
            .await
            .context("Error while sending first forward from origin")?;
        let computation = computations
            .entry(computation_id)
            .or_insert(Computation::default());
        computation.successors_mut().insert(matched);
        Ok(())
    }
    pub async fn get_forwarder_client(endpoint: SocketAddr) -> Result<rpcs::ForwarderClient> {
        let mut transport =
            tarpc::serde_transport::tcp::connect(&endpoint, formats::Bincode::default);
        transport.config_mut().max_frame_length(usize::MAX);
        let transport = transport.await.context(format!(
            "Error creating TCP Bincode connect with client at {:?}",
            endpoint
        ))?;
        rpcs::ForwarderClient::new(client::Config::default(), transport)
            .spawn()
            .context("Error spawning forwarder client")
    }
    pub async fn on_tek_forward(self: Arc<Self>, params: ForwardParams) -> Result<()> {
        logger::info!("New TEK forward request");
        let tekrp = self.system_params.tek_rolling_period;
        let predecessor_tek = params.predecessor_tek(tekrp);
        let predecessor_tek_keyring = Validity::<TekKeyring>::try_from(predecessor_tek.clone())
            .context("Error deriving RPIK and AEMK from TEK")?;
        let bluetooth_layer = self.bluetooth_layer.lock().await;
        let matched = match bluetooth_layer.match_with(predecessor_tek_keyring) {
            Some(matched) => matched,
            None => {
                logger::info!("Dropping TEK forwarding due to missing match");
                return Ok(());
            }
        };
        let computation_id = params.computation_id();
        let mut computations = self.computations.lock().await;
        let computation = match computations.get_mut(&computation_id) {
            Some(computation) => computation,
            None => {
                logger::info!(
                    "Dropping TEK forwarding due to unknown computation {:?}",
                    computation_id
                );
                return Ok(());
            }
        };
        if params.is_first_forward() {
            match computation.redlist_mut().insert(predecessor_tek) {
                false => logger::warn!("Trying to add TEK to redlist for the second time, possible double match of TEK?"),
                true => logger::info!("Added new TEK to redlist"),
            }
        }
        if !computation.redlist().contains(&predecessor_tek) {
            logger::info!("Dropping TEK forwarding due to missing entry of predecessor in the computation's redlist");
            return Ok(());
        }
        let shared_encounter_times: ExposureTimeSet = matched
            .high_risk()
            .intersection(params.shared_encounter_times())
            .cloned()
            .collect();
        if shared_encounter_times.is_empty() {
            logger::info!("Dropping TEK forwarding due to a missing shared encounter time");
            return Ok(());
        }
        if computation.is_own() {
            let mut diagnosis_keys = HashSet::with_capacity(1);
            diagnosis_keys.insert(params.origin_tek(tekrp));
            logger::info!("Announcing TEK to greylist on Diagnosis Server");
            // TODO: retry strategy
            self.diagnosis_server_client
                .greylist_upload(
                    context::current(),
                    GreylistUploadParams {
                        computation_id,
                        diagnosis_keys,
                    },
                )
                .await
                .context("Pooling node could not upload received DK to greylist")?;
        } else {
            let valid_from = matched.tek().valid_from();
            let keys = self.keys.lock().await;
            let own_tek = match keys.exposure_keyring(valid_from, tekrp) {
                Some(exposure_keyring) =>
                TemporaryExposureKey::from(exposure_keyring.clone()),
                None => unreachable!("There should *always* be an own TEK for a tekrp during which a foreign TEK was matched"),
            };
            for successor in computation.successors() {
                let next_shared_encounter_times: ExposureTimeSet = shared_encounter_times
                    .intersection(successor.high_risk())
                    .cloned()
                    .collect();
                if next_shared_encounter_times.is_empty() {
                    logger::info!("Skipping forwarding to successor candidate due to a missing shared encounter time");
                } else {
                    let mut params = params.clone();
                    params.update(own_tek, next_shared_encounter_times);
                    logger::info!("Forwarding TEK to successor");
                    let client =
                        Self::get_forwarder_client(successor.connection_identifier()).await?;
                    client
                        .forward(context::current(), params)
                        .await
                        .context("Error while forwarding tek to next successor")?;
                }
            }
        }
        Ok(())
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
    pub fn successors(&self) -> &HashSet<Match> {
        &self.successors
    }
    pub fn successors_mut(&mut self) -> &mut HashSet<Match> {
        &mut self.successors
    }
    pub fn redlist(&self) -> &HashSet<Validity<TemporaryExposureKey>> {
        &self.redlist
    }
    pub fn redlist_mut(&mut self) -> &mut HashSet<Validity<TemporaryExposureKey>> {
        &mut self.redlist
    }
}
