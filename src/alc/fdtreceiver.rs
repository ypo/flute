use super::{
    alc,
    fdtinstance::FdtInstance,
    lct,
    objectreceiver::{self, ObjectReceiver},
    objectwriter::ObjectWriterSession,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Receiving,
    Complete,
    Error,
}

#[derive(Debug)]
pub struct FdtReceiver {
    pub fdt_id: u32,
    obj: Box<ObjectReceiver>,
    writer: Rc<FdtWriter>,
    fdt_instance: Option<FdtInstance>,
}

#[derive(Debug)]
struct FdtWriter {
    inner: RefCell<FdtWriterInner>,
}

#[derive(Debug)]
struct FdtWriterInner {
    data: Vec<u8>,
    fdt: Option<FdtInstance>,
    state: State,
}

impl FdtReceiver {
    pub fn new(fdt_id: u32) -> FdtReceiver {
        let writer = Rc::new(FdtWriter {
            inner: RefCell::new(FdtWriterInner {
                data: Vec::new(),
                fdt: None,
                state: State::Receiving,
            }),
        });

        FdtReceiver {
            fdt_id,
            obj: Box::new(ObjectReceiver::new(&lct::TOI_FDT, writer.clone())),
            writer,
            fdt_instance: None,
        }
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt) {
        self.obj.push(pkt);
        if self.obj.state == objectreceiver::State::Error {
            self.writer.inner.borrow_mut().state = State::Error;
        }
    }

    pub fn state(&self) -> State {
        self.writer.inner.borrow().state
    }

    pub fn fdt_instance(&mut self) -> Option<&FdtInstance> {
        if self.fdt_instance.is_none() {
            let inner = self.writer.inner.borrow();
            let instance = inner.fdt.as_ref();
            self.fdt_instance = instance.map(|f| f.clone())
        }
        self.fdt_instance.as_ref()
    }
}

impl ObjectWriterSession for FdtWriter {
    fn open(&self, _content_location: Option<&str>) {}

    fn write(&self, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        inner.data.extend(data)
    }

    fn complete(&self) {
        let mut inner = self.inner.borrow_mut();
        match FdtInstance::parse(&inner.data) {
            Ok(inst) => {
                inner.fdt = Some(inst);
                inner.state = State::Complete
            }
            Err(_) => inner.state = State::Error,
        };
    }

    fn error(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.state = State::Error;
    }
}
