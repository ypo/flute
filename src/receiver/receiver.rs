use super::fdtreceiver;
use super::fdtreceiver::FdtReceiver;
use super::objectreceiver;
use super::objectreceiver::ObjectReceiver;
use super::writer::{ObjectMetadata, ObjectWriterBuilder};
use crate::common::udpendpoint::UDPEndpoint;
use crate::common::{alc, lct};
use crate::receiver::writer::ObjectCacheControl;
use crate::tools::error::FluteError;
use crate::tools::error::Result;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

/// Configuration of the FLUTE Receiver
///
/// The FLUTE receiver uses the `Config` struct to specify various settings and timeouts for the FLUTE session.
///
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Config {
    /// Max number of objects with error that the receiver is keeping track of.
    /// Packets received for an object in error state are discarded
    pub max_objects_error: usize,
    /// The receiver expires if no data has been received before this timeout
    /// `None` the receiver never expires except if a close session packet is received
    pub session_timeout: Option<Duration>,
    /// Objects expire if no data has been received before this timeout
    /// `None` Objects never expires, not recommended as object that are not fully reconstructed might continue to consume memory for an finite amount of time.
    pub object_timeout: Option<Duration>,
    /// Maximum cache size that can be allocated to received an object. Default is 10MB.
    pub object_max_cache_size: Option<usize>,
    /// When set to `true`, the receiver will only reconstruct each object once.
    /// If the same object is transferred again, it will be automatically discarded.
    pub object_receive_once: bool,
    /// When set to `true`, the receiver will check the expiration date of the FDT.
    pub enable_fdt_expiration_check: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_objects_error: 0,
            session_timeout: None,
            object_timeout: Some(Duration::from_secs(10)),
            object_max_cache_size: None,
            object_receive_once: true,
            enable_fdt_expiration_check: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectCompletedMeta {
    metadata: ObjectMetadata,
}

///
/// FLUTE `Receiver` able to re-construct objects from ALC/LCT packets
///
#[derive(Debug)]
pub struct Receiver {
    tsi: u64,
    objects: HashMap<u128, Box<ObjectReceiver>>,
    objects_completed: BTreeMap<u128, ObjectCompletedMeta>,
    objects_error: BTreeSet<u128>,
    fdt_receivers: BTreeMap<u32, Box<FdtReceiver>>,
    fdt_current: VecDeque<Box<FdtReceiver>>,
    writer: Rc<dyn ObjectWriterBuilder>,
    config: Config,
    last_activity: Instant,
    closed_is_imminent: bool,
    endpoint: UDPEndpoint,
    last_timestamp: Option<SystemTime>,
}

impl Receiver {
    ///
    /// Create a new FLUTE Receiver
    /// # Arguments
    ///
    /// * `endpoint` - The `UDPEndpoint` from where the data are received.
    /// * `tsi` - The Transport Session Identifier of this FLUTE Session.
    /// * `writer` - An `ObjectWriterBuilder` used for writing received objects.
    /// * `config` - Configuration for the `Receiver`.
    ///
    /// # Returns
    ///
    /// A new `Receiver` instance.
    ///
    pub fn new(
        endpoint: &UDPEndpoint,
        tsi: u64,
        writer: Rc<dyn ObjectWriterBuilder>,
        config: Option<Config>,
    ) -> Self {
        Self {
            tsi,
            objects: HashMap::new(),
            fdt_receivers: BTreeMap::new(),
            fdt_current: VecDeque::new(),
            writer,
            objects_completed: BTreeMap::new(),
            objects_error: BTreeSet::new(),
            config: config.unwrap_or_default(),
            last_activity: Instant::now(),
            closed_is_imminent: false,
            endpoint: endpoint.clone(),
            last_timestamp: None,
        }
    }

    /// Check if the receiver is expired.
    ///
    /// This method checks whether the receiver is expired and returns `true` if it is.
    /// This indicates that the receiver should be destroyed.
    ///
    /// # Returns
    ///
    /// `true` if the receiver is expired, otherwise `false`.
    ///
    pub fn is_expired(&self) -> bool {
        if self.config.session_timeout.is_none() {
            return false;
        }

        log::debug!("Check elapsed {:?}", self.last_activity.elapsed());
        self.last_activity
            .elapsed()
            .gt(self.config.session_timeout.as_ref().unwrap())
    }

    /// Get the number of objects being received.
    ///
    /// This method returns the number of objects that are currently being received by the `Receiver`.
    ///
    /// # Returns
    ///
    /// The number of objects being received.
    ///
    pub fn nb_objects(&self) -> usize {
        self.objects.len()
    }

    /// Get the number of objects in error state.
    ///
    /// This method returns the number of objects that are currently in an error state
    /// in the `Receiver`.
    ///
    /// # Returns
    ///
    /// The number of objects in error state.
    ///
    pub fn nb_objects_error(&self) -> usize {
        self.objects_error.len()
    }

    /// Free objects that timed out.
    ///
    /// This method performs cleanup operations on the `Receiver`, freeing objects that
    /// have timed out.
    ///
    /// # Arguments
    ///
    /// * `now` - The current `SystemTime` to use for time-related operations.
    ///
    pub fn cleanup(&mut self, now: std::time::SystemTime) {
        self.last_timestamp = Some(now);
        self.cleanup_objects();
        self.cleanup_fdt(now);
    }

    fn cleanup_fdt(&mut self, now: std::time::SystemTime) {
        self.fdt_receivers.iter_mut().for_each(|fdt| {
            fdt.1.update_expired_state(now);
        });

        self.fdt_receivers.retain(|_, fdt| {
            let state = fdt.state();
            state == fdtreceiver::FDTState::Complete || state == fdtreceiver::FDTState::Receiving
        });
    }

    fn cleanup_objects(&mut self) {
        if self.config.object_timeout.is_none() {
            return;
        }
        let object_timeout = self.config.object_timeout.as_ref().unwrap();
        let now = Instant::now();

        let expired_objects_toi: std::collections::HashSet<u128> = self
            .objects
            .iter()
            .filter_map(|(key, object)| {
                let duration = object.last_activity_duration_since(now);
                if duration.gt(object_timeout) {
                    log::warn!(
                        "Object Expired ! tsi={} toi={} state : {:?} 
                        location: {:?} attached={:?} blocks completed={}/{} last activity={:?} max={:?} 
                        transfer_length={:?} byte_left={:?}",
                        object.tsi,
                        object.toi,
                        object.state,
                        object.content_location.as_ref().map(|u| u.to_string()),
                        object.fdt_instance_id,
                        object.nb_block_completed(),
                        object.nb_block(),
                        duration,
                        object_timeout,
                        object.transfer_length,
                        object.byte_left()
                    );
                    Some(*key)
                } else {
                    None
                }
            })
            .collect();

        for toi in expired_objects_toi {
            self.objects_error.remove(&toi);
            self.objects.remove(&toi);
        }
    }

    /// Push an ALC/LCT packet to the `Receiver`.
    ///
    /// This method is used to push data (the payload of a UDP/IP packet) to the `Receiver`.
    ///
    /// # Arguments
    ///
    /// * `data` - The payload of the UDP/IP packet.
    /// * `now` - The current `SystemTime` to use for time-related operations.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success (`Ok`) or an error (`Err`).
    ///
    /// # Errors
    ///
    /// Returns as error if the packet is not a valid
    ///
    pub fn push_data(&mut self, data: &[u8], now: std::time::SystemTime) -> Result<()> {
        self.last_timestamp = Some(now);
        let alc = alc::parse_alc_pkt(data)?;
        if alc.lct.tsi != self.tsi {
            return Ok(());
        }

        self.push(&alc, now)
    }

    /// Push ALC/LCT packets to the `Receiver`.
    ///
    /// This method is used to push ALC/LCT packets to the `Receiver`.
    ///
    /// # Arguments
    ///
    /// * `alc_pkt` - The ALC/LCT packet to push.
    /// * `now` - The current `SystemTime` to use for time-related operations.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success (`Ok`) or an error (`Err`).
    ///
    pub fn push(&mut self, alc_pkt: &alc::AlcPkt, now: std::time::SystemTime) -> Result<()> {
        debug_assert!(self.tsi == alc_pkt.lct.tsi);
        self.last_activity = Instant::now();
        self.last_timestamp = Some(now);

        if alc_pkt.lct.close_session {
            log::info!("Close session");
            self.closed_is_imminent = true;
        }

        match alc_pkt.lct.toi {
            toi if toi == lct::TOI_FDT => self.push_fdt_obj(alc_pkt, now),
            _ => self.push_obj(alc_pkt, now),
        }
    }

    fn is_fdt_received(&self, fdt_instance_id: u32) -> bool {
        self.fdt_current
            .iter()
            .any(|fdt| fdt.fdt_id == fdt_instance_id)
    }

    fn push_fdt_obj(&mut self, alc_pkt: &alc::AlcPkt, now: std::time::SystemTime) -> Result<()> {
        if alc_pkt.fdt_info.is_none() {
            if alc_pkt.lct.close_object {
                return Ok(());
            }

            if alc_pkt.lct.close_session {
                return Ok(());
            }

            return Err(FluteError::new("FDT pkt received without FDT Extension"));
        }
        let fdt_instance_id = alc_pkt
            .fdt_info
            .as_ref()
            .map(|f| f.fdt_instance_id)
            .unwrap();

        if self.config.object_receive_once && self.is_fdt_received(fdt_instance_id) {
            return Ok(());
        }

        {
            let fdt_receiver = self
                .fdt_receivers
                .entry(fdt_instance_id)
                .or_insert(Box::new(FdtReceiver::new(
                    &self.endpoint,
                    self.tsi,
                    fdt_instance_id,
                    self.config.enable_fdt_expiration_check,
                    now,
                )));

            if fdt_receiver.state() != fdtreceiver::FDTState::Receiving {
                log::warn!(
                    "TSI={} FDT state is {:?}, bug ?",
                    self.tsi,
                    fdt_receiver.state()
                );
                return Ok(());
            }

            fdt_receiver.push(alc_pkt, now);

            if fdt_receiver.state() == fdtreceiver::FDTState::Complete {
                fdt_receiver.update_expired_state(now);
            }

            match fdt_receiver.state() {
                fdtreceiver::FDTState::Receiving => return Ok(()),
                fdtreceiver::FDTState::Complete => {}
                fdtreceiver::FDTState::Error => return Err(FluteError::new("Fail to decode FDT")),
                fdtreceiver::FDTState::Expired => {
                    let expiration = fdt_receiver.get_expiration_time().unwrap_or(now);
                    let server_time = fdt_receiver.get_server_time(now);

                    let expiration: chrono::DateTime<chrono::Utc> = expiration.into();
                    let server_time: chrono::DateTime<chrono::Utc> = server_time.into();

                    log::warn!(
                        "TSI={} FDT has been received but is already expired expiration time={} server time={}",
                        self.tsi,
                        expiration.to_rfc3339(),
                        server_time.to_rfc3339()
                    );
                    return Ok(());
                }
            };
        }

        if let Some(previous_fdt) = self.fdt_current.front() {
            if previous_fdt.fdt_id + 1 != fdt_instance_id && previous_fdt.fdt_id != fdt_instance_id
            {
                log::warn!(
                    "TSI={} Previous FDT ID {} was current is {} is there an FDT missing ?",
                    self.tsi,
                    previous_fdt.fdt_id,
                    fdt_instance_id
                );
            }
        }

        let fdt_current = self.fdt_receivers.remove(&fdt_instance_id);
        if let Some(mut fdt_current) = fdt_current {
            if let Some(xml) = fdt_current.fdt_xml_str() {
                let expiration_date = fdt_current
                    .fdt_instance()
                    .map(|inst| inst.get_expiration_date().unwrap_or(now))
                    .unwrap_or(now);

                let meta = fdt_current.fdt_meta().unwrap();
                let transfer_duration = now
                    .duration_since(fdt_current.reception_start_time)
                    .unwrap_or(std::time::Duration::new(0, 0));

                self.writer.fdt_received(
                    &self.endpoint,
                    &self.tsi,
                    &xml,
                    expiration_date,
                    meta,
                    transfer_duration,
                    now,
                    fdt_current.ext_time,
                );
            }
            self.fdt_current.push_front(fdt_current);
            self.attach_latest_fdt_to_objects(now);
            self.gc_object_completed();
            self.update_expiration_date_of_completed_objects_using_latest_fdt(now);

            if self.fdt_current.len() > 10 {
                self.fdt_current.pop_back();
            }
        }

        Ok(())
    }

    fn attach_latest_fdt_to_objects(&mut self, now: std::time::SystemTime) -> Option<()> {
        let fdt = self.fdt_current.front_mut()?;
        let fdt_id = fdt.fdt_id;
        let fdt_instance = fdt.fdt_instance()?;
        log::debug!("TSI={} Attach FDT id {}", self.tsi, fdt_id);
        let mut check_state = Vec::new();
        for obj in &mut self.objects {
            let success = obj.1.attach_fdt(fdt_id, fdt_instance, now);
            if success {
                check_state.push(*obj.0);
            }
        }

        for toi in check_state {
            self.check_object_state(toi);
        }

        Some(())
    }

    fn update_expiration_date_of_completed_objects_using_latest_fdt(
        &mut self,
        now: std::time::SystemTime,
    ) -> Option<()> {
        let fdt = self.fdt_current.front_mut()?;
        let fdt_instance = fdt.fdt_instance()?;
        let files = fdt_instance.file.as_ref()?;
        let fdt_expiration_date = fdt_instance.get_expiration_date();

        for file in files {
            let toi: u128 = file.toi.parse().unwrap_or_default();
            let cache_control = file.get_object_cache_control(fdt_expiration_date);
            if let Some(obj) = self.objects_completed.get_mut(&toi) {
                if obj.metadata.cache_control.should_update(cache_control) {
                    obj.metadata.cache_control = cache_control;
                    self.writer.update_cache_control(
                        &self.endpoint,
                        &self.tsi,
                        &toi,
                        &obj.metadata,
                        now,
                    );
                }
            }
        }

        Some(())
    }

    fn push_obj(&mut self, pkt: &alc::AlcPkt, now: SystemTime) -> Result<()> {
        if self.objects_completed.contains_key(&pkt.lct.toi) {
            if self.config.object_receive_once {
                return Ok(());
            }

            let payload_id = alc::get_fec_inline_payload_id(pkt)?;
            if payload_id.sbn == 0 && payload_id.esi == 0 {
                self.objects_completed.remove(&pkt.lct.toi);
            } else {
                return Ok(());
            }
        }
        if self.objects_error.contains(&pkt.lct.toi) {
            let payload_id = alc::get_fec_inline_payload_id(pkt)?;
            if payload_id.sbn == 0 && payload_id.esi == 0 {
                log::warn!("Re-download object after errors");
                self.objects_error.remove(&pkt.lct.toi);
            } else {
                return Ok(());
            }
        }

        let mut obj = self.objects.get_mut(&pkt.lct.toi);
        if obj.is_none() {
            self.create_obj(&pkt.lct.toi, now);
            obj = self.objects.get_mut(&pkt.lct.toi);
        }

        let obj = match obj {
            Some(obj) => obj.as_mut(),
            None => return Err(FluteError::new("Bug ? Object not found")),
        };

        obj.push(pkt, now);
        self.check_object_state(pkt.lct.toi);

        Ok(())
    }

    fn check_object_state(&mut self, toi: u128) {
        let obj = self.objects.get_mut(&toi);
        if obj.is_none() {
            return;
        }
        let mut remove_object = false;

        {
            let obj = obj.unwrap();

            match obj.state {
                objectreceiver::State::Receiving => {}
                objectreceiver::State::Completed => {
                    remove_object = true;
                    log::debug!(
                        "Object state is completed {:?} tsi={} toi={}",
                        self.endpoint,
                        self.tsi,
                        obj.toi
                    );

                    if obj.cache_control != Some(ObjectCacheControl::NoCache) {
                        self.objects_completed.insert(
                            obj.toi,
                            ObjectCompletedMeta {
                                metadata: obj.create_meta(),
                            },
                        );
                    } else {
                        if obj.cache_control.is_none() {
                            log::error!("No cache expiration date for {:?}", obj.content_location);
                        }
                    }
                }
                objectreceiver::State::Interrupted => {
                    log::debug!(
                        "Object transmission interrupted tsi={} toi={}",
                        self.tsi,
                        obj.toi
                    );
                    remove_object = true;
                    self.objects_error.insert(toi);
                    self.gc_object_error();
                }
                objectreceiver::State::Error => {
                    log::error!("Object in error state tsi={} toi={}", self.tsi, obj.toi);
                    remove_object = true;
                    self.objects_error.insert(toi);
                    self.gc_object_error();
                }
            }
        }

        if remove_object {
            log::debug!(
                "Remove object {:?} tsi={} toi={}",
                self.endpoint,
                self.tsi,
                toi
            );
            self.objects.remove(&toi);
        }
    }

    fn gc_object_completed(&mut self) {
        let current_fdt = match self.fdt_current.front_mut() {
            Some(fdt) => fdt,
            None => return,
        };

        let instance = match current_fdt.fdt_instance() {
            Some(instance) => instance,
            None => return,
        };

        let before = self.objects_completed.len();
        if let Some(files) = instance.file.as_ref() {
            let current_tois: std::collections::HashSet<u128> = files
                .iter()
                .map(|file| file.toi.parse().unwrap_or(0))
                .collect();
            self.objects_completed
                .retain(|toi, _meta| current_tois.contains(toi));
        }
        let after = self.objects_completed.len();
        if before != after {
            log::debug!("GC remove {} / {} objects", before - after, before);
        }
    }

    fn gc_object_error(&mut self) {
        while self.objects_error.len() > self.config.max_objects_error {
            let toi = self.objects_error.pop_first().unwrap();
            self.objects.remove(&toi);
        }
    }

    fn create_obj(&mut self, toi: &u128, now: SystemTime) {
        let mut obj = Box::new(ObjectReceiver::new(
            &self.endpoint,
            self.tsi,
            toi,
            None,
            self.writer.clone(),
            self.config
                .object_max_cache_size
                .unwrap_or(10 * 1024 * 1024),
            now,
        ));

        let mut is_attached = false;
        for (fdt_index, fdt) in (&mut self.fdt_current.iter_mut()).enumerate() {
            let fdt_id = fdt.fdt_id;
            fdt.update_expired_state(now);
            if fdt.state() == fdtreceiver::FDTState::Complete {
                if let Some(fdt_instance) = fdt.fdt_instance() {
                    let success = obj.attach_fdt(fdt_id, fdt_instance, now);
                    if success {
                        is_attached = true;
                        if fdt_index != 0 {
                            log::warn!(
                                "TSI={} TOI={} CL={:?} Attaching an object to an FDT that is not the latest (index={}) ",
                                self.tsi,
                                obj.toi,
                                obj.content_location,
                                fdt_index
                            );
                        }

                        break;
                    }
                }
            }
        }

        if !is_attached {
            log::warn!(
                "Object received before the FDT TSI={} TOI={}",
                self.tsi,
                toi
            );
        }

        self.objects.insert(*toi, obj);
    }
}
