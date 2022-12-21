use super::fdt::Fdt;
use super::sendersession::SenderSession;
use super::{objectdesc, oti};
use crate::tools::error::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

pub struct Sender {
    fdt: Rc<RefCell<Fdt>>,
    sessions: Vec<SenderSession>,
    session_index: usize,
}

impl Sender {
    pub fn new(tsi: u64, fdtid: u32, oti: &oti::Oti) -> Sender {
        let fdt = Rc::new(RefCell::new(Fdt::new(fdtid, oti)));
        let sessions = (0..4)
            .map(|index| SenderSession::new(tsi, fdt.clone(), 4, index == 0))
            .collect();

        Sender {
            fdt,
            sessions,
            session_index: 0,
        }
    }

    pub fn add_object(&self, obj: Box<objectdesc::ObjectDesc>) {
        let mut fdt = self.fdt.borrow_mut();
        fdt.add_object(obj);
    }

    pub fn publish(&self, now: &SystemTime) -> Result<()> {
        let mut fdt = self.fdt.borrow_mut();
        fdt.publish(now)
    }

    pub fn set_complete(&self) {
        let mut fdt = self.fdt.borrow_mut();
        fdt.set_complete();
    }

    pub fn run(&mut self) -> Option<Vec<u8>> {
        self.run_send_object()
    }

    fn run_send_object(&mut self) -> Option<Vec<u8>> {
        let session_index_orig = self.session_index;
        loop {
            let session = self.sessions.get_mut(self.session_index).unwrap();
            let data = session.run();

            self.session_index += 1;
            if self.session_index == self.sessions.len() {
                self.session_index = 0;
            }

            if data.is_some() {
                return data;
            }

            if self.session_index == session_index_orig {
                break;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {

    use super::objectdesc;
    use super::oti;
    use std::time::SystemTime;

    #[test]
    pub fn test_sender() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        // oti.fec = oti::FECEncodingID::ReedSolomonGF28;
        let mut sender = super::Sender::new(1, 1, &oti);
        let mut buffer: Vec<u8> = Vec::new();
        let nb_pkt = oti.encoding_symbol_length as usize * 3;
        buffer.extend(vec![0xAA; nb_pkt]);
        sender.add_object(
            objectdesc::ObjectDesc::create_from_buffer(
                &buffer,
                "text",
                &url::Url::parse("file:///hello").unwrap(),
                1,
                None,
            )
            .unwrap(),
        );
        sender.publish(&SystemTime::now()).unwrap();
        loop {
            let data = sender.run();
            if data.is_none() {
                break;
            }
        }
    }
}
