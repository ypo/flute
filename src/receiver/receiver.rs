use super::fdtreceiver::FdtReceiver;
use super::objectreceiver;
use super::objectreceiver::ObjectReceiver;
use super::writer::ObjectWriterBuilder;
use super::{fdtreceiver, UDPEndpoint};
use crate::common::{alc, lct};
use crate::tools::error::FluteError;
use crate::tools::error::Result;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
    /// Max number of successfully completed objects that the receiver is keeping track of.
    /// Packets received for an object that is already completed are discarded.  
    ///
    /// `None` to keep track of an infinite number of objects.
    /// Should be set to `None` if objects are transmitted in a carousel to avoid multiple reconstruction
    ///
    pub max_objects_completed: Option<usize>,
    /// Max number of objects with error that the receiver is keeping track of.
    /// Packets received for an object in error state are discarded
    pub max_objects_error: usize,
    /// The receiver expires if no data has been received before this timeout
    /// `None` the receiver never expires except if a close session packet is received
    pub session_timeout: Option<Duration>,
    /// Objects expire if no data has been received before this timeout
    /// `None` Objects never expires, not recommended as object that are not fully reconstructed might continue to consume memory for an finite amount of time.
    pub object_timeout: Option<Duration>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_objects_completed: Some(100),
            max_objects_error: 0,
            session_timeout: None,
            object_timeout: Some(Duration::from_secs(10)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectCompletedMeta {
    expiration_date: SystemTime,
    content_location: url::Url,
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
    fdt_current: Option<Box<FdtReceiver>>,
    writer: Rc<dyn ObjectWriterBuilder>,
    config: Config,
    last_activity: Instant,
    closed_is_imminent: bool,
    endpoint: UDPEndpoint,
}

impl Receiver {
    /// Return a new `Receiver`
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
            fdt_current: None,
            writer,
            objects_completed: BTreeMap::new(),
            objects_error: BTreeSet::new(),
            config: config.unwrap_or_default(),
            last_activity: Instant::now(),
            closed_is_imminent: false,
            endpoint: endpoint.clone(),
        }
    }

    ///
    /// Check is the receiver is expired
    /// true -> the receiver should be destroyed to free the resources
    ///
    pub fn is_expired(&self) -> bool {
        if self.config.session_timeout.is_none() {
            return false;
        }

        log::info!("Check elapsed {:?}", self.last_activity.elapsed());
        self.last_activity
            .elapsed()
            .gt(self.config.session_timeout.as_ref().unwrap())
    }

    ///
    /// Number of objects that are we are receiving
    ///
    pub fn nb_objects(&self) -> usize {
        self.objects.len()
    }

    ///
    /// Number objects in error state
    ///
    pub fn nb_objects_error(&self) -> usize {
        self.objects_error.len()
    }

    ///
    /// Free objects after `object_timeout`
    ///
    pub fn cleanup(&mut self, now: std::time::SystemTime) {
        self.cleanup_objects();
        self.cleanup_fdt(now);
    }

    fn cleanup_fdt(&mut self, now: std::time::SystemTime) {
        self.fdt_receivers.iter_mut().for_each(|fdt| {
            fdt.1.update_expired_state(now);
        });

        self.fdt_receivers.retain(|_, fdt| {
            let state = fdt.state();
            state == fdtreceiver::State::Complete || state == fdtreceiver::State::Receiving
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
            .filter_map(|(key, value)| {
                let duration = value.last_activity_duration_since(now);
                if duration.gt(object_timeout) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for toi in expired_objects_toi {
            log::warn!("Remove expired object tsi={} toi={}", self.tsi, toi);
            self.objects_error.remove(&toi);
            self.objects.remove(&toi);
        }
    }

    /// Push data to the `Receiver`
    ///
    /// # Arguments
    /// * `data`- Payload of the UDP/IP packet.
    pub fn push_data(&mut self, data: &[u8], now: std::time::SystemTime) -> Result<()> {
        let alc = alc::parse_alc_pkt(data)?;
        if alc.lct.tsi != self.tsi {
            return Ok(());
        }

        self.push(&alc, now)
    }

    /// Push ALC/LCT packets to the `Receiver`
    pub fn push(&mut self, alc_pkt: &alc::AlcPkt, now: std::time::SystemTime) -> Result<()> {
        assert!(self.tsi == alc_pkt.lct.tsi);
        self.last_activity = Instant::now();

        if alc_pkt.lct.close_session {
            log::info!("Close session");
            self.closed_is_imminent = true;
        }

        match alc_pkt.lct.toi {
            toi if toi == lct::TOI_FDT => self.push_fdt_obj(alc_pkt, now),
            _ => self.push_obj(alc_pkt, now),
        }
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

        let current_fdt_instance_id = self.fdt_current.as_ref().map(|fdt| fdt.fdt_id);
        if current_fdt_instance_id == Some(fdt_instance_id) {
            // FDT already received
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
                    now,
                )));

            if fdt_receiver.state() != fdtreceiver::State::Receiving {
                return Ok(());
            }

            fdt_receiver.push(alc_pkt, now);

            if fdt_receiver.state() == fdtreceiver::State::Complete {
                fdt_receiver.update_expired_state(now);
            }

            match fdt_receiver.state() {
                fdtreceiver::State::Receiving => return Ok(()),
                fdtreceiver::State::Complete => {}
                fdtreceiver::State::Error => return Err(FluteError::new("Fail to decode FDT")),
                fdtreceiver::State::Expired => {
                    log::warn!("FDT has been received but is already expired");
                    return Ok(());
                }
            };
        }

        self.fdt_current = self.fdt_receivers.remove(&fdt_instance_id);
        assert!(self.fdt_current.is_some());

        self.attach_fdt_to_objects(now);
        self.update_expiration_date_of_completed_objects(now);

        Ok(())
    }

    fn attach_fdt_to_objects(&mut self, now: std::time::SystemTime) -> Option<()> {
        let fdt_current = self.fdt_current.as_mut()?;
        let fdt_id = fdt_current.fdt_id;
        let server_time = fdt_current.get_server_time(now);
        let fdt_instance = fdt_current.fdt_instance()?;
        let mut check_state = Vec::new();
        for obj in &mut self.objects {
            let success = obj.1.attach_fdt(fdt_id, fdt_instance, now, server_time);
            if success {
                check_state.push(obj.0.clone());
            }
        }

        for toi in check_state {
            self.check_object_state(toi, now);
        }

        Some(())
    }

    fn update_expiration_date_of_completed_objects(
        &mut self,
        now: std::time::SystemTime,
    ) -> Option<()> {
        let fdt_current = self.fdt_current.as_mut()?;
        let server_time = fdt_current.get_server_time(now);

        let fdt_instance = fdt_current.fdt_instance()?;
        let files = fdt_instance.file.as_ref()?;
        let expiration_date = fdt_instance.get_expiration_date();

        for file in files {
            let toi: u128 = file.toi.parse().unwrap_or_default();
            let cache_duration = file.get_cache_duration(expiration_date, server_time);
            if let Some(obj) = self.objects_completed.get_mut(&toi) {
                if let Some(cache_duration) = cache_duration {
                    obj.expiration_date = now
                        .checked_add(cache_duration)
                        .unwrap_or(now + std::time::Duration::from_secs(3600 * 24 * 360 * 10));
                    self.writer.set_cache_duration(
                        &self.endpoint,
                        &self.tsi,
                        &toi,
                        &obj.content_location,
                        &cache_duration,
                    );
                }
            }
        }

        Some(())
    }

    fn push_obj(&mut self, pkt: &alc::AlcPkt, now: SystemTime) -> Result<()> {
        if self.objects_completed.contains_key(&pkt.lct.toi) {
            self.gc_object_completed(now);
            if self.objects_completed.contains_key(&pkt.lct.toi) {
                return Ok(());
            }
        }
        if self.objects_error.contains(&pkt.lct.toi) {
            return Ok(());
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
        self.check_object_state(pkt.lct.toi, now);

        Ok(())
    }

    fn check_object_state(&mut self, toi: u128, now: SystemTime) {
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
                    log::info!(
                        "Object state is completed {:?} tsi={} toi={}",
                        self.endpoint,
                        self.tsi,
                        obj.toi
                    );

                    if obj.cache_expiration_date.is_some() {
                        assert!(obj.content_location.is_some());
                        log::debug!(
                            "Insert {:?} for a duration of {:?}",
                            obj.content_location,
                            obj.cache_expiration_date.unwrap().duration_since(now)
                        );
                        self.objects_completed.insert(
                            obj.toi,
                            ObjectCompletedMeta {
                                expiration_date: obj.cache_expiration_date.unwrap(),
                                content_location: obj.content_location.as_ref().unwrap().clone(),
                            },
                        );
                        self.gc_object_completed(now);
                    } else {
                        log::error!("No cache expiration date for {:?}", obj.content_location);
                    }
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

    fn gc_object_completed(&mut self, now: SystemTime) {
        let before = self.objects_completed.len();
        self.objects_completed
            .retain(|_toi, meta| meta.expiration_date > now);
        let after = self.objects_completed.len();
        if before != after {
            log::info!("GC remove {} objects", before - after);
        }

        if self.config.max_objects_completed.is_none() {
            return;
        }

        let max = self.config.max_objects_completed.unwrap();
        while self.objects_completed.len() > max {
            let (toi, _meta) = self.objects_completed.pop_first().unwrap();
            self.objects.remove(&toi);
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
            self.writer.clone(),
        ));

        if let Some(fdt) = self.fdt_current.as_mut() {
            let fdt_id = fdt.fdt_id;
            let server_time = fdt.get_server_time(now);
            fdt.update_expired_state(now);
            if fdt.state() == fdtreceiver::State::Complete {
                if let Some(fdt_instance) = fdt.fdt_instance() {
                    obj.attach_fdt(fdt_id, fdt_instance, now, server_time);
                }
            }
        }

        self.objects.insert(toi.clone(), obj);
    }
}
