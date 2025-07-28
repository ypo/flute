use crate::common::oti::{self, Oti, SchemeSpecific};
use crate::fec::{self, FecShard};
use crate::fec::{DataFecShard, FecEncoder};
use crate::tools::error::{FluteError, Result};

#[derive(Debug)]
pub struct Block {
    sbn: u32,
    read_index: u32,
    shards: Vec<Box<dyn FecShard>>,
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
        log::debug!(
            "nb_source_symbols={} encoding_symbol_length={}",
            nb_source_symbols,
            oti.encoding_symbol_length
        );
        let shards: Vec<Box<dyn FecShard>> = match oti.fec_encoding_id {
            oti::FECEncodingID::NoCode => Block::create_shards_no_code(oti, buffer),
            oti::FECEncodingID::ReedSolomonGF28 => Block::create_shards_reed_solomon_gf8(
                oti,
                nb_source_symbols,
                block_length as usize,
                buffer,
            )?,
            oti::FECEncodingID::ReedSolomonGF28UnderSpecified => {
                Block::create_shards_reed_solomon_gf8(
                    oti,
                    nb_source_symbols,
                    block_length as usize,
                    buffer,
                )?
            }
            oti::FECEncodingID::ReedSolomonGF2M => return Err(FluteError::new("Not implemented")),
            oti::FECEncodingID::RaptorQ => {
                Block::create_shards_raptorq(oti, nb_source_symbols, block_length as usize, buffer)?
            }
            oti::FECEncodingID::Raptor => {
                Block::create_shards_raptor(oti, nb_source_symbols, block_length as usize, buffer)?
            }
        };

        Ok(Box::new(Block {
            sbn,
            read_index: 0,
            shards,
            nb_source_symbols,
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.read_index as usize == self.shards.len()
    }

    pub fn read<'a>(&'a mut self) -> Option<(EncodingSymbol<'a>, bool)> {
        if self.is_empty() {
            return None;
        }
        let shard = self.shards[self.read_index as usize].as_ref();
        let esi = shard.esi();
        let is_source_symbol = (esi as usize) < self.nb_source_symbols;
        let symbol = EncodingSymbol {
            sbn: self.sbn,
            esi: shard.esi(),
            symbols: shard.data(),
            is_source_symbol,
        };
        self.read_index += 1;
        Some((symbol, self.is_empty()))
    }

    fn create_shards_no_code(oti: &Oti, buffer: &[u8]) -> Vec<Box<dyn FecShard>> {
        buffer
            .chunks(oti.encoding_symbol_length as usize)
            .enumerate()
            .map(|(index, chunk)| {
                Box::new(DataFecShard::new(chunk, index as u32)) as Box<dyn FecShard>
            })
            .collect()
    }

    fn create_shards_reed_solomon_gf8(
        oti: &Oti,
        nb_source_symbols: usize,
        block_length: usize,
        buffer: &[u8],
    ) -> Result<Vec<Box<dyn FecShard>>> {
        debug_assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
        debug_assert!(nb_source_symbols <= block_length);
        let encoder = fec::rscodec::RSGalois8Codec::new(
            nb_source_symbols,
            oti.max_number_of_parity_symbols as usize,
            oti.encoding_symbol_length as usize,
        )?;
        let shards = encoder.encode(buffer)?;
        Ok(shards)
    }

    fn create_shards_raptorq(
        oti: &Oti,
        nb_source_symbols: usize,
        block_length: usize,
        buffer: &[u8],
    ) -> Result<Vec<Box<dyn FecShard>>> {
        debug_assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
        debug_assert!(nb_source_symbols <= block_length);
        debug_assert!(oti.scheme_specific.is_some());

        if let Some(SchemeSpecific::RaptorQ(scheme)) = oti.scheme_specific.as_ref() {
            let encoder = fec::raptorq::RaptorQEncoder::new(
                nb_source_symbols,
                oti.max_number_of_parity_symbols as usize,
                oti.encoding_symbol_length as usize,
                scheme,
            );

            let shards = encoder.encode(buffer)?;
            Ok(shards)
        } else {
            Err(FluteError::new("Scheme specific for Raptorq not defined"))
        }
    }

    fn create_shards_raptor(
        oti: &Oti,
        nb_source_symbols: usize,
        block_length: usize,
        buffer: &[u8],
    ) -> Result<Vec<Box<dyn FecShard>>> {
        debug_assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
        debug_assert!(nb_source_symbols <= block_length);
        debug_assert!(oti.scheme_specific.is_some());

        let encoder = fec::raptor::RaptorEncoder::new(
            nb_source_symbols,
            oti.max_number_of_parity_symbols as usize,
        );
        let shards = encoder.encode(buffer)?;
        Ok(shards)
    }
}
