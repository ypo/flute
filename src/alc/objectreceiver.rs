use std::rc::Rc;

use super::alc;
use super::blockdecoder::BlockDecoder;
use super::objectwriter::ObjectWriterSession;
use super::oti;
use crate::alc::lct;
use crate::tools::error::{FluteError, Result};

#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Receiving,
    Completed,
    Error,
}



pub struct ObjectReceiver {
    state: State,
    oti: Option<oti::Oti>,
    cache: Vec<Box<alc::AlcPktCache>>,
    cache_size: usize,
    blocks: Vec<BlockDecoder>,
    blocks_variable_size: bool,
    transfer_length: Option<u64>,
    a_large: u64,
    a_small: u64,
    nb_a_large: u64,
    waiting_for_fdt: bool,
    writer_session: Rc<dyn ObjectWriterSession>
}

impl ObjectReceiver {
    pub fn new(toi: &u128, writer_session: Rc<dyn ObjectWriterSession>) -> ObjectReceiver {
        log::info!("Create new Object Receiver");
        ObjectReceiver {
            state: State::Receiving,
            oti: None,
            cache: Vec::new(),
            cache_size: 0,
            blocks: Vec::new(),
            transfer_length: None,
            blocks_variable_size: false,
            a_large: 0,
            a_small: 0,
            nb_a_large: 0,
            waiting_for_fdt: toi.clone() != lct::TOI_FDT,
            writer_session
        }
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt) -> Result<bool> {
        if self.state != State::Receiving {
            return Ok(false);
        }

        self.set_oti_from_pkt(pkt);

        if self.oti.is_none() {
            self.cache(pkt);
            return Ok(true);
        }
        let oti = self.oti.as_ref().unwrap();
        let payload_id = alc::parse_payload_id(pkt, self.oti.as_ref().unwrap())?;
        log::debug!("Receive snb {} esi {}", payload_id.snb, payload_id.esi);

        if payload_id.snb as usize >= self.blocks.len() {
            if self.blocks_variable_size == false {
                return Err(FluteError::new(format!(
                    "SNB {} > max SNB {}",
                    payload_id.snb,
                    self.blocks.len()
                )));
            } else {
                self.blocks
                    .resize_with(payload_id.snb as usize + 1, || BlockDecoder::new());
            }
        }

        let block = &mut self.blocks[payload_id.snb as usize];
        if block.completed {
            return Ok(true);
        }

        if block.initialized == false {
            let source_block_length = payload_id.source_block_length.unwrap_or_else(|| {
                match payload_id.snb < self.nb_a_large as u32 {
                    true => self.a_large as u32,
                    _ => self.a_small as u32,
                }
            });

            match block.init(oti, source_block_length) {
                Ok(_) => {}
                Err(_) => {
                    self.state = State::Error;
                    return Err(FluteError::new("Fail to init source block decoder"));
                }
            }
        }

        block.push(pkt, &payload_id);
        if block.completed {
            log::info!("block {} is completed", payload_id.snb);
            // TODO Write blocks to file
        }

        Ok(true)
    }

    fn write_blocks(&self, snb_start: u32) {
        if self.waiting_for_fdt {
            return;
        }
    }

    fn set_oti_from_pkt(&mut self, pkt: &alc::AlcPkt) {
        if self.oti.is_some() {
            return;
        }

        self.oti = pkt.oti.clone();
        if self.oti.is_none() {
            log::warn!("Object received before OTI");
            return;
        }
        if pkt.transfer_length.is_none() {
            log::warn!("Bug? Pkt contains OTI without transfer length");
            return;
        }

        self.transfer_length = pkt.transfer_length;
        self.block_partitioning();
    }

    fn cache(&mut self, pkt: &alc::AlcPkt) {
        self.cache.push(Box::new(pkt.to_cache()));
        self.cache_size += pkt.data.len()
    }

    ///  Block Partitioning Algorithm
    ///  See https://tools.ietf.org/html/rfc5052
    fn block_partitioning(&mut self) {
        assert!(self.oti.is_some());
        assert!(self.transfer_length.is_some());
        assert!(self.blocks.is_empty());

        let oti = self.oti.as_ref().unwrap();

        let b = oti.maximum_source_block_length as u64;
        let e = oti.encoding_symbol_length as u64;
        let l = self.transfer_length.unwrap_or_default();
        let t = num_integer::div_ceil(l, e);
        let mut n = num_integer::div_ceil(t, b);
        if n == 0 {
            n = 1;
        }

        self.a_large = num_integer::div_ceil(t, n);
        self.a_small = num_integer::div_floor(t, n);
        self.nb_a_large = t - (self.a_small * n);

        self.blocks_variable_size = oti.fec == oti::FECEncodingID::ReedSolomonGF28;
        log::info!(
            "Preallocate {} blocks of {} or {} symbols",
            n,
            self.a_large,
            self.a_small
        );
        self.blocks.resize_with(n as usize, || BlockDecoder::new());
    }
}
