use super::fdtreceiver;
use super::fdtreceiver::FdtReceiver;
use super::objectreceiver;
use super::objectreceiver::ObjectReceiver;
use super::writer::ObjectWriterBuilder;
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
    /// The receive expires if no data has been received before this timeout
    /// `None` the receive never expires except if a close session packet is received
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

///
/// FLUTE `Receiver` able to re-construct objects from ALC/LCT packets
///
#[derive(Debug)]
pub struct Receiver {
    tsi: u64,
    objects: HashMap<u128, Box<ObjectReceiver>>,
    objects_completed: BTreeSet<u128>,
    objects_error: BTreeSet<u128>,
    fdt_receivers: BTreeMap<u32, Box<FdtReceiver>>,
    fdt_current: Option<Box<FdtReceiver>>,
    writer: Rc<dyn ObjectWriterBuilder>,
    config: Config,
    last_activity: Instant,
    closed_is_imminent: bool,
}

impl Receiver {
    /// Return a new `Receiver`
    ///
    pub fn new(tsi: u64, writer: Rc<dyn ObjectWriterBuilder>, config: Option<Config>) -> Self {
        Self {
            tsi,
            objects: HashMap::new(),
            fdt_receivers: BTreeMap::new(),
            fdt_current: None,
            writer,
            objects_completed: BTreeSet::new(),
            objects_error: BTreeSet::new(),
            config: config.unwrap_or_default(),
            last_activity: Instant::now(),
            closed_is_imminent: false,
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
                .or_insert(Box::new(FdtReceiver::new(fdt_instance_id, now)));

            if fdt_receiver.state() != fdtreceiver::State::Receiving {
                return Ok(());
            }

            fdt_receiver.push(alc_pkt);

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

        log::info!("FDT Received !");
        self.fdt_current = self.fdt_receivers.remove(&fdt_instance_id);
        assert!(self.fdt_current.is_some());

        self.attach_fdt_to_objects();

        Ok(())
    }

    fn attach_fdt_to_objects(&mut self) -> Option<()> {
        let fdt_current = self.fdt_current.as_mut()?;
        let fdt_id = fdt_current.fdt_id;
        let fdt_instance = fdt_current.fdt_instance()?;
        for obj in &mut self.objects {
            obj.1.attach_fdt(fdt_id, fdt_instance);
        }

        Some(())
    }

    fn push_obj(&mut self, pkt: &alc::AlcPkt, now: SystemTime) -> Result<()> {
        if self.objects_completed.contains(&pkt.lct.toi) {
            return Ok(());
        }
        if self.objects_error.contains(&pkt.lct.toi) {
            return Ok(());
        }

        let mut remove_object = false;
        {
            let mut obj = self.objects.get_mut(&pkt.lct.toi);
            if obj.is_none() {
                self.create_obj(&pkt.lct.toi, now);
                obj = self.objects.get_mut(&pkt.lct.toi);
            }

            let obj = match obj {
                Some(obj) => obj.as_mut(),
                None => return Err(FluteError::new("Bug ? Object not found")),
            };

            obj.push(pkt);
            match obj.state {
                objectreceiver::State::Receiving => {}
                objectreceiver::State::Completed => {
                    remove_object = true;
                    self.objects_completed.insert(obj.toi);
                    self.gc_object_completed();
                }
                objectreceiver::State::Error => {
                    remove_object = true;
                    self.objects_error.insert(obj.toi);
                    self.gc_object_error();
                }
            }
        }

        if remove_object {
            log::info!("Remove object {}", pkt.lct.toi);
            self.objects.remove(&pkt.lct.toi);
        }

        Ok(())
    }

    fn gc_object_completed(&mut self) {
        if self.config.max_objects_completed.is_none() {
            return;
        }

        let max = self.config.max_objects_completed.unwrap();
        while self.objects_completed.len() > max {
            let toi = self.objects_completed.pop_first().unwrap();
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
        let session = self.writer.new_object_writer(&self.tsi, toi);
        let mut obj = Box::new(ObjectReceiver::new(toi, session));

        if let Some(fdt) = self.fdt_current.as_mut() {
            let fdt_id = fdt.fdt_id;
            fdt.update_expired_state(now);
            if fdt.state() == fdtreceiver::State::Complete {
                if let Some(fdt_instance) = fdt.fdt_instance() {
                    obj.attach_fdt(fdt_id, fdt_instance);
                }
            }
        }

        self.objects.insert(toi.clone(), obj);
    }
}
