use crate::error::FluteError;

use super::FecDecoder;

/// Compact No-Code FEC Payload ID has a 16-bit Encoding Symbol ID (RFC 5445)
pub const MAX_SOURCE_SYMBOLS: usize = 1 << 16;

pub struct NoCodeDecoder {
    shards: Vec<Option<Vec<u8>>>,
    nb_symbols: usize,
    data: Option<Vec<u8>>,
}

impl NoCodeDecoder {
    pub fn new(nb_source_symbols: usize) -> crate::error::Result<NoCodeDecoder> {
        if nb_source_symbols == 0 || nb_source_symbols > MAX_SOURCE_SYMBOLS {
            return Err(FluteError::new(format!(
                "Invalid number of source symbols {} for No-Code FEC, must be in 1..={}",
                nb_source_symbols, MAX_SOURCE_SYMBOLS
            )));
        }

        Ok(NoCodeDecoder {
            shards: vec![None; nb_source_symbols],
            nb_symbols: 0,
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

        let mut output = Vec::new();
        for shard in &self.shards {
            output.extend(shard.as_ref().unwrap());
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

    #[test]
    fn test_nocode_decoder_source_symbols_limit() {
        crate::tests::init();
        assert!(NoCodeDecoder::new(0).is_err());
        assert!(NoCodeDecoder::new(1).is_ok());
        assert!(NoCodeDecoder::new(MAX_SOURCE_SYMBOLS).is_ok());
        assert!(NoCodeDecoder::new(MAX_SOURCE_SYMBOLS + 1).is_err());
        assert!(NoCodeDecoder::new(u32::MAX as usize).is_err());
    }
}
