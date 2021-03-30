use crate::state::ClientState;
use exposurelib::logger;
use exposurelib::rpcs::{ForwardParams, Forwarder};
use std::net::SocketAddr;
use std::sync::Arc;
use tarpc::context::Context;

#[derive(Clone)]
pub struct ConnectionHandler {
    peer_addr: SocketAddr,
    state: Arc<ClientState>,
}

impl ConnectionHandler {
    pub fn new(peer_addr: SocketAddr, state: Arc<ClientState>) -> Self {
        Self { peer_addr, state }
    }
}

#[tarpc::server]
impl Forwarder for ConnectionHandler {
    async fn forward(self, context: Context, params: ForwardParams) -> () {
        logger::debug!(
            "New forward() RPC from {:?} with context {:?} and params {:?}",
            self.peer_addr,
            context,
            params
        );
        if let Err(e) = self.state.on_tek_forward(params).await {
            logger::warn!("Error while forwarding TEK: {:?}", e);
        }
    }
}
