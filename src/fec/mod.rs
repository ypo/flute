pub mod raptorq;
pub mod rscodec;

use crate::tools::error::Result;

#[derive(Debug, Copy, Clone)]
pub enum ShardType {
    SourceSymbol,
    RepairSymbol,
}

pub trait FecShard: Send + std::fmt::Debug {
    fn data(&self) -> &[u8];
    fn esi(&self) -> u32;
    fn get_type(&self) -> ShardType;
}

#[derive(Debug)]
pub struct DataFecShard {
    shard: Vec<u8>,
    index: u32,
    shard_type: ShardType,
}

impl FecShard for DataFecShard {
    fn data(&self) -> &[u8] {
        self.shard.as_ref()
    }

    fn esi(&self) -> u32 {
        self.index
    }

    fn get_type(&self) -> ShardType {
        self.shard_type
    }
}

impl DataFecShard {
    pub fn new(shard: &[u8], index: u32, shard_type: ShardType) -> Self {
        DataFecShard {
            shard: shard.to_vec(),
            index,
            shard_type,
        }
    }
}

pub trait FecCodec {
    fn encode(&self, data: &[u8]) -> Result<Vec<Box<dyn FecShard>>>;
    fn decode(&self, sbn: u32, shards: &mut Vec<Option<Vec<u8>>>) -> bool;
    // , fountain codes ,potentially limitless sequence of encoding symbols
    fn is_fountain(&self) -> bool;
}

impl std::fmt::Debug for dyn FecCodec {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "FecCodec {{  }}")
    }
}
