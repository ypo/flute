use std::{cell::RefCell, rc::Rc};

pub trait ObjectWriter {
    fn create_session(&self, tsi: &u64, toi: &u128) -> Rc<dyn ObjectWriterSession>;
}

pub trait ObjectWriterSession {
    fn open(&self, transfer_length: usize);
    fn write(&self, data: &Vec<u8>);
    fn close(&self);
}

pub struct ObjectWriterBuffer {
    pub sessions: RefCell<Vec<Rc<ObjectWriterSessionBuffer>>>,
}
pub struct ObjectWriterSessionBuffer {
    pub data: RefCell<Vec<u8>>,
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
            data: RefCell::new(Vec::new()),
        });
        let mut sessions = self.sessions.borrow_mut();
        sessions.push(obj.clone());
        obj
    }
}

impl ObjectWriterSession for ObjectWriterSessionBuffer {
    fn open(&self, transfer_length: usize) {
        let mut self_data = self.data.borrow_mut();
        assert!(self_data.is_empty());
        self_data.reserve(transfer_length)
    }

    fn write(&self, data: &Vec<u8>) {
        let mut self_data = self.data.borrow_mut();
        self_data.extend(data)
    }

    fn close(&self) {}
}
