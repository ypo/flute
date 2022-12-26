use std::rc::Rc;
use std::time::SystemTime;

use serde::Serialize;

use super::fdtinstance::FdtInstance;
use super::filedesc::FileDesc;
use super::lct;
use super::objectdesc;
use super::oti;
use crate::tools;
use crate::tools::error::{FluteError, Result};

#[derive(Debug)]
pub struct Fdt {
    fdtid: u32,
    oti: oti::Oti,
    toi: u128,
    files_transfer_queue: Vec<Rc<FileDesc>>,
    fdt_transfer_queue: Vec<Rc<FileDesc>>,
    files: std::collections::HashMap<u128, Rc<FileDesc>>,
    current_fdt_transfer: Option<Rc<FileDesc>>,
    complete: Option<bool>,
    cenc: lct::CENC,
}

impl Fdt {
    pub fn new(fdtid: u32, default_oti: &oti::Oti, cenc: lct::CENC) -> Fdt {
        Fdt {
            fdtid,
            oti: default_oti.clone(),
            toi: 1,
            files_transfer_queue: Vec::new(),
            fdt_transfer_queue: Vec::new(),
            files: std::collections::HashMap::new(),
            current_fdt_transfer: None,
            complete: None,
            cenc,
        }
    }

    fn get_fdt_instance(&self, now: &SystemTime) -> FdtInstance {
        let (seconds_ntp, _) = tools::system_time_to_ntp(now).unwrap_or((0, 0));
        let expires_ntp = seconds_ntp + 3600;

        let oti_attributes = self.oti.get_attributes();
        FdtInstance {
            xmlns_xsi: "http://www.w3.org/2001/XMLSchema-instance".into(),
            xsi_schema_location: "urn:ietf:params:xml:ns:fdt ietf-flute-fdt.xsd".into(),
            expires: expires_ntp.to_string(),
            complete: self.complete,
            content_type: None,
            content_encoding: None,
            fec_oti_fec_encoding_id: oti_attributes.fec_oti_fec_encoding_id,
            fec_oti_encoding_symbol_length: oti_attributes.fec_oti_encoding_symbol_length,
            fec_oti_fec_instance_id: oti_attributes.fec_oti_fec_instance_id,
            fec_oti_max_number_of_encoding_symbols: oti_attributes
                .fec_oti_max_number_of_encoding_symbols,
            fec_oti_maximum_source_block_length: oti_attributes.fec_oti_maximum_source_block_length,
            fec_oti_scheme_specific_info: oti_attributes.fec_oti_scheme_specific_info,
            file: self
                .files
                .iter()
                .map(|(_, desc)| desc.to_file_xml())
                .collect(),
        }
    }

    pub fn add_object(&mut self, obj: Box<objectdesc::ObjectDesc>) {
        let filedesc = FileDesc::new(obj, &self.oti, &self.toi, None);
        self.toi += 1;
        if self.toi == lct::TOI_FDT {
            self.toi = 1;
        }

        assert!(self.files.contains_key(&filedesc.toi) == false);

        self.files.insert(filedesc.toi, filedesc.clone());
        self.files_transfer_queue.push(filedesc);
    }

    pub fn publish(&mut self, now: &SystemTime) -> Result<()> {
        let content = self.to_xml(now)?;
        let obj = objectdesc::ObjectDesc::create_from_buffer(
            &content,
            "text/xml",
            &url::Url::parse("file:///").unwrap(),
            1,
            Some(std::time::Duration::new(1, 0)),
            self.cenc,
            true,
            None,
            true,
        )?;
        let filedesc = FileDesc::new(obj, &self.oti, &lct::TOI_FDT, Some(self.fdtid));
        self.fdt_transfer_queue.push(filedesc);
        self.fdtid = (self.fdtid + 1) & 0xFFFFF;
        Ok(())
    }

    pub fn get_next_fdt_transfer(&mut self) -> Option<Rc<FileDesc>> {
        if self.current_fdt_transfer.is_some() {
            if self
                .current_fdt_transfer
                .as_ref()
                .unwrap()
                .is_transferring()
            {
                return None;
            }
        }

        if !self.fdt_transfer_queue.is_empty() {
            self.current_fdt_transfer = self.fdt_transfer_queue.pop();
        }

        if self.current_fdt_transfer.is_none() {
            return None;
        }

        let now = std::time::Instant::now();
        match &self.current_fdt_transfer {
            Some(value) if value.should_transfer_now(now) => {
                value.transfer_started();
                Some(value.clone())
            }
            _ => None,
        }
    }

    pub fn get_next_file_transfer(&mut self) -> Option<Rc<FileDesc>> {
        let now = std::time::Instant::now();

        let (index, _) = self
            .files_transfer_queue
            .iter()
            .enumerate()
            .find(|(_, item)| item.should_transfer_now(now))?;

        let file = self.files_transfer_queue.swap_remove(index);
        log::info!(
            "Start transmission for {}",
            file.object.content_location.as_str()
        );
        file.transfer_started();
        Some(file.clone())
    }

    pub fn transfer_done(&mut self, file: Rc<FileDesc>) {
        log::debug!("Tranfer done for toi {}", file.toi);
        file.transfer_done();

        if file.toi == lct::TOI_FDT {
            if file.is_expired() {
                self.current_fdt_transfer = None;
            }
        } else {
            log::info!(
                "Stop transmission for {}",
                file.object.content_location.as_str()
            );
            if file.is_expired() == false {
                log::debug!("Transfer file again");
                self.files_transfer_queue.push(file);
            } else {
                self.files.remove(&file.toi);
                self.publish(&SystemTime::now()).ok();
            }
        }
    }

    pub fn set_complete(&mut self) {
        self.complete = Some(true)
    }

    fn to_xml(&self, now: &SystemTime) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut writer = quick_xml::Writer::new_with_indent(&mut buffer, b' ', 2);

        match writer.write_event(quick_xml::events::Event::Decl(
            quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None),
        )) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        let mut ser = quick_xml::se::Serializer::with_root(writer, Some("FDT-Instance"));
        match self.get_fdt_instance(&now).serialize(&mut ser) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {

    use std::time::SystemTime;

    use crate::alc::lct;

    use super::objectdesc;
    use super::oti;

    #[test]
    pub fn test_fdt() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let mut fdt = super::Fdt::new(1, &oti, lct::CENC::Null);
        let obj = objectdesc::ObjectDesc::create_from_buffer(
            &Vec::new(),
            "txt",
            &url::Url::parse("file:///").unwrap(),
            2,
            None,
            lct::CENC::Null,
            true,
            None,
            true,
        )
        .unwrap();

        fdt.add_object(obj);

        let buffer = fdt.to_xml(&SystemTime::now()).unwrap();
        let content = String::from_utf8(buffer.clone()).unwrap();
        log::info!("content={}", content);
    }
}
