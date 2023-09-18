use super::{ObjectMetadata, ObjectWriter, ObjectWriterBuilder};
use crate::{receiver::UDPEndpoint, tools::error::Result};
use std::{cell::RefCell, rc::Rc};

///
/// Write objects received by the `receiver` to a buffers
///
#[derive(Debug)]
pub struct ObjectWriterBufferBuilder {
    /// List of all objects received
    pub objects: RefCell<Vec<Rc<RefCell<ObjectWriterBuffer>>>>,
}

///
/// Write a FLUTE object to a buffer
///
#[derive(Debug)]
struct ObjectWriterBufferWrapper {
    inner: Rc<RefCell<ObjectWriterBuffer>>,
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
    pub meta: Option<ObjectMetadata>,
}

impl ObjectWriterBufferBuilder {
    /// Return a new `ObjectWriterBuffer`
    pub fn new() -> ObjectWriterBufferBuilder {
        ObjectWriterBufferBuilder {
            objects: RefCell::new(Vec::new()),
        }
    }
}

impl ObjectWriterBuilder for ObjectWriterBufferBuilder {
    fn new_object_writer(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        meta: Option<&ObjectMetadata>,
    ) -> Box<dyn ObjectWriter> {
        let obj = Rc::new(RefCell::new(ObjectWriterBuffer {
            complete: false,
            error: false,
            data: Vec::new(),
            meta: meta.map(|m| m.clone()),
        }));

        let obj_wrapper = Box::new(ObjectWriterBufferWrapper { inner: obj.clone() });
        self.objects.borrow_mut().push(obj);
        obj_wrapper
    }

    fn set_cache_duration(
        &self,
        _endpoint: &UDPEndpoint,
        _tsi: &u64,
        _toi: &u128,
        _content_location: &url::Url,
        _duration: &std::time::Duration,
    ) {
    }

    fn fdt_received(&self, _endpoint: &UDPEndpoint, _tsi: &u64, _fdt_xml: &str) {}
}

impl ObjectWriter for ObjectWriterBufferWrapper {
    fn open(&self) -> Result<()> {
        Ok(())
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        log::info!("Object complete !");
        inner.complete = true;
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        log::error!("Object received with error");
        inner.error = true;
    }
}
