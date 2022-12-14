use quick_xml::se;

use super::oti::Oti;
use super::pkt::Pkt;
use crate::fec;
use crate::tools::error::{FluteError, Result};

pub struct Block {
    snb: u32,
    esi: u32,
    shards: Vec<Vec<u8>>,
}

pub struct EncodingSymbol<'a> {
    pub snb: u32,
    pub esi: u32,
    pub symbols: &'a [u8],
}

impl Block {
    pub fn new_from_buffer(
        snb: u32,
        buffer: &[u8],
        block_length: u64,
        oti: &Oti,
    ) -> Result<Box<Block>> {
        let shards: Vec<Vec<u8>> = match oti.fec {
            super::oti::FECEncodingID::NoCode => buffer
                .chunks(oti.encoding_symbol_length as usize)
                .map(|chunck| chunck.to_vec())
                .collect(),
            super::oti::FECEncodingID::ReedSolomonGF28 => {
                let nb_source_symbols =
                    num_integer::div_ceil(buffer.len(), oti.encoding_symbol_length as usize);
                assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
                assert!(nb_source_symbols <= block_length as usize);
                let encoder = fec::rs::Encoder::new(
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
        }))
    }

    pub fn read(&mut self) -> Option<EncodingSymbol> {
        log::debug!("Read esi={}/{}", self.esi, self.shards.len());
        if self.esi as usize == self.shards.len() {
            return None;
        }

        let shard = self.shards[self.esi as usize].as_slice();
        let symbol = EncodingSymbol {
            snb: self.snb,
            esi: self.esi,
            symbols: shard,
        };
        self.esi += 1;
        Some(symbol)
    }
}
