use super::fdt::Fdt;
use super::sendersession::SenderSession;
use super::{objectdesc, oti};
use std::cell::RefCell;
use std::rc::Rc;

struct Sender {
    fdt: Rc<RefCell<Fdt>>,
    sessions: Vec<SenderSession>,
    session_index: usize,
}

impl Sender {
    pub fn new(fdtid: u32, oti: &oti::Oti) -> Sender {
        let fdt = Rc::new(RefCell::new(Fdt::new(fdtid, oti)));
        let sessions = (0..4).map(|_| SenderSession::new(fdt.clone(), 4)).collect();

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

    pub fn publish(&self) {
        let mut fdt = self.fdt.borrow_mut();
        fdt.publish();
    }

    pub fn run(&mut self) -> bool {
        self.run_send_object()
    }

    pub fn run_send_object(&mut self) -> bool {
        log::debug!("Send object");
        let mut ret = false;
        let session_index_orig = self.session_index;
        loop {
            log::debug!("Get session {}", self.session_index);
            let session = self.sessions.get_mut(self.session_index).unwrap();
            let data = session.run();

            self.session_index += 1;
            if self.session_index == self.sessions.len() {
                self.session_index = 0;
            }

            if data.is_some() {
                let pkt = data.as_ref().unwrap();
                log::info!("Send data {:?}", pkt);
                ret = true;
                break;
            }

            if self.session_index == session_index_orig {
                break;
            }
        }
        ret
    }
}

#[cfg(test)]
mod tests {

    use super::objectdesc;
    use super::oti;

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_sender() {
        init();

        let mut oti: oti::Oti = Default::default();
        oti.fec = oti::FECEncodingID::ReedSolomonGF28;
        let mut sender = super::Sender::new(1, &oti);
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(vec![0xAA; oti.encoding_symbol_length as usize / 2]);
        buffer.extend(vec![0xBB; oti.encoding_symbol_length as usize / 2]);
        buffer.extend(vec![0xCC; oti.encoding_symbol_length as usize / 2]);
        sender.add_object(
            objectdesc::ObjectDesc::create_from_buffer(
                &buffer,
                "text",
                &url::Url::parse("file:///hello").unwrap(),
            )
            .unwrap(),
        );
        sender.publish();
        loop {
            let success = sender.run();
            if success == false {
                break;
            }
        }
    }
}
