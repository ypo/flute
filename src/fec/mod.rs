pub mod nocode;
pub mod raptor;
pub mod raptorq;
pub mod rscodec;

use crate::tools::error::Result;

pub trait FecShard: Send + Sync + std::fmt::Debug {
    fn data(&self) -> &[u8];
    fn esi(&self) -> u32;
}

#[derive(Debug)]
pub struct DataFecShard {
    shard: Vec<u8>,
    index: u32,
}

impl FecShard for DataFecShard {
    fn data(&self) -> &[u8] {
        self.shard.as_ref()
    }

    fn esi(&self) -> u32 {
        self.index
    }
}

impl DataFecShard {
    pub fn new(shard: &[u8], index: u32) -> Self {
        DataFecShard {
            shard: shard.to_vec(),
            index,
        }
    }
}

pub trait FecEncoder {
    fn encode(&self, data: &[u8]) -> Result<Vec<Box<dyn FecShard>>>;
}

pub trait FecDecoder {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32);
    fn can_decode(&self) -> bool;
    fn decode(&mut self) -> bool;
    fn source_block(&self) -> Result<&[u8]>;
}

impl std::fmt::Debug for dyn FecEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FecEncoder {{  }}")
    }
}

impl std::fmt::Debug for dyn FecDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FecDecoder {{  }}")
    }
}
