use std::cell::RefCell;
use std::rc::Rc;

use super::blockencoder::BlockEncoder;
use super::fdt::Fdt;
use super::filedesc::FileDesc;

pub struct SenderSession {
    fdt: Rc<RefCell<Fdt>>,
    file: Option<Rc<FileDesc>>,
    encoder: Option<BlockEncoder>,
    block_interlace: u32,
}

impl SenderSession {
    pub fn new(fdt: Rc<RefCell<Fdt>>, block_interlace: u32) -> SenderSession {
        SenderSession {
            fdt,
            file: None,
            encoder: None,
            block_interlace
        }
    }

    pub fn run(&mut self) -> Option<Vec<u8>> {
        if self.file.is_none() {
            self.get_next();
        }

        if self.file.is_none() {
            log::info!("No more file");
            return None;
        }

        None
    }

    pub fn get_next(&mut self) {
        let mut fdt = self.fdt.borrow_mut();
        self.encoder = None;
        self.file = fdt.get_next_file();
        if self.file.is_none() {
            return;
        }
        self.encoder = Some(BlockEncoder::new(self.file.as_ref().unwrap().clone()));
    }
}
