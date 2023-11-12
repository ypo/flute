use super::fdt::Fdt;
use super::observer::ObserverList;
use super::sendersession::SenderSession;
use super::{objectdesc, Subscriber, Toi};
use crate::common::{alc, lct, oti, Profile};
use crate::core::UDPEndpoint;
use crate::tools::error::Result;
use std::sync::Arc;
use std::time::SystemTime;

/// Maximum number of bits to encode the TOI
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TOIMaxLength {
    /// 16 bits
    ToiMax16,
    /// 32 bits
    ToiMax32,
    /// 48 bits
    ToiMax48,
    /// 64 bits
    ToiMax64,
    /// 80 bits
    ToiMax80,
    /// 112 bits
    ToiMax112,
}

///
/// Configuration of the `Sender`
///
#[derive(Debug)]
pub struct Config {
    /// Max duration of the FDT before expiration.
    pub fdt_duration: std::time::Duration,
    /// Repeat duration of the FDT in the carousel
    pub fdt_carousel: std::time::Duration,
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
    /// Max number of blocks that are interleaved during the transmission of a file.  
    /// Blocks interleave permits to spread out errors that may occur during transmission.
    /// Combined with error recovery, it can improve resilience to burst error, but can increase the complexity of the reception.
    pub interleave_blocks: u8,
    /// Select FLUTE sender profile used during the transmission
    pub profile: Profile,
    /// Max number of bits to encode the TOI
    pub toi_max_length: TOIMaxLength,
    /// Value of the first TOI of a FLUTE session
    /// TOI value must be > 0
    /// None : Initialize the TOI to a random value
    pub toi_initial_value: Option<u128>,
    /// List of groups added to the FDT-Instance
    pub groups: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fdt_duration: std::time::Duration::from_secs(3600),
            fdt_carousel: std::time::Duration::from_secs(1),
            fdt_start_id: 1,
            fdt_cenc: lct::Cenc::Null,
            fdt_inband_sct: true,
            multiplex_files: 3,
            interleave_blocks: 4,
            profile: Profile::RFC6726,
            toi_max_length: TOIMaxLength::ToiMax112,
            toi_initial_value: Some(1),
            groups: None,
        }
    }
}

///
/// FLUTE `Sender` session
/// Transform objects (files) to ALC/LCT packet
///
#[derive(Debug)]
pub struct Sender {
    fdt: Fdt,
    fdt_session: SenderSession,
    sessions: Vec<SenderSession>,
    session_index: usize,
    observers: ObserverList,
    tsi: u64,
    endpoint: UDPEndpoint,
}

impl Sender {
    ///
    /// Creation of a FLUTE Sender
    ///
    pub fn new(endpoint: UDPEndpoint, tsi: u64, oti: &oti::Oti, config: &Config) -> Sender {
        let observers = ObserverList::new();

        let fdt = Fdt::new(
            tsi,
            config.fdt_start_id,
            oti,
            config.fdt_cenc,
            config.fdt_duration,
            config.fdt_carousel,
            config.fdt_inband_sct,
            observers.clone(),
            config.toi_max_length,
            config.toi_initial_value,
            config.groups.clone(),
        );

        let multiplex_files = match config.multiplex_files {
            0 => 2,
            n => n + 1,
        };

        let fdt_session = SenderSession::new(
            tsi,
            config.interleave_blocks as usize,
            true,
            config.profile,
            endpoint.clone(),
        );

        let sessions = (0..multiplex_files - 1)
            .map(|_| {
                SenderSession::new(
                    tsi,
                    config.interleave_blocks as usize,
                    false,
                    config.profile,
                    endpoint.clone(),
                )
            })
            .collect();

        Sender {
            fdt,
            fdt_session,
            sessions,
            session_index: 0,
            observers,
            tsi,
            endpoint,
        }
    }

    /// Add an observer
    pub fn subscribe(&mut self, s: Arc<dyn Subscriber>) {
        self.observers.subscribe(s);
    }

    /// Remove an observer
    pub fn unsubscribe(&mut self, s: Arc<dyn Subscriber>) {
        self.observers.unsubscribe(s);
    }

    /// Get UDP endpoint
    pub fn get_udp_endpoint(&self) -> &UDPEndpoint {
        &self.endpoint
    }

    /// Get TSI
    pub fn get_tsi(&self) -> u64 {
        self.tsi
    }

    /// Add an object to the FDT
    ///
    /// After calling this function, a call to `publish()` to publish your modifications
    ///
    /// If a TOI as been set to the ObjectDesc, there is no need to release it
    ///
    /// # Returns
    ///
    /// A `Result` containing an `u128` representing the unique identifier of the added object (TOI), if the operation was successful.
    pub fn add_object(&mut self, obj: Box<objectdesc::ObjectDesc>) -> Result<u128> {
        self.fdt.add_object(obj)
    }

    /// Check if the object is inside the FDT
    pub fn is_added(&self, toi: u128) -> bool {
        self.fdt.is_added(toi)
    }

    /// Remove an object from the FDT
    ///
    /// After calling this function, a call to `publish()` to publish your modifications
    ///
    /// Warning, if the object is being transferred, the transfer is not canceled
    ///
    /// # Returns
    ///
    /// `true`if the object has been removed from the FDT
    pub fn remove_object(&mut self, toi: u128) -> bool {
        self.fdt.remove_object(toi)
    }

    /// Number of objects signalled in the FDT
    pub fn nb_objects(&self) -> usize {
        self.fdt.nb_objects()
    }

    /// Publish modification to the FDT
    /// An updated version of the FDT will be generated and transferred
    /// Multiple modification can be made (ex: several call to 'add_object()`) before publishing a new FDT version
    pub fn publish(&mut self, now: SystemTime) -> Result<()> {
        self.fdt.publish(now)
    }

    /// Inform that the FDT is complete, no new object should be added after this call
    /// You must not call `add_object()`after
    /// After calling this function, a call to `publish()` to publish your modifications
    pub fn set_complete(&mut self) {
        self.fdt.set_complete();
    }

    /// Generate a close_session packet
    pub fn read_close_session(&mut self, _now: SystemTime) -> Vec<u8> {
        alc::new_alc_pkt_close_session(&0u128, self.tsi)
    }

    /// Allocate a TOI
    /// TOI must be either release or assigned to an object and call add_object()`
    pub fn allocate_toi(&mut self) -> Arc<Toi> {
        self.fdt.allocate_toi()
    }

    /// Read the next ALC/LCT packet
    /// return None if there is no new packet to be transferred
    /// ALC/LCT packet should be encapsulated into a UDP/IP payload and transferred via UDP/multicast
    pub fn read(&mut self, now: SystemTime) -> Option<Vec<u8>> {
        if let Some(fdt_data) = self.fdt_session.run(&mut self.fdt, now) {
            return Some(fdt_data);
        }

        let session_index_orig = self.session_index;
        loop {
            let session = self.sessions.get_mut(self.session_index).unwrap();
            let data = session.run(&mut self.fdt, now);

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

    use crate::common::lct;
    use crate::core::UDPEndpoint;

    use super::objectdesc;
    use super::oti;

    fn create_obj(length: usize) -> Box<objectdesc::ObjectDesc> {
        let buffer = vec![0u8; length];
        objectdesc::ObjectDesc::create_from_buffer(
            &buffer,
            "text",
            &url::Url::parse("file:///hello").unwrap(),
            1,
            None,
            None,
            None,
            lct::Cenc::Null,
            true,
            None,
            true,
        )
        .unwrap()
    }

    #[test]
    pub fn test_sender() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 1234);
        let mut sender = super::Sender::new(endpoint, 1, &oti, &Default::default());

        let nb_pkt = oti.encoding_symbol_length as usize * 3;

        sender.add_object(create_obj(nb_pkt)).unwrap();
        sender.publish(std::time::SystemTime::now()).unwrap();
        loop {
            let data = sender.read(std::time::SystemTime::now());
            if data.is_none() {
                break;
            }
        }
    }

    #[test]
    pub fn test_sender_file_too_large() {
        crate::tests::init();
        let oti = oti::Oti::new_no_code(4, 2);
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 1234);
        // Create a buffer larger that the max transfer length
        let object = create_obj(oti.max_transfer_length() + 1);
        let mut sender = super::Sender::new(endpoint, 1, &oti, &Default::default());
        let res = sender.add_object(object);
        assert!(res.is_err());
    }

    #[test]
    pub fn test_sender_remove_object() {
        crate::tests::init();
        let oti = Default::default();

        let object = create_obj(1024);
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 1234);
        let mut sender = super::Sender::new(endpoint, 1, &oti, &Default::default());
        assert!(sender.nb_objects() == 0);

        let toi = sender.add_object(object).unwrap();
        assert!(sender.nb_objects() == 1);

        let success = sender.remove_object(toi);
        assert!(success == true);
        assert!(sender.nb_objects() == 0);
    }

    #[test]
    pub fn sender_complete() {
        crate::tests::init();

        let oti = Default::default();
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 1234);
        let mut sender = super::Sender::new(endpoint, 1, &oti, &Default::default());

        let object1 = create_obj(1024);
        let object2 = create_obj(1024);

        let result = sender.add_object(object1);
        assert!(result.is_ok());

        sender.set_complete();
        let result = sender.add_object(object2);
        assert!(result.is_err());
    }
}
