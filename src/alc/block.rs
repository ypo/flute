use super::oti::{self, Oti};
use crate::fec;
use crate::fec::FecCodec;
use crate::tools::error::{FluteError, Result};

#[derive(Debug)]
pub struct Block {
    sbn: u32,
    esi: u32,
    shards: Vec<Vec<u8>>,
    pub nb_source_symbols: usize,
}

pub struct EncodingSymbol<'a> {
    pub sbn: u32,
    pub esi: u32,
    pub symbols: &'a [u8],
    pub is_source_symbol: bool,
}

impl Block {
    pub fn new_from_buffer(
        sbn: u32,
        buffer: &[u8],
        block_length: u64,
        oti: &Oti,
    ) -> Result<Box<Block>> {
        let nb_source_symbols: usize =
            num_integer::div_ceil(buffer.len(), oti.encoding_symbol_length as usize);
        let shards: Vec<Vec<u8>> = match oti.fec_encoding_id {
            oti::FECEncodingID::NoCode => buffer
                .chunks(oti.encoding_symbol_length as usize)
                .map(|chunk| chunk.to_vec())
                .collect(),
            oti::FECEncodingID::ReedSolomonGF28 => Block::create_shards_reed_solomon_gf8(
                oti,
                nb_source_symbols,
                block_length as usize,
                buffer,
            )?,

            oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic => {
                Block::create_shards_reed_solomon_gf8(
                    oti,
                    nb_source_symbols,
                    block_length as usize,
                    buffer,
                )?
            }
            oti::FECEncodingID::ReedSolomonGF2M => return Err(FluteError::new("Not implemented")),
        };

        Ok(Box::new(Block {
            sbn,
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
            sbn: self.sbn,
            esi: self.esi,
            symbols: shard,
            is_source_symbol,
        };
        self.esi += 1;
        Some(symbol)
    }

    fn create_shards_reed_solomon_gf8(
        oti: &Oti,
        nb_source_symbols: usize,
        block_length: usize,
        buffer: &[u8],
    ) -> Result<Vec<Vec<u8>>> {
        assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
        assert!(nb_source_symbols <= block_length as usize);
        let encoder = fec::rscodec::RSGalois8Codec::new(
            nb_source_symbols,
            oti.max_number_of_parity_symbols as usize,
            oti.encoding_symbol_length as usize,
        )?;
        let shards = encoder.encode(&buffer)?;
        Ok(shards)
    }
}
