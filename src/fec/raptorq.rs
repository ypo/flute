use crate::common::oti::RaptorQSchemeSpecific;
use crate::error::{FluteError, Result};

use super::{FecDecoder, FecEncoder, FecShard};

/// RaptorQ parameters of a source block
///
/// The raptorq crate panics if the parameters are not valid
/// <https://www.rfc-editor.org/rfc/rfc6330.html#section-4.4.1.2>
fn object_transmission_information(
    nb_source_symbols: usize,
    encoding_symbol_length: usize,
    scheme: &RaptorQSchemeSpecific,
) -> Result<raptorq::ObjectTransmissionInformation> {
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

    // K'max <https://www.rfc-editor.org/rfc/rfc6330.html#section-5.1.2>
    if nb_source_symbols > 56403 {
        return Err(FluteError::new(format!(
            "Number of source symbols K={} must not exceed K'max=56403",
            nb_source_symbols
        )));
    }

    Ok(raptorq::ObjectTransmissionInformation::new(
        (nb_source_symbols * encoding_symbol_length) as u64,
        encoding_symbol_length as u16,
        1,
        scheme.sub_blocks_length,
        scheme.symbol_alignment,
    ))
}

pub struct RaptorQEncoder {
    config: raptorq::ObjectTransmissionInformation,
    nb_parity_symbols: usize,
}

#[derive(Debug)]
struct RaptorFecShard {
    pkt: raptorq::EncodingPacket,
}

impl FecShard for RaptorFecShard {
    fn data(&self) -> &[u8] {
        self.pkt.data()
    }
    fn esi(&self) -> u32 {
        self.pkt.payload_id().encoding_symbol_id()
    }
}

impl RaptorQEncoder {
    pub fn new(
        nb_source_symbols: usize,
        nb_parity_symbols: usize,
        encoding_symbol_length: usize,
        scheme: &RaptorQSchemeSpecific,
    ) -> Result<Self> {
        Ok(RaptorQEncoder {
            nb_parity_symbols,
            config: object_transmission_information(
                nb_source_symbols,
                encoding_symbol_length,
                scheme,
            )?,
        })
    }
}

impl FecEncoder for RaptorQEncoder {
    fn encode(&self, data: &[u8]) -> crate::error::Result<Vec<Box<dyn FecShard>>> {
        let symbol_aligned = data.len() % self.config.symbol_size() as usize;
        let encoder = match data.len() % self.config.symbol_size() as usize {
            0 => raptorq::SourceBlockEncoder::new(0, &self.config.clone(), data),
            _ => {
                let mut data = data.to_vec();
                data.resize(
                    data.len() + (self.config.symbol_size() as usize - symbol_aligned),
                    0,
                );
                raptorq::SourceBlockEncoder::new(0, &self.config.clone(), &data)
            }
        };

        let src_pkt = encoder.source_packets();
        let repair_pkt = encoder.repair_packets(0, self.nb_parity_symbols as u32);
        let mut output: Vec<Box<dyn FecShard>> = Vec::new();

        for pkt in src_pkt {
            output.push(Box::new(RaptorFecShard { pkt }));
        }

        for pkt in repair_pkt {
            output.push(Box::new(RaptorFecShard { pkt }));
        }

        Ok(output)
    }
}

pub struct RaptorQDecoder {
    decoder: raptorq::SourceBlockDecoder,
    encoding_symbol_length: usize,
    data: Option<Vec<u8>>,
    sbn: u32,
}

impl RaptorQDecoder {
    pub fn new(
        sbn: u32,
        nb_source_symbols: usize,
        encoding_symbol_length: usize,
        scheme: &RaptorQSchemeSpecific,
    ) -> Result<RaptorQDecoder> {
        let config =
            object_transmission_information(nb_source_symbols, encoding_symbol_length, scheme)?;

        let block_length = nb_source_symbols as u64 * encoding_symbol_length as u64;
        let decoder = raptorq::SourceBlockDecoder::new(sbn as u8, &config, block_length);
        Ok(RaptorQDecoder {
            decoder,
            encoding_symbol_length,
            data: None,
            sbn,
        })
    }
}

impl FecDecoder for RaptorQDecoder {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.data.is_some() {
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
        // https://www.rfc-editor.org/rfc/rfc6330.html#section-4.4.2
        let mut symbol = encoding_symbol.to_vec();
        symbol.resize(self.encoding_symbol_length, 0);

        let pkt =
            raptorq::EncodingPacket::new(raptorq::PayloadId::new(self.sbn as u8, esi), symbol);

        self.data = self.decoder.decode(vec![pkt]);
    }

    fn can_decode(&self) -> bool {
        self.data.is_some()
    }

    fn decode(&mut self) -> bool {
        self.data.is_some()
    }

    fn source_block(&self) -> Result<&[u8]> {
        if self.data.is_none() {
            return Err(FluteError::new("Source block not decoded"));
        }

        Ok(self.data.as_ref().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::{RaptorQDecoder, RaptorQEncoder};
    use crate::common::oti::RaptorQSchemeSpecific;
    use crate::fec::{FecDecoder, FecEncoder};

    fn create_scheme(sub_blocks_length: u16, symbol_alignment: u8) -> RaptorQSchemeSpecific {
        RaptorQSchemeSpecific {
            source_blocks_length: 1,
            sub_blocks_length,
            symbol_alignment,
        }
    }

    fn create_source_block(length: usize) -> Vec<u8> {
        (0..length).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    pub fn test_raptorq_encode() {
        crate::tests::init();

        let nb_source_symbols = 10usize;
        let nb_parity_symbols = 2usize;
        let symbols_length = 1024usize;

        let data = vec![0xAAu8; nb_source_symbols * symbols_length];

        let scheme = RaptorQSchemeSpecific {
            source_blocks_length: 1,
            sub_blocks_length: 1,
            symbol_alignment: 8,
        };

        let r = RaptorQEncoder::new(
            nb_source_symbols,
            nb_parity_symbols,
            symbols_length,
            &scheme,
        )
        .unwrap();
        let encoded_data = r.encode(data.as_ref()).unwrap();
        log::info!("NB source symbols={}", encoded_data.len());
    }

    #[test]
    pub fn test_raptorq_decode_padding_not_sent() {
        crate::tests::init();
        let k = 10;
        let t = 1024;
        let data = create_source_block(k * t - 500);
        let scheme = create_scheme(1, 4);
        let encoder = RaptorQEncoder::new(k, 2, t, &scheme).unwrap();
        let shards = encoder.encode(&data).unwrap();

        let mut decoder = RaptorQDecoder::new(0, k, t, &scheme).unwrap();
        for shard in shards.iter().take(k) {
            let end = std::cmp::min(t, data.len() - shard.esi() as usize * t);
            decoder.push_symbol(&shard.data()[..end], shard.esi());
        }
        assert!(decoder.can_decode());
        assert!(decoder.decode());
        assert_eq!(
            &decoder.source_block().unwrap()[..data.len()],
            data.as_slice()
        );
    }

    #[test]
    pub fn test_raptorq_invalid_parameters() {
        crate::tests::init();
        assert!(RaptorQEncoder::new(10, 2, 1024, &create_scheme(0, 4)).is_err());
        assert!(RaptorQEncoder::new(10, 2, 16, &create_scheme(5, 4)).is_err());
        assert!(RaptorQEncoder::new(10, 2, 1022, &create_scheme(1, 4)).is_err());
        assert!(RaptorQEncoder::new(10, 2, 1024, &create_scheme(1, 0)).is_err());
        assert!(RaptorQEncoder::new(56404, 2, 16, &create_scheme(1, 4)).is_err());
        assert!(RaptorQDecoder::new(0, 10, 1024, &create_scheme(0, 4)).is_err());
        assert!(RaptorQDecoder::new(0, 56404, 16, &create_scheme(1, 4)).is_err());
        assert!(RaptorQDecoder::new(0, 56403, 16, &create_scheme(1, 4)).is_ok());
    }
}
