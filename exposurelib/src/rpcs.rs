use crate::diagnosis_server_state::Chunks;
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
    async fn download(params: DownloadParams) -> Chunks;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlacklistUploadParams {
    diagnosis_keys: HashSet<Validity<TemporaryExposureKey>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GreylistUploadParams {
    computation_id: ComputationId,
    diagnosis_keys: HashSet<Validity<TemporaryExposureKey>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadParams {
    from: DateTime<Utc>,
}

#[tarpc::service]
pub trait Forwarder {
    async fn forward(params: ForwardParams) -> ();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardParams {
    computation_id: ComputationId,
    info: Validity<ForwardInfo>,
    shared_encounter_times: ExposureTimeSet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForwardInfo {
    predecessor: PredecessorInfo,
    origin: OriginInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PredecessorInfo {
    tek: TemporaryExposureKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OriginInfo {
    tek: TemporaryExposureKey,
    // epk: EncryptedPublicKey,
}
