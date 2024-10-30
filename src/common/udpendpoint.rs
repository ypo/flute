use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::SystemTime,
};

use chrono::Datelike;
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

    /// Generate a u128bits Trace-ID
    pub fn trace_id(
        &self,
        tsi: u64,
        toi: u128,
        fdt_instance_id: Option<u32>,
        now: SystemTime,
    ) -> u128 {
        let mut hasher_endpoint = DefaultHasher::new();
        let mut hasher_tsi_toi = DefaultHasher::new();
        self.hash(&mut hasher_endpoint);

        tsi.hash(&mut hasher_tsi_toi);
        toi.hash(&mut hasher_tsi_toi);
        fdt_instance_id.hash(&mut hasher_tsi_toi);

        let date: chrono::DateTime<chrono::Utc> = now.into();
        let day = date.day();
        day.hash(&mut hasher_tsi_toi);

        let endpoint_hash = hasher_endpoint.finish();
        let toi_tsi_hash = hasher_tsi_toi.finish();

        ((endpoint_hash as u128) << 64) | toi_tsi_hash as u128
    }
}
