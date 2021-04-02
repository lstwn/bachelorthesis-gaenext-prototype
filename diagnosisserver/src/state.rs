use chrono::prelude::*;
use chrono::Duration;
use exposurelib::diagnosis_server_state::{Chunk, ListType};
use exposurelib::logger;
use exposurelib::primitives::ComputationId;
use exposurelib::rpcs::{BlacklistUploadParams, DownloadParams, GreylistUploadParams};
use exposurelib::time::TimeInterval;
use exposurelib::{
    config::DiagnosisServerConfig,
    primitives::{TemporaryExposureKey, Validity},
};
use std::collections::{HashSet, VecDeque};
use std::iter::IntoIterator;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time;

pub struct DiagnosisServerState {
    current_chunk: Arc<Mutex<Chunk>>,
    done_chunks: Arc<Mutex<Chunks>>,
    computation_id_seed: Mutex<u32>,
}

impl DiagnosisServerState {
    pub fn new(config: &DiagnosisServerConfig) -> Self {
        let chunk_period = Duration::from(config.params.chunk_period);
        let retention_period = config
            .params
            .infection_period
            .as_duration(config.params.tek_rolling_period);
        let done_chunks = Chunks::new(retention_period);
        let current_chunk = Chunk::new(TimeInterval::with_alignment(chunk_period));
        let diagnosis_server_state = Self {
            done_chunks: Arc::new(Mutex::new(done_chunks)),
            current_chunk: Arc::new(Mutex::new(current_chunk)),
            computation_id_seed: Mutex::new(0),
        };
        diagnosis_server_state.update();
        diagnosis_server_state
    }
    fn update(&self) -> () {
        let done_chunks = Arc::clone(&self.done_chunks);
        let current_chunk = Arc::clone(&self.current_chunk);
        task::spawn(async move {
            loop {
                // new scope is important to release the lock before sleep
                let sleep = {
                    let current_chunk = current_chunk.lock().await;
                    let next_deadline = current_chunk.covers().to_excluding();
                    let time_to_next_deadline = *next_deadline - Utc::now();
                    time_to_next_deadline
                        .to_std()
                        .unwrap_or_else(|_| std::time::Duration::from_millis(0))
                };
                logger::debug!("Sleeping for {:?} before advancing next chunk", sleep);
                time::sleep(sleep).await;
                let mut current_chunk = current_chunk.lock().await;
                let mut done_chunks = done_chunks.lock().await;
                let next_chunk = current_chunk.next_chunk();
                logger::debug!(
                    "Replacing current chunk with validity {:?} with next chunk with validity {:?}",
                    current_chunk.covers(),
                    next_chunk.covers()
                );
                let current_chunk = std::mem::replace(&mut *current_chunk, next_chunk);
                done_chunks.add_done_chunk(current_chunk);
            }
        });
    }
    pub async fn add_to_blacklist(&self, data: BlacklistUploadParams) -> ComputationId {
        let mut current_chunk = self.current_chunk.lock().await;
        let computation_id = self.next_computation_id().await;
        // deduplication not strictly necessary here but let's make it more robust..
        let done_chunks = self.done_chunks.lock().await;
        let diagnosis_keys_refs = &data.diagnosis_keys.iter().collect();
        let (deduplicated, duplicates) =
            done_chunks.deduplicate(ListType::Blacklist, computation_id, &diagnosis_keys_refs);
        if !duplicates.is_empty() {
            logger::info!(
                "Not adding to blacklist with {:?} the following duplicate DKs: {:?}",
                computation_id,
                duplicates
            );
        }
        if !deduplicated.is_empty() {
            logger::info!(
                "Adding to blacklist with {:?} the following DKs: {:?}",
                computation_id,
                deduplicated
            );
            // deduplication for current chunk not necessary due to set usage
            current_chunk.insert(ListType::Blacklist, computation_id, deduplicated);
        }
        computation_id
    }
    pub async fn add_to_greylist(&self, data: GreylistUploadParams) -> () {
        let mut current_chunk = self.current_chunk.lock().await;
        let computation_id = data.computation_id;
        let done_chunks = self.done_chunks.lock().await;
        let diagnosis_keys_refs = &data.diagnosis_keys.iter().collect();
        let (deduplicated, duplicates) =
            done_chunks.deduplicate(ListType::Greylist, computation_id, &diagnosis_keys_refs);
        if !duplicates.is_empty() {
            logger::info!(
                "Not adding to greylist with {:?} the following duplicate DKs: {:?}",
                computation_id,
                duplicates
            );
        }
        if !deduplicated.is_empty() {
            logger::info!(
                "Adding to greylist with {:?} the following DKs: {:?}",
                computation_id,
                deduplicated
            );
            // deduplication for current chunk not necessary due to set usage
            current_chunk.insert(ListType::Greylist, computation_id, deduplicated);
        }
    }
    pub async fn request_chunks(&self, data: DownloadParams) -> Vec<Chunk> {
        let done_chunks = self.done_chunks.lock().await;
        logger::debug!("Client requests chunks from {}", data.from);
        done_chunks.get_chunks(&data.from)
    }
    async fn next_computation_id(&self) -> ComputationId {
        let mut computation_id_seed = self.computation_id_seed.lock().await;
        let current = *computation_id_seed;
        logger::debug!(
            "Advancing computation id from {} to {}",
            current,
            current + 1
        );
        *computation_id_seed += 1;
        ComputationId::from(current)
    }
}

#[derive(Debug)]
struct Chunks {
    retention_period: Duration,
    inner: VecDeque<Chunk>,
}

impl Chunks {
    fn new(retention_period: Duration) -> Self {
        Self {
            retention_period,
            inner: VecDeque::new(),
        }
    }
    fn add_done_chunk(&mut self, chunk: Chunk) -> () {
        self.inner.push_front(chunk);
        if let Some(chunk) = self.inner.back() {
            if *chunk.covers().to_excluding() <= Utc::now() - self.retention_period {
                logger::info!(
                    "Pruning oldest chunk with validity {:?} due to it exceeding the retention period of {:?}",
                    chunk.covers(),
                    self.retention_period
                );
                self.inner.pop_back();
            }
        }
    }
    fn get_chunks(&self, from: &DateTime<Utc>) -> Vec<Chunk> {
        match self.inner.front() {
            Some(newest) => {
                if from >= newest.covers().from_including() {
                    Vec::new()
                } else {
                    self.inner
                        .iter()
                        .take_while(|chunk| !chunk.covers().contains(from))
                        .cloned()
                        .collect()
                }
            }
            None => Vec::new(),
        }
    }
    fn deduplicate<'a>(
        &'a self,
        list: ListType,
        computation_id: ComputationId,
        candidates: &'a HashSet<&'a Validity<TemporaryExposureKey>>,
    ) -> (
        HashSet<&'a Validity<TemporaryExposureKey>>,
        HashSet<&'a Validity<TemporaryExposureKey>>,
    ) {
        let mut deduplicated: HashSet<&Validity<TemporaryExposureKey>> = candidates.clone();
        for chunk in self.into_iter() {
            match chunk.data().get(&computation_id) {
                Some(computation_state) => {
                    let list: HashSet<&Validity<TemporaryExposureKey>> = match list {
                        ListType::Blacklist => computation_state.blacklist().iter().collect(),
                        ListType::Greylist => computation_state.greylist().iter().collect(),
                    };
                    deduplicated = deduplicated
                        .difference(&list)
                        .into_iter()
                        .map(|x| *x)
                        .collect();
                }
                None => continue,
            }
        }
        let duplicates: HashSet<&Validity<TemporaryExposureKey>> = candidates
            .difference(&deduplicated)
            .into_iter()
            .map(|x| *x)
            .collect();
        (deduplicated, duplicates)
    }
}

// let's make Chunks iterable because why not
impl<'a> IntoIterator for &'a Chunks {
    type Item = &'a Chunk;
    type IntoIter = std::collections::vec_deque::Iter<'a, Chunk>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}
