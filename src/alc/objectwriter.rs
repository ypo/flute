use std::{cell::RefCell, rc::Rc};

///
/// Used to write Objects received by a FLUTE receiver to a destination
/// 
pub trait ObjectWriter {
    /// Return a new writer session for the object defined by its TSI and TOI
    fn create_session(&self, tsi: &u64, toi: &u128) -> Rc<dyn ObjectWriterSession>;
}

///
/// Write s single object to its final destination
/// 
pub trait ObjectWriterSession {
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
pub struct ObjectWriterBuffer {
    /// List of all objects received
    pub sessions: RefCell<Vec<Rc<ObjectWriterSessionBuffer>>>,
}

///
/// Writer session to write a single object to a buffer
/// 
#[derive(Debug)]
pub struct ObjectWriterSessionBuffer {
    inner: RefCell<ObjectWriterSessionBufferInner>,
}

#[derive(Debug)]
struct ObjectWriterSessionBufferInner {
    complete: bool,
    error: bool,
    data: Vec<u8>,
    content_location: Option<String>,
}

impl std::fmt::Debug for dyn ObjectWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriter {{  }}")
    }
}

impl std::fmt::Debug for dyn ObjectWriterSession {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ObjectWriterSession {{  }}")
    }
}


impl ObjectWriterBuffer {
    /// Return a new `ObjectWriterBuffer`
    pub fn new() -> Rc<ObjectWriterBuffer> {
        Rc::new(ObjectWriterBuffer {
            sessions: RefCell::new(Vec::new()),
        })
    }
}

impl ObjectWriter for ObjectWriterBuffer {
    fn create_session(&self, _tsi: &u64, _toi: &u128) -> Rc<dyn ObjectWriterSession> {
        let obj = Rc::new(ObjectWriterSessionBuffer {
            inner: RefCell::new(ObjectWriterSessionBufferInner {
                complete: false,
                error: false,
                data: Vec::new(),
                content_location: None,
            }),
        });
        let mut sessions = self.sessions.borrow_mut();
        sessions.push(obj.clone());
        obj
    }
}

impl ObjectWriterSessionBuffer {
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

impl ObjectWriterSession for ObjectWriterSessionBuffer {
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
