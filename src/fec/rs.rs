use crate::tools::error::{FluteError, Result};
use reed_solomon_erasure::galois_8::ReedSolomon;

pub struct Encoder {
    nb_source_symbols: usize,
    nb_parity_symbols: usize,
    encoding_symbol_length: usize,
    rs: ReedSolomon,
}

impl Encoder {
    pub fn new(
        nb_source_symbols: usize,
        nb_parity_symbols: usize,
        encoding_symbol_length: usize,
    ) -> Result<Encoder> {
        log::debug!(
            "Create encoder nb source_symbols={} nb parity={}",
            nb_source_symbols,
            nb_parity_symbols
        );
        let rs = ReedSolomon::new(nb_source_symbols, nb_parity_symbols)
            .map_err(|_| FluteError::new("Fail to create RS encoder"))?;

        Ok(Encoder {
            nb_source_symbols,
            nb_parity_symbols,
            encoding_symbol_length,
            rs,
        })
    }

    pub fn encode(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        let mut shards = self.create_shards(data)?;
        self.rs
            .encode(&mut shards)
            .map_err(|_| FluteError::new("Fail to encode RS"))?;
        Ok(shards)
    }

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

#[cfg(test)]
mod tests {
    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_encoder() {
        init();
        let mut data = vec![1, 2, 3, 4, 5];
        let encoder = super::Encoder::new(2, 3, 4).unwrap();
        let shards = encoder.encode(&data).unwrap();
        log::info!("{:?}", shards);
    }
}
