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
        if oti.encoding_symbol_length == 0 {
            return Err(FluteError::new("Encoding symbol length is null"));
        }

        let nb_source_symbols: usize =
            num_integer::div_ceil(buffer.len(), oti.encoding_symbol_length as usize);
        log::debug!(
            "nb_source_symbols={} encoding_symbol_length={}",
            nb_source_symbols,
            oti.encoding_symbol_length
        );
        let shards: Vec<Box<dyn FecShard>> = match oti.fec_encoding_id {
            oti::FECEncodingID::NoCode => Block::create_shards_no_code(oti, buffer),
            oti::FECEncodingID::ReedSolomonGF28
            | oti::FECEncodingID::ReedSolomonGF28UnderSpecified
            | oti::FECEncodingID::ReedSolomonGF2M => Block::create_shards_reed_solomon(
                oti,
                nb_source_symbols,
                block_length as usize,
                buffer,
            )?,
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

    /// Reed-Solomon over GF(2^m) <https://www.rfc-editor.org/rfc/rfc5510.html#section-8>,
    /// with m = 8 for FEC Encoding ID 5 and FEC Encoding ID 129 / FEC Instance ID 0
    fn create_shards_reed_solomon(
        oti: &Oti,
        nb_source_symbols: usize,
        block_length: usize,
        buffer: &[u8],
    ) -> Result<Vec<Box<dyn FecShard>>> {
        debug_assert!(nb_source_symbols <= oti.maximum_source_block_length as usize);
        debug_assert!(nb_source_symbols <= block_length);
        let encoder = fec::rsgf2m::RSGalois2MCodec::new(
            nb_source_symbols,
            nb_source_symbols + oti.max_number_of_parity_symbols as usize,
            oti.encoding_symbol_length as usize,
            &oti.reed_solomon_gf2m_scheme(),
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
            )?;

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

        if let Some(SchemeSpecific::Raptor(scheme)) = oti.scheme_specific.as_ref() {
            let encoder = fec::raptor::RaptorEncoder::new(
                nb_source_symbols,
                oti.max_number_of_parity_symbols as usize,
                oti.encoding_symbol_length as usize,
                scheme,
            )?;
            let shards = encoder.encode(buffer)?;
            Ok(shards)
        } else {
            Err(FluteError::new("Scheme specific for Raptor not defined"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Block;
    use crate::common::oti::Oti;

    #[test]
    pub fn test_reed_solomon_gf28_repair_symbols() {
        crate::tests::init();
        let data: Vec<u8> = (0..8usize).map(|i| (i * 31 + i / 7) as u8).collect();
        // FEC Encoding ID 5, 129 and 2 (m = 8) share the same code (RFC 5510 §8),
        // repair symbols computed by zfec and OpenFEC (interop/rs/run.sh)
        for oti in [
            Oti::new_reed_solomon_rs28(2, 4, 3).unwrap(),
            Oti::new_reed_solomon_rs28_under_specified(2, 4, 3).unwrap(),
            Oti::new_reed_solomon_rs2m(2, 4, 3, 8).unwrap(),
        ] {
            let mut block = Block::new_from_buffer(0, &data, 4, &oti).unwrap();
            let mut repair = Vec::new();
            while let Some((symbol, _)) = block.read() {
                if !symbol.is_source_symbol {
                    repair.push(symbol.symbols.to_vec());
                }
            }
            assert_eq!(
                repair,
                vec![vec![0x56, 0x21], vec![0x1d, 0xc3], vec![0x96, 0xed]],
                "{:?}",
                oti.fec_encoding_id
            );
        }
    }
}
