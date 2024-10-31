use base64::Engine;

use super::objectdesc::{create_fdt_cache_control, ObjectDesc};
use crate::common::oti::SchemeSpecific;
use crate::common::{fdtinstance, oti, partition};
use crate::error::{FluteError, Result};
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use std::time::SystemTime;

#[derive(Debug)]
struct TransferInfo {
    transferring: bool,
    transfer_count: u32,
    total_nb_transfer: u64,
    last_transfer: Option<SystemTime>,
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
        Ok(FileDesc {
            priority,
            object,
            oti,
            fdt_id,
            sender_current_time,
            transfer_info: RwLock::new(TransferInfo {
                transferring: false,
                transfer_count: 0,
                last_transfer: None,
                total_nb_transfer: 0,
            }),
            published: AtomicBool::new(false),
            toi,
        })
    }

    pub fn total_nb_transfer(&self) -> u64 {
        let info = self.transfer_info.read().unwrap();
        info.total_nb_transfer
    }

    pub fn transfer_started(&self) {
        let mut info = self.transfer_info.write().unwrap();
        info.transferring = true;

        if info.transfer_count == self.object.max_transfer_count
            && self.object.carousel_delay.is_some()
        {
            info.transfer_count = 0;
        }
    }

    pub fn transfer_done(&self, now: SystemTime) {
        let mut info = self.transfer_info.write().unwrap();
        debug_assert!(info.transferring);
        info.transferring = false;
        info.transfer_count += 1;
        info.total_nb_transfer += 1;
        info.last_transfer = Some(now);
    }

    pub fn is_expired(&self) -> bool {
        let info = self.transfer_info.read().unwrap();
        if self.object.max_transfer_count > info.transfer_count {
            return false;
        }
        self.object.carousel_delay.is_none()
    }

    pub fn is_transferring(&self) -> bool {
        let info = self.transfer_info.read().unwrap();
        info.transferring
    }

    pub fn is_last_transfer(&self) -> bool {
        if self.object.carousel_delay.is_some() {
            return false;
        }

        let info = self.transfer_info.read().unwrap();
        self.object.max_transfer_count == info.transfer_count + 1
    }

    pub fn should_transfer_now(&self, priority: u32, now: SystemTime) -> bool {
        if self.priority != priority {
            return false;
        }

        if !self.is_published() {
            log::warn!("File with TOI {} is not published", self.toi);
            return false;
        }

        let info = self.transfer_info.read().unwrap();
        if self.object.max_transfer_count > info.transfer_count {
            return true;
        }

        if self.object.carousel_delay.is_none() || info.last_transfer.is_none() {
            return true;
        }

        let delay = self.object.carousel_delay.as_ref().unwrap();
        let last_transfer = info.last_transfer.as_ref().unwrap();
        now.duration_since(*last_transfer).unwrap_or_default() > *delay
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
            file_etag: None,
            independent_unit_positions: None,
            delimiter: Some(0),
            delimiter2: Some(0),
            group: None,
            optel_propagator,
        }
    }
}
