use crate::tools::error::{FluteError, Result};

use super::{DataFecShard, FecDecoder, FecEncoder, FecShard};

#[derive(Debug)]
pub struct RSCodecParam {
    nb_source_symbols: usize,
    nb_parity_symbols: usize,
    encoding_symbol_length: usize,
}

#[derive(Debug)]
pub struct RSGalois8Codec {
    params: RSCodecParam,
    rs: reed_solomon_erasure::galois_8::ReedSolomon,
    decode_shards: Vec<Option<Vec<u8>>>,
    decode_block: Option<Vec<u8>>,
    nb_source_symbols_received: usize,
    nb_encoding_symbols_received: usize,
}

impl RSCodecParam {
    fn create_shards(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut shards: Vec<Vec<u8>> = data
            .chunks(self.encoding_symbol_length)
            .map(|chunck| chunck.to_vec())
            .collect();

        let last = shards.last_mut().unwrap();
        if last.len() < self.encoding_symbol_length {
            last.resize(self.encoding_symbol_length, 0)
        }
        if shards.len() != self.nb_source_symbols {
            return Err(FluteError::new(format!(
                "nb source symbols is {} instead of {}",
                shards.len(),
                self.nb_source_symbols
            )));
        }

        for _ in 0..self.nb_parity_symbols {
            shards.push(vec![0; self.encoding_symbol_length]);
        }
        Ok(shards)
    }
}

impl RSGalois8Codec {
    pub fn new(
        nb_source_symbols: usize,
        nb_parity_symbols: usize,
        encoding_symbol_length: usize,
    ) -> Result<RSGalois8Codec> {
        let rs =
            reed_solomon_erasure::galois_8::ReedSolomon::new(nb_source_symbols, nb_parity_symbols)
                .map_err(|_| FluteError::new("Fail to create RS codec"))?;

        Ok(RSGalois8Codec {
            params: RSCodecParam {
                nb_source_symbols,
                nb_parity_symbols,
                encoding_symbol_length,
            },
            rs,
            decode_shards: vec![None; nb_source_symbols + nb_parity_symbols],
            decode_block: None,
            nb_source_symbols_received: 0,
            nb_encoding_symbols_received: 0,
        })
    }
}

impl FecDecoder for RSGalois8Codec {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.decode_block.is_some() {
            return;
        }

        log::info!("Receive ESI {}", esi);
        if self.decode_shards.len() <= esi as usize {
            return;
        }

        if self.decode_shards[esi as usize].is_some() {
            return;
        }

        self.decode_shards[esi as usize] = Some(encoding_symbol.to_vec());
        if esi < self.params.nb_source_symbols as u32 {
            self.nb_source_symbols_received += 1;
        }
        self.nb_encoding_symbols_received += 1;
    }

    fn can_decode(&self) -> bool {
        self.nb_encoding_symbols_received >= self.params.nb_source_symbols
    }

    fn decode(&mut self) -> bool {
        if self.decode_block.is_some() {
            return true;
        }

        if self.nb_source_symbols_received < self.params.nb_source_symbols {
            match self.rs.reconstruct(&mut self.decode_shards) {
                Ok(_) => {
                    log::info!("Reconstruct with success !");
                }
                Err(e) => {
                    log::error!("{:?}", e);
                    return false;
                }
            };
        }

        let mut output = Vec::new();
        for i in 0..self.params.nb_source_symbols {
            if self.decode_shards[i].is_none() {
                log::error!("BUG? a shard is missing");
                return false;
            }
            output.extend(self.decode_shards[i].as_ref().unwrap());
        }

        self.decode_block = Some(output);
        true
    }

    fn source_block(&self) -> Result<&[u8]> {
        match self.decode_block.as_ref() {
            Some(e) => Ok(e),
            None => Err(FluteError::new("Block not decoded")),
        }
    }
}

impl FecEncoder for RSGalois8Codec {
    fn encode(&self, data: &[u8]) -> Result<Vec<Box<dyn FecShard>>> {
        let mut shards = self.params.create_shards(data)?;
        self.rs
            .encode(&mut shards)
            .map_err(|_| FluteError::new("Fail to encode RS"))?;

        let shards: Vec<Box<dyn FecShard>> = shards
            .into_iter()
            .enumerate()
            .map(|(index, shard)| {
                Box::new(DataFecShard {
                    shard,
                    index: index as u32,
                }) as Box<dyn FecShard>
            })
            .collect();

        Ok(shards)
    }
}

#[cfg(test)]
mod tests {
    use crate::fec::FecEncoder;
    #[test]
    pub fn test_encoder() {
        crate::tests::init();
        let data = vec![1, 2, 3, 4, 5];
        let encoder = super::RSGalois8Codec::new(2, 3, 4).unwrap();
        let _shards = encoder.encode(&data).unwrap();
    }
}
