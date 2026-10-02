//! Internals exposed to the benchmarks (`cargo bench --features bench`), not a stable API

use super::rsgf2m::RSGalois2MCodec;
use super::{FecDecoder, FecEncoder};
use crate::common::oti::ReedSolomonGF2MSchemeSpecific;

pub use super::gf256::{mul_add_with, Kernel};

/// Reed-Solomon codec over GF(2^m) of FEC Encoding ID 2 (and of FEC Encoding ID 5 and 129 when m = 8)
#[derive(Debug, Clone, Copy)]
pub struct ReedSolomon {
    m: u8,
    nb_source_symbols: usize,
    nb_encoding_symbols: usize,
    encoding_symbol_length: usize,
}

impl ReedSolomon {
    /// Code of k source symbols and n encoding symbols of E bytes, over GF(2^m)
    pub fn new(m: u8, k: usize, n: usize, e: usize) -> Self {
        let codec = ReedSolomon {
            m,
            nb_source_symbols: k,
            nb_encoding_symbols: n,
            encoding_symbol_length: e,
        };
        codec.codec();
        codec
    }

    /// Returns the n encoding symbols of a source block of k * E bytes
    pub fn encode(&self, data: &[u8]) -> Vec<Vec<u8>> {
        self.codec()
            .encode(data)
            .unwrap()
            .iter()
            .map(|shard| shard.data().to_vec())
            .collect()
    }

    /// Decodes the source block from k encoding symbols (ESI, symbol)
    pub fn decode(&self, symbols: &[(u32, &[u8])]) -> Vec<u8> {
        let mut codec = self.codec();
        for &(esi, symbol) in symbols {
            codec.push_symbol(symbol, esi);
        }
        assert!(codec.decode());
        codec.source_block().unwrap().to_vec()
    }

    fn codec(&self) -> RSGalois2MCodec {
        let scheme = ReedSolomonGF2MSchemeSpecific { m: self.m, g: 1 };
        RSGalois2MCodec::new(
            self.nb_source_symbols,
            self.nb_encoding_symbols,
            self.encoding_symbol_length,
            &scheme,
        )
        .unwrap()
    }
}
