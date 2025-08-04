use super::{ObjectMetadata, ObjectWriter, ObjectWriterBuilder, ObjectWriterBuilderResult};
use crate::{
    common::udpendpoint::UDPEndpoint,
    error::{FluteError, Result},
};
use std::{cell::RefCell, io::Write, time::SystemTime};

///
/// Write objects received by the `receiver` to a filesystem
///
#[derive(Debug)]
pub struct ObjectWriterFSBuilder {
    dest: std::path::PathBuf,
    enable_md5_check: bool,
}

impl ObjectWriterFSBuilder {
    /// Return a new `ObjectWriterBuffer`
    pub fn new(dest: &std::path::Path, enable_md5_check: bool) -> Result<ObjectWriterFSBuilder> {
        if !dest.is_dir() {
            return Err(FluteError::new(format!("{:?} is not a directory", dest)));
        }

        Ok(ObjectWriterFSBuilder {
            dest: dest.to_path_buf(),
            enable_md5_check,
        })
    }
}

impl ObjectWriterBuilder for ObjectWriterFSBuilder {
    fn new_object_writer(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        meta: &ObjectMetadata,
        _now: std::time::SystemTime,
    ) -> ObjectWriterBuilderResult {
        ObjectWriterBuilderResult::StoreObject(Box::new(ObjectWriterFS {
            dest: self.dest.clone(),
            inner: RefCell::new(ObjectWriterFSInner {
                destination: None,
                writer: None,
            }),
            meta: meta.clone(),
            enable_md5_check: self.enable_md5_check,
        }))
    }

    fn update_cache_control(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        _meta: &ObjectMetadata,
        _now: std::time::SystemTime,
    ) {
    }

    fn fdt_received(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _fdt_xml: &str,
        _expires: std::time::SystemTime,
        _meta: &ObjectMetadata,
        _transfer_duration: std::time::Duration,
        _now: std::time::SystemTime,
        _ext_time: Option<std::time::SystemTime>,
    ) {
    }
}

///
/// Write an object to a file system.  
/// Uses the content-location to create the destination path of the object.  
/// If the destination path does not exists, the folder hierarchy is created.  
/// Existing files will be overwritten by this object.
///
#[derive(Debug)]
pub struct ObjectWriterFS {
    /// Folder destination were the object will be written
    dest: std::path::PathBuf,
    inner: RefCell<ObjectWriterFSInner>,
    meta: ObjectMetadata,
    enable_md5_check: bool,
}

///
///
#[derive(Debug)]
pub struct ObjectWriterFSInner {
    destination: Option<std::path::PathBuf>,
    writer: Option<std::io::BufWriter<std::fs::File>>,
}

impl ObjectWriter for ObjectWriterFS {
    fn open(&self, _now: SystemTime) -> Result<()> {
        let url = url::Url::parse(&self.meta.content_location);
        let content_location_path = match &url {
            Ok(url) => url.path(),
            Err(e) => match e {
                url::ParseError::RelativeUrlWithoutBase => &self.meta.content_location,
                url::ParseError::RelativeUrlWithCannotBeABaseBase => &self.meta.content_location,
                _ => {
                    log::error!(
                        "Fail to parse content location {:?} {:?}",
                        self.meta.content_location,
                        e
                    );
                    return Err(FluteError::new(format!(
                        "Fail to parse content location {:?} {:?}",
                        self.meta.content_location, e
                    )));
                }
            },
        };
        let relative_path = content_location_path
            .strip_prefix('/')
            .unwrap_or(content_location_path);
        let destination = self.dest.join(relative_path);
        log::info!(
            "Create destination {:?} {:?} {:?}",
            self.dest,
            relative_path,
            destination
        );
        let parent = destination.parent();
        if parent.is_some() {
            let parent = parent.unwrap();
            if !parent.is_dir() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let file = std::fs::File::create(&destination)?;
        let mut inner = self.inner.borrow_mut();
        inner.writer = Some(std::io::BufWriter::new(file));
        inner.destination = Some(destination.to_path_buf());
        Ok(())
    }

    fn write(&self, _sbn: u32, data: &[u8], _now: SystemTime) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        if inner.writer.is_none() {
            return Ok(());
        }
        inner
            .writer
            .as_mut()
            .unwrap()
            .write_all(data)
            .map_err(|e| {
                log::error!("Fail to write data to file {:?} {:?}", inner.destination, e);
                FluteError::new(format!(
                    "Fail to write data to file {:?} {:?}",
                    inner.destination, e
                ))
            })?;
        Ok(())
    }

    fn complete(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        if inner.writer.is_none() {
            return;
        }

        println!("File {:?} is completed !", inner.destination);
        inner.writer.as_mut().unwrap().flush().ok();
        inner.writer = None;
        inner.destination = None
    }

    fn error(&self, _now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        inner.writer = None;
        if inner.destination.is_some() {
            log::error!("Remove file {:?}", inner.destination);
            std::fs::remove_file(inner.destination.as_ref().unwrap()).ok();
            inner.destination = None;
        }
    }

    fn interrupted(&self, now: SystemTime) {
        self.error(now);
    }

    fn enable_md5_check(&self) -> bool {
        self.enable_md5_check
    }
}
