use std::cell::RefCell;
use std::rc::Rc;

use super::alc;
use super::blockencoder::BlockEncoder;
use super::fdt::Fdt;
use super::filedesc::FileDesc;

#[derive(Debug)]
pub struct SenderSession {
    tsi: u64,
    fdt: Rc<RefCell<Fdt>>,
    file: Option<Rc<FileDesc>>,
    encoder: Option<BlockEncoder>,
    block_interlace_windows: usize,
    transfer_fdt_only: bool,
}

impl SenderSession {
    pub fn new(
        tsi: u64,
        fdt: Rc<RefCell<Fdt>>,
        block_interlace_windows: usize,
        transfer_fdt_only: bool,
    ) -> SenderSession {
        SenderSession {
            tsi,
            fdt,
            file: None,
            encoder: None,
            block_interlace_windows,
            transfer_fdt_only,
        }
    }

    pub fn run(&mut self) -> Option<Vec<u8>> {
        loop {
            if self.encoder.is_none() {
                self.get_next();
            }

            if self.encoder.is_none() {
                return None;
            }

            assert!(self.file.is_some());
            let encoder = self.encoder.as_mut().unwrap();
            let file = self.file.as_ref().unwrap();
            let pkt = encoder.read();
            if pkt.is_none() {
                self.release_file();
                continue;
            }
            let pkt = pkt.as_ref().unwrap();
            log::info!(
                "Create ALC Pkt toi={} snb={} esi={}",
                pkt.toi,
                pkt.snb,
                pkt.esi
            );
            return Some(alc::create_alc_pkt(&file.oti, &0u128, self.tsi, pkt, None));
        }
    }

    fn get_next(&mut self) {
        let mut fdt = self.fdt.borrow_mut();
        self.encoder = None;
        if self.transfer_fdt_only {
            self.file = fdt.get_next_fdt_transfer();
        } else {
            self.file = fdt.get_next_file_transfer();
        }
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
            Some(file) => fdt.transfer_done(file.clone()),
            _ => {}
        };
        self.file = None;
        self.encoder = None;
    }
}
