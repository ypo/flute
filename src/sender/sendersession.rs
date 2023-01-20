use super::blockencoder::BlockEncoder;
use super::fdt::Fdt;
use super::filedesc::FileDesc;
use crate::common::alc;
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug)]
pub struct SenderSession {
    tsi: u64,
    file: Option<Arc<FileDesc>>,
    encoder: Option<BlockEncoder>,
    interleave_blocks: usize,
    transfer_fdt_only: bool,
}

impl SenderSession {
    pub fn new(tsi: u64, interleave_blocks: usize, transfer_fdt_only: bool) -> SenderSession {
        SenderSession {
            tsi,
            file: None,
            encoder: None,
            interleave_blocks,
            transfer_fdt_only,
        }
    }

    pub fn run(&mut self, fdt: &mut Fdt, now: SystemTime) -> Option<Vec<u8>> {
        loop {
            if self.encoder.is_none() {
                self.get_next(fdt, now);
            }

            if self.encoder.is_none() {
                return None;
            }

            assert!(self.file.is_some());
            let encoder = self.encoder.as_mut().unwrap();
            let file = self.file.as_ref().unwrap();
            let pkt = encoder.read();
            if pkt.is_none() {
                self.release_file(fdt, now);
                continue;
            }
            let pkt = pkt.as_ref().unwrap();
            return Some(alc::new_alc_pkt(&file.oti, &0u128, self.tsi, pkt));
        }
    }

    fn get_next(&mut self, fdt: &mut Fdt, now: SystemTime) {
        self.encoder = None;
        if self.transfer_fdt_only {
            self.file = fdt.get_next_fdt_transfer(now);
        } else {
            self.file = fdt.get_next_file_transfer(now);
        }
        if self.file.is_none() {
            return;
        }
        self.encoder = Some(BlockEncoder::new(
            self.file.as_ref().unwrap().clone(),
            self.interleave_blocks,
        ));
    }

    fn release_file(&mut self, fdt: &mut Fdt, now: SystemTime) {
        match &self.file {
            Some(file) => fdt.transfer_done(file.clone(), now),
            _ => {}
        };
        self.file = None;
        self.encoder = None;
    }
}
