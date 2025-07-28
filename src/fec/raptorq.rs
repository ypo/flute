use crate::common::oti::RaptorQSchemeSpecific;
use crate::error::{FluteError, Result};

use super::{FecDecoder, FecEncoder, FecShard};

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
    ) -> Self {
        RaptorQEncoder {
            nb_parity_symbols,
            config: raptorq::ObjectTransmissionInformation::new(
                (nb_source_symbols * encoding_symbol_length) as u64,
                encoding_symbol_length as u16,
                1,
                scheme.sub_blocks_length,
                scheme.symbol_alignment,
            ),
        }
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
    data: Option<Vec<u8>>,
    sbn: u32,
}

impl RaptorQDecoder {
    pub fn new(
        sbn: u32,
        nb_source_symbols: usize,
        encoding_symbol_length: usize,
        scheme: &RaptorQSchemeSpecific,
    ) -> RaptorQDecoder {
        let config = raptorq::ObjectTransmissionInformation::new(
            (nb_source_symbols * encoding_symbol_length) as u64,
            encoding_symbol_length as u16,
            1,
            scheme.sub_blocks_length,
            scheme.symbol_alignment,
        );

        let block_length = nb_source_symbols as u64 * encoding_symbol_length as u64;
        let decoder = raptorq::SourceBlockDecoder::new(sbn as u8, &config, block_length);
        RaptorQDecoder {
            decoder,
            data: None,
            sbn,
        }
    }
}

impl FecDecoder for RaptorQDecoder {
    fn push_symbol(&mut self, encoding_symbol: &[u8], esi: u32) {
        if self.data.is_some() {
            return;
        }

        let pkt = raptorq::EncodingPacket::new(
            raptorq::PayloadId::new(self.sbn as u8, esi),
            encoding_symbol.to_vec(),
        );

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
    use crate::{common::oti::RaptorQSchemeSpecific, fec::FecEncoder};

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

        let r = super::RaptorQEncoder::new(
            nb_source_symbols,
            nb_parity_symbols,
            symbols_length,
            &scheme,
        );
        let encoded_data = r.encode(data.as_ref()).unwrap();
        log::info!("NB source symbols={}", encoded_data.len());
    }
}
