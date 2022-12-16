use super::fdt::Fdt;
use super::pkt::PktWriter;
use super::sendersession::SenderSession;
use super::{objectdesc, oti};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;
use crate::tools::error::Result;

pub struct Sender {
    tsi: u64,
    fdt: Rc<RefCell<Fdt>>,
    sessions: Vec<SenderSession>,
    session_index: usize,
    writer: Rc<dyn PktWriter>,
}

impl Sender {
    pub fn new(tsi: u64, fdtid: u32, oti: &oti::Oti, writer: Rc<dyn PktWriter>) -> Sender {
        let fdt = Rc::new(RefCell::new(Fdt::new(fdtid, oti)));
        let sessions = (0..4)
            .map(|index| SenderSession::new(tsi, fdt.clone(), 4, index == 0))
            .collect();

        Sender {
            tsi,
            fdt,
            sessions,
            session_index: 0,
            writer,
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

    pub fn run(&mut self) -> bool {
        self.run_send_object()
    }

    pub fn run_send_object(&mut self) -> bool {
        let mut ret = false;
        let session_index_orig = self.session_index;
        loop {
            let session = self.sessions.get_mut(self.session_index).unwrap();
            let data = session.run();

            self.session_index += 1;
            if self.session_index == self.sessions.len() {
                self.session_index = 0;
            }

            if data.is_some() {
                let alc_pkt = data.as_ref().unwrap();
                match self.writer.write(alc_pkt) {
                    Ok(_) => {}
                    Err(e) => log::error!("Fail to write ALC pkt: {:?}", e),
                }
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

    use std::rc::Rc;
    use std::time::SystemTime;

    use crate::alc::pkt::DummyWriter;

    use super::objectdesc;
    use super::oti;

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_sender() {
        init();

        let writer = Rc::new(DummyWriter::new());
        let oti: oti::Oti = Default::default();
        // oti.fec = oti::FECEncodingID::ReedSolomonGF28;
        let mut sender = super::Sender::new(1, 1, &oti, writer);
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(vec![0xAA; oti.encoding_symbol_length as usize / 2]);
        // buffer.extend(vec![0xBB; oti.encoding_symbol_length as usize / 2]);
        // buffer.extend(vec![0xCC; oti.encoding_symbol_length as usize / 2]);
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
        sender.publish(&SystemTime::now());
        let mut nb = 0;
        loop {
            let success = sender.run();
            std::thread::sleep(std::time::Duration::from_secs(1));
            if success == false {
                nb += 1;
            }
            if nb > 2 {
                break;
            }
        }
    }
}
