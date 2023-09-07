use crate::error::FluteError;

use super::FecDecoder;

pub struct NoCodeDecoder {
    shards: Vec<Option<Vec<u8>>>,
    nb_symbols: usize,
    data: Option<Vec<u8>>,
}

impl NoCodeDecoder {
    pub fn new(nb_source_symbols: usize) -> NoCodeDecoder {
        NoCodeDecoder {
            shards: vec![None; nb_source_symbols],
            nb_symbols: 0,
            data: None,
        }
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
