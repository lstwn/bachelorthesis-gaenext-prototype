use crate::diagnosis_server_state::Chunk;
use crate::primitives::{ComputationId, TemporaryExposureKey, Validity};
use crate::time::ExposureTimeSet;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[tarpc::service]
pub trait DiagnosisServer {
    async fn hello(world: String) -> String;
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardParams {
    pub computation_id: ComputationId,
    pub info: Validity<ForwardInfo>,
    pub shared_encounter_times: ExposureTimeSet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardInfo {
    pub predecessor: PredecessorInfo,
    pub origin: OriginInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PredecessorInfo {
    pub tek: TemporaryExposureKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OriginInfo {
    pub tek: TemporaryExposureKey,
    // epk: EncryptedPublicKey,
}
