use crate::alc::oti::RaptorQSchemeSpecific;

use super::{FecCodec, FecShard, ShardType};

pub struct RaptorQ {
    config: raptorq::ObjectTransmissionInformation,
    nb_parity_symbols: usize,
}

#[derive(Debug)]
struct RaptorFecShard {
    pkt: raptorq::EncodingPacket,
    shard_type: ShardType,
}

impl FecShard for RaptorFecShard {
    fn data(&self) -> &[u8] {
        self.pkt.data()
    }
    fn esi(&self) -> u32 {
        self.pkt.payload_id().encoding_symbol_id()
    }
    fn get_type(&self) -> ShardType {
        self.shard_type
    }
}

impl RaptorQ {
    pub fn new(
        nb_source_symbols: usize,
        nb_parity_symbols: usize,
        encoding_symbol_length: usize,
        scheme: &RaptorQSchemeSpecific,
    ) -> Self {
        RaptorQ {
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

impl FecCodec for RaptorQ {
    fn encode(&self, data: &[u8]) -> crate::error::Result<Vec<Box<dyn FecShard>>> {
        log::info!("OTI={:?}", self.config);
        let encoder = raptorq::SourceBlockEncoder::new2(0, &self.config.clone(), data);

        let src_pkt = encoder.source_packets();
        let repair_pkt = encoder.repair_packets(0, self.nb_parity_symbols as u32);
        let mut output: Vec<Box<dyn FecShard>> = Vec::new();

        for pkt in src_pkt {
            output.push(Box::new(RaptorFecShard {
                pkt,
                shard_type: ShardType::SourceSymbol,
            }));
        }

        for pkt in repair_pkt {
            output.push(Box::new(RaptorFecShard {
                pkt,
                shard_type: ShardType::RepairSymbol,
            }));
        }

        Ok(output)
    }

    fn decode(&self, sbn: u32, shards: &mut Vec<Option<Vec<u8>>>) -> bool {
        let block_length = shards.len() * self.config.symbol_size() as usize;
        let mut decoder =
            raptorq::SourceBlockDecoder::new2(sbn as u8, &self.config, block_length as u64);

        let packets = shards
            .iter()
            .enumerate()
            .filter(|(_, shard)| shard.is_some())
            .map(|(esi, shard)| {
                raptorq::EncodingPacket::new(
                    raptorq::PayloadId::new(sbn as u8, esi as u32),
                    shard.as_ref().unwrap().clone(),
                )
            });

        let result = decoder.decode(packets);
        if result.is_none() {
            log::error!("Fail to decode");
            return false;
        }
        result
            .unwrap()
            .chunks(self.config.symbol_size() as usize)
            .enumerate()
            .for_each(|(esi, shard)| {
                if shards[esi].is_none() {
                    let s = &mut shards[esi];
                    s.replace(shard.to_vec());
                }
            });
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::{alc::oti::RaptorQSchemeSpecific, fec::FecCodec};

    #[test]
    pub fn test_raptorq_encode() {
        crate::tests::init();

        let nb_source_symbols = 10usize;
        let nb_parity_symbols = 2usize;
        let symbols_length = 1024usize;

        let data = vec![0xAAu8; nb_source_symbols * symbols_length];

        let scheme = RaptorQSchemeSpecific {
            source_block_length: 1,
            sub_blocks_length: 1,
            symbol_alignment: 8,
        };

        let r = super::RaptorQ::new(
            nb_source_symbols,
            nb_parity_symbols,
            symbols_length,
            &scheme,
        );
        let encoded_data = r.encode(data.as_ref()).unwrap();
        log::info!("NB source symbols={}", encoded_data.len());
    }
}
