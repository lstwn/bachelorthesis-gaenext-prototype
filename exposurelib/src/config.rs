use serde::{Serialize, Deserialize};
use std::net::IpAddr;

#[derive(Serialize, Deserialize)]
pub struct ClientConfig {
    name: String,
    positively_tested: bool,
    endpoint: IpAddr,
    // bluetooth_layer: 
}

#[derive(Serialize, Deserialize)]
pub struct DiagnosisServerConfig {
    endpoint: IpAddr,
}
