use super::blockencoder::BlockEncoder;
use super::fdt::Fdt;
use super::filedesc::FileDesc;
#[cfg(feature = "opentelemetry")]
use super::objectsenderlogger::ObjectSenderLogger;
use super::Profile;
use crate::common::alc;
use crate::core::UDPEndpoint;
use std::sync::Arc;
use std::time::SystemTime;

#[allow(dead_code)]
#[derive(Debug)]
pub struct SenderSession {
    priority: u32,
    endpoint: UDPEndpoint,
    tsi: u64,
    file: Option<Arc<FileDesc>>,
    encoder: Option<BlockEncoder>,
    interleave_blocks: usize,
    transfer_fdt_only: bool,
    profile: Profile,
    #[cfg(feature = "opentelemetry")]
    logger: Option<ObjectSenderLogger>,
}

impl SenderSession {
    pub fn new(
        priority: u32,
        tsi: u64,
        interleave_blocks: usize,
        transfer_fdt_only: bool,
        profile: Profile,
        endpoint: UDPEndpoint,
    ) -> SenderSession {
        SenderSession {
            priority,
            endpoint,
            tsi,
            file: None,
            encoder: None,
            interleave_blocks,
            transfer_fdt_only,
            profile,
            #[cfg(feature = "opentelemetry")]
            logger: None,
        }
    }

    pub fn run(&mut self, fdt: &mut Fdt, now: SystemTime) -> Option<Vec<u8>> {
        loop {
            if self.encoder.is_none() {
                self.get_next(fdt, now);
            }

            if !self.transfer_fdt_only {
                // Stop emitting packets if a new FDT is needed
                if fdt.need_transfer_fdt() {
                    return None;
                }
            }

            let encoder = self.encoder.as_mut()?;

            debug_assert!(self.file.is_some());
            let file = self.file.as_ref().unwrap();
            let must_stop_transfer = !self.transfer_fdt_only
                && file.can_transfer_be_stopped()
                && !fdt.is_added(file.toi);

            if must_stop_transfer {
                log::debug!("File has already been transferred and is removed from the FDT, stop the transfer {}", file.object.content_location.to_string());
            }

            if let Some(next_timestamp) = file.get_next_transfer_timestamp() {
                if next_timestamp > now {
                    return None;
                }
            }

            let pkt = encoder.read(must_stop_transfer);
            if pkt.is_none() {
                self.release_file(fdt, now);
                continue;
            }

            file.inc_next_transfer_timestamp();
            let pkt = pkt.as_ref().unwrap();
            return Some(alc::new_alc_pkt(
                &file.oti,
                &0u128,
                self.tsi,
                pkt,
                self.profile,
                now,
            ));
        }
    }

    fn get_next(&mut self, fdt: &mut Fdt, now: SystemTime) {
        self.encoder = None;
        if self.transfer_fdt_only {
            self.file = fdt.get_next_fdt_transfer(now);
        } else {
            self.file = fdt.get_next_file_transfer(self.priority, now);
        }
        if self.file.is_none() {
            return;
        }

        #[cfg(feature = "opentelemetry")]
        if !self.transfer_fdt_only {
            let file = self.file.as_ref().unwrap();
            if file.total_nb_transfer() == 0 {
                self.logger = Some(ObjectSenderLogger::new(
                    &self.endpoint,
                    self.tsi,
                    file.toi,
                    file.object.optel_propagator.as_ref(),
                ));
            }
        }

        let file = self.file.as_ref().unwrap().clone();
        let is_last_transfer = file.is_last_transfer();
        let block_encoder = BlockEncoder::new(file, self.interleave_blocks, is_last_transfer);
        if block_encoder.is_err() {
            log::error!("Fail to open Block Encoder");
            self.release_file(fdt, now);
            return;
        }

        self.encoder = block_encoder.ok();
    }

    fn release_file(&mut self, fdt: &mut Fdt, now: SystemTime) {
        if let Some(file) = &self.file {
            fdt.transfer_done(file.clone(), now)
        };

        self.file = None;
        self.encoder = None;

        #[cfg(feature = "opentelemetry")]
        {
            self.logger = None;
        }
    }
}
