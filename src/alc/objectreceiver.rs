use super::alc::{self, AlcPkt};
use super::blockdecoder::BlockDecoder;
use super::blockwriter::BlockWriter;
use super::fdtinstance::FdtInstance;
use super::objectwriter::ObjectWriterSession;
use super::oti;
use crate::alc::lct;
use crate::tools::error::{FluteError, Result};
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Receiving,
    Completed,
    Error,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ObjectWriterSessionState {
    Closed,
    Opened,
    Error,
}

pub struct ObjectReceiver {
    pub state: State,
    pub toi: u128,
    oti: Option<oti::Oti>,
    cache: Vec<Box<alc::AlcPktCache>>,
    cache_size: usize,
    blocks: Vec<BlockDecoder>,
    blocks_variable_size: bool,
    transfer_length: Option<u64>,
    cenc: Option<lct::CENC>,
    a_large: u64,
    a_small: u64,
    nb_a_large: u64,
    waiting_for_fdt: bool,
    writer_session: Rc<dyn ObjectWriterSession>,
    writer_session_state: ObjectWriterSessionState,
    block_writer: Option<BlockWriter>,
    fdt_instance_id: Option<u32>,
    content_location: Option<String>,
    last_activity: Instant,
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
            cenc: None,
            blocks_variable_size: false,
            a_large: 0,
            a_small: 0,
            nb_a_large: 0,
            waiting_for_fdt: toi.clone() != lct::TOI_FDT,
            writer_session,
            writer_session_state: ObjectWriterSessionState::Closed,
            block_writer: None,
            fdt_instance_id: None,
            toi: toi.clone(),
            content_location: None,
            last_activity: Instant::now(),
        }
    }

    pub fn last_activity_duration_since(&self, earlier: Instant) -> Duration {
        self.last_activity.duration_since(earlier)
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt) -> Result<()> {
        if self.state != State::Receiving {
            return Ok(());
        }

        self.last_activity = Instant::now();
        self.set_cenc_from_pkt(pkt);
        self.set_oti_from_pkt(pkt);

        if self.oti.is_none() {
            self.cache(pkt);
            return Ok(());
        }

        self.push_to_block(pkt)
    }

    fn push_to_block(&mut self, pkt: &alc::AlcPkt) -> Result<()> {
        assert!(self.oti.is_some());
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
            return Ok(());
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
            self.write_blocks(payload_id.snb);
        }

        Ok(())
    }

    pub fn attach_fdt(&mut self, fdt_instance_id: u32, fdt: &FdtInstance) -> bool {
        assert!(self.toi != lct::TOI_FDT);
        if self.fdt_instance_id.is_some() {
            return false;
        }

        let file = match fdt.get_file(&self.toi) {
            Some(file) => file,
            None => return false,
        };

        if self.cenc.is_none() {
            self.cenc = match &file.content_encoding {
                Some(str) => Some(str.as_str().try_into().unwrap_or(lct::CENC::Null)),
                None => Some(lct::CENC::Null),
            };
            log::info!("Set cenc from FDT {:?}", self.cenc);
        }

        if self.oti.is_none() {
            self.oti = fdt.get_oti_for_file(file);
            self.transfer_length = file.transfer_length.clone();
        }

        if self.oti.is_some() && self.transfer_length.is_some() && self.block_writer.is_none() {
            self.setup_block_writer();
        }

        log::info!("FDT attached to object TOI={}", self.toi);
        self.fdt_instance_id = Some(fdt_instance_id);
        self.waiting_for_fdt = false;
        self.content_location = Some(file.content_location.clone());
        self.open_object_writer();
        self.push_from_cache();

        if self.oti.is_some() {
            self.write_blocks(0);
        }
        true
    }

    fn open_object_writer(&mut self) {
        if self.writer_session_state != ObjectWriterSessionState::Closed {
            return;
        }

        if self.oti.is_none() || self.waiting_for_fdt || self.block_writer.is_none() {
            return;
        }

        self.writer_session
            .open(self.content_location.as_ref().map(|f| f.as_str()));
        self.writer_session_state = ObjectWriterSessionState::Opened;
    }

    fn write_blocks(&mut self, snb_start: u32) {
        if self.waiting_for_fdt {
            return;
        }

        if self.writer_session_state != ObjectWriterSessionState::Opened {
            return;
        }

        assert!(self.block_writer.is_some());
        let mut snb = snb_start as usize;
        let writer = self.block_writer.as_mut().unwrap();
        while snb < self.blocks.len() {
            let block = &mut self.blocks[snb as usize];
            if !block.completed {
                break;
            }

            let success = writer.write(snb as u32, block, self.writer_session.as_ref());
            if !success {
                break;
            }
            snb += 1;
            block.deallocate();

            if writer.completed() {
                log::info!("Object completed");
                self.complete();
                break;
            }
        }
    }

    fn complete(&mut self) {
        self.state = State::Completed;
        self.writer_session.complete();
        // Free space by removing blocks
        self.blocks.clear();
        self.cache.clear();
    }

    fn push_from_cache(&mut self) {
        if self.oti.is_none() {
            return;
        }

        while !self.cache.is_empty() {
            let item = self.cache.pop().unwrap();
            let pkt = item.to_pkt();
            self.push_to_block(&pkt).ok();
        }
    }

    fn set_cenc_from_pkt(&mut self, pkt: &alc::AlcPkt) {
        if self.cenc.is_some() {
            return;
        }
        self.cenc = pkt.cenc;
        if self.toi == lct::TOI_FDT && self.cenc.is_none() {
            log::info!("Force CENC to Null for the FDT");
            self.cenc = Some(lct::CENC::Null);
        }
    }

    fn set_oti_from_pkt(&mut self, pkt: &alc::AlcPkt) {
        if self.oti.is_some() {
            return;
        }

        if pkt.oti.is_none() {
            return;
        }

        self.oti = pkt.oti.clone();
        self.transfer_length = pkt.transfer_length;

        if pkt.transfer_length.is_none() {
            log::warn!("Bug? Pkt contains OTI without transfer length");
            return;
        }

        if self.cenc.is_none() {
            log::warn!("Cenc is unknown ?");
            assert!(self.toi != lct::TOI_FDT);
            return;
        }

        self.setup_block_writer();
        self.open_object_writer();
        self.push_from_cache();
    }

    fn setup_block_writer(&mut self) {
        assert!(self.oti.is_some());
        assert!(self.transfer_length.is_some());
        assert!(self.cenc.is_some());

        if self.block_writer.is_some() {
            return;
        }

        self.block_writer = Some(BlockWriter::new(
            self.transfer_length.unwrap() as usize,
            self.cenc.unwrap(),
        ));
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

        self.blocks_variable_size =
            oti.fec_encoding_id == oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic;
        log::info!(
            "Preallocate {} blocks of {} or {} symbols",
            n,
            self.a_large,
            self.a_small
        );
        self.blocks.resize_with(n as usize, || BlockDecoder::new());
    }
}

impl Drop for ObjectReceiver {
    fn drop(&mut self) {
        if self.writer_session_state == ObjectWriterSessionState::Opened {
            self.writer_session.error();
            self.writer_session_state = ObjectWriterSessionState::Error;
        }
    }
}
