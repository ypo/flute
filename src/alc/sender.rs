use super::fdt::Fdt;
use super::sendersession::SenderSession;
use super::{lct, objectdesc, oti};
use crate::tools::error::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::SystemTime;

///
/// Configuration of the `Sender`
///
#[derive(Debug)]
pub struct Config {
    /// Max duration of the FDT before expiration.
    pub fdt_duration: std::time::Duration,
    /// First FDT ID.
    pub fdt_start_id: u32,
    /// Content Encoding of the FDT.
    pub fdt_cenc: lct::Cenc,
    /// Insert Sender Current Time inside ALC/LCT packets containing the FDT.
    pub fdt_inband_sct: bool,
    /// Max number of files that are multiplexed during the transmission  
    /// 0..1 : files are transmitted one after the other.  
    /// 2.. : multiple files might be transmitted in parallel.   
    ///
    pub multiplex_files: u8,
    /// Max number of blocks that are multiplexed during the transmission of a file.  
    /// Increasing the block multiplexing with error recovery can improve resilience to burst lost packets, 
    /// as the erasure symbols will be distributed among multiple blocks.
    pub multiplex_blocks: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fdt_duration: std::time::Duration::from_secs(3600),
            fdt_start_id: 1,
            fdt_cenc: lct::Cenc::Null,
            fdt_inband_sct: true,
            multiplex_files: 3,
            multiplex_blocks: 4
        }
    }
}

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
    pub fn new(tsi: u64, oti: &oti::Oti, config: &Config) -> Sender {
        let fdt = Rc::new(RefCell::new(Fdt::new(
            config.fdt_start_id,
            oti,
            config.fdt_cenc,
            config.fdt_duration,
            config.fdt_inband_sct,
        )));

        let multiplex_files = match config.multiplex_files {
            0 => 2,
            n => n + 1,
        };

        let sessions = (0..multiplex_files)
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
    pub fn publish(&self, now: SystemTime) -> Result<()> {
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
    pub fn read(&mut self, now: SystemTime) -> Option<Vec<u8>> {
        let session_index_orig = self.session_index;
        loop {
            let session = self.sessions.get_mut(self.session_index).unwrap();
            let data = session.run(now);

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

    #[test]
    pub fn test_sender() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let mut sender = super::Sender::new(1, &oti, &Default::default());
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
                lct::Cenc::Null,
                true,
                None,
                true,
            )
            .unwrap(),
        );
        sender.publish(std::time::SystemTime::now()).unwrap();
        loop {
            let data = sender.read(std::time::SystemTime::now());
            if data.is_none() {
                break;
            }
        }
    }
}
