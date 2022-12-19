use super::oti::Oti;
use crate::fec;
use crate::tools::error::{FluteError, Result};
use crate::fec::FecCodec;

pub struct Block {
    snb: u32,
    esi: u32,
    shards: Vec<Vec<u8>>,
    nb_source_symbols: usize,
}

pub struct EncodingSymbol<'a> {
    pub snb: u32,
    pub esi: u32,
    pub symbols: &'a [u8],
    pub is_source_symbol: bool,
}

impl Block {
    pub fn new_from_buffer(
        snb: u32,
        buffer: &[u8],
        block_length: u64,
        oti: &Oti,
    ) -> Result<Box<Block>> {
        let nb_source_symbols: usize =
            num_integer::div_ceil(buffer.len(), oti.encoding_symbol_length as usize);
        let shards: Vec<Vec<u8>> = match oti.fec {
            super::oti::FECEncodingID::NoCode => buffer
                .chunks(oti.encoding_symbol_length as usize)
                .map(|chunk| chunk.to_vec())
                .collect(),
            super::oti::FECEncodingID::ReedSolomonGF28 => {
                assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
                assert!(nb_source_symbols <= block_length as usize);
                let encoder = fec::rscodec::RSCodec::new(
                    nb_source_symbols,
                    oti.max_number_of_parity_symbols as usize,
                    oti.encoding_symbol_length as usize,
                )?;
                let shards = encoder.encode(&buffer)?;
                shards
            }
            super::oti::FECEncodingID::ReedSolomonGF2M => {
                return Err(FluteError::new("Not implemented"))
            }
        };

        Ok(Box::new(Block {
            snb,
            esi: 0,
            shards: shards,
            nb_source_symbols,
        }))
    }

    pub fn read(&mut self) -> Option<EncodingSymbol> {
        if self.esi as usize == self.shards.len() {
            return None;
        }
        let shard = self.shards[self.esi as usize].as_slice();
        let is_source_symbol = (self.esi as usize) < self.nb_source_symbols;
        let symbol = EncodingSymbol {
            snb: self.snb,
            esi: self.esi,
            symbols: shard,
            is_source_symbol,
        };
        self.esi += 1;
        Some(symbol)
    }
}
