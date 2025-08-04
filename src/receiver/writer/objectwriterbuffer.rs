use super::{ObjectMetadata, ObjectWriter, ObjectWriterBuilder, ObjectWriterBuilderResult};
use crate::{common::udpendpoint::UDPEndpoint, tools::error::Result};
use std::{cell::RefCell, rc::Rc, time::SystemTime};

///
/// Write objects received by the `receiver` to a buffers
///
#[derive(Debug)]
pub struct ObjectWriterBufferBuilder {
    /// List of all objects received
    pub objects: RefCell<Vec<Rc<RefCell<ObjectWriterBuffer>>>>,
    /// True when MD5 check is enabled
    pub enable_md5_check: bool,
}

///
/// Write a FLUTE object to a buffer
///
#[derive(Debug)]
struct ObjectWriterBufferWrapper {
    inner: Rc<RefCell<ObjectWriterBuffer>>,
    enable_md5_check: bool,
}

#[derive(Debug)]
/// Object stored in a buffer
pub struct ObjectWriterBuffer {
    /// true when the object is fully received
    pub complete: bool,
    /// true when an error occured during the reception
    pub error: bool,
    /// buffer containing the data of the object
    pub data: Vec<u8>,
    /// Metadata of the object
    pub meta: ObjectMetadata,
    /// Time when the object reception started
    pub start_time: SystemTime,
    /// Time when the object reception ended
    pub end_time: Option<SystemTime>,
}

impl ObjectWriterBufferBuilder {
    /// Return a new `ObjectWriterBuffer`
    pub fn new(enable_md5_check: bool) -> ObjectWriterBufferBuilder {
        ObjectWriterBufferBuilder {
            objects: RefCell::new(Vec::new()),
            enable_md5_check,
        }
    }
}

impl Default for ObjectWriterBufferBuilder {
    fn default() -> Self {
        Self::new(true)
    }
}

impl ObjectWriterBuilder for ObjectWriterBufferBuilder {
    fn new_object_writer(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        meta: &ObjectMetadata,
        now: std::time::SystemTime,
    ) -> ObjectWriterBuilderResult {
        let obj = Rc::new(RefCell::new(ObjectWriterBuffer {
            complete: false,
            error: false,
            data: Vec::new(),
            meta: meta.clone(),
            start_time: now,
            end_time: None,
        }));

        let obj_wrapper = Box::new(ObjectWriterBufferWrapper {
            inner: obj.clone(),
            enable_md5_check: self.enable_md5_check,
        });
        self.objects.borrow_mut().push(obj);
        ObjectWriterBuilderResult::StoreObject(obj_wrapper)
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

impl ObjectWriter for ObjectWriterBufferWrapper {
    fn open(&self, _now: SystemTime) -> Result<()> {
        Ok(())
    }

    fn write(&self, _sbn: u32, data: &[u8], _now: SystemTime) -> Result<()> {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
        Ok(())
    }

    fn complete(&self, now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        log::info!("Object complete !");
        inner.complete = true;
        inner.end_time = Some(now);
    }

    fn error(&self, now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        log::error!("Object received with error");
        inner.error = true;
        inner.end_time = Some(now);
    }

    fn interrupted(&self, now: SystemTime) {
        let mut inner = self.inner.borrow_mut();
        log::error!("Object reception interrupted");
        inner.error = true;
        inner.end_time = Some(now);
    }

    fn enable_md5_check(&self) -> bool {
        self.enable_md5_check
    }
}
