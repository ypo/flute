use super::fdt::Fdt;
use super::sendersession::SenderSession;
use super::{lct, objectdesc, oti};
use crate::tools::error::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

///
/// FLUTE `Sender` session
/// Transform objects (files) to ALC/LCT packet
///
#[derive(Debug)]
pub struct Sender {
    fdt: Rc<RefCell<Fdt>>,
    sessions: Vec<SenderSession>,
    session_index: usize,
}

impl Sender {
    ///
    /// Creation of a FLUTE Sender
    ///
    pub fn new(tsi: u64, fdtid: u32, oti: &oti::Oti, fdt_cenc: lct::CENC) -> Sender {
        let fdt = Rc::new(RefCell::new(Fdt::new(fdtid, oti, fdt_cenc)));
        let sessions = (0..4)
            .map(|index| SenderSession::new(tsi, fdt.clone(), 4, index == 0))
            .collect();

        Sender {
            fdt,
            sessions,
            session_index: 0,
        }
    }

    /// Add an object to the FDT
    /// After calling this function, a call to `publish()` to publish your modifications
    pub fn add_object(&self, obj: Box<objectdesc::ObjectDesc>) {
        let mut fdt = self.fdt.borrow_mut();
        fdt.add_object(obj);
    }

    /// Publish modification to the FDT
    /// An updated version of the FDT will be generated and transferred
    /// Multiple modification can be made (ex: several call to 'add_object()`) before publishing a new FDT version
    pub fn publish(&self, now: &SystemTime) -> Result<()> {
        let mut fdt = self.fdt.borrow_mut();
        fdt.publish(now)
    }

    /// Inform that the FDT is complete, no new object should be added after this call
    /// You must not call `add_object()`after
    /// After calling this function, a call to `publish()` to publish your modifications
    pub fn set_complete(&self) {
        let mut fdt = self.fdt.borrow_mut();
        fdt.set_complete();
    }

    /// Read the next ALC/LCT packet
    /// return None if there is no new packet to be transferred
    /// ALC/LCT packet should be encapsulated into a UDP/IP payload and transferred via UDP/multicast
    pub fn read(&mut self) -> Option<Vec<u8>> {
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

    use crate::alc::lct;

    use super::objectdesc;
    use super::oti;
    use std::time::SystemTime;

    #[test]
    pub fn test_sender() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let mut sender = super::Sender::new(1, 1, &oti, lct::CENC::Null);
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
                lct::CENC::Null,
                true,
                None,
                true,
            )
            .unwrap(),
        );
        sender.publish(&SystemTime::now()).unwrap();
        loop {
            let data = sender.read();
            if data.is_none() {
                break;
            }
        }
    }
}
