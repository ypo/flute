use std::{cell::RefCell, rc::Rc};

///
/// Used to write Objects received by a FLUTE receiver to a destination
///
pub trait FluteWriter {
    /// Return a new writer session for the object defined by its TSI and TOI
    fn create_session(&self, tsi: &u64, toi: &u128) -> Rc<dyn ObjectWriter>;
}

///
/// Write s single object to its final destination
///
pub trait ObjectWriter {
    /// Open the destination
    fn open(&self, content_location: Option<&str>);
    /// Write data
    fn write(&self, data: &[u8]);
    /// Called when all the data has been written
    fn complete(&self);
    /// Called when an error occured during the reception of this object
    fn error(&self);
}

///
/// Write objects received by the `receiver` to a buffer
///
#[derive(Debug)]
pub struct FluteWriterBuffer {
    /// List of all objects received
    pub objects: RefCell<Vec<Rc<ObjectWriterBuffer>>>,
}

///
/// Writer session to write a single object to a buffer
///
#[derive(Debug)]
pub struct ObjectWriterBuffer {
    inner: RefCell<ObjectWriterBufferInner>,
}

#[derive(Debug)]
struct ObjectWriterBufferInner {
    complete: bool,
    error: bool,
    data: Vec<u8>,
    content_location: Option<String>,
}

impl std::fmt::Debug for dyn FluteWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriter {{  }}")
    }
}

impl std::fmt::Debug for dyn ObjectWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriter {{  }}")
    }
}

impl FluteWriterBuffer {
    /// Return a new `ObjectWriterBuffer`
    pub fn new() -> Rc<FluteWriterBuffer> {
        Rc::new(FluteWriterBuffer {
            objects: RefCell::new(Vec::new()),
        })
    }
}

impl FluteWriter for FluteWriterBuffer {
    fn create_session(&self, _tsi: &u64, _toi: &u128) -> Rc<dyn ObjectWriter> {
        let obj = Rc::new(ObjectWriterBuffer {
            inner: RefCell::new(ObjectWriterBufferInner {
                complete: false,
                error: false,
                data: Vec::new(),
                content_location: None,
            }),
        });
        let mut sessions = self.objects.borrow_mut();
        sessions.push(obj.clone());
        obj
    }
}

impl ObjectWriterBuffer {
    /// Get a copy of the buffer with the content of the received object
    pub fn data(&self) -> Vec<u8> {
        let inner = self.inner.borrow();
        inner.data.clone()
    }

    /// Get the Content-Location of the received object
    pub fn content_location(&self) -> Option<String> {
        let inner = self.inner.borrow();
        inner.content_location.clone()
    }
}

impl ObjectWriter for ObjectWriterBuffer {
    fn open(&self, content_location: Option<&str>) {
        let mut inner = self.inner.borrow_mut();
        inner.content_location = content_location.map(|s| s.to_string());
    }

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data);
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.complete = true;
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.error = true;
    }
}
