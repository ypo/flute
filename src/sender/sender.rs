use super::fdt::Fdt;
use super::observer::ObserverList;
use super::sendersession::SenderSession;
use super::{objectdesc, ObjectDesc, Subscriber, Toi};
use crate::common::{alc, lct, oti, Profile};
use crate::core::UDPEndpoint;
use crate::error::FluteError;
use crate::sender::objectdesc::CarouselRepeatMode;
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
/// Configuration of a priority queue
///
#[derive(Debug)]
pub struct PriorityQueue {
    /// Max number of files that are multiplexed in this queue during the transmission
    /// 0..1 : files are transmitted one after the other.  
    /// 2.. : multiple files might be transmitted in parallel.   
    pub multiplex_files: u32,
}

impl PriorityQueue {
    /// Associated constant representing the highest priority level
    pub const HIGHEST: u32 = 0;
    /// Associated constant representing a high priority level
    pub const HIGH: u32 = 1;
    /// Associated constant representing a medium priority level
    pub const MEDIUM: u32 = 2;
    /// Associated constant representing a low priority level
    pub const LOW: u32 = 3;
    /// Associated constant representing a very low priority level
    pub const VERYLOW: u32 = 4;

    /// Creates a new priority queue configuration.
    ///
    /// The `multiplex_files` parameter determines the maximum number of files that can be interleaved in this queue
    /// during transmission.
    ///
    /// # Arguments
    ///
    /// * `multiplex_files` - The maximum number of files that can be interleaved in this priority queue.
    ///
    pub fn new(multiplex_files: u32) -> Self {
        PriorityQueue { multiplex_files }
    }
}

/// Specifies how the File Delivery Table (FDT) is published.
///
// This enum defines when and how the FDT is updated and sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FDTPublishMode {
    /// FullFDT publishing mode.
    ///
    /// - The FDT is published only when `publish()` is explicitly called.
    /// - It includes all objects that have been inserted up to the time of publication.
    /// - Provides full control over when the FDT is updated and sent.
    FullFDT,

    /// Automatic publishing mode.
    ///
    /// - The FDT is automatically published before the transmission of each object.
    /// - It contains only the objects that are currently being transferred.
    /// - Ensures that each transmission is accompanied by an up-to-date FDT,  
    ///   May result in smaller but more frequent FDT updates.
    ObjectsBeingTransferred,
}

///
/// Configuration of the `Sender`
///
#[derive(Debug)]
pub struct Config {
    /// Max duration of the FDT before expiration.
    pub fdt_duration: std::time::Duration,
    /// Controls how the FDT is repeatedly transferred in a carousel loop.
    pub fdt_carousel_mode: CarouselRepeatMode,
    /// First FDT ID.
    pub fdt_start_id: u32,
    /// Content Encoding of the FDT.
    pub fdt_cenc: lct::Cenc,
    /// Insert Sender Current Time inside ALC/LCT packets containing the FDT.
    pub fdt_inband_sct: bool,
    /// FDT publish mode
    pub fdt_publish_mode: FDTPublishMode,
    /// A struct representing a set of priority queues for file transmission.
    /// Each priority queue is associated with a specific priority level determined by the key in the `BTreeMap`.
    /// A lower key indicates a higher priority.
    /// Files added to higher priority queues are transferred with higher precedence.
    pub priority_queues: std::collections::BTreeMap<u32, PriorityQueue>,
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

impl Config {
    /// Inserts a priority queue into the Sender configuration.
    ///
    /// # Arguments
    ///
    /// * `priority` - The priority level at which the new priority queue should be inserted. Lower value has higher priority
    /// * `config` - The configuration of the priority queue.
    ///
    pub fn set_priority_queue(&mut self, priority: u32, config: PriorityQueue) {
        self.priority_queues.insert(priority, config);
    }

    /// Remove a priority queue from the Sender configuration
    ///
    /// # Arguments
    ///
    /// * `priority` - The priority level of the priority queue to be removed.
    ///
    pub fn remove_priority_queue(&mut self, priority: u32) {
        self.priority_queues.remove(&priority);
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            fdt_duration: std::time::Duration::from_secs(3600),
            fdt_carousel_mode: CarouselRepeatMode::DelayBetweenTransfers(
                std::time::Duration::from_secs(1),
            ),
            fdt_start_id: 1,
            fdt_cenc: lct::Cenc::Null,
            fdt_inband_sct: true,
            priority_queues: std::collections::BTreeMap::from([(
                0,
                PriorityQueue { multiplex_files: 3 },
            )]),
            interleave_blocks: 4,
            profile: Profile::RFC6726,
            toi_max_length: TOIMaxLength::ToiMax112,
            toi_initial_value: Some(1),
            groups: None,
            fdt_publish_mode: FDTPublishMode::FullFDT,
        }
    }
}

#[derive(Debug)]
struct SenderSessionList {
    index: usize,
    sessions: Vec<SenderSession>,
}

///
/// FLUTE `Sender` session
/// Transform objects (files) to ALC/LCT packet
///
#[derive(Debug)]
pub struct Sender {
    fdt: Fdt,
    fdt_session: SenderSession,
    sessions: std::collections::BTreeMap<u32, SenderSessionList>,
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
            config.fdt_carousel_mode,
            config.fdt_inband_sct,
            observers.clone(),
            config.toi_max_length,
            config.toi_initial_value,
            config.groups.clone(),
            config.fdt_publish_mode,
        );

        let fdt_session = SenderSession::new(
            0,
            tsi,
            config.interleave_blocks as usize,
            true,
            config.profile,
            endpoint.clone(),
        );

        let mut sessions = std::collections::BTreeMap::new();

        for (priority, priority_queue_config) in &config.priority_queues {
            let multiplex_files = match priority_queue_config.multiplex_files {
                0 => 1,
                n => n,
            };

            let new_sessions = (0..multiplex_files)
                .map(|_| {
                    SenderSession::new(
                        *priority,
                        tsi,
                        config.interleave_blocks as usize,
                        false,
                        config.profile,
                        endpoint.clone(),
                    )
                })
                .collect();
            sessions.insert(
                *priority,
                SenderSessionList {
                    index: 0,
                    sessions: new_sessions,
                },
            );
        }

        Sender {
            fdt,
            fdt_session,
            sessions,
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
    /// If FDT is configured in FullFDT mode, after calling this function, a call to `publish()` to publish your modifications
    ///
    /// If a TOI as been set to the ObjectDesc, there is no need to release it
    ///
    /// # Arguments
    ///
    /// * `priority` - Selects the priority queue used to transfer the object.
    /// * `obj` - The object description to be added to the FDT.
    ///
    ///
    /// # Returns
    ///
    /// A `Result` containing an `u128` representing the unique identifier of the added object (TOI), if the operation was successful.
    pub fn add_object(&mut self, priority: u32, obj: Box<objectdesc::ObjectDesc>) -> Result<u128> {
        if !self.sessions.contains_key(&priority) {
            return Err(FluteError::new(
                format! {"Priority queue {} does not exist", priority},
            ));
        }

        self.fdt.add_object(priority, obj)
    }

    /// Initiates the transfer of an object that is broadcasted in a carousel.
    ///
    /// - The object must be listed in the File Delivery Table (FDT).
    /// - If the object is already being transferred, no action is taken.
    ///
    /// # Arguments
    /// * `toi` - TOI of the Object.
    /// * `timestamp` - Optional timestamp to set when the object transfer must start. If None, the transfer will start immediately.
    ///
    /// # Returns
    /// - `true` if the object is found the triggered is set.
    /// - `false` if the object is not listed in the FDT or the trigger could not be set.
    pub fn trigger_transfer_at(&mut self, toi: u128, timestamp: Option<SystemTime>) -> bool {
        self.fdt.trigger_transfer_at(toi, timestamp)
    }

    /// Check if the object is inside the FDT
    pub fn is_added(&self, toi: u128) -> bool {
        self.fdt.is_added(toi)
    }

    /// Remove an object from the FDT
    ///
    /// After calling this function, a call to `publish()` to publish your modifications
    ///
    /// Warning, if the object has not been transferred at least once and is being transferred
    /// the transfer is not canceled
    ///
    /// # Arguments
    ///
    /// * `toi` - TOI of the Object.
    ///
    /// # Returns
    ///
    /// `true`if the object has been removed from the FDT
    pub fn remove_object(&mut self, toi: u128) -> bool {
        self.fdt.remove_object(toi)
    }

    /// Return the number of times an object has been transferred,
    /// or None if the object is not in the FDT anymore.
    ///
    /// # Arguments
    ///
    /// * `toi` - TOI of the Object.
    ///
    /// # Returns
    ///
    /// * `Some(count)` - The number of times the object has been transferred, if it is found in the FDT.
    /// * `None` - If the object with the specified `toi` is not present in the FDT.
    ///
    pub fn nb_transfers(&mut self, toi: u128) -> Option<u64> {
        self.fdt.nb_transfers(toi)
    }

    /// Number of objects available in the FDT
    pub fn nb_objects(&self) -> usize {
        self.fdt.nb_objects()
    }

    /// Publish modifications to the FDT
    /// An updated version of the FDT will be generated and transferred
    /// Multiple modifications can be made (ex: several call to 'add_object()`) before publishing a new FDT version
    ///
    /// Required only if fdt_publish_mode is set to FullFDT
    pub fn publish(&mut self, now: SystemTime) -> Result<()> {
        self.fdt.publish(now)
    }

    /// Inform that the FDT is complete, no new object should be added after this call
    /// You must not call `add_object()`after
    /// After calling this function, a call to `publish()` is required to publish your modifications
    pub fn set_complete(&mut self) {
        self.fdt.set_complete();
    }

    /// Generate a close_session packet
    pub fn read_close_session(&mut self, _now: SystemTime) -> Vec<u8> {
        alc::new_alc_pkt_close_session(&0u128, self.tsi)
    }

    /// Allocate a TOI
    /// TOI must be either release or assigned to an object and call add_object()`
    pub fn allocate_toi(&mut self) -> Box<Toi> {
        self.fdt.allocate_toi()
    }

    /// Convert current FDT to XML
    pub fn fdt_xml_data(&self, now: SystemTime) -> Result<Vec<u8>> {
        self.fdt.to_xml(now)
    }

    /// Get List of objects inside the FDT
    pub fn get_objects_in_fdt(&self) -> std::collections::HashMap<u128, &ObjectDesc> {
        self.fdt.get_objects_in_fdt()
    }

    /// Read the next ALC/LCT packet
    /// return None if there is no new packet to be transferred
    /// ALC/LCT packet should be encapsulated into a UDP/IP payload and transferred via UDP/multicast
    pub fn read(&mut self, now: SystemTime) -> Option<Vec<u8>> {
        if let Some(fdt_data) = self.fdt_session.run(&mut self.fdt, now) {
            return Some(fdt_data);
        }

        let fdt = &mut self.fdt;
        for session in &mut self.sessions {
            let data = Self::read_priority_queue(fdt, session.1, now);
            if data.is_some() {
                return data;
            }
        }

        if let Some(fdt_data) = self.fdt_session.run(&mut self.fdt, now) {
            return Some(fdt_data);
        }

        None
    }

    fn read_priority_queue(
        fdt: &mut Fdt,
        sessions: &mut SenderSessionList,
        now: SystemTime,
    ) -> Option<Vec<u8>> {
        let session_index_orig = sessions.index;
        loop {
            let session = sessions.sessions.get_mut(sessions.index).unwrap();
            let data = session.run(fdt, now);

            sessions.index += 1;
            if sessions.index == sessions.sessions.len() {
                sessions.index = 0;
            }

            if data.is_some() {
                return data;
            }

            if sessions.index == session_index_orig {
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
            buffer,
            "text",
            &url::Url::parse("file:///hello").unwrap(),
            1,
            None,
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

        sender.add_object(0, create_obj(nb_pkt)).unwrap();
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
        let res = sender.add_object(0, object);
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

        let toi = sender.add_object(0, object).unwrap();
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

        let result = sender.add_object(0, object1);
        assert!(result.is_ok());

        sender.set_complete();
        let result = sender.add_object(0, object2);
        assert!(result.is_err());
    }
}
