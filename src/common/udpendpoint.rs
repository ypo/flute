use std::hash::Hash;

use serde::{Deserialize, Serialize};

/// UDP Endpoint
#[derive(Debug, PartialEq, Deserialize, Serialize, Clone, Eq, Hash)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UDPEndpoint {
    /// Network source adress
    pub source_address: Option<String>,
    /// Network destination group address (multicast ip)
    pub destination_group_address: String,
    /// port
    pub port: u16,
}

impl UDPEndpoint {
    /// Create a new UDP Endpoint
    pub fn new(src: Option<String>, dest: String, port: u16) -> Self {
        Self {
            source_address: src,
            destination_group_address: dest,
            port,
        }
    }
}
