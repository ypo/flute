use super::fdtreceiver;
use super::fdtreceiver::FdtReceiver;
use super::objectreceiver;
use super::objectreceiver::ObjectReceiver;
use super::writer::ObjectWriterBuilder;
use crate::common::udpendpoint::UDPEndpoint;
use crate::common::{alc, lct};
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
    /// Enable MD5 check of the received objects. Default `true`
    pub enable_md5_check: bool,
    /// CHeck if FDT is already received
    pub check_fdt_received: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_objects_error: 0,
            session_timeout: None,
            object_timeout: Some(Duration::from_secs(10)),
            object_max_cache_size: None,
            enable_md5_check: true,
            check_fdt_received: true
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
    fdt_current: VecDeque<Box<FdtReceiver>>,
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
            fdt_current: VecDeque::new(),
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

        log::debug!("Check elapsed {:?}", self.last_activity.elapsed());
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
                    Some(key.clone())
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
        debug_assert!(self.tsi == alc_pkt.lct.tsi);
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

    fn is_fdt_received(&self, fdt_instance_id: u32) -> bool {
        self.fdt_current
            .iter()
            .find(|fdt| fdt.fdt_id == fdt_instance_id)
            .is_some()
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

        if self.config.check_fdt_received && self.is_fdt_received(fdt_instance_id) {
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
                log::warn!(
                    "TSI={} FDT state is {:?}, bug ?",
                    self.tsi,
                    fdt_receiver.state()
                );
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
            if previous_fdt.fdt_id + 1 != fdt_instance_id {
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

                self.writer
                    .fdt_received(&self.endpoint, &self.tsi, &xml, expiration_date, now);
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
        let server_time = fdt.get_server_time(now);
        let fdt_instance = fdt.fdt_instance()?;
        log::info!("TSI={} Attach FDT id {}", self.tsi, fdt_id);
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

    fn update_expiration_date_of_completed_objects_using_latest_fdt(
        &mut self,
        now: std::time::SystemTime,
    ) -> Option<()> {
        let fdt = self.fdt_current.front_mut()?;
        let server_time = fdt.get_server_time(now);
        let fdt_instance = fdt.fdt_instance()?;
        let files = fdt_instance.file.as_ref()?;
        let expiration_date = fdt_instance.get_expiration_date();

        if let Some(true) = fdt_instance.full_fdt {
            let files_toi: std::collections::HashMap<u128, Option<&String>> = files.iter().map(|f| (f.toi.parse().unwrap_or_default(), f.content_md5.as_ref())).collect();
            let remove_candidates: std::collections::HashMap<u128, ObjectCompletedMeta> = self.objects_completed.iter().filter_map(|(toi, meta)| match  files_toi.contains_key(toi) {
                true => None, 
                false => Some((toi.clone(), meta.clone()))
            }).collect();
            
            if !remove_candidates.is_empty() {
                let content_locations: std::collections::HashSet<&str> = files.iter().map(|f| f.content_location.as_str()).collect();
                let duration = std::time::Duration::from_secs(4);
                for (toi, meta) in &remove_candidates {
                    let content_location = meta.content_location.to_string();
                    if !content_locations.contains(content_location.as_str()) && meta.expiration_date > now + duration {
                        self.writer.set_cache_duration(
                            &self.endpoint,
                            &self.tsi,
                            &toi,
                            &meta.content_location,
                            &duration,
                        );
                    }
                }
                self.objects_completed.retain(|f, _| !remove_candidates.contains_key(f));
            }
        }
        
        for file in files {
            let toi: u128 = file.toi.parse().unwrap_or_default();
            let cache_duration = file.get_cache_duration(expiration_date, server_time);
            if let Some(obj) = self.objects_completed.get_mut(&toi) {
                if let Some(cache_duration) = cache_duration {
                    let new_duration = now
                        .checked_add(cache_duration)
                        .unwrap_or(now + std::time::Duration::from_secs(3600 * 24 * 360 * 10));

                    let diff = match new_duration < obj.expiration_date {
                        true => obj.expiration_date.duration_since(new_duration),
                        false => new_duration.duration_since(obj.expiration_date),
                    }
                    .unwrap();

                    if diff.as_secs() > 1 {
                        obj.expiration_date = new_duration;
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
        }

        Some(())
    }

    fn push_obj(&mut self, pkt: &alc::AlcPkt, now: SystemTime) -> Result<()> {
        if self.objects_completed.contains_key(&pkt.lct.toi) {
                return Ok(());
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
                    log::debug!(
                        "Object state is completed {:?} tsi={} toi={}",
                        self.endpoint,
                        self.tsi,
                        obj.toi
                    );

                    if obj.cache_expiration_date.is_some() {
                        debug_assert!(obj.content_location.is_some());
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
            let _success = self.objects.remove(&toi);
            debug_assert!(_success.is_some());
        }
    }

    fn gc_object_completed(&mut self) {

        let current_fdt = match self.fdt_current.front_mut() {
            Some(fdt) => fdt,
            None => return
        };

        let instance = match current_fdt.fdt_instance() {
            Some(instance) => instance,
            None => return
        };
        
        if let Some(true) = instance.full_fdt {
            return;
        }
             
        
        let before = self.objects_completed.len();
        if let Some(files) = instance.file.as_ref() {
            let current_tois: std::collections::HashSet<u128> = files.iter().map(|file| file.toi.parse().unwrap_or(0)).collect();
            self.objects_completed
            .retain(|toi, _meta| current_tois.contains(toi) );
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
            self.config.enable_md5_check,
            self.config.object_max_cache_size.unwrap_or(10 * 1024 * 1024),
            now
        ));


        let mut is_attached = false;
        let mut fdt_index = 0;
        for fdt in &mut self.fdt_current.iter_mut() {
            let fdt_id = fdt.fdt_id;
            let server_time = fdt.get_server_time(now);
            fdt.update_expired_state(now);
            if fdt.state() == fdtreceiver::State::Complete {
                if let Some(fdt_instance) = fdt.fdt_instance() {
                    let success = obj.attach_fdt(fdt_id, fdt_instance, now, server_time);
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
            fdt_index += 1;
        }

        if is_attached == false {
            log::warn!("Object received before the FDT TSI={} TOI={}", self.tsi, toi);
        }

        self.objects.insert(toi.clone(), obj);
    }
}



impl Drop for Receiver {
    fn drop(&mut self) {
        
        log::info!("Drop Flute Receiver");

        if let Some(fdt) = self.fdt_current.front_mut() {

            if let Some(instance) = fdt.fdt_instance() {
                if instance.full_fdt == Some(true) {
                    let duration = std::time::Duration::from_secs(0);
                    for obj in &self.objects_completed {
                        log::info!("Remove from cache {}", &obj.1.content_location.to_string());
                        self.writer.set_cache_duration(
                            &self.endpoint,
                            &self.tsi,
                            &obj.0,
                            &obj.1.content_location,
                            &duration,
                        );
                    }

                }
            }

        }
    }
}
