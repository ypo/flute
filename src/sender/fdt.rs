use super::filedesc::FileDesc;
use super::observer::ObserverList;
use super::sender::FDTPublishMode;
use super::toiallocator::{Toi, ToiAllocator};
use super::{objectdesc, ObjectDesc};
use crate::common::{fdtinstance::FdtInstance, lct, oti};
use crate::sender::objectdesc::CarouselRepeatMode;
use crate::sender::observer;
use crate::sender::TOIMaxLength;
use crate::tools;
use crate::tools::error::{FluteError, Result};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug)]
pub struct Fdt {
    _tsi: u64,
    fdtid: u32,
    oti: oti::Oti,
    files_transfer_queue: VecDeque<Arc<FileDesc>>,
    fdt_transfer_queue: VecDeque<Arc<FileDesc>>,
    files: std::collections::HashMap<u128, Arc<FileDesc>>,
    current_fdt_transfer: Option<Arc<FileDesc>>,
    complete: Option<bool>,
    cenc: lct::Cenc,
    duration: std::time::Duration,
    carousel_mode: CarouselRepeatMode,
    inband_sct: bool,
    last_publish: Option<SystemTime>,
    observers: ObserverList,
    groups: Option<Vec<String>>,
    toi_allocator: Arc<ToiAllocator>,
    publish_mode: FDTPublishMode,
}

impl Fdt {
    pub fn new(
        tsi: u64,
        fdtid: u32,
        default_oti: &oti::Oti,
        cenc: lct::Cenc,
        duration: std::time::Duration,
        carousel_mode: CarouselRepeatMode,
        inband_sct: bool,
        observers: ObserverList,
        toi_max_length: TOIMaxLength,
        toi_initial_value: Option<u128>,
        groups: Option<Vec<String>>,
        publish_mode: FDTPublishMode,
    ) -> Fdt {
        Fdt {
            _tsi: tsi,
            fdtid,
            oti: default_oti.clone(),
            files_transfer_queue: VecDeque::new(),
            fdt_transfer_queue: VecDeque::new(),
            files: std::collections::HashMap::new(),
            current_fdt_transfer: None,
            complete: None,
            cenc,
            duration,
            carousel_mode,
            inband_sct,
            last_publish: None,
            observers,
            groups,
            toi_allocator: ToiAllocator::new(toi_max_length, toi_initial_value),
            publish_mode,
        }
    }

    fn get_fdt_instance(&self, now: SystemTime) -> FdtInstance {
        let ntp = tools::system_time_to_ntp(now).unwrap_or(0);
        let expires_ntp = (ntp >> 32) + self.duration.as_secs();

        let oti_attributes = match self.oti.fec_encoding_id {
            oti::FECEncodingID::RaptorQ => None, // RaptorA scheme parameters is object dependent
            _ => Some(self.oti.get_attributes()),
        };

        let files = match self.publish_mode {
            FDTPublishMode::ObjectsBeingTransferred => self.get_files_being_transferred(),
            FDTPublishMode::FullFDT => self.files.values().map(|desc| desc.as_ref()).collect(),
        };

        FdtInstance {
            xmlns: None,
            xmlns_xsi: None,
            expires: expires_ntp.to_string(),
            complete: self.complete,
            content_type: None,
            content_encoding: None,
            fec_oti_fec_encoding_id: match &oti_attributes {
                None => None,
                Some(attr) => attr.fec_oti_fec_encoding_id,
            },
            fec_oti_encoding_symbol_length: match &oti_attributes {
                None => None,
                Some(attr) => attr.fec_oti_encoding_symbol_length,
            },
            fec_oti_fec_instance_id: match &oti_attributes {
                None => None,
                Some(attr) => attr.fec_oti_fec_instance_id,
            },
            fec_oti_max_number_of_encoding_symbols: match &oti_attributes {
                None => None,
                Some(attr) => attr.fec_oti_max_number_of_encoding_symbols,
            },
            fec_oti_maximum_source_block_length: match &oti_attributes {
                None => None,
                Some(attr) => attr.fec_oti_maximum_source_block_length,
            },
            fec_oti_scheme_specific_info: match &oti_attributes {
                None => None,
                Some(attr) => attr.fec_oti_scheme_specific_info.clone(),
            },

            file: Some(files.iter().map(|desc| desc.to_file_xml(now)).collect()),
            xmlns_mbms_2005: None,
            xmlns_mbms_2007: None,
            xmlns_mbms_2008: None,
            xmlns_mbms_2009: None,
            xmlns_mbms_2012: None,
            xmlns_mbms_2015: None,
            xmlns_sv: None,
            full_fdt: match self.publish_mode {
                FDTPublishMode::FullFDT => Some(true),
                FDTPublishMode::ObjectsBeingTransferred => None,
            },
            base_url_1: None,
            base_url_2: None,
            group: self.groups.clone(),
            mbms_session_identity_expiry: None,
            schema_version: Some(4),
            delimiter: Some(0),
        }
    }

    pub fn get_files_being_transferred(&self) -> Vec<&FileDesc> {
        self.files
            .iter()
            .filter(|file| file.1.as_ref().is_transferring())
            .map(|f| f.1.as_ref())
            .collect()
    }

    pub fn allocate_toi(&mut self) -> Box<Toi> {
        ToiAllocator::allocate(&self.toi_allocator)
    }

    pub fn add_object(
        &mut self,
        priority: u32,
        mut obj: Box<objectdesc::ObjectDesc>,
    ) -> Result<u128> {
        if self.complete == Some(true) {
            return Err(FluteError::new(
                "FDT is complete, no new object should be added",
            ));
        }

        if obj.toi.is_none() {
            obj.set_toi(self.allocate_toi());
        }

        let filedesc = Arc::new(FileDesc::new(priority, obj, &self.oti, None, false)?);
        let ret = filedesc.toi;
        debug_assert!(!self.files.contains_key(&filedesc.toi));
        self.files.insert(filedesc.toi, filedesc.clone());
        self.files_transfer_queue.push_back(filedesc);
        Ok(ret)
    }

    pub fn trigger_transfer_at(&mut self, toi: u128, timestamp: Option<SystemTime>) -> bool {
        if let Some(file) = self.files.get(&toi) {
            if file.is_transferring() {
                return true;
            }

            file.reset_last_transfer(timestamp);
            return true;
        }
        false
    }

    pub fn get_objects_in_fdt(&self) -> std::collections::HashMap<u128, &ObjectDesc> {
        self.files
            .iter()
            .map(|obj| (*obj.0, obj.1.object.as_ref()))
            .collect()
    }

    pub fn is_added(&self, toi: u128) -> bool {
        self.files.contains_key(&toi)
    }

    pub fn remove_object(&mut self, toi: u128) -> bool {
        match self.files.remove(&toi) {
            Some(_) => {}
            None => return false,
        };
        self.files_transfer_queue.retain(|obj| obj.toi != toi);
        true
    }

    pub fn nb_transfers(&mut self, toi: u128) -> Option<u64> {
        let obj = self.files.get(&toi)?;
        Some(obj.total_nb_transfer())
    }

    pub fn nb_objects(&self) -> usize {
        if self.files.len() > 100 {
            let uri: Vec<String> = self
                .files
                .iter()
                .map(|f| f.1.object.content_location.to_string())
                .collect();
            log::error!("{:?}", uri);
        }

        self.files.len()
    }

    pub fn publish(&mut self, now: SystemTime) -> Result<()> {
        log::debug!("TSI={} Publish new FDT", self._tsi);
        let content = self.to_xml(now)?;
        let mut obj = objectdesc::ObjectDesc::create_from_buffer(
            content,
            "text/xml",
            &url::Url::parse("file:///").unwrap(),
            1,
            Some(self.carousel_mode),
            None,
            None,
            self.groups.clone(),
            self.cenc,
            true,
            None,
            true,
        )?;
        obj.toi = Some(ToiAllocator::allocate_toi_fdt(&self.toi_allocator));
        let filedesc = Arc::new(FileDesc::new(
            0,
            obj,
            &self.oti,
            Some(self.fdtid),
            self.inband_sct,
        )?);
        filedesc.set_published();
        self.fdt_transfer_queue.push_back(filedesc);
        self.fdtid = (self.fdtid + 1) & 0xFFFFF;
        self.last_publish = Some(now);
        self.files.iter().for_each(|(_, file)| file.set_published());
        Ok(())
    }

    pub fn need_transfer_fdt(&self) -> bool {
        !self.fdt_transfer_queue.is_empty()
    }

    fn current_fdt_will_expire(&self, now: SystemTime) -> bool {
        if !self.fdt_transfer_queue.is_empty() {
            return false;
        }

        if self.current_fdt_transfer.is_none() || self.last_publish.is_none() {
            return true;
        }

        let duration = now
            .duration_since(self.last_publish.unwrap())
            .unwrap_or_default();

        if self.duration > std::time::Duration::from_secs(30) {
            return self.duration + std::time::Duration::from_secs(5) < duration;
        }

        self.duration <= duration
    }

    pub fn get_next_fdt_transfer(&mut self, now: SystemTime) -> Option<Arc<FileDesc>> {
        if let Some(current_fdt_transfer) = self.current_fdt_transfer.as_ref() {
            if current_fdt_transfer.is_transferring() {
                return None;
            }
        }

        if self.current_fdt_will_expire(now) {
            log::debug!("FDT will expire soon, publish new version");
            self.publish(now).ok();
        }

        if !self.fdt_transfer_queue.is_empty() {
            self.current_fdt_transfer = self.fdt_transfer_queue.pop_front();
        }

        let current_fdt_transfer = self.current_fdt_transfer.as_ref()?;
        if !current_fdt_transfer.should_transfer_now(0, self.publish_mode, now) {
            return None;
        }

        log::debug!("TSI={} Start transmission of FDT", self._tsi);
        current_fdt_transfer.transfer_started(now);
        Some(current_fdt_transfer.clone())
    }

    pub fn get_next_file_transfer(
        &mut self,
        priority: u32,
        now: SystemTime,
    ) -> Option<Arc<FileDesc>> {
        let (index, _) = self
            .files_transfer_queue
            .iter()
            .enumerate()
            .find(|(_, item)| item.should_transfer_now(priority, self.publish_mode, now))?;

        let file = self.files_transfer_queue.remove(index).unwrap();
        log::info!(
            "Start transmission of {} toi={}",
            file.object.content_location.as_str(),
            file.toi
        );

        let evt = observer::Event::StartTransfer(observer::FileInfo { toi: file.toi });
        self.observers.dispatch(&evt, now);

        file.transfer_started(now);

        match self.publish_mode {
            FDTPublishMode::ObjectsBeingTransferred => {
                self.publish(now).ok();
            }
            FDTPublishMode::FullFDT => {}
        }

        Some(file.clone())
    }

    pub fn transfer_done(&mut self, file: Arc<FileDesc>, now: SystemTime) {
        file.transfer_done(now);

        if file.toi == lct::TOI_FDT {
            log::debug!("TSI={} Stop transmission of FDT", self._tsi);

            if file.is_expired() {
                self.current_fdt_transfer = None;
            }
        } else {
            let evt = observer::Event::StopTransfer(observer::FileInfo { toi: file.toi });
            self.observers.dispatch(&evt, now);

            if !self.files.contains_key(&file.toi) {
                log::debug!("Transfer is finished and file has been removed from FDT");
                return;
            }

            log::info!(
                "Stop transmission of {} toi={}",
                file.object.content_location.as_str(),
                file.toi
            );
            if !file.is_expired() {
                log::debug!("Transfer file again");
                self.files_transfer_queue.push_back(file);
            } else {
                self.files.remove(&file.toi);
                //self.publish(now).ok();
            }
        }
    }

    pub fn set_complete(&mut self) {
        self.complete = Some(true)
    }

    pub fn to_xml(&self, now: SystemTime) -> Result<Vec<u8>> {
        let mut buffer = ToFmtWrite(Vec::new());
        let mut writer = quick_xml::Writer::new_with_indent(&mut buffer, b' ', 2);

        match writer.write_event(quick_xml::events::Event::Decl(
            quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None),
        )) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        let ser = match quick_xml::se::Serializer::with_root(&mut buffer, Some("FDT-Instance")) {
            Ok(ser) => ser,
            Err(e) => return Err(FluteError::new(e.to_string())),
        };
        match self.get_fdt_instance(now).serialize(ser) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        Ok(buffer.0)
    }
}

struct ToFmtWrite<T>(pub T);

impl<T> std::fmt::Write for ToFmtWrite<T>
where
    T: std::io::Write,
{
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

impl<T> std::io::Write for ToFmtWrite<T>
where
    T: std::io::Write,
{
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }

    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }
}

#[cfg(test)]
mod tests {

    use std::time::SystemTime;

    use crate::common::lct;
    use crate::sender::observer::ObserverList;

    use super::objectdesc;
    use super::oti;

    fn create_fdt() -> super::Fdt {
        let oti: oti::Oti = Default::default();
        let mut fdt = super::Fdt::new(
            10,
            1,
            &oti,
            lct::Cenc::Null,
            std::time::Duration::from_secs(3600),
            crate::sender::objectdesc::CarouselRepeatMode::DelayBetweenTransfers(
                std::time::Duration::from_secs(1),
            ),
            true,
            ObserverList::new(),
            crate::sender::TOIMaxLength::ToiMax112,
            Some(1),
            Some(vec!["Group1".to_owned()]),
            crate::sender::FDTPublishMode::FullFDT,
        );
        let obj1 = objectdesc::ObjectDesc::create_from_buffer(
            Vec::new(),
            "plain/txt",
            &url::Url::parse("file:///object1").unwrap(),
            2,
            None,
            None,
            None,
            Some(vec!["Test1".to_owned()]),
            lct::Cenc::Null,
            true,
            None,
            true,
        )
        .unwrap();

        let obj2 = objectdesc::ObjectDesc::create_from_buffer(
            Vec::new(),
            "plain/txt",
            &url::Url::parse("file:///object2").unwrap(),
            2,
            None,
            None,
            None,
            None,
            lct::Cenc::Gzip,
            true,
            None,
            true,
        )
        .unwrap();

        fdt.add_object(0, obj1).unwrap();
        fdt.add_object(0, obj2).unwrap();
        fdt.groups = Some(vec!["Group1".to_owned(), "Group2".to_owned()]);
        fdt
    }

    #[test]
    pub fn test_fdt() {
        use std::{io::Write, process::Command};

        crate::tests::init();
        let fdt = create_fdt();
        let buffer = fdt.to_xml(SystemTime::now()).unwrap();
        let content = String::from_utf8(buffer.clone()).unwrap();
        log::info!("{}", content);

        let check_fdt_folder = "./assets/xsd/";
        let xsd_filename = "FLUTE-FDT-3GPP-Main.xsd";
        let xsd_path = std::path::Path::new(check_fdt_folder).join(xsd_filename);

        let xml_generated_data = String::from_utf8(buffer).unwrap();
        let tmp_fdt_file = tempfile::Builder::new()
            .prefix("TempFile")
            .suffix(".xml")
            .tempfile()
            .unwrap();
        write!(&tmp_fdt_file, "{}", &xml_generated_data).unwrap();

        let output = Command::new("xmllint")
            .arg("--schema")
            .arg(xsd_path)
            .arg(&tmp_fdt_file.path())
            .arg("--noout")
            .output()
            .expect("failed to execute process");

        let output_print = std::str::from_utf8(&output.stderr).expect("ascii to text went wrong ");

        assert!(
            output.status.success(),
            "\n\nValidation failed\n\n{}\n\n",
            output_print
        );
        // log::info!("content={}", content);
    }
}
