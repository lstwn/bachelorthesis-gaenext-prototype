use anyhow::{Context, Result};
use chrono::prelude::*;
use exposurelib::config::{ClientConfig, Participant, SystemParams};
use exposurelib::diagnosis_server_state::Chunk;
use exposurelib::logger;
use exposurelib::primitives::*;
use exposurelib::rpcs;
use exposurelib::rpcs::{BlacklistUploadParams, ForwardParams, GreylistUploadParams};
use exposurelib::time::ExposureTimeSet;
use exposurelib::{
    client_state::{BluetoothLayer, Keys, Match},
    diagnosis_server_state::ListType,
};
use std::sync::Arc;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};
use std::{convert::TryFrom, time::Duration};
use tarpc::{client, context, tokio_serde::formats};
use tokio::sync::mpsc;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Event {
    NewChunks {
        last_from: DateTime<Utc>,
        chunks: Vec<Chunk>,
        resp: oneshot::Sender<DateTime<Utc>>,
    },
    NewForwardRequest {
        params: ForwardParams,
        resp: oneshot::Sender<Result<()>>,
    },
    ComputationPeriodExpired,
}

pub struct ClientState {
    participant: Participant,
    system_params: SystemParams,
    keys: Keys,
    bluetooth_layer: BluetoothLayer,
    computations: HashMap<ComputationId, Computation>,
    requests: mpsc::Receiver<Event>,
    listener: mpsc::Sender<Duration>,
    diagnosis_server: Arc<rpcs::DiagnosisServerClient>,
    traced_contact: bool,
    transitive_contact: bool,
}

impl ClientState {
    pub fn new(
        config: ClientConfig,
        diagnosis_server: Arc<rpcs::DiagnosisServerClient>,
        requests: mpsc::Receiver<Event>,
        listener: mpsc::Sender<Duration>,
    ) -> Self {
        Self {
            participant: config.participant,
            system_params: config.params,
            keys: config.state.keys,
            bluetooth_layer: config.state.bluetooth_layer,
            computations: HashMap::new(),
            requests,
            listener,
            diagnosis_server,
            traced_contact: false,
            transitive_contact: false,
        }
    }
    pub async fn run(mut self) -> ! {
        self.init().await.unwrap(); // insert favorite retry strategy here
        loop {
            let event = match self.requests.recv().await {
                Some(event) => event,
                None => panic!("Client sender all dropped"),
            };
            match event {
                Event::NewChunks {
                    last_from,
                    chunks,
                    resp,
                } => {
                    let mut next_from = last_from;
                    for chunk in chunks {
                        if *chunk.covers().to_excluding() > next_from {
                            next_from = chunk.covers().from_including().clone();
                        }
                        self.process_chunk(chunk).await;
                    }
                    resp.send(next_from).unwrap();
                }
                Event::NewForwardRequest { params, resp } => {
                    resp.send(self.on_tek_forward(params).await).unwrap();
                }
                Event::ComputationPeriodExpired => {
                    if self.participant.to_be_warned() {
                        if self.traced_contact
                            || self.transitive_contact
                            || self.participant.positively_tested()
                        {
                            logger::info!("Computation detected SSEV participant which is correct! :)")
                        } else {
                            logger::error!("Computation detected SSEV participant which is incorrect! :(");
                        }
                    } else {
                        if self.traced_contact
                            || self.transitive_contact
                            || self.participant.positively_tested()
                        {
                            logger::error!(
                                "Computation did not detect SSEV participant which is incorrect! :("
                            )
                        } else {
                            logger::info!(
                                "Computation did not detect SSEV participant which is correct! :)"
                            )
                        }
                    }
                }
            }
        }
    }
    async fn init(&mut self) -> Result<()> {
        if self.participant.positively_tested() {
            logger::warn!(
                "Participant is positively tested and announcing its TEKs to the blacklist"
            );
            let diagnosis_keys = self.keys.all_teks();
            self.listener
                .send(Duration::from(self.system_params.computation_period))
                .await
                .unwrap();
            let computation_id = self // insert favorite retry strategy here
                .diagnosis_server
                .blacklist_upload(context::current(), BlacklistUploadParams { diagnosis_keys })
                .await?;
            match self
                .computations
                .insert(computation_id, Computation::default())
            {
                Some(old_computation) => logger::error!(
                    "Computation with {:?} already present with old: {:?}",
                    computation_id,
                    old_computation
                ),
                None => {
                    logger::info!("Adding new computation with {:?}", computation_id);
                }
            }
        }
        Ok(())
    }
    async fn process_chunk(&mut self, chunk: Chunk) -> () {
        for (computation_id, computation_state) in chunk.to_data().into_iter() {
            let (blacklist, greylist) = computation_state.to_data();
            if self.computations.contains_key(&computation_id) {
                if let Some(_) = greylist.iter().find(|tek| self.keys.is_own_tek(tek)) {
                    if !self.traced_contact {
                        logger::warn!(
                            "WARNING: SSEV alert: participant had a high-risk \
                            transitive contact with another infected participant."
                        );
                    } else {
                        logger::info!(
                            "Participant is already a traced contact \
                            and therefore her transitive contact warning is omitted"
                        );
                    }
                    self.transitive_contact = true;
                }
            }
            for tek in blacklist.into_iter() {
                if let Err(e) = self
                    .on_tek_match(tek, ListType::Blacklist, computation_id)
                    .await
                {
                    logger::error!("Error during blacklist TEK match event: {}", e);
                }
            }
            for tek in greylist.into_iter() {
                if let Err(e) = self
                    .on_tek_match(tek, ListType::Greylist, computation_id)
                    .await
                {
                    logger::error!("Error during greylist TEK match event: {}", e);
                }
            }
        }
    }
    async fn on_tek_match(
        &mut self,
        tek: Validity<TemporaryExposureKey>,
        from: ListType,
        computation_id: ComputationId,
    ) -> Result<()> {
        let tek_keyring = Validity::<TekKeyring>::try_from(tek)
            .context(format!("Error deriving RPIK and AEMK from {:?}", tek))?;
        let matched = match self.bluetooth_layer.match_with(tek_keyring) {
            Some(matched) => matched,
            None => return Ok(()),
        };
        if from == ListType::Blacklist {
            if !matched.high_risk().is_empty() {
                logger::warn!(
                    "WARNING: participant had a high-risk traced contact with an infected participant"
                ); // TODO: when? From<ExposureTime> for DateTime<Utc>
                self.traced_contact = true;
            } else {
                logger::warn!(
                    "WARNING: participant had a low-risk traced contact with an infected participant"
                );
            }
        }
        if matched.high_risk().is_empty() {
            logger::info!(
                "Skipping TEK match due missing high risk encounter of {:?}",
                matched.tek()
            );
            return Ok(());
        }
        if let Some(computation) = self.computations.get(&computation_id) {
            if from == ListType::Greylist && computation.redlist().contains(matched.tek()) {
                logger::info!(
                    "Skipping TEK match due to redlist and greylist presence of {:?}",
                    matched.tek()
                );
                return Ok(());
            }
        }
        let tekrp = self.system_params.tek_rolling_period;
        let valid_from = matched.tek().valid_from();
        let own_tek = match self.keys.exposure_keyring(valid_from, tekrp) {
            Some(exposure_keyring) => Validity::new(
                valid_from,
                tekrp,
                TemporaryExposureKey::from(exposure_keyring.clone()),
            ),
            None => unreachable!("There should *always* be an own TEK for a tekrp during which a foreign TEK was matched"),
        };
        self.listener
            .send(Duration::from(self.system_params.computation_period))
            .await
            .unwrap();
        logger::info!(
            "New forwarding chain from origin to successor at {:?} of {:?}",
            matched.connection_identifier(),
            own_tek,
        );
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
        let computation = self
            .computations
            .entry(computation_id)
            .or_insert(Computation::default());
        computation.successors_mut().insert(matched);
        Ok(())
    }
    pub async fn on_tek_forward(&mut self, params: ForwardParams) -> Result<()> {
        let tekrp = self.system_params.tek_rolling_period;
        let origin_tek = params.origin_tek(tekrp);
        logger::info!("New forward request of {:?}", origin_tek);
        let predecessor_tek = params.predecessor_tek(tekrp);
        let predecessor_tek_keyring = Validity::<TekKeyring>::try_from(predecessor_tek.clone())
            .context(format!(
                "Error deriving RPIK and AEMK from {:?}",
                predecessor_tek
            ))?;
        let matched = match self.bluetooth_layer.match_with(predecessor_tek_keyring) {
            Some(matched) => matched,
            None => {
                logger::info!(
                    "Dropping forwarding due to missing match of {:?}",
                    origin_tek
                );
                return Ok(());
            }
        };
        let computation_id = params.computation_id();
        let computation = match self.computations.get_mut(&computation_id) {
            Some(computation) => computation,
            None => {
                logger::info!(
                    "Dropping forwarding due to unknown {:?} of {:?}",
                    computation_id,
                    origin_tek,
                );
                return Ok(());
            }
        };
        if params.is_first_forward() {
            match computation.redlist_mut().insert(predecessor_tek) {
                false => logger::warn!("Trying to add {:?} to redlist for the second time, possible double match of TEK?", origin_tek),
                true => logger::info!("Added to redlist with {:?} new {:?}", computation_id, origin_tek),
            }
        }
        if !computation.redlist().contains(&predecessor_tek) {
            logger::info!("Dropping {:?} forwarding due to missing entry of predecessor in the computation's redlist with {:?}", origin_tek, computation_id);
            return Ok(());
        }
        let shared_encounter_times: ExposureTimeSet = matched
            .high_risk()
            .intersection(params.shared_encounter_times())
            .cloned()
            .collect();
        if shared_encounter_times.is_empty() {
            logger::info!(
                "Dropping forwarding due to a missing shared encounter time of {:?}",
                origin_tek
            );
            return Ok(());
        }
        if computation.is_own() {
            let mut diagnosis_keys = HashSet::with_capacity(1);
            diagnosis_keys.insert(origin_tek);
            logger::info!(
                "Announcing to greylist on diagnosis server {:?}",
                origin_tek
            );
            self.diagnosis_server // insert favorite retry strategy here
                .greylist_upload(
                    context::current(),
                    GreylistUploadParams {
                        computation_id,
                        diagnosis_keys,
                    },
                )
                .await
                .context(format!(
                    "Pooling node could not upload received {:?} to greylist",
                    origin_tek
                ))?;
        } else {
            let valid_from = matched.tek().valid_from();
            let own_tek = match self.keys.exposure_keyring(valid_from, tekrp) {
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
                    logger::info!("Skipping forwarding to successor candidate at {:?} due to a missing shared encounter time", successor.connection_identifier());
                } else {
                    let mut params = params.clone();
                    params.update(own_tek, next_shared_encounter_times);
                    logger::info!(
                        "Forwarding to successor at {:?} {:?}",
                        successor.connection_identifier(),
                        origin_tek,
                    );
                    let client =
                        Self::get_forwarder_client(successor.connection_identifier()).await?;
                    client
                        .forward(context::current(), params)
                        .await
                        .context(format!(
                            "Error while forwarding {:?} to next successor at {:?}",
                            origin_tek,
                            successor.connection_identifier()
                        ))?;
                }
            }
        }
        Ok(())
    }
    async fn get_forwarder_client(endpoint: SocketAddr) -> Result<rpcs::ForwarderClient> {
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
