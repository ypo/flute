pub mod rscodec;

use crate::tools::error::Result;

pub trait FecCodec {
    fn encode(&self, data: &[u8]) -> Result<Vec<Vec<u8>>>;
    fn decode(&self, shards: &mut Vec<Option<Vec<u8>>>) -> bool;
}

impl std::fmt::Debug for dyn FecCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FecCodec {{  }}")
    }
}
