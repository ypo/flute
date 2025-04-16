use super::receiver::{Config, Receiver};
use super::tsifilter::TSIFilter;
use super::writer::ObjectWriterBuilder;
use crate::common::alc;
use crate::common::udpendpoint::UDPEndpoint;
use crate::tools::error::Result;
use core::fmt::Debug;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

/// Receiver endpoint
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct ReceiverEndpoint {
    /// UDP endpoint
    pub endpoint: UDPEndpoint,
    /// TSI value
    pub tsi: u64,
}

/// MultiReceiverListener
pub trait MultiReceiverListener {
    /// Called when a FLUTE session is opened
    fn on_session_open(&self, endpoint: &ReceiverEndpoint);
    /// Called when a FLUTE session is being closed
    fn on_session_closed(&self, endpoint: &ReceiverEndpoint);
}

type MultiReceiverListenerBox = Box<dyn MultiReceiverListener>;

impl Debug for dyn MultiReceiverListener {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MultiReceiverListener")
    }
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
    listeners: HashMap<u64, MultiReceiverListenerBox>,
    listeners_id: u64,
}

impl MultiReceiver {
    ///
    /// Creates a new `MultiReceiver` instance, which allows receiving multiple interlaced FLUTE sessions.
    ///
    /// # Arguments
    ///
    /// * `writer` - Responsible to write object to its final destination.
    ///
    /// * `config` - Configuration of the FLUTE `Receiver`. if `None`, default `Config` will be used
    ///
    /// * `enable_tsi_filtering` - Enable TSI filter mechanism
    /// # Example
    /// ```
    /// // Receive objects from Transport Session 1
    /// use flute::receiver::writer::ObjectWriterBufferBuilder;
    /// use flute::receiver::{MultiReceiver};
    /// use flute::core::UDPEndpoint;
    /// use std::rc::Rc;
    ///
    /// let tsi: u64 = 1;
    /// // Write object to a buffer
    /// let enable_md5_check = true;
    /// let writer = Rc::new(ObjectWriterBufferBuilder::new(enable_md5_check));
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
            listeners: HashMap::new(),
            listeners_id: 0,
        }
    }

    ///
    /// Add a listener to the MultiReceiver
    /// # Arguments
    /// * `listener` - The listener to add
    /// # Returns
    /// The id of the listener
    pub fn add_listener<L>(&mut self, listener: L) -> u64
    where
        L: MultiReceiverListener + 'static,
    {
        let id = self.listeners_id;
        self.listeners_id += 1;
        self.listeners.insert(id, Box::new(listener));
        id
    }

    /// Remove a listener from the MultiReceiver
    ///
    /// # Arguments
    /// * `id` - The id of the listener to remove
    pub fn remove_listener(&mut self, id: u64) {
        self.listeners.remove(&id);
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
    /// * `endpoint` - Add the TSI filter for this endpoint.
    ///
    /// * `tsi` - tsi The TSI value to filter.
    ///
    pub fn add_listen_tsi(&mut self, endpoint: UDPEndpoint, tsi: u64) {
        if !self.enable_tsi_filtering {
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
    /// * `tsi` - The TSI value to remove the filter for.
    ///
    pub fn remove_listen_tsi(&mut self, endpoint: &UDPEndpoint, tsi: u64) {
        self.tsifilter.remove(endpoint, tsi);
    }

    /// Accepts all TSI sessions for a given endpoint   
    pub fn add_listen_all_tsi(&mut self, endpoint: UDPEndpoint) {
        log::info!("Listen all TSI for {:?}", endpoint);
        if !self.enable_tsi_filtering {
            log::warn!("TSI filtering is disabled");
        }

        self.tsifilter.add_endpoint_bypass(endpoint);
    }

    /// Remove the acceptance of all TSI sessions for a given endpoint   
    pub fn remove_listen_all_tsi(&mut self, endpoint: &UDPEndpoint) {
        self.tsifilter.remove_endpoint_bypass(endpoint);
    }

    /// Push an ALC/LCT packet to the `Receiver`.
    ///
    /// This method is used to push an ALC/LCT packet (the payload of a UDP/IP packet)
    /// to the `Receiver`.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - The `UDPEndpoint` from where the packet is received.
    /// * `pkt` - The payload of the UDP/IP packet.
    /// * `now` - The current `SystemTime` to use for time-related operations.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success (`Ok`) or an error (`Err`).
    ///
    /// # Errors
    ///
    /// Returns an error if the packet is not valid or the receiver is in an error state.
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
            log::info!("Close session is set");
            let mut remove_session = false;
            let ret = match self.get_receiver(&key) {
                Some(receiver) => {
                    remove_session = true;
                    receiver.push(&alc, now)
                }
                None => {
                    log::warn!(
                        "A session that is not allocated is about to be closed, skip the session"
                    );
                    Ok(())
                }
            };

            if remove_session {
                log::warn!("Remove closed session");
                self.alc_receiver.remove(&key);
                for listener in self.listeners.values() {
                    listener.on_session_closed(&key);
                }
            }
            ret
        } else {
            let receiver = self.get_receiver_or_create(&key);
            receiver.push(&alc, now)
        }
    }

    ///
    /// Remove FLUTE session that are closed or expired
    /// Remove Objects that are expired
    ///
    /// Cleanup shall be call from time to time to avoid consuming to much memory    
    pub fn cleanup(&mut self, now: SystemTime) {
        let mut output = Vec::new();
        for receiver in &self.alc_receiver {
            if receiver.1.is_expired() {
                output.push(receiver.0.clone());
            }
        }

        self.alc_receiver.retain(|_, v| !v.is_expired());
        for receiver in &mut self.alc_receiver.values_mut() {
            receiver.cleanup(now);
        }

        for endpoint in &output {
            for listener in self.listeners.values() {
                listener.on_session_closed(&endpoint);
            }
        }
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

                for listener in self.listeners.values() {
                    listener.on_session_open(&key);
                }

                Box::new(Receiver::new(
                    &key.endpoint,
                    key.tsi,
                    self.writer.clone(),
                    self.config,
                ))
            })
            .as_mut()
    }
}

impl Drop for MultiReceiver {
    fn drop(&mut self) {
        for endpoint in self.alc_receiver.keys() {
            for listener in self.listeners.values() {
                listener.on_session_closed(endpoint);
            }
        }
    }
}
