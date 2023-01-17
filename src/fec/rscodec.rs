use crate::tools::error::{FluteError, Result};

use super::{DataFecShard, FecCodec, FecShard, ShardType};

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
}

impl RSCodecParam {
    fn create_shards(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut shards: Vec<Vec<u8>> = data
            .chunks(self.encoding_symbol_length as usize)
            .map(|chunck| chunck.to_vec())
            .collect();

        let last = shards.last_mut().unwrap();
        if last.len() < self.encoding_symbol_length as usize {
            last.resize(self.encoding_symbol_length as usize, 0)
        }
        if shards.len() != self.nb_source_symbols {
            return Err(FluteError::new(format!(
                "nb source symbols is {} instead of {}",
                shards.len(),
                self.nb_source_symbols
            )));
        }

        for _ in 0..self.nb_parity_symbols {
            shards.push(vec![0; self.encoding_symbol_length as usize]);
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
        })
    }
}

impl FecCodec for RSGalois8Codec {
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
                    shard_type: match index < self.params.nb_source_symbols {
                        true => ShardType::SourceSymbol,
                        _ => ShardType::RepairSymbol,
                    },
                }) as Box<dyn FecShard>
            })
            .collect();

        Ok(shards)
    }

    fn decode(&self, _sbn: u32, shards: &mut Vec<Option<Vec<u8>>>) -> bool {
        match self.rs.reconstruct(shards) {
            Ok(_) => {
                log::info!("Reconstruct with success !");
                true
            }
            Err(e) => {
                log::error!("{:?}", e);
                false
            }
        }
    }

    fn is_fountain(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::fec::FecCodec;
    #[test]
    pub fn test_encoder() {
        crate::tests::init();
        let data = vec![1, 2, 3, 4, 5];
        let encoder = super::RSGalois8Codec::new(2, 3, 4).unwrap();
        let _shards = encoder.encode(&data).unwrap();
    }
}
