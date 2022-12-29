use super::alc;
use super::oti;
use crate::fec::rscodec;
use crate::fec::FecCodec;
use crate::tools::error::Result;

#[derive(Debug)]
pub struct BlockDecoder {
    pub completed: bool,
    pub initialized: bool,
    shards: Vec<Option<Vec<u8>>>,
    nb_shards: usize,
    nb_shards_source_symbol: usize,
    decoder: Option<Box<dyn FecCodec>>,
    source_block_length: usize,
}

impl BlockDecoder {
    pub fn new() -> BlockDecoder {
        BlockDecoder {
            completed: false,
            initialized: false,
            shards: Vec::new(),
            nb_shards: 0,
            nb_shards_source_symbol: 0,
            decoder: None,
            source_block_length: 0,
        }
    }

    pub fn init(&mut self, oti: &oti::Oti, source_block_length: u32) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        let nb_shards = oti.max_number_of_parity_symbols + source_block_length;
        self.shards.resize_with(nb_shards as usize, || None);
        self.source_block_length = source_block_length as usize;

        match oti.fec_encoding_id {
            oti::FECEncodingID::NoCode => {}
            oti::FECEncodingID::ReedSolomonGF28 => {
                let codec = rscodec::RSGalois8Codec::new(
                    source_block_length as usize,
                    oti.max_number_of_parity_symbols as usize,
                    oti.encoding_symbol_length as usize,
                )?;
                self.decoder = Some(Box::new(codec));
            }
            oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic => {
                let codec = rscodec::RSGalois8Codec::new(
                    source_block_length as usize,
                    oti.max_number_of_parity_symbols as usize,
                    oti.encoding_symbol_length as usize,
                )?;
                self.decoder = Some(Box::new(codec));
            }
            oti::FECEncodingID::ReedSolomonGF2M => {
                log::warn!("Not implemented")
            }
        }

        self.initialized = true;
        Ok(())
    }

    pub fn source_encoding_symbols(&self) -> &[Option<Vec<u8>>] {
        &self.shards[..self.source_block_length]
    }

    pub fn deallocate(&mut self) {
        self.shards.clear()
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt, payload_id: &alc::PayloadID) {
        assert!(self.initialized);
        if payload_id.esi as usize >= self.shards.len() {
            log::error!(
                "esi {} is outside snb {} of max length {}",
                payload_id.esi,
                payload_id.snb,
                self.shards.len()
            );
            return;
        }

        let shard = &mut self.shards[payload_id.esi as usize];
        if shard.is_some() {
            log::debug!(
                "snb/esi {}/{} already received",
                payload_id.snb,
                payload_id.esi
            );
            return;
        }

        let payload = &pkt.data[pkt.data_payload_offset..];
        let _ = shard.insert(payload.to_vec());
        self.nb_shards += 1;

        if (payload_id.esi as usize) < self.source_block_length {
            self.nb_shards_source_symbol += 1;
        }

        self.repair();

        if self.nb_shards_source_symbol == self.source_block_length {
            self.completed = true;
        }
    }

    fn repair(&mut self) {
        if self.decoder.is_none() {
            return;
        }

        if self.nb_shards_source_symbol == self.source_block_length
            || self.nb_shards < self.source_block_length
        {
            return;
        }

        let success = self.decoder.as_ref().unwrap().decode(&mut self.shards);
        let source_block_length = self.source_block_length;
        let nb_shards_source_symbol = self.nb_shards_source_symbol;
        self.nb_shards_source_symbol = self
            .shards
            .iter()
            .enumerate()
            .filter(|(index, item)| *index < source_block_length && item.is_some())
            .count();
        assert!(self.nb_shards_source_symbol >= nb_shards_source_symbol);
        log::info!(
            "Run FEC Repair success={} {} symbols {} / {} nb shards = {}",
            success,
            self.nb_shards_source_symbol - nb_shards_source_symbol,
            self.nb_shards_source_symbol,
            self.source_block_length,
            self.nb_shards,
        );
    }
}
