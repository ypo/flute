use super::objectdesc::ObjectDesc;
use crate::common::{fdtinstance, oti, partition};
use crate::error::{FluteError, Result};
use std::sync::RwLock;
use std::time::SystemTime;

#[derive(Debug)]
struct TransferInfo {
    transferring: bool,
    transfer_count: u32,
    last_transfer: Option<SystemTime>,
}

#[derive(Debug)]
pub struct FileDesc {
    pub object: Box<ObjectDesc>,
    pub oti: oti::Oti,
    pub toi: u128,
    pub fdt_id: Option<u32>,
    pub sender_current_time: Option<SystemTime>,
    transfer_info: RwLock<TransferInfo>,
}

impl FileDesc {
    pub fn new(
        object: Box<ObjectDesc>,
        default_oti: &oti::Oti,
        toi: &u128,
        fdt_id: Option<u32>,
        sender_current_time: Option<SystemTime>,
    ) -> Result<FileDesc> {
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
                object.transfer_length as u64,
                oti.encoding_symbol_length as u64,
            );

            if oti.fec_encoding_id == oti::FECEncodingID::RaptorQ {
                if oti.raptorq_scheme_specific.is_none() {
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

                let scheme = oti.raptorq_scheme_specific.as_mut().unwrap();
                scheme.source_blocks_length = nb_blocks;
            } else if oti.fec_encoding_id == oti::FECEncodingID::Raptor {
                if oti.raptor_scheme_specific.is_none() {
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

                let scheme = oti.raptor_scheme_specific.as_mut().unwrap();
                scheme.source_blocks_length = nb_blocks;
            }
        }

        Ok(FileDesc {
            object,
            oti,
            toi: toi.clone(),
            fdt_id,
            sender_current_time,
            transfer_info: RwLock::new(TransferInfo {
                transferring: false,
                transfer_count: 0,
                last_transfer: None,
            }),
        })
    }

    pub fn transfer_started(&self) {
        let mut info = self.transfer_info.write().unwrap();
        info.transferring = true;

        if info.transfer_count == self.object.max_transfer_count {
            if self.object.carousel_delay.is_some() {
                info.transfer_count = 0;
            }
        }
    }

    pub fn transfer_done(&self, now: SystemTime) {
        let mut info = self.transfer_info.write().unwrap();
        assert!(info.transferring == true);
        info.transferring = false;
        info.transfer_count += 1;
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

    pub fn should_transfer_now(&self, now: SystemTime) -> bool {
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

    pub fn to_file_xml(&self) -> fdtinstance::File {
        let oti_attributes = match self.oti.fec_encoding_id {
            oti::FECEncodingID::RaptorQ => Some(self.oti.get_attributes()), // for RaptorQ we need to add OTI for each object
            _ => self.object.oti.as_ref().map(|oti| oti.get_attributes()),
        };

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
                .map_or(None, |f| f.fec_oti_fec_encoding_id),
            fec_oti_fec_instance_id: oti_attributes
                .as_ref()
                .map_or(None, |f| f.fec_oti_fec_instance_id),
            fec_oti_maximum_source_block_length: oti_attributes
                .as_ref()
                .map_or(None, |f| f.fec_oti_maximum_source_block_length),
            fec_oti_encoding_symbol_length: oti_attributes
                .as_ref()
                .map_or(None, |f| f.fec_oti_encoding_symbol_length),
            fec_oti_max_number_of_encoding_symbols: oti_attributes
                .as_ref()
                .map_or(None, |f| f.fec_oti_max_number_of_encoding_symbols),
            fec_oti_scheme_specific_info: oti_attributes
                .map_or(None, |f| f.fec_oti_scheme_specific_info),
        }
    }
}
