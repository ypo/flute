use super::{DataFecShard, FecDecoder, FecEncoder, FecShard};
use crate::common::oti::RaptorSchemeSpecific;
use crate::error::{FluteError, Result};

/// Length of the sub-symbols of each sub-block
///
/// (TL, TS, NL, NS) = Partition[T/Al, N]  
/// The first NL sub-blocks have sub-symbols of TL*Al bytes, the remaining NS sub-blocks have sub-symbols of TS*Al bytes
/// <https://www.rfc-editor.org/rfc/rfc5053.html#section-5.3.1.2>
fn sub_symbols_length(
    encoding_symbol_length: usize,
    scheme: &RaptorSchemeSpecific,
) -> Result<Vec<usize>> {
    let al = scheme.symbol_alignment as usize;
    let n = scheme.sub_blocks_length as usize;

    if al == 0 {
        return Err(FluteError::new("Al must be at least 1"));
    }

    if encoding_symbol_length % al != 0 {
        return Err(FluteError::new(
            "Encoding symbols length must be a multiple of Al",
        ));
    }

    // A sub-symbol is at least Al bytes
    let nb_al = encoding_symbol_length / al;
    if n == 0 || n > nb_al {
        return Err(FluteError::new(format!(
            "Number of sub-blocks N={} must be between 1 and T/Al={}",
            n, nb_al
        )));
    }

    let tl = num_integer::div_ceil(nb_al, n);
    let ts = nb_al / n;
    let nl = nb_al - ts * n;
    Ok((0..n)
        .map(|j| if j < nl { tl * al } else { ts * al })
        .collect())
}

pub struct RaptorEncoder {
    nb_parity_symbols: usize,
    nb_source_symbols: usize,
    encoding_symbol_length: usize,
    sub_symbols_length: Vec<usize>,
}

impl RaptorEncoder {
    pub fn new(
        nb_source_symbols: usize,
        nb_parity_symbols: usize,
        encoding_symbol_length: usize,
        scheme: &RaptorSchemeSpecific,
    ) -> Result<RaptorEncoder> {
        Ok(RaptorEncoder {
            nb_parity_symbols,
            nb_source_symbols,
            encoding_symbol_length,
            sub_symbols_length: sub_symbols_length(encoding_symbol_length, scheme)?,
        })
    }
}

impl FecEncoder for RaptorEncoder {
    fn encode(&self, data: &[u8]) -> Result<Vec<Box<dyn super::FecShard>>> {
        let k = self.nb_source_symbols;
        if data.len() > k * self.encoding_symbol_length {
            return Err(FluteError::new(format!(
                "Source block of {} bytes does not fit into {} symbols of {} bytes",
                data.len(),
                k,
                self.encoding_symbol_length
            )));
        }

        // The last source symbol is padded with zeros
        let mut source_block = data.to_vec();
        source_block.resize(k * self.encoding_symbol_length, 0);

        // Each sub-block is made of K contiguous sub-symbols and is encoded separately
        let mut encoders = Vec::new();
        let mut offset = 0;
        for sub_symbol_length in &self.sub_symbols_length {
            let sub_block = &source_block[offset..offset + k * sub_symbol_length];
            let encoder = raptor_code::SourceBlockEncoder::new(sub_block, k)
                .map_err(|_| FluteError::new("Fail to create Raptor codec"))?;
            encoders.push(encoder);
            offset += k * sub_symbol_length;
        }

        let n = k + self.nb_parity_symbols;
        let mut output: Vec<Box<dyn FecShard>> = Vec::new();

        // An encoding symbol is the concatenation of the sub-symbols with the same ESI of each sub-block
        for esi in 0..n as u32 {
            let mut shard = Vec::with_capacity(self.encoding_symbol_length);
            for (encoder, sub_symbol_length) in encoders.iter_mut().zip(&self.sub_symbols_length) {
                let mut sub_symbol = encoder.fountain(esi);
                sub_symbol.resize(*sub_symbol_length, 0);
                shard.extend(sub_symbol);
            }
            log::info!("Encode shard {}", shard.len());
            output.push(Box::new(DataFecShard { shard, index: esi }));
        }

        Ok(output)
    }
}

/// Raptor decoder of a source block
///
/// All the sub-blocks share the same K and the same ESI, so the Raptor code is applied identically
/// to every byte of an encoding symbol. The full encoding symbols are therefore decoded with a single
/// Raptor decoder, and the sub-blocks are de-interleaved once the K source symbols are recovered.
///
/// The Raptor code is systematic: when all the source symbols (ESI < K) are received, the block is
/// rebuilt without running the Raptor decoder. The decoder is only started when at least K symbols
/// are received and some source symbols are missing.
pub struct RaptorDecoder {
    nb_source_symbols: usize,
    source_block_size: usize,
    encoding_symbol_length: usize,
    sub_symbols_length: Vec<usize>,
    /// Received source symbols, the source symbol `esi` is stored at `esi * T`
    source_symbols: Vec<u8>,
    source_received: Vec<bool>,
    nb_source_received: usize,
    /// Repair symbols received before the Raptor decoder is started
    repair_symbols: Vec<(u32, Vec<u8>)>,
    decoder: Option<raptor_code::SourceBlockDecoder>,
    data: Option<Vec<u8>>,
}

impl RaptorDecoder {
    pub fn new(
        nb_source_symbols: usize,
        source_block_size: usize,
        encoding_symbol_length: usize,
        scheme: &RaptorSchemeSpecific,
    ) -> Result<RaptorDecoder> {
        log::info!(
            "new RaptorDecoder nb_source_symbols={} source_block_size={}",
            nb_source_symbols,
            source_block_size
        );
        let sub_symbols_length = sub_symbols_length(encoding_symbol_length, scheme)?;

        Ok(RaptorDecoder {
            nb_source_symbols,
            source_block_size,
            encoding_symbol_length,
            sub_symbols_length,
            source_symbols: vec![0; nb_source_symbols * encoding_symbol_length],
            source_received: vec![false; nb_source_symbols],
            nb_source_received: 0,
            repair_symbols: Vec::new(),
            decoder: None,
            data: None,
        })
    }

    fn all_source_symbols_received(&self) -> bool {
        self.nb_source_received == self.nb_source_symbols
    }

    fn start_decoder(&mut self) {
        log::debug!(
            "Start Raptor decoder, {}/{} source symbols received",
            self.nb_source_received,
            self.nb_source_symbols
        );
        let t = self.encoding_symbol_length;
        let mut decoder = raptor_code::SourceBlockDecoder::new(self.nb_source_symbols);
        for (esi, _) in self
            .source_received
            .iter()
            .enumerate()
            .filter(|(_, received)| **received)
        {
            decoder.push_encoding_symbol(&self.source_symbols[esi * t..(esi + 1) * t], esi as u32);
        }

        for (esi, symbol) in std::mem::take(&mut self.repair_symbols) {
            decoder.push_encoding_symbol(&symbol, esi);
        }

        self.decoder = Some(decoder);
    }

    /// The m-th source symbol is the concatenation of the m-th sub-symbol of each sub-block
    /// and each sub-block is made of K contiguous sub-symbols
    fn deinterleave_sub_blocks(&self, source_symbols: Vec<u8>) -> Vec<u8> {
        if self.sub_symbols_length.len() == 1 {
            return source_symbols;
        }

        let mut source_block = Vec::with_capacity(source_symbols.len());
        let mut offset = 0;
        for sub_symbol_length in &self.sub_symbols_length {
            for symbol in source_symbols.chunks_exact(self.encoding_symbol_length) {
                source_block.extend_from_slice(&symbol[offset..offset + sub_symbol_length]);
            }
            offset += sub_symbol_length;
        }
        source_block
    }
}

impl FecDecoder for RaptorDecoder {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.data.is_some() || self.all_source_symbols_received() {
            return;
        }

        if encoding_symbol.len() > self.encoding_symbol_length {
            log::error!(
                "Encoding symbol of {} bytes is bigger than T={}",
                encoding_symbol.len(),
                self.encoding_symbol_length
            );
            return;
        }

        // The padding of the last source symbol might not be sent
        // https://www.rfc-editor.org/rfc/rfc5053.html#section-5.3.2
        let t = self.encoding_symbol_length;
        if (esi as usize) < self.nb_source_symbols {
            let esi = esi as usize;
            if self.source_received[esi] {
                return;
            }

            let symbol = &mut self.source_symbols[esi * t..(esi + 1) * t];
            symbol[..encoding_symbol.len()].copy_from_slice(encoding_symbol);
            self.source_received[esi] = true;
            self.nb_source_received += 1;

            if let Some(decoder) = self.decoder.as_mut() {
                decoder.push_encoding_symbol(symbol, esi as u32);
            }
        } else {
            let mut symbol = encoding_symbol.to_vec();
            symbol.resize(t, 0);
            match self.decoder.as_mut() {
                Some(decoder) => decoder.push_encoding_symbol(&symbol, esi),
                None => self.repair_symbols.push((esi, symbol)),
            }
        }

        if self.decoder.is_none()
            && !self.all_source_symbols_received()
            && self.nb_source_received + self.repair_symbols.len() >= self.nb_source_symbols
        {
            self.start_decoder();
        }
    }

    fn can_decode(&self) -> bool {
        self.all_source_symbols_received()
            || self
                .decoder
                .as_ref()
                .map(|decoder| decoder.fully_specified())
                .unwrap_or(false)
    }

    fn decode(&mut self) -> bool {
        if self.data.is_some() {
            return true;
        }

        log::debug!("Decode source block length {}", self.source_block_size);
        let source_symbols = if self.all_source_symbols_received() {
            std::mem::take(&mut self.source_symbols)
        } else {
            let length = self.nb_source_symbols * self.encoding_symbol_length;
            match self
                .decoder
                .as_mut()
                .and_then(|decoder| decoder.decode(length))
            {
                Some(source_symbols) => source_symbols,
                None => return false,
            }
        };

        let mut source_block = self.deinterleave_sub_blocks(source_symbols);

        // Remove the padding of the last source symbol
        source_block.truncate(self.source_block_size);
        self.data = Some(source_block);

        // Release the decoding buffers
        self.decoder = None;
        self.source_symbols = Vec::new();
        self.source_received = Vec::new();
        self.repair_symbols = Vec::new();
        true
    }

    fn source_block(&self) -> Result<&[u8]> {
        match self.data.as_ref() {
            Some(e) => Ok(e),
            None => Err(FluteError::new("Block not decoded")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RaptorDecoder, RaptorEncoder};
    use crate::common::oti::RaptorSchemeSpecific;
    use crate::fec::{FecDecoder, FecEncoder};

    fn create_scheme(sub_blocks_length: u8, symbol_alignment: u8) -> RaptorSchemeSpecific {
        RaptorSchemeSpecific {
            source_blocks_length: 1,
            sub_blocks_length,
            symbol_alignment,
        }
    }

    fn create_source_block(length: usize) -> Vec<u8> {
        (0..length).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    pub fn test_raptor_encode_sub_blocks() {
        crate::tests::init();

        // (K, T, N, Al, sub-symbols length)
        let cases = [
            (10usize, 1024usize, 1u8, 4u8, vec![1024usize]),
            (10, 1024, 4, 4, vec![256, 256, 256, 256]),
            (10, 1000, 3, 4, vec![336, 332, 332]),
            (4, 12, 3, 4, vec![4, 4, 4]),
            (7, 9, 2, 1, vec![5, 4]),
        ];

        for (k, t, n, al, sub_symbols_length) in cases {
            let data = create_source_block(k * t - 3);
            let encoder = RaptorEncoder::new(k, 2, t, &create_scheme(n, al)).unwrap();
            let shards = encoder.encode(&data).unwrap();
            assert_eq!(shards.len(), k + 2);
            assert!(shards.iter().all(|shard| shard.data().len() == t));

            // The m-th source symbol is the concatenation of the m-th sub-symbol of each sub-block
            let mut source_block = data.clone();
            source_block.resize(k * t, 0);
            for (m, shard) in shards.iter().take(k).enumerate() {
                let mut expected: Vec<u8> = Vec::new();
                let mut offset = 0;
                for sub_symbol_length in &sub_symbols_length {
                    let start = offset + m * sub_symbol_length;
                    expected.extend(&source_block[start..start + sub_symbol_length]);
                    offset += k * sub_symbol_length;
                }
                assert_eq!(shard.esi() as usize, m);
                assert_eq!(shard.data(), expected.as_slice(), "K={} T={} N={}", k, t, n);
            }
        }
    }

    #[test]
    pub fn test_raptor_decode_sub_blocks() {
        crate::tests::init();
        let k = 20;
        let t = 1000;
        let data = create_source_block(k * t - 100);

        for n in [1u8, 2, 3, 4] {
            let scheme = create_scheme(n, 4);
            let encoder = RaptorEncoder::new(k, 10, t, &scheme).unwrap();
            let shards = encoder.encode(&data).unwrap();

            let mut decoder = RaptorDecoder::new(k, data.len(), t, &scheme).unwrap();
            for shard in shards.iter().filter(|shard| shard.esi() % 5 != 0) {
                decoder.push_symbol(shard.data(), shard.esi());
            }
            assert!(decoder.can_decode());
            assert!(decoder.decode());
            assert_eq!(decoder.source_block().unwrap(), data.as_slice());
        }
    }

    #[test]
    pub fn test_raptor_decode_padding_not_sent() {
        crate::tests::init();
        let k = 10;
        let t = 1024;
        let data = create_source_block(k * t - 500);
        let scheme = create_scheme(1, 4);
        let encoder = RaptorEncoder::new(k, 2, t, &scheme).unwrap();
        let shards = encoder.encode(&data).unwrap();

        let mut decoder = RaptorDecoder::new(k, data.len(), t, &scheme).unwrap();
        for shard in shards.iter().take(k) {
            let end = std::cmp::min(t, data.len() - shard.esi() as usize * t);
            decoder.push_symbol(&shard.data()[..end], shard.esi());
        }
        assert!(decoder.can_decode());
        assert!(decoder.decode());
        assert_eq!(decoder.source_block().unwrap(), data.as_slice());
    }

    #[test]
    pub fn test_raptor_decode_systematic_sub_blocks() {
        crate::tests::init();
        let k = 20;
        let t = 1000;
        let data = create_source_block(k * t - 100);

        for n in [1u8, 2, 3, 4] {
            let scheme = create_scheme(n, 4);
            let encoder = RaptorEncoder::new(k, 10, t, &scheme).unwrap();
            let shards = encoder.encode(&data).unwrap();

            // Source symbols in reverse order, with a duplicate
            let mut decoder = RaptorDecoder::new(k, data.len(), t, &scheme).unwrap();
            decoder.push_symbol(shards[k - 1].data(), shards[k - 1].esi());
            for shard in shards.iter().take(k).rev() {
                assert!(!decoder.can_decode());
                decoder.push_symbol(shard.data(), shard.esi());
            }
            assert!(decoder.can_decode());
            assert!(
                decoder.decoder.is_none(),
                "Raptor decoder must not be started"
            );
            assert!(decoder.decode());
            assert_eq!(decoder.source_block().unwrap(), data.as_slice(), "N={}", n);
        }
    }

    #[test]
    pub fn test_raptor_decode_repair_first() {
        crate::tests::init();
        let k = 20;
        let t = 1000;
        let data = create_source_block(k * t - 100);
        let scheme = create_scheme(3, 4);
        let encoder = RaptorEncoder::new(k, 10, t, &scheme).unwrap();
        let shards = encoder.encode(&data).unwrap();

        let mut decoder = RaptorDecoder::new(k, data.len(), t, &scheme).unwrap();
        for shard in shards.iter().skip(k).chain(shards.iter().take(k).skip(5)) {
            decoder.push_symbol(shard.data(), shard.esi());
        }
        assert!(decoder.decoder.is_some());
        assert!(decoder.can_decode());
        assert!(decoder.decode());
        assert_eq!(decoder.source_block().unwrap(), data.as_slice());
    }

    #[test]
    pub fn test_raptor_decode_late_source_symbol() {
        crate::tests::init();
        let k = 20;
        let t = 1000;
        let data = create_source_block(k * t - 100);
        let scheme = create_scheme(2, 4);
        let encoder = RaptorEncoder::new(k, 10, t, &scheme).unwrap();
        let shards = encoder.encode(&data).unwrap();

        // The Raptor decoder is started, then the missing source symbol completes the block
        let mut decoder = RaptorDecoder::new(k, data.len(), t, &scheme).unwrap();
        for shard in shards.iter().skip(1).take(k) {
            decoder.push_symbol(shard.data(), shard.esi());
        }
        assert!(decoder.decoder.is_some());
        decoder.push_symbol(shards[0].data(), shards[0].esi());
        assert!(decoder.can_decode());
        assert!(decoder.decode());
        assert_eq!(decoder.source_block().unwrap(), data.as_slice());
    }

    #[test]
    pub fn test_raptor_invalid_sub_blocks() {
        crate::tests::init();
        assert!(RaptorEncoder::new(10, 2, 1024, &create_scheme(0, 4)).is_err());
        assert!(RaptorEncoder::new(10, 2, 16, &create_scheme(5, 4)).is_err());
        assert!(RaptorEncoder::new(10, 2, 1022, &create_scheme(1, 4)).is_err());
        assert!(RaptorEncoder::new(10, 2, 1024, &create_scheme(1, 0)).is_err());
        assert!(RaptorDecoder::new(10, 10240, 16, &create_scheme(5, 4)).is_err());
        assert!(RaptorDecoder::new(10, 10240, 1024, &create_scheme(0, 4)).is_err());
    }
}
