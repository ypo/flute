use std::rc::Rc;

use super::filedesc;
use super::filedesc::FileDesc;
use super::lct;
use super::objectdesc;
use super::oti;
use crate::tools::error::{FluteError, Result};
use serde::Serialize;

#[derive(Debug, PartialEq, Serialize, Clone)]
struct FdtInstance {
    #[serde(rename = "xmlns:xsi")]
    xmlns_xsi: String,
    #[serde(rename = "xmlns:schemaLocation")]
    xsi_schema_location: String,
    #[serde(rename = "Expired")]
    expired: String,
    #[serde(rename = "Complete")]
    complete: Option<bool>,
    #[serde(rename = "Content-Type")]
    content_type: Option<String>,
    #[serde(rename = "Content-Encoding")]
    content_encoding: Option<String>,
    #[serde(rename = "FEC-OTI-FEC-Encoding-ID")]
    fec_oti_fec_encoding_id: Option<u8>,
    #[serde(rename = "FEC-OTI-FEC-Instance-ID")]
    fec_oti_fec_instance_id: Option<u64>,
    #[serde(rename = "FEC-OTI-Maximum-Source-Block-Length")]
    fec_oti_maximum_source_block_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Encoding-Symbol-Length")]
    fec_oti_encoding_symbol_length: Option<u64>,
    #[serde(rename = "FEC-OTI-Max-Number-of-Encoding-Symbols")]
    fec_oti_max_number_of_encoding_symbols: Option<u64>,
    #[serde(rename = "FEC-OTI-Scheme-Specific-Info")]
    fec_oti_scheme_specific_info: Option<String>, // Base64
    file: Vec<filedesc::File>,
}

pub struct Fdt {
    fdtid: u32,
    oti: oti::Oti,
    toi: u128,
    files_transfer_queue: Vec<Rc<FileDesc>>,
    fdt_transfer_queue: Vec<Rc<FileDesc>>,
    files: std::collections::HashMap<u128, Rc<FileDesc>>,
}

impl Fdt {
    pub fn new(fdtid: u32, default_oti: &oti::Oti) -> Fdt {
        Fdt {
            fdtid,
            oti: default_oti.clone(),
            toi: 1,
            files_transfer_queue: Vec::new(),
            fdt_transfer_queue: Vec::new(),
            files: std::collections::HashMap::new(),
        }
    }

    fn get_fdt_instance(&self) -> FdtInstance {
        let oti_attributes = self.oti.get_attributes();
        FdtInstance {
            xmlns_xsi: "http://www.w3.org/2001/XMLSchema-instance".into(),
            xsi_schema_location: "urn:ietf:params:xml:ns:fdt ietf-flute-fdt.xsd".into(),
            expired: "1234".into(),
            complete: None,
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

    pub fn publish(&mut self) {}

    pub fn get_next_fdt(&mut self) -> Option<Rc<FileDesc>> {
        self.fdt_transfer_queue.pop()
    }

    pub fn get_next_file(&mut self) -> Option<Rc<FileDesc>> {
        self.files_transfer_queue.pop()
    }

    pub fn release_next_file(&mut self, desc: Rc<FileDesc>) {}

    pub fn to_xml(&self) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let mut writer = quick_xml::Writer::new_with_indent(&mut buffer, b' ', 2);

        match writer.write_event(quick_xml::events::Event::Decl(
            quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None),
        )) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        let mut ser = quick_xml::se::Serializer::with_root(writer, Some("FDT-Instance"));
        match self.get_fdt_instance().serialize(&mut ser) {
            Ok(_) => {}
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {

    use super::objectdesc;
    use super::oti;

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_fdt() {
        init();

        let oti: oti::Oti = Default::default();
        let mut fdt = super::Fdt::new(1, &oti);
        let obj = objectdesc::ObjectDesc::create_from_buffer(
            &Vec::new(),
            "txt",
            &url::Url::parse("file:///").unwrap(),
        )
        .unwrap();

        fdt.add_object(obj);

        let buffer = fdt.to_xml().unwrap();
        let content = String::from_utf8(buffer.clone()).unwrap();
        log::info!("content={}", content);
    }
}
