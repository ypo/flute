use std::{cell::RefCell, rc::Rc};

pub trait ObjectWriter {
    fn create_session(&self, tsi: &u64, toi: &u128) -> Rc<dyn ObjectWriterSession>;
}

pub trait ObjectWriterSession {
    fn open(&self, content_location: Option<&str>);
    fn write(&self, data: &[u8]);
    fn complete(&self);
    fn error(&self);
}

pub struct ObjectWriterBuffer {
    pub sessions: RefCell<Vec<Rc<ObjectWriterSessionBuffer>>>,
}

pub struct ObjectWriterSessionBuffer {
    inner: RefCell<ObjectWriterSessionBufferInner>,
}

struct ObjectWriterSessionBufferInner {
    pub complete: bool,
    pub error: bool,
    pub data: Vec<u8>,
}

impl ObjectWriterBuffer {
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
            }),
        });
        let mut sessions = self.sessions.borrow_mut();
        sessions.push(obj.clone());
        obj
    }
}

impl ObjectWriterSession for ObjectWriterSessionBuffer {
    fn open(&self, content_location: Option<&str>) {
        log::info!("Open {:?}", content_location);
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
