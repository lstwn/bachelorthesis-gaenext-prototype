use crate::primitives::{ComputationId, TekRollingPeriod, TemporaryExposureKey, Validity};
use crate::time::ExposureTimeSet;
use crate::{diagnosis_server_state::Chunk, time::ExposureTime};
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[tarpc::service]
pub trait DiagnosisServer {
    async fn blacklist_upload(params: BlacklistUploadParams) -> ComputationId;
    async fn greylist_upload(params: GreylistUploadParams) -> ();
    async fn download(params: DownloadParams) -> Vec<Chunk>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlacklistUploadParams {
    pub diagnosis_keys: HashSet<Validity<TemporaryExposureKey>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GreylistUploadParams {
    pub computation_id: ComputationId,
    pub diagnosis_keys: HashSet<Validity<TemporaryExposureKey>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadParams {
    pub from: DateTime<Utc>,
}

#[tarpc::service]
pub trait Forwarder {
    async fn forward(params: ForwardParams) -> ();
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForwardParams {
    pub computation_id: ComputationId,
    pub info: Validity<ForwardInfo>,
    pub shared_encounter_times: ExposureTimeSet,
}

impl ForwardParams {
    pub fn new(
        computation_id: ComputationId,
        valid_from: ExposureTime,
        tekrp: TekRollingPeriod,
        own_tek: TemporaryExposureKey,
        shared_encounter_times: ExposureTimeSet,
    ) -> Self {
        let forward_info = ForwardInfo {
            predecessor: PredecessorInfo::new(own_tek),
            origin: OriginInfo::new(own_tek),
        };
        Self {
            computation_id,
            info: Validity::new(valid_from, tekrp, forward_info),
            shared_encounter_times,
        }
    }
    pub fn update(
        &mut self,
        next_predecessor_tek: TemporaryExposureKey,
        next_shared_encounter_times: ExposureTimeSet,
    ) -> () {
        self.shared_encounter_times = next_shared_encounter_times;
        self.info
            .keyring_mut()
            .predecessor
            .update(next_predecessor_tek);
    }
    pub fn is_first_forward(&self) -> bool {
        self.info.keyring().origin.tek == self.info.keyring().predecessor.tek
    }
    pub fn computation_id(&self) -> ComputationId {
        self.computation_id
    }
    pub fn predecessor_tek(&self, tekrp: TekRollingPeriod) -> Validity<TemporaryExposureKey> {
        Validity::new(
            self.info.valid_from(),
            tekrp,
            self.info.keyring().predecessor.tek,
        )
    }
    pub fn origin_tek(&self, tekrp: TekRollingPeriod) -> Validity<TemporaryExposureKey> {
        Validity::new(
            self.info.valid_from(),
            tekrp,
            self.info.keyring().origin.tek,
        )
    }
    pub fn shared_encounter_times(&self) -> &ExposureTimeSet {
        &self.shared_encounter_times
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForwardInfo {
    pub predecessor: PredecessorInfo,
    pub origin: OriginInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredecessorInfo {
    pub tek: TemporaryExposureKey,
}

impl PredecessorInfo {
    pub fn new(origin_tek: TemporaryExposureKey) -> Self {
        Self { tek: origin_tek }
    }
    pub fn update(&mut self, next_predecessor_tek: TemporaryExposureKey) -> () {
        self.tek = next_predecessor_tek;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OriginInfo {
    pub tek: TemporaryExposureKey,
    // epk: EncryptedPublicKey,
}

impl OriginInfo {
    pub fn new(origin_tek: TemporaryExposureKey) -> Self {
        Self { tek: origin_tek }
    }
}
