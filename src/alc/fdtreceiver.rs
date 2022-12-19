use std::{cell::RefCell, rc::Rc};

use super::{lct, objectreceiver::ObjectReceiver, objectwriter::ObjectWriterSession};

pub struct FdtReceiver {
    fdt_id: u32,
    obj: Box<ObjectReceiver>,
    receiver_session: Rc<FdtReceiverSession>,
}

struct FdtReceiverSession {
    data: RefCell<Vec<u8>>,
}

impl FdtReceiver {
    pub fn new(fdt_id: u32) -> FdtReceiver {
        let receiver_session = Rc::new(FdtReceiverSession {
            data: RefCell::new(Vec::new()),
        });

        FdtReceiver {
            fdt_id,
            obj: Box::new(ObjectReceiver::new(&lct::TOI_FDT, receiver_session.clone())),
            receiver_session,
        }
    }

    pub fn push(&mut self) {}
}

impl ObjectWriterSession for FdtReceiverSession {
    fn open(&self, transfer_length: usize) {
        todo!()
    }

    fn write(&self, data: &Vec<u8>) {
        todo!()
    }

    fn close(&self) {
        todo!()
    }
}
