use crate::error::FluteError;

use super::FecDecoder;

/// Compact No-Code FEC Payload ID has a 16-bit Encoding Symbol ID (RFC 5445)
pub const MAX_SOURCE_SYMBOLS: usize = 1 << 16;

pub struct NoCodeDecoder {
    shards: Vec<Option<Vec<u8>>>,
    nb_symbols: usize,
    encoding_symbol_length: usize,
    data: Option<Vec<u8>>,
}

impl NoCodeDecoder {
    pub fn new(
        nb_source_symbols: usize,
        encoding_symbol_length: usize,
    ) -> crate::error::Result<NoCodeDecoder> {
        if nb_source_symbols == 0 || nb_source_symbols > MAX_SOURCE_SYMBOLS {
            return Err(FluteError::new(format!(
                "Invalid number of source symbols {} for No-Code FEC, must be in 1..={}",
                nb_source_symbols, MAX_SOURCE_SYMBOLS
            )));
        }

        if encoding_symbol_length == 0 {
            return Err(FluteError::new("Encoding symbol length is 0"));
        }

        Ok(NoCodeDecoder {
            shards: vec![None; nb_source_symbols],
            nb_symbols: 0,
            encoding_symbol_length,
            data: None,
        })
    }
}

impl FecDecoder for NoCodeDecoder {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.shards.len() <= esi as usize {
            log::error!("ESI {} > {}", esi, self.shards.len());
            return;
        }

        if encoding_symbol.is_empty() || encoding_symbol.len() > self.encoding_symbol_length {
            log::error!(
                "Encoding symbol of {} bytes, expected 1..={}",
                encoding_symbol.len(),
                self.encoding_symbol_length
            );
            return;
        }

        if self.shards[esi as usize].is_some() {
            return;
        }

        self.shards[esi as usize] = Some(encoding_symbol.to_vec());
        self.nb_symbols += 1;
    }

    fn can_decode(&self) -> bool {
        self.nb_symbols == self.shards.len()
    }

    fn decode(&mut self) -> bool {
        if self.data.is_some() {
            return true;
        }

        if !self.can_decode() {
            return false;
        }

        // Every symbol takes E bytes in the source block; only the last symbol of the object
        // is expected to be shorter, its padding is removed by the block writer
        let mut output = Vec::with_capacity(self.shards.len() * self.encoding_symbol_length);
        for shard in &self.shards {
            output.extend(shard.as_ref().unwrap());
            output.resize(
                output.len().next_multiple_of(self.encoding_symbol_length),
                0,
            );
        }

        self.data = Some(output);
        true
    }

    fn source_block(&self) -> crate::error::Result<&[u8]> {
        match self.data.as_ref() {
            Some(e) => Ok(e),
            None => Err(FluteError::new("Block not decoded")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_SOURCE_SYMBOLS, NoCodeDecoder};
    use crate::fec::FecDecoder;

    #[test]
    fn test_nocode_decoder_source_symbols_limit() {
        crate::tests::init();
        assert!(NoCodeDecoder::new(0, 1).is_err());
        assert!(NoCodeDecoder::new(1, 1).is_ok());
        assert!(NoCodeDecoder::new(MAX_SOURCE_SYMBOLS, 1).is_ok());
        assert!(NoCodeDecoder::new(MAX_SOURCE_SYMBOLS + 1, 1).is_err());
        assert!(NoCodeDecoder::new(u32::MAX as usize, 1).is_err());
        assert!(NoCodeDecoder::new(1, 0).is_err());
    }

    #[test]
    fn test_nocode_decoder_symbol_size() {
        crate::tests::init();
        let mut decoder = NoCodeDecoder::new(3, 4).unwrap();
        // Empty and oversized symbols are ignored
        decoder.push_symbol(&[], 0);
        decoder.push_symbol(&[9; 5], 0);
        assert!(!decoder.can_decode());

        // A short symbol keeps its E-byte slot in the source block
        decoder.push_symbol(&[1, 1], 0);
        decoder.push_symbol(&[2; 4], 1);
        decoder.push_symbol(&[3], 2);
        assert!(decoder.decode());
        assert_eq!(
            decoder.source_block().unwrap(),
            &[1, 1, 0, 0, 2, 2, 2, 2, 3, 0, 0, 0]
        );
    }
}
