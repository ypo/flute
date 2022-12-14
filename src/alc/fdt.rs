use super::fdtinstance::FdtInstance;
use super::filedesc::FileDesc;
use super::lct;
use super::objectdesc;
use super::oti;
use crate::tools;
use crate::tools::error::{FluteError, Result};
use serde::Serialize;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug)]
pub struct Fdt {
    fdtid: u32,
    oti: oti::Oti,
    toi: u128,
    files_transfer_queue: Vec<Arc<FileDesc>>,
    fdt_transfer_queue: Vec<Arc<FileDesc>>,
    files: std::collections::HashMap<u128, Arc<FileDesc>>,
    current_fdt_transfer: Option<Arc<FileDesc>>,
    complete: Option<bool>,
    cenc: lct::Cenc,
    duration: std::time::Duration,
    inband_sct: bool,
    last_publish: Option<SystemTime>,
}

impl Fdt {
    pub fn new(
        fdtid: u32,
        default_oti: &oti::Oti,
        cenc: lct::Cenc,
        duration: std::time::Duration,
        inband_sct: bool,
    ) -> Fdt {
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
            duration,
            inband_sct,
            last_publish: None,
        }
    }

    fn get_fdt_instance(&self, now: SystemTime) -> FdtInstance {
        let ntp = tools::system_time_to_ntp(now).unwrap_or(0);
        let expires_ntp = (ntp >> 32) + self.duration.as_secs();

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
            file: Some(
                self.files
                    .iter()
                    .map(|(_, desc)| desc.to_file_xml())
                    .collect(),
            ),
        }
    }

    pub fn add_object(&mut self, obj: Box<objectdesc::ObjectDesc>) -> Result<u128> {
        let toi = self.toi;
        let filedesc = Arc::new(FileDesc::new(obj, &self.oti, &toi, None, None)?);
        self.toi += 1;
        if self.toi == lct::TOI_FDT {
            self.toi = 1;
        }

        assert!(self.files.contains_key(&filedesc.toi) == false);
        self.files.insert(filedesc.toi, filedesc.clone());
        self.files_transfer_queue.push(filedesc);
        Ok(toi)
    }

    pub fn remove_object(&mut self, toi: u128) -> bool {
        match self.files.remove(&toi) {
            Some(_) => {}
            None => return false,
        };
        self.fdt_transfer_queue.retain(|obj| obj.toi != toi);
        true
    }

    pub fn nb_objects(&self) -> usize {
        self.files.len()
    }

    pub fn publish(&mut self, now: SystemTime) -> Result<()> {
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
        let filedesc = Arc::new(FileDesc::new(
            obj,
            &self.oti,
            &lct::TOI_FDT,
            Some(self.fdtid),
            match self.inband_sct {
                true => Some(now.clone()),
                false => None,
            },
        )?);
        self.fdt_transfer_queue.push(filedesc);
        self.fdtid = (self.fdtid + 1) & 0xFFFFF;
        self.last_publish = Some(now);
        Ok(())
    }

    fn current_fdt_will_expire(&self, now: SystemTime) -> bool {
        if self.current_fdt_transfer.is_none() || self.last_publish.is_none() {
            return true;
        }

        let duration = now
            .duration_since(self.last_publish.unwrap())
            .unwrap_or_default();

        self.duration + std::time::Duration::from_secs(5) < duration
    }

    pub fn get_next_fdt_transfer(&mut self, now: SystemTime) -> Option<Arc<FileDesc>> {
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

        if self.current_fdt_will_expire(now) {
            log::info!("FDT will expire soon, publish new version");
            self.publish(now).ok();
        }

        if !self.fdt_transfer_queue.is_empty() {
            self.current_fdt_transfer = self.fdt_transfer_queue.pop();
        }

        if self.current_fdt_transfer.is_none() {
            return None;
        }

        match &self.current_fdt_transfer {
            Some(value) if value.should_transfer_now(now) => {
                value.transfer_started();
                Some(value.clone())
            }
            _ => None,
        }
    }

    pub fn get_next_file_transfer(&mut self, now: SystemTime) -> Option<Arc<FileDesc>> {
        let (index, _) = self
            .files_transfer_queue
            .iter()
            .enumerate()
            .find(|(_, item)| item.should_transfer_now(now))?;

        let file = self.files_transfer_queue.swap_remove(index);
        log::info!(
            "Start transmission of {}",
            file.object.content_location.as_str()
        );
        file.transfer_started();
        Some(file.clone())
    }

    pub fn transfer_done(&mut self, file: Arc<FileDesc>, now: SystemTime) {
        file.transfer_done(now);

        if file.toi == lct::TOI_FDT {
            if file.is_expired() {
                self.current_fdt_transfer = None;
            }
        } else {
            if !self.files.contains_key(&file.toi) {
                log::debug!("Transfer is finished and file has been removed from FDT");
                return;
            }

            log::info!(
                "Stop transmission of {}",
                file.object.content_location.as_str()
            );
            if file.is_expired() == false {
                log::debug!("Transfer file again");
                self.files_transfer_queue.push(file);
            } else {
                self.files.remove(&file.toi);
                self.publish(now).ok();
            }
        }
    }

    pub fn set_complete(&mut self) {
        self.complete = Some(true)
    }

    fn to_xml(&self, now: SystemTime) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut writer = quick_xml::Writer::new_with_indent(&mut buffer, b' ', 2);

        match writer.write_event(quick_xml::events::Event::Decl(
            quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None),
        )) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        let mut ser = quick_xml::se::Serializer::with_root(writer, Some("FDT-Instance"));
        match self.get_fdt_instance(now).serialize(&mut ser) {
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
        let mut fdt = super::Fdt::new(
            1,
            &oti,
            lct::Cenc::Null,
            std::time::Duration::from_secs(3600),
            true,
        );
        let obj = objectdesc::ObjectDesc::create_from_buffer(
            &Vec::new(),
            "txt",
            &url::Url::parse("file:///").unwrap(),
            2,
            None,
            lct::Cenc::Null,
            true,
            None,
            true,
        )
        .unwrap();

        fdt.add_object(obj).unwrap();

        let buffer = fdt.to_xml(SystemTime::now()).unwrap();
        let content = String::from_utf8(buffer.clone()).unwrap();
        log::info!("content={}", content);
    }
}
