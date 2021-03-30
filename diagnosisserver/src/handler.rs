use crate::state::DiagnosisServerState;
use exposurelib::diagnosis_server_state::Chunk;
use exposurelib::logger;
use exposurelib::primitives::ComputationId;
use exposurelib::rpcs::{
    BlacklistUploadParams, DiagnosisServer, DownloadParams, GreylistUploadParams,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tarpc::context::Context;

#[derive(Clone)]
pub struct ConnectionHandler {
    peer_addr: SocketAddr,
    state: Arc<DiagnosisServerState>,
}

impl ConnectionHandler {
    pub fn new(peer_addr: SocketAddr, state: Arc<DiagnosisServerState>) -> Self {
        Self { peer_addr, state }
    }
}

#[tarpc::server]
impl DiagnosisServer for ConnectionHandler {
    async fn blacklist_upload(
        self,
        _context: Context,
        params: BlacklistUploadParams,
    ) -> ComputationId {
        logger::trace!(
            "New blacklist_upload() RPC from {:?} with context {:?} and params {:?}",
            self.peer_addr,
            _context,
            params
        );
        self.state.add_to_blacklist(&params).await
    }
    async fn greylist_upload(self, context: Context, params: GreylistUploadParams) -> () {
        logger::trace!(
            "New greylist_upload() RPC from {:?} with context {:?} and params {:?}",
            self.peer_addr,
            context,
            params
        );
        self.state.add_to_greylist(&params).await
    }
    async fn download(self, context: Context, params: DownloadParams) -> Vec<Chunk> {
        logger::trace!(
            "New download() RPC from {:?} with context {:?} and params {:?}",
            self.peer_addr,
            context,
            params
        );
        self.state.request_chunks(&params).await
    }
}
