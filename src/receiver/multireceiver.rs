use super::receiver::{Config, Receiver};
use super::writer::ObjectWriterBuilder;
use crate::common::alc;
use crate::tools::error::Result;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

///
/// Multi-sessions FLUTE receiver
/// Demultiplex multiple FLUTE Transport Sessions
///
#[derive(Debug)]
pub struct MultiReceiver {
    alc_receiver: HashMap<u64, Box<Receiver>>,
    tsi: Option<Vec<u64>>,
    writer: Rc<dyn ObjectWriterBuilder>,
    config: Option<Config>,
}

impl MultiReceiver {
    ///
    /// Creates a new `MultiReceiver` instance, which allows receiving multiple interlaced FLUTE sessions.
    ///
    /// # Arguments
    /// * `tsi` - Optional List of Transport Session Identifier (TSI) accepted by the receiver.
    /// if `None`, all Transport Session are accepted
    ///
    /// * `writer` - Responsible to write object to its final destination.
    ///
    /// * `config` - Configuration of the FLUTE `Receiver`. if `None`, default `Config` will be used
    ///
    /// # Example
    /// ```
    /// // Receive objects from Transport Session 1
    /// use flute::receiver::writer::ObjectWriterBufferBuilder;
    /// use flute::receiver::MultiReceiver;
    /// use std::rc::Rc;
    ///
    /// let tsi: u64 = 1;
    /// // Write object to a buffer
    /// let writer = Rc::new(ObjectWriterBufferBuilder::new());
    /// let receiver = MultiReceiver::new(Some(&vec![1]), writer.clone(), None);
    /// ```
    pub fn new(
        tsi: Option<&[u64]>,
        writer: Rc<dyn ObjectWriterBuilder>,
        config: Option<Config>,
    ) -> MultiReceiver {
        MultiReceiver {
            alc_receiver: HashMap::new(),
            tsi: tsi.map(|f| f.to_vec()),
            writer,
            config,
        }
    }

    ///
    /// Push a ALC/LCT packet to the receiver.
    /// Returns as error the the packet is not a valid ALC/LCT format
    ///
    /// # Arguments
    /// * `pkt`- Payload of the UDP/IP packet.
    ///
    pub fn push(&mut self, pkt: &[u8], now: std::time::SystemTime) -> Result<()> {
        let alc = alc::parse_alc_pkt(pkt)?;

        let can_handle = match &self.tsi {
            Some(tsi) => tsi.contains(&alc.lct.tsi),
            None => true,
        };

        if !can_handle {
            log::debug!("skip pkt with tsi {}", alc.lct.tsi);
            return Ok(());
        }

        if alc.lct.close_session {
            match self.get_receiver(alc.lct.tsi) {
                Some(receiver) => receiver.push(&alc, now),
                None => {
                    log::warn!(
                        "A session that is not allocated is about to be closed, skip the session"
                    );
                    return Ok(());
                }
            }
        } else {
            let receiver = self.get_receiver_or_create(alc.lct.tsi);
            receiver.push(&alc, now)
        }
    }

    ///
    /// Remove FLUTE session that are closed or expired
    /// Remove Objects that are expired
    ///
    /// Cleanup should be call from time to time to avoid consuming to much memory
    ///
    pub fn cleanup(&mut self, now: SystemTime) {
        self.alc_receiver.retain(|_, v| !v.is_expired());
        for (_, receiver) in &mut self.alc_receiver {
            receiver.cleanup(now);
        }
    }

    fn get_receiver(&mut self, tsi: u64) -> Option<&mut Receiver> {
        self.alc_receiver
            .get_mut(&tsi)
            .map(|receiver| receiver.as_mut())
    }

    fn get_receiver_or_create(&mut self, tsi: u64) -> &mut Receiver {
        self.alc_receiver
            .entry(tsi)
            .or_insert_with(|| {
                Box::new(Receiver::new(tsi, self.writer.clone(), self.config.clone()))
            })
            .as_mut()
    }
}
