use base64::Engine;

use super::objectdesc::{create_fdt_cache_control, ObjectDesc};
use super::FDTPublishMode;
use crate::common::oti::SchemeSpecific;
use crate::common::{fdtinstance, oti, partition};
use crate::error::{FluteError, Result};
use crate::sender::objectdesc::CarouselRepeatMode;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use std::time::SystemTime;

#[derive(Debug)]
struct TransferInfo {
    transferring: bool,
    transfer_count: u32,
    total_nb_transfer: u64,
    last_transfer_end_time: Option<SystemTime>,
    last_transfer_start_time: Option<SystemTime>,
    next_transfer_timestamp: Option<SystemTime>,
    packet_transmission_tick: Option<std::time::Duration>,
    transfer_start_time: Option<SystemTime>,
}

impl TransferInfo {
    fn init(&mut self, object: &ObjectDesc, oti: &oti::Oti, now: SystemTime) {
        self.transferring = true;
        self.last_transfer_start_time = Some(now);
        let mut packet_transmission_tick = None;
        if let Some(target_acquisition_latency) = object.target_acquisition.as_ref() {
            packet_transmission_tick = match target_acquisition_latency {
                crate::sender::objectdesc::TargetAcquisition::AsFastAsPossible => None,
                crate::sender::objectdesc::TargetAcquisition::WithinDuration(duration) => {
                    let nb_packets = object
                        .transfer_length
                        .div_ceil(oti.encoding_symbol_length as u64);
                    // TODO should we take into account the FEC encoding symbol length ?
                    Some(duration.div_f64(nb_packets as f64))
                }
                crate::sender::objectdesc::TargetAcquisition::WithinTime(target_time) => {
                    let duration = target_time.duration_since(now).unwrap_or_default();
                    if duration.is_zero() {
                        log::warn!(
                            "Target acquisition time is in the past target={:?} now={:?} for={}",
                            target_time,
                            now,
                            object.content_location
                        );
                    }
                    let nb_packets = object
                        .transfer_length
                        .div_ceil(oti.encoding_symbol_length as u64);
                    Some(duration.div_f64(nb_packets as f64))
                }
            }
        }

        self.packet_transmission_tick = packet_transmission_tick;
        if self.packet_transmission_tick.is_some() {
            self.next_transfer_timestamp = Some(now)
        }

        if self.transfer_count == object.max_transfer_count && object.carousel_mode.is_some() {
            self.transfer_count = 0;
        }
    }

    fn done(&mut self, now: SystemTime) {
        self.transferring = false;
        self.transfer_count += 1;
        self.total_nb_transfer += 1;
        self.last_transfer_end_time = Some(now);
    }

    fn tick(&mut self) {
        if let Some(tick) = self.packet_transmission_tick {
            if let Some(next_transfer_timestamp) = self.next_transfer_timestamp.as_mut() {
                if let Some(next) = next_transfer_timestamp.checked_add(tick) {
                    *next_transfer_timestamp = next;
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct FileDesc {
    pub priority: u32,
    pub object: Box<ObjectDesc>,
    pub oti: oti::Oti,
    pub fdt_id: Option<u32>,
    pub sender_current_time: bool,
    pub published: AtomicBool,
    pub toi: u128,
    transfer_info: RwLock<TransferInfo>,
}

impl FileDesc {
    pub fn new(
        priority: u32,
        object: Box<ObjectDesc>,
        default_oti: &oti::Oti,
        fdt_id: Option<u32>,
        sender_current_time: bool,
    ) -> Result<FileDesc> {
        assert!(object.toi.is_some());
        let mut oti = match &object.oti {
            Some(res) => res.clone(),
            None => default_oti.clone(),
        };

        let max_transfer_length = oti.max_transfer_length();
        if object.transfer_length as usize > max_transfer_length {
            return Err(FluteError::new(format!(
                "Object transfer length of {} is bigger than {}, so is incompatible with the parameters of your OTI",
                object.transfer_length, max_transfer_length
            )));
        }

        if oti.fec_encoding_id == oti::FECEncodingID::RaptorQ
            || oti.fec_encoding_id == oti::FECEncodingID::Raptor
        {
            // Calculate the source block length of Raptor / RaptorQ

            let (_, _, _, nb_blocks) = partition::block_partitioning(
                oti.maximum_source_block_length as u64,
                object.transfer_length,
                oti.encoding_symbol_length as u64,
            );

            if oti.fec_encoding_id == oti::FECEncodingID::RaptorQ {
                if oti.scheme_specific.is_none() {
                    return Err(FluteError::new(
                        "FEC RaptorQ is selected, however scheme parameters are not defined",
                    ));
                }

                let nb_blocks:u8 = nb_blocks.try_into().map_err(|_| {
                    FluteError::new(format!(
                        "Object transfer length of {} requires the transmission of {} source blocks, the maximum is {}, your object is incompatible with the FEC parameters of your OTI",
                        object.transfer_length,
                        nb_blocks, u8::MAX
                    ))
                })?;

                if let SchemeSpecific::RaptorQ(scheme) = oti.scheme_specific.as_mut().unwrap() {
                    scheme.source_blocks_length = nb_blocks;
                }
            } else if oti.fec_encoding_id == oti::FECEncodingID::Raptor {
                if oti.scheme_specific.is_none() {
                    return Err(FluteError::new(
                        "FEC Raptor is selected, however scheme parameters are not defined",
                    ));
                }

                let nb_blocks:u16 = nb_blocks.try_into().map_err(|_| {
                    FluteError::new(format!(
                        "Object transfer length of {} requires the transmission of {} source blocks, the maximum is {}, your object is incompatible with the FEC parameters of your OTI",
                        object.transfer_length,
                        nb_blocks, u8::MAX
                    ))
                })?;

                if let SchemeSpecific::Raptor(scheme) = oti.scheme_specific.as_mut().unwrap() {
                    scheme.source_blocks_length = nb_blocks;
                }
            }
        }

        let toi = object.toi.as_ref().unwrap().get();
        let transfer_start_time = object.transfer_start_time.clone();
        Ok(FileDesc {
            priority,
            object,
            oti,
            fdt_id,
            sender_current_time,
            transfer_info: RwLock::new(TransferInfo {
                transferring: false,
                transfer_count: 0,
                last_transfer_start_time: None,
                last_transfer_end_time: None,
                total_nb_transfer: 0,
                next_transfer_timestamp: None,
                packet_transmission_tick: None,
                transfer_start_time,
            }),
            published: AtomicBool::new(false),
            toi,
        })
    }

    pub fn total_nb_transfer(&self) -> u64 {
        let info = self.transfer_info.read().unwrap();
        info.total_nb_transfer
    }

    pub fn can_transfer_be_stopped(&self) -> bool {
        if self.object.allow_immediate_stop_before_first_transfer == Some(true) {
            return true;
        }

        self.total_nb_transfer() > 0
    }

    pub fn transfer_started(&self, now: SystemTime) {
        let mut info = self.transfer_info.write().unwrap();
        info.init(&self.object, &self.oti, now);
    }

    pub fn transfer_done(&self, now: SystemTime) {
        let mut info = self.transfer_info.write().unwrap();
        info.done(now);
    }

    pub fn is_expired(&self) -> bool {
        let info = self.transfer_info.read().unwrap();
        if self.object.max_transfer_count > info.transfer_count {
            return false;
        }
        self.object.carousel_mode.is_none()
    }

    pub fn is_transferring(&self) -> bool {
        let info = self.transfer_info.read().unwrap();
        info.transferring
    }

    pub fn get_next_transfer_timestamp(&self) -> Option<SystemTime> {
        let info = self.transfer_info.read().unwrap();
        info.next_transfer_timestamp
    }

    pub fn inc_next_transfer_timestamp(&self) {
        let mut info = self.transfer_info.write().unwrap();
        info.tick();
    }

    pub fn reset_last_transfer(&self, start_time: Option<SystemTime>) {
        let mut info = self.transfer_info.write().unwrap();
        info.last_transfer_end_time = None;
        info.last_transfer_start_time = None;
        if start_time.is_some() {
            info.transfer_start_time = start_time;
        }
    }

    pub fn is_last_transfer(&self) -> bool {
        if self.object.carousel_mode.is_some() {
            return false;
        }

        let info = self.transfer_info.read().unwrap();
        self.object.max_transfer_count == info.transfer_count + 1
    }

    pub fn should_transfer_now(
        &self,
        priority: u32,
        fdt_publish_mode: FDTPublishMode,
        now: SystemTime,
    ) -> bool {
        if self.priority != priority {
            return false;
        }

        if fdt_publish_mode == FDTPublishMode::FullFDT && !self.is_published() {
            log::warn!("File with TOI {} is not published", self.toi);
            return false;
        }

        let info = self.transfer_info.read().unwrap();
        if let Some(start_time) = info.transfer_start_time {
            if now < start_time {
                return false;
            }
        }

        if info.transferring {
            return false;
        }

        if self.object.max_transfer_count > info.transfer_count {
            return true;
        }

        if self.object.carousel_mode.is_none()
            || info.last_transfer_end_time.is_none()
            || info.last_transfer_start_time.is_none()
        {
            return true;
        }

        let carousel_mode = self.object.carousel_mode.as_ref().unwrap();
        let (last_time, interval) = match carousel_mode {
            CarouselRepeatMode::DelayBetweenTransfers(interval) => {
                (info.last_transfer_end_time.as_ref().unwrap(), interval)
            }
            CarouselRepeatMode::IntervalBetweenStartTimes(interval) => {
                (info.last_transfer_start_time.as_ref().unwrap(), interval)
            }
        };

        let last_transfer_interval = now.duration_since(*last_time).unwrap_or_default();
        last_transfer_interval > *interval
    }

    pub fn is_published(&self) -> bool {
        self.published.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_published(&self) {
        self.published
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn to_file_xml(&self, now: SystemTime) -> fdtinstance::File {
        let oti_attributes = match self.oti.fec_encoding_id {
            oti::FECEncodingID::RaptorQ => Some(self.oti.get_attributes()), // for RaptorQ we need to add OTI for each object
            _ => self.object.oti.as_ref().map(|oti| oti.get_attributes()),
        };

        let optel_propagator = self.object.optel_propagator.as_ref().map(|propagator| {
            let s = serde_json::to_string(&propagator).unwrap();
            base64::engine::general_purpose::STANDARD.encode(s)
        });

        fdtinstance::File {
            content_location: self.object.content_location.to_string(),
            toi: self.toi.to_string(),
            content_length: Some(self.object.content_length),
            transfer_length: Some(self.object.transfer_length),
            content_type: Some(self.object.content_type.clone()),
            content_encoding: Some(self.object.cenc.to_str().to_string()),
            content_md5: self.object.md5.clone(),
            fec_oti_fec_encoding_id: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_fec_encoding_id),
            fec_oti_fec_instance_id: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_fec_instance_id),
            fec_oti_maximum_source_block_length: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_maximum_source_block_length),
            fec_oti_encoding_symbol_length: oti_attributes
                .as_ref()
                .map(|f| f.fec_oti_encoding_symbol_length)
                .unwrap_or_default(),
            fec_oti_max_number_of_encoding_symbols: oti_attributes
                .as_ref()
                .and_then(|f| f.fec_oti_max_number_of_encoding_symbols),
            fec_oti_scheme_specific_info: oti_attributes
                .and_then(|f| f.fec_oti_scheme_specific_info),
            cache_control: self
                .object
                .cache_control
                .as_ref()
                .map(|cc| create_fdt_cache_control(cc, now)),
            alternate_content_location_1: None,
            alternate_content_location_2: None,
            mbms_session_identity: None,
            decryption_key_uri: None,
            fec_redundancy_level: None,
            file_etag: self.object.e_tag.clone(),
            independent_unit_positions: None,
            delimiter: Some(0),
            delimiter2: Some(0),
            group: None,
            optel_propagator,
        }
    }
}
