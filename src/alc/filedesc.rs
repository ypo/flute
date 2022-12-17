use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;

use super::objectdesc::ObjectDesc;
use super::oti;

struct TransferInfo {
    transferring: bool,
    transfer_count: u32,
    last_transfer: Option<std::time::Instant>,
}

pub struct FileDesc {
    pub object: Box<ObjectDesc>,
    pub oti: oti::Oti,
    pub toi: u128,
    pub fdt_id: Option<u32>,
    transfer_info: RefCell<TransferInfo>,
}

impl FileDesc {
    pub fn new(
        object: Box<ObjectDesc>,
        default_oti: &oti::Oti,
        toi: &u128,
        fdt_id: Option<u32>,
    ) -> Rc<FileDesc> {
        let oti = match &object.oti {
            Some(res) => res.clone(),
            None => default_oti.clone(),
        };
        Rc::new(FileDesc {
            object,
            oti,
            toi: toi.clone(),
            fdt_id,
            transfer_info: RefCell::new(TransferInfo {
                transferring: false,
                transfer_count: 0,
                last_transfer: None,
            }),
        })
    }

    pub fn transfer_started(&self) {
        let mut info = self.transfer_info.borrow_mut();
        info.transferring = true;

        if info.transfer_count == self.object.max_transfer_count {
            if self.object.carousel_delay_ns.is_some() {
                info.transfer_count = 0;
            }
        }
    }

    pub fn transfer_done(&self) {
        let mut info = self.transfer_info.borrow_mut();
        assert!(info.transferring == true);
        info.transferring = false;
        info.transfer_count += 1;
        info.last_transfer = Some(std::time::Instant::now());
    }

    pub fn is_expired(&self) -> bool {
        let info = self.transfer_info.borrow();
        if self.object.max_transfer_count > info.transfer_count {
            return false;
        }
        self.object.carousel_delay_ns.is_none()
    }

    pub fn is_transferring(&self) -> bool {
        let info = self.transfer_info.borrow();
        info.transferring
    }

    pub fn should_transfer_now(&self, now: std::time::Instant) -> bool {
        let info = self.transfer_info.borrow();
        if self.object.max_transfer_count > info.transfer_count {
            return true;
        }

        if self.object.carousel_delay_ns.is_none() || info.last_transfer.is_none() {
            return true;
        }

        let delay = self.object.carousel_delay_ns.as_ref().unwrap();
        let last_transfer = info.last_transfer.as_ref().unwrap();
        now.duration_since(*last_transfer) > *delay
    }

    pub fn to_file_xml(&self) -> File {
        let oti_attributes = self.object.oti.as_ref().map(|oti| oti.get_attributes());

        File {
            content_location: self.object.content_location.to_string(),
            toi: self.toi.to_string(),
            content_length: match self.object.content_length {
                value if value > 0 => Some(value),
                _ => None,
            },
            transfer_length: match self.object.transfer_length {
                value if value > 0 => Some(value),
                _ => None,
            },
            content_type: Some(self.object.content_type.clone()),
            content_encoding: Some(self.object.content_encoding().to_string()),
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

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct File {
    #[serde(rename = "Content-Location")]
    content_location: String,
    #[serde(rename = "TOI")]
    toi: String,
    #[serde(rename = "Content-Length")]
    content_length: Option<u64>,
    #[serde(rename = "Transfer-Length")]
    transfer_length: Option<u64>,
    #[serde(rename = "Content-Type")]
    content_type: Option<String>,
    #[serde(rename = "Content-Encoding")]
    content_encoding: Option<String>,
    #[serde(rename = "Content-MD5")]
    content_md5: Option<String>,
    #[serde(rename = "FEC-OTI-FEC-Encoding-ID")]
    pub fec_oti_fec_encoding_id: Option<u8>,
    #[serde(rename = "FEC-OTI-FEC-Instance-ID")]
    pub fec_oti_fec_instance_id: Option<u64>,
    #[serde(rename = "FEC-OTI-Maximum-Source-Block-Length")]
    pub fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Encoding-Symbol-Length")]
    pub fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Max-Number-of-Encoding-Symbols")]
    pub fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(rename = "FEC-OTI-Scheme-Specific-Info")]
    pub fec_oti_scheme_specific_info: Option<String>, // Base64
}
