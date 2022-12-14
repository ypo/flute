use std::cell::RefCell;
use std::rc::Rc;

use super::blockencoder::BlockEncoder;
use super::fdt::Fdt;
use super::filedesc::FileDesc;
use super::pkt;

pub struct SenderSession {
    fdt: Rc<RefCell<Fdt>>,
    file: Option<Rc<FileDesc>>,
    encoder: Option<BlockEncoder>,
    block_interlace_windows: usize,
}

impl SenderSession {
    pub fn new(fdt: Rc<RefCell<Fdt>>, block_interlace_windows: usize) -> SenderSession {
        SenderSession {
            fdt,
            file: None,
            encoder: None,
            block_interlace_windows,
        }
    }

    pub fn run(&mut self) -> Option<pkt::Pkt> {
        loop {
            if self.encoder.is_none() {
                self.get_next();
            }

            if self.encoder.is_none() {
                log::info!("No more file");
                return None;
            }

            let encoder = self.encoder.as_mut().unwrap();
            log::debug!("Read pkt");
            let pkt = encoder.read();
            if pkt.is_none() {
                self.release_file();
                continue;
            }

            return Some(pkt.unwrap());
        }
    }

    fn get_next(&mut self) {
        let mut fdt = self.fdt.borrow_mut();
        self.encoder = None;
        self.file = fdt.get_next_file();
        if self.file.is_none() {
            return;
        }
        self.encoder = Some(BlockEncoder::new(
            self.file.as_ref().unwrap().clone(),
            self.block_interlace_windows,
        ));
    }

    fn release_file(&mut self) {
        let mut fdt = self.fdt.borrow_mut();
        match &self.file {
            Some(file) => fdt.release_next_file(file.clone()),
            _ => {}
        };
        self.file = None;
        self.encoder = None;
    }
}
