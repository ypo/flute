use super::{DataFecShard, FecDecoder, FecEncoder, FecShard, ShardType};
use crate::error::{FluteError, Result};

pub struct RaptorEncoder {
    nb_parity_symbols: usize,
    nb_source_symbols: usize,
}

impl RaptorEncoder {
    pub fn new(nb_parity_symbols: usize, nb_source_symbols: usize) -> RaptorEncoder {
        RaptorEncoder {
            nb_parity_symbols,
            nb_source_symbols,
        }
    }
}

impl FecEncoder for RaptorEncoder {
    fn encode(&self, data: &[u8]) -> Result<Vec<Box<dyn super::FecShard>>> {
        let mut encoder = raptor_code::SourceBlockEncoder::new(&data, self.nb_source_symbols);
        let nb_source_symbols = encoder.nb_source_symbols() as usize;
        let n = nb_source_symbols + self.nb_parity_symbols;

        let mut output: Vec<Box<dyn FecShard>> = Vec::new();

        for esi in 0..n {
            let shard = DataFecShard {
                shard: encoder.fountain(esi as u32),
                index: esi as u32,
                shard_type: match esi >= nb_source_symbols {
                    true => ShardType::RepairSymbol,
                    false => ShardType::SourceSymbol,
                },
            };
            output.push(Box::new(shard));
        }

        Ok(output)
    }
}

pub struct RaptorDecoder {
    source_block_size: usize,
    decoder: raptor_code::SourceBlockDecoder,
    data: Option<Vec<u8>>,
}

impl RaptorDecoder {
    pub fn new(nb_source_symbols: usize, encoding_symbol_length: usize) -> RaptorDecoder {
        let source_block_size = nb_source_symbols * encoding_symbol_length;
        RaptorDecoder {
            decoder: raptor_code::SourceBlockDecoder::new(nb_source_symbols),
            source_block_size,
            data: None,
        }
    }
}

impl FecDecoder for RaptorDecoder {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.data.is_some() {
            return;
        }

        self.decoder.push_encoding_symbol(encoding_symbol, esi)
    }

    fn can_decode(&self) -> bool {
        self.decoder.fully_specified()
    }

    fn decode(&mut self) -> bool {
        self.data = self.decoder.decode(self.source_block_size);
        self.data.is_some()
    }

    fn source_block(&self) -> Result<&[u8]> {
        match self.data.as_ref() {
            Some(e) => Ok(e),
            None => Err(FluteError::new("Block not decoded")),
        }
    }
}
