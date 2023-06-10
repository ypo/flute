use super::receiver::{Config, Receiver};
use super::tsifilter::TSIFilter;
use super::writer::ObjectWriterBuilder;
use super::UDPEndpoint;
use crate::common::alc;
use crate::tools::error::Result;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

/// Receiver endpoint
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct ReceiverEndpoint {
    pub endpoint: UDPEndpoint,
    pub tsi: u64,
}

///
/// Multi-sessions FLUTE receiver
/// Demultiplex multiple FLUTE Transport Sessions
///
#[derive(Debug)]
pub struct MultiReceiver {
    alc_receiver: HashMap<ReceiverEndpoint, Box<Receiver>>,
    tsifilter: TSIFilter,
    writer: Rc<dyn ObjectWriterBuilder>,
    config: Option<Config>,
    enable_tsi_filtering: bool,
}

impl MultiReceiver {
    ///
    /// Creates a new `MultiReceiver` instance, which allows receiving multiple interlaced FLUTE sessions.
    ///
    /// # Arguments
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
    /// use flute::receiver::{MultiReceiver, UDPEndpoint};
    /// use std::rc::Rc;
    ///
    /// let tsi: u64 = 1;
    /// // Write object to a buffer
    /// let writer = Rc::new(ObjectWriterBufferBuilder::new());
    /// let mut receiver = MultiReceiver::new(writer.clone(), None, true);
    /// let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 3000);
    /// receiver.add_listen_tsi(endpoint, tsi)
    /// ```
    pub fn new(
        writer: Rc<dyn ObjectWriterBuilder>,
        config: Option<Config>,
        enable_tsi_filtering: bool,
    ) -> MultiReceiver {
        MultiReceiver {
            alc_receiver: HashMap::new(),
            writer,
            config,
            tsifilter: TSIFilter::new(),
            enable_tsi_filtering,
        }
    }

    ///
    /// Number of objects that are we are receiving
    ///
    pub fn nb_objects(&self) -> usize {
        self.alc_receiver
            .iter()
            .map(|session| session.1.nb_objects())
            .sum()
    }

    ///
    /// Number objects in error state
    ///
    pub fn nb_objects_error(&self) -> usize {
        self.alc_receiver
            .iter()
            .map(|session| session.1.nb_objects_error())
            .sum()
    }

    ///
    /// Enable/Disable  TSI filtering
    ///
    pub fn set_tsi_filtering(&mut self, enable: bool) {
        self.enable_tsi_filtering = enable;
    }

    ///
    /// Accept a TSI session for a given endpoint and TSI
    ///
    /// # Arguments
    /// * `maddendpointr` - Add the TSI filter for this endpoint.
    ///
    /// * `param` - tsi The TSI value to filter.
    ///
    pub fn add_listen_tsi(&mut self, endpoint: UDPEndpoint, tsi: u64) {
        if self.enable_tsi_filtering == false {
            log::warn!("TSI filtering is disabled");
        }

        log::info!("Listen TSI {} for {:?}", tsi, endpoint);
        self.tsifilter.add(endpoint, tsi);
    }

    ///
    /// Removes a TSI filter for a given endpoint and TSI
    ///
    /// # Arguments
    /// * `endpoint` - remove the TSI filter for this endpoint.
    ///
    /// * `param` - The TSI value to remove the filter for.
    ///
    pub fn remove_listen_tsi(&mut self, endpoint: &UDPEndpoint, tsi: u64) {
        self.tsifilter.remove(endpoint, tsi);
    }

    /// Accepts all TSI sessions for a given endpoint   
    pub fn add_listen_all_tsi(&mut self, endpoint: UDPEndpoint) {
        log::info!("Listen all TSI for {:?}", endpoint);
        if self.enable_tsi_filtering == false {
            log::warn!("TSI filtering is disabled");
        }

        self.tsifilter.add_endpoint_bypass(endpoint);
    }

    /// Remove the acceptance of all TSI sessions for a given endpoint   
    pub fn remove_listen_all_tsi(&mut self, endpoint: &UDPEndpoint) {
        self.tsifilter.remove_endpoint_bypass(endpoint);
    }

    ///
    /// Push a ALC/LCT packet to the receiver.
    /// Returns as error the the packet is not a valid ALC/LCT format
    ///
    /// # Arguments
    /// * `pkt`- Payload of the UDP/IP packet.
    ///
    pub fn push(
        &mut self,
        endpoint: &UDPEndpoint,
        pkt: &[u8],
        now: std::time::SystemTime,
    ) -> Result<()> {
        let alc = alc::parse_alc_pkt(pkt)?;

        if self.enable_tsi_filtering {
            let can_handle = self.tsifilter.is_valid(endpoint, alc.lct.tsi);

            if !can_handle {
                log::debug!(
                    "skip pkt with tsi {} and endpoint {:?}",
                    alc.lct.tsi,
                    endpoint
                );
                return Ok(());
            }
        }

        let key = ReceiverEndpoint {
            endpoint: endpoint.clone(),
            tsi: alc.lct.tsi,
        };

        if alc.lct.close_session {
            match self.get_receiver(&key) {
                Some(receiver) => receiver.push(&alc, now),
                None => {
                    log::warn!(
                        "A session that is not allocated is about to be closed, skip the session"
                    );
                    return Ok(());
                }
            }
        } else {
            let receiver = self.get_receiver_or_create(&key);
            receiver.push(&alc, now)
        }
    }

    ///
    /// Remove FLUTE session that are closed or expired
    /// Remove Objects that are expired
    ///
    /// Cleanup should be call from time to time to avoid consuming to much memory
    ///
    /// Return List of receiver endpoint that has been removed
    pub fn cleanup(&mut self, now: SystemTime) -> Vec<ReceiverEndpoint> {
        let mut output = Vec::new();
        for receiver in &self.alc_receiver {
            if receiver.1.is_expired() {
                output.push(receiver.0.clone());
            }
        }

        self.alc_receiver.retain(|_, v| !v.is_expired());
        for (_, receiver) in &mut self.alc_receiver {
            receiver.cleanup(now);
        }

        output
    }

    fn get_receiver(&mut self, key: &ReceiverEndpoint) -> Option<&mut Receiver> {
        self.alc_receiver
            .get_mut(key)
            .map(|receiver| receiver.as_mut())
    }

    fn get_receiver_or_create(&mut self, key: &ReceiverEndpoint) -> &mut Receiver {
        self.alc_receiver
            .entry(key.clone())
            .or_insert_with(|| {
                log::info!("Create FLUTE Receiver {:?}", key);
                Box::new(Receiver::new(
                    &key.endpoint,
                    key.tsi,
                    self.writer.clone(),
                    self.config.clone(),
                ))
            })
            .as_mut()
    }
}
