use super::blockdecoder::BlockDecoder;
use super::blockwriter::BlockWriter;
use super::writer::ObjectWriterBuilder;
use crate::common::udpendpoint::UDPEndpoint;
use crate::common::{alc, fdtinstance::FdtInstance, lct, oti, partition};
use crate::receiver::writer::{
    ObjectCacheControl, ObjectMetadata, ObjectWriter, ObjectWriterBuilderResult,
};
use crate::tools::error::{FluteError, Result};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Instant;
use std::time::{Duration, SystemTime};

#[cfg(feature = "opentelemetry")]
use super::objectreceiverlogger::ObjectReceiverLogger;

const MAX_PREALLOCATED_BLOCKS: usize = 2048;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Receiving,
    Completed,
    Interrupted,
    Error,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ObjectWriterSessionState {
    Idle,
    Closed,
    Opened,
    Error,
}

#[derive(Debug)]
struct ObjectWriterSession {
    writer: Box<dyn ObjectWriter>,
    state: ObjectWriterSessionState,
}

#[derive(Debug)]
pub struct ObjectReceiver {
    pub state: State,
    pub toi: u128,
    pub tsi: u64,
    pub endpoint: UDPEndpoint,
    oti: Option<oti::Oti>,
    cache: Vec<Box<alc::AlcPktCache>>,
    cache_size: usize,
    max_size_allocated: usize,
    blocks: VecDeque<BlockDecoder>,
    blocks_offset: usize,
    blocks_variable_size: bool,
    pub transfer_length: Option<u64>,
    cenc: Option<lct::Cenc>,
    pub content_md5: Option<String>,
    enable_md5_check: bool,
    a_large: u64,
    a_small: u64,
    nb_a_large: u64,
    object_writer_builder: Rc<dyn ObjectWriterBuilder>,
    object_writer: Option<ObjectWriterSession>,
    block_writer: Option<BlockWriter>,
    pub fdt_instance_id: Option<u32>,
    last_activity: Instant,
    pub content_location: Option<String>,
    nb_allocated_blocks: usize,
    total_allocated_blocks_size: usize,
    #[cfg(feature = "opentelemetry")]
    logger: Option<ObjectReceiverLogger>,
    content_length: Option<usize>,
    content_type: Option<String>,
    pub cache_control: Option<ObjectCacheControl>,
    groups: Vec<String>,
    last_timestamp: SystemTime,
    pub e_tag: Option<String>,
}

impl ObjectReceiver {
    pub fn new(
        endpoint: &UDPEndpoint,
        tsi: u64,
        toi: &u128,
        _fdt_instance_id: Option<u32>,
        object_writer_builder: Rc<dyn ObjectWriterBuilder>,
        max_size_allocated: usize,
        now: SystemTime,
    ) -> ObjectReceiver {
        log::debug!("Create new Object Receiver with toi {}", toi);
        ObjectReceiver {
            state: State::Receiving,
            oti: None,
            cache: Vec::new(),
            cache_size: 0,
            max_size_allocated,
            blocks: VecDeque::new(),
            blocks_offset: 0,
            transfer_length: None,
            cenc: None,
            content_md5: None,
            enable_md5_check: false,
            blocks_variable_size: false,
            a_large: 0,
            a_small: 0,
            nb_a_large: 0,
            object_writer_builder,
            object_writer: None,
            block_writer: None,
            fdt_instance_id: None,
            tsi,
            toi: *toi,
            endpoint: endpoint.clone(),
            last_activity: Instant::now(),
            content_location: match *toi == lct::TOI_FDT {
                true => Some("flute://fdt".to_string()),
                false => None,
            },
            nb_allocated_blocks: 0,
            total_allocated_blocks_size: 0,
            #[cfg(feature = "opentelemetry")]
            logger: None,
            content_length: None,
            content_type: None,
            cache_control: None,
            groups: Vec::new(),
            last_timestamp: now,
            e_tag: None,
        }
    }

    pub fn last_activity_duration_since(&self, earlier: Instant) -> Duration {
        earlier.duration_since(self.last_activity)
    }

    pub fn nb_block_completed(&self) -> usize {
        self.blocks_offset + self.blocks.iter().filter(|block| block.completed).count()
    }

    pub fn nb_block(&self) -> usize {
        self.blocks_offset + self.blocks.len()
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt, now: std::time::SystemTime) {
        self.last_timestamp = now;
        if self.state != State::Receiving {
            return;
        }

        self.last_activity = Instant::now();
        self.set_fdt_id_from_pkt(pkt);
        self.set_cenc_from_pkt(pkt);
        self.set_oti_from_pkt(pkt, now);

        self.init_blocks_partitioning();
        self.init_object_writer(now);
        self.push_from_cache(now);

        if self.oti.is_none() {
            self.cache(pkt)
                .unwrap_or_else(|_| self.error("Fail to push pkt to cache", now, false));
            return;
        }

        self.push_to_block(pkt, now)
            .unwrap_or_else(|_| self.error("Fail to push pkt to block", now, false));
    }

    fn push_to_block(&mut self, pkt: &alc::AlcPkt, now: std::time::SystemTime) -> Result<()> {
        self.push_to_block2(pkt, now)?;
        if pkt.lct.close_object {
            if self.state == State::Receiving {
                self.error("No more packet for this object", now, true);
            }
        }
        Ok(())
    }

    fn push_to_block2(&mut self, pkt: &alc::AlcPkt, now: std::time::SystemTime) -> Result<()> {
        debug_assert!(self.oti.is_some());
        debug_assert!(self.transfer_length.is_some());
        let payload_id = alc::parse_payload_id(pkt, self.oti.as_ref().unwrap())?;
        let nb_blocks = self.blocks.len();

        if self.transfer_length.unwrap() == 0 {
            debug_assert!(self.block_writer.is_none());
            self.complete(now);
            return Ok(());
        }

        if payload_id.sbn < self.blocks_offset as u32 {
            // already completed
            return Ok(());
        }

        let block_offset = payload_id.sbn as usize - self.blocks_offset;
        if block_offset as usize >= self.blocks.len() {
            if block_offset > 2 * MAX_PREALLOCATED_BLOCKS {
                log::error!(
                    "Request to allocate {} blocks which is greater than the max {}",
                    block_offset,
                    2 * MAX_PREALLOCATED_BLOCKS
                );
                self.state = State::Error;
                return Err(FluteError::new("Too many blocks allocated"));
            }

            self.blocks
                .resize_with(block_offset as usize + 1, BlockDecoder::new);
        }

        let block = &mut self.blocks[block_offset];
        if block.completed {
            return Ok(());
        }

        if !block.initialized {
            let source_block_length = payload_id.source_block_length.unwrap_or(
                match payload_id.sbn < self.nb_a_large as u32 {
                    true => self.a_large as u32,
                    _ => self.a_small as u32,
                },
            );

            let oti = self.oti.as_ref().unwrap();

            let block_length: usize = match payload_id.source_block_length {
                Some(_) => source_block_length as usize * oti.encoding_symbol_length as usize,
                None => partition::block_length(
                    self.a_large,
                    self.a_small,
                    self.nb_a_large,
                    *self.transfer_length.as_ref().unwrap(),
                    oti.encoding_symbol_length as u64,
                    payload_id.sbn,
                ) as usize,
            };

            if self.nb_allocated_blocks >= 2
                && self.total_allocated_blocks_size + block_length > self.max_size_allocated
            {
                log::error!(
                    "NB Allocated blocks={}/{} total_allocated={}/{} block_length={}",
                    self.nb_allocated_blocks,
                    nb_blocks,
                    self.total_allocated_blocks_size,
                    self.max_size_allocated,
                    block_length,
                );

                self.state = State::Error;
                return Err(FluteError::new(
                    "Maximum number of blocks allocated is reached",
                ));
            }

            log::debug!("Init block {} with length {}", payload_id.sbn, block_length);
            match block.init(oti, source_block_length, block_length, payload_id.sbn) {
                Ok(_) => {}
                Err(_) => {
                    self.state = State::Error;
                    return Err(FluteError::new("Fail to init source block decoder"));
                }
            }
            self.nb_allocated_blocks += 1;
            self.total_allocated_blocks_size += block_length;
        }

        block.push(pkt, &payload_id);
        if block.completed {
            log::debug!("block {} is completed", payload_id.sbn);
            self.write_blocks(payload_id.sbn, now)?;
        }

        Ok(())
    }

    #[cfg(feature = "opentelemetry")]
    fn init_logger(&mut self, propagator: Option<&std::collections::HashMap<String, String>>) {
        if self.logger.is_some() {
            return;
        }

        self.logger = Some(ObjectReceiverLogger::new(
            &self.endpoint,
            self.tsi,
            self.toi,
            propagator,
        ))
    }

    pub fn attach_fdt(
        &mut self,
        fdt_instance_id: u32,
        fdt: &FdtInstance,
        now: std::time::SystemTime,
    ) -> bool {
        debug_assert!(self.toi != lct::TOI_FDT);
        self.last_timestamp = now;
        if self.fdt_instance_id.is_some() {
            return false;
        }

        let file = match fdt.get_file(&self.toi) {
            Some(file) => file,
            None => return false,
        };

        #[cfg(feature = "opentelemetry")]
        if self.logger.is_none() {
            let propagator = file.get_optel_propagator();
            self.init_logger(propagator.as_ref());
        }

        if self.cenc.is_none() {
            self.cenc = match &file.content_encoding {
                Some(str) => Some(str.as_str().try_into().unwrap_or(lct::Cenc::Null)),
                None => Some(lct::Cenc::Null),
            };
            log::debug!("Set cenc from FDT {:?}", self.cenc);
        }

        if self.oti.is_none() {
            self.oti = fdt.get_oti_for_file(file);
            if self.oti.is_some() {
                debug_assert!(self.transfer_length.is_none());
                self.transfer_length = Some(file.get_transfer_length());
            }
        }

        if self.transfer_length.is_none() {
            self.transfer_length = Some(file.get_transfer_length());
        } else {
            let fdt_transfer_length = file.get_transfer_length();
            if self.transfer_length.unwrap() != fdt_transfer_length {
                log::warn!(
                    "Transfer length mismatch {} != {}",
                    self.transfer_length.unwrap(),
                    fdt_transfer_length
                );
            }
        }

        self.content_location = Some(file.content_location.clone());

        let mut groups = match fdt.group.as_ref() {
            Some(groups) => groups.clone(),
            None => vec![],
        };

        if let Some(group) = file.group.as_ref() {
            groups.append(&mut group.to_vec())
        }

        #[cfg(feature = "opentelemetry")]
        let _span = self.logger.as_mut().map(|l| l.fdt_attached());

        self.content_md5 = file.content_md5.clone();
        self.fdt_instance_id = Some(fdt_instance_id);

        self.cache_control = Some(file.get_object_cache_control(fdt.get_expiration_date()));
        self.content_length = file.content_length.map(|c| c as usize);
        self.content_type = file.content_type.clone();
        self.groups = groups;
        self.e_tag = file.file_etag.clone();

        self.init_blocks_partitioning();
        self.init_object_writer(now);
        self.push_from_cache(now);
        self.write_blocks(0, now)
            .unwrap_or_else(|_| self.error("Fail to write blocks to storage", now, false));
        self.push_from_cache(now);
        true
    }

    pub fn create_meta(&self) -> ObjectMetadata {
        ObjectMetadata {
            content_location: self
                .content_location
                .clone()
                .unwrap_or("file:///".to_string()),
            content_length: self.content_length.clone(),
            content_type: self.content_type.clone(),
            cache_control: self.cache_control.unwrap_or(ObjectCacheControl::NoCache),
            groups: match self.groups.is_empty() {
                true => None,
                false => Some(self.groups.clone()),
            },
            md5: self.content_md5.clone(),
            #[cfg(feature = "opentelemetry")]
            optel_propagator: self.logger.as_ref().map(|l| l.get_propagator()),
            #[cfg(not(feature = "opentelemetry"))]
            optel_propagator: None,
            oti: self.oti.clone(),
            transfer_length: self.transfer_length.map(|s| s as usize),
            cenc: self.cenc.clone(),
            e_tag: self.e_tag.clone(),
        }
    }

    pub fn byte_left(&self) -> usize {
        if let Some(w) = self.block_writer.as_ref() {
            return w.left();
        }
        usize::MAX
    }

    fn init_object_writer(&mut self, now: SystemTime) {
        if self.object_writer.is_some() {
            return;
        }

        if self.fdt_instance_id.is_none()
            || self.cenc.is_none()
            || self.transfer_length.is_none()
            || self.oti.is_none()
        {
            return;
        }

        let object_writer = match self.object_writer_builder.new_object_writer(
            &self.endpoint,
            &self.tsi,
            &self.toi,
            &self.create_meta(),
            now,
        ) {
            ObjectWriterBuilderResult::StoreObject(object_writer) => object_writer,
            ObjectWriterBuilderResult::ObjectAlreadyReceived => {
                self.state = State::Completed;
                return;
            }
            ObjectWriterBuilderResult::Abort => {
                self.state = State::Error;
                return;
            }
        };

        if self.content_md5.is_some() {
            self.enable_md5_check = object_writer.enable_md5_check();
        }

        debug_assert!(self.block_writer.is_none());
        self.object_writer = Some(ObjectWriterSession {
            writer: object_writer,
            state: ObjectWriterSessionState::Idle,
        });

        let object_writer = self.object_writer.as_mut().unwrap();

        if object_writer.writer.open(now).is_err() {
            self.error("Fail to create destination on storage", now, false);
            return;
        };

        let transfer_length = self.transfer_length.unwrap();
        if transfer_length != 0 {
            self.block_writer = Some(BlockWriter::new(
                transfer_length as usize,
                self.content_length.clone(),
                self.cenc.unwrap(),
                self.enable_md5_check,
            ));
        }

        object_writer.state = ObjectWriterSessionState::Opened;
    }

    fn write_blocks(&mut self, sbn_start: u32, now: std::time::SystemTime) -> Result<()> {
        if self.object_writer.is_none() {
            return Ok(());
        }

        if self.object_writer.as_ref().unwrap().state != ObjectWriterSessionState::Opened {
            return Ok(());
        }

        if self.block_writer.is_none() {
            return Ok(());
        }

        debug_assert!(self.block_writer.is_some());
        let mut sbn = sbn_start as usize;
        let writer = self.block_writer.as_mut().unwrap();
        while sbn >= self.blocks_offset && sbn - self.blocks_offset < self.blocks.len() {
            let block_offset = sbn - self.blocks_offset;
            let block = &mut self.blocks[block_offset];
            if !block.completed {
                break;
            }

            let success = writer.write(
                sbn as u32,
                block,
                self.object_writer.as_ref().unwrap().writer.as_ref(),
                now,
            )?;
            if !success {
                break;
            }
            sbn += 1;
            debug_assert!(self.total_allocated_blocks_size >= block.block_size);
            self.total_allocated_blocks_size -= block.block_size;
            self.nb_allocated_blocks -= 1;

            if block_offset == 0 {
                self.blocks_offset += 1;
                self.blocks.pop_front();
            } else {
                block.deallocate();
            }

            if writer.is_completed() {
                let md5_valid = self
                    .content_md5
                    .as_ref()
                    .map(|md5| writer.check_md5(md5))
                    .unwrap_or(true);

                if md5_valid {
                    self.complete(now);
                } else {
                    let md5 = writer.get_md5().map(|f| f.to_owned());
                    log::error!(
                        "MD5 does not match expects {:?} received {:?} {:?}",
                        self.content_md5,
                        &md5,
                        self.content_location
                    );

                    self.error(
                        &format!(
                            "MD5 does not match expects {:?} received {:?}",
                            self.content_md5, &md5
                        ),
                        now,
                        false,
                    );
                }
                break;
            }
        }
        Ok(())
    }

    fn complete(&mut self, now: std::time::SystemTime) {
        #[cfg(feature = "opentelemetry")]
        let _span = self.logger.as_mut().map(|l| l.complete());

        self.state = State::Completed;

        if let Some(object_writer) = self.object_writer.as_mut() {
            object_writer.state = ObjectWriterSessionState::Closed;
            object_writer.writer.complete(now);
        }

        // Free space by removing blocks
        self.blocks.clear();
        self.cache.clear();
        self.cache_size = 0;
    }

    fn error(&mut self, description: &str, now: SystemTime, interrupted: bool) {
        #[cfg(feature = "opentelemetry")]
        self.init_logger(None);

        #[cfg(feature = "opentelemetry")]
        let _span = self.logger.as_mut().map(|l| match interrupted {
            true => l.interrupted(description),
            false => l.error(description),
        });

        log::debug!("{}", description);
        self.state = match interrupted {
            true => State::Interrupted,
            false => State::Error,
        };

        if let Some(object_writer) = self.object_writer.as_mut() {
            object_writer.state = ObjectWriterSessionState::Error;
            if interrupted {
                object_writer.writer.interrupted(now);
            } else {
                object_writer.writer.error(now);
            }
        }

        self.blocks.clear();
        self.cache.clear();
        self.cache_size = 0;
    }

    fn push_from_cache(&mut self, now: std::time::SystemTime) {
        if self.nb_block() == 0 {
            return;
        }

        while let Some(item) = self.cache.pop() {
            let pkt = item.to_pkt();
            if self.push_to_block(&pkt, now).is_err() {
                self.error("Fail to push block", now, false);
                break;
            }
        }
        self.cache_size = 0;
    }

    fn set_cenc_from_pkt(&mut self, pkt: &alc::AlcPkt) {
        if self.cenc.is_some() {
            return;
        }
        self.cenc = pkt.cenc;
        if self.toi == lct::TOI_FDT && self.cenc.is_none() {
            log::debug!("Force Cenc to Null for the FDT");
            self.cenc = Some(lct::Cenc::Null);
        } else if self.cenc.is_some() {
            log::debug!("Set cenc from pkt {:?}", self.cenc);
        }
    }

    fn set_fdt_id_from_pkt(&mut self, pkt: &alc::AlcPkt) {
        if self.fdt_instance_id.is_some() || pkt.lct.toi != lct::TOI_FDT {
            return;
        }
        self.fdt_instance_id = pkt.fdt_info.as_ref().map(|info| info.fdt_instance_id);
    }

    fn set_oti_from_pkt(&mut self, pkt: &alc::AlcPkt, now: SystemTime) {
        if self.oti.is_some() {
            return;
        }

        if pkt.oti.is_none() {
            return;
        }

        self.oti = pkt.oti.clone();
        if self.transfer_length.is_none() {
            self.transfer_length = pkt.transfer_length;
        } else {
            if pkt.transfer_length.is_some() {
                if self.transfer_length.unwrap() != pkt.transfer_length.unwrap() {
                    log::warn!(
                        "Transfer length mismatch {} != {}",
                        self.transfer_length.unwrap(),
                        pkt.transfer_length.unwrap()
                    );
                }
            }
        }

        if pkt.transfer_length.is_none() {
            log::warn!("Bug? Pkt contains OTI without transfer length");
            self.error("Bug? Pkt contains OTI without transfer length", now, false);
            return;
        }

        if self.cenc.is_none() {
            log::warn!("Cenc is unknown ?");
            debug_assert!(self.toi != lct::TOI_FDT);
        }
    }

    fn cache(&mut self, pkt: &alc::AlcPkt) -> Result<()> {
        if self.cache_size == 0 {
            log::warn!(
                "TSI={} TOI={} Packet without FTI received before the FDT",
                self.tsi,
                self.toi
            );
        }

        if self.cache_size >= self.max_size_allocated {
            return Err(FluteError::new("Pkt cache is full"));
        }

        match self.cache_size.checked_add(pkt.data.len()) {
            Some(_) => Ok(()),
            None => Err(FluteError::new("add overflow")),
        }?;
        self.cache.push(Box::new(pkt.to_cache()));
        Ok(())
    }

    ///  Block Partitioning Algorithm
    ///  See https://tools.ietf.org/html/rfc5052
    fn init_blocks_partitioning(&mut self) {
        if self.nb_block() > 0 {
            return;
        }

        if self.oti.is_none() || self.transfer_length.is_none() {
            return;
        }

        debug_assert!(self.blocks.is_empty());
        let oti = self.oti.as_ref().unwrap();

        let (a_large, a_small, nb_a_large, nb_blocks) = partition::block_partitioning(
            oti.maximum_source_block_length as u64,
            self.transfer_length.unwrap_or_default(),
            oti.encoding_symbol_length as u64,
        );

        log::debug!(
            "Block partitioning
        toi={}
         tl={:?} a_large={} a_small={} nb_a_large={} maximum_source_block_length={}",
            self.toi,
            self.transfer_length,
            a_large,
            a_small,
            nb_a_large,
            oti.maximum_source_block_length
        );

        log::debug!("oti={:?}", oti);
        self.a_large = a_large;
        self.a_small = a_small;
        self.nb_a_large = nb_a_large;

        self.blocks_variable_size =
            oti.fec_encoding_id == oti::FECEncodingID::ReedSolomonGF28UnderSpecified;
        // || oti.fec_encoding_id == oti::FECEncodingID::RaptorQ;
        log::debug!(
            "Preallocate {} blocks of {} or {} symbols to decode a file of {:?} bytes with toi {}",
            nb_blocks,
            self.a_large,
            self.a_small,
            self.transfer_length,
            self.toi
        );
        self.blocks.resize_with(
            std::cmp::min(nb_blocks as usize, MAX_PREALLOCATED_BLOCKS),
            BlockDecoder::new,
        );
    }
}

impl Drop for ObjectReceiver {
    fn drop(&mut self) {
        if let Some(object_writer) = self.object_writer.as_mut() {
            if object_writer.state == ObjectWriterSessionState::Opened
                || object_writer.state == ObjectWriterSessionState::Idle
            {
                log::error!(
                    "Drop object received with state {:?} TOI={} Endpoint={:?} Content-Location={:?}",
                    object_writer.state,
                    self.toi,
                    self.endpoint,
                    self.content_location.as_ref().map(|u| u.to_string())
                );
                self.error(
                    "Drop object in open state, pkt missing ?",
                    self.last_timestamp,
                    false,
                );
            } else if object_writer.state == ObjectWriterSessionState::Error {
                if self.state != State::Interrupted {
                    log::error!(
                        "Drop object received with state {:?} TOI={} Endpoint={:?} Content-Location={:?}",
                        object_writer.state,
                        self.toi,
                        self.endpoint,
                        self.content_location.as_ref().map(|u| u.to_string())
                    );
                } else {
                    log::warn!(
                        "Interrupted object  TOI={} Endpoint={:?} Content-Location={:?}",
                        self.toi,
                        self.endpoint,
                        self.content_location.as_ref().map(|u| u.to_string())
                    );
                }
            }
        }
    }
}
