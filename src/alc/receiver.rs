use super::alc;
use super::fdtreceiver;
use super::fdtreceiver::FdtReceiver;
use super::lct;
use super::objectreceiver;
use super::objectreceiver::ObjectReceiver;
use super::objectwriter::ObjectWriter;
use crate::tools::error::FluteError;
use crate::tools::error::Result;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

///
/// Configuration of the FLUTE Receiver
///
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Config {
    /// Keep a reference of a maximum of `gc_max_objects_completed` objects that are completed.
    /// Packets
    pub gc_max_objects_completed: usize,
    /// Keep
    pub gc_max_objects_error: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gc_max_objects_completed: 25,
            gc_max_objects_error: 0,
        }
    }
}

///
/// FLUTE `Receiver`
/// Used to re-construct objects from ALC/LCT packets
///
#[derive(Debug)]
pub struct Receiver {
    tsi: u64,
    objects: HashMap<u128, Box<ObjectReceiver>>,
    objects_completed: BTreeSet<u128>,
    objects_error: BTreeSet<u128>,
    fdt_receivers: BTreeMap<u32, Box<FdtReceiver>>,
    writer: Rc<dyn ObjectWriter>,
    config: Config,
}

impl Receiver {
    /// Return a new `Receiver`
    ///
    pub fn new(tsi: u64, writer: Rc<dyn ObjectWriter>, config: Option<Config>) -> Self {
        Self {
            tsi,
            objects: HashMap::new(),
            fdt_receivers: BTreeMap::new(),
            writer,
            objects_completed: BTreeSet::new(),
            objects_error: BTreeSet::new(),
            config: config.unwrap_or_default(),
        }
    }

    /// Push ALC/LCT packets to the `Receiver`
    pub fn push(&mut self, alc_pkt: &alc::AlcPkt) -> Result<()> {
        assert!(self.tsi == alc_pkt.lct.tsi);

        match alc_pkt.lct.toi {
            toi if toi == lct::TOI_FDT => self.push_fdt_obj(alc_pkt),
            _ => self.push_obj(alc_pkt),
        }
    }

    fn push_fdt_obj(&mut self, alc_pkt: &alc::AlcPkt) -> Result<()> {
        let fdt_ext = alc::find_ext_fdt(&alc_pkt)?;
        if fdt_ext.is_none() {
            if alc_pkt.lct.close_object {
                return Ok(());
            }

            if alc_pkt.lct.close_session {
                // TODO close this receiver
                return Ok(());
            }

            return Err(FluteError::new("FDT pkt received without FDT Extension"));
        }
        let fdt_ext = fdt_ext.unwrap();

        {
            let fdt_receiver = self
                .fdt_receivers
                .entry(fdt_ext.fdt_instance_id)
                .or_insert(Box::new(FdtReceiver::new(fdt_ext.fdt_instance_id)));

            if fdt_receiver.state() != fdtreceiver::State::Receiving {
                return Ok(());
            }

            fdt_receiver.push(alc_pkt).ok();
            if fdt_receiver.state() != fdtreceiver::State::Complete {
                return Ok(());
            }
        }

        log::info!("FDT Received !");
        self.attach_fdt_to_objects(fdt_ext.fdt_instance_id);

        Ok(())
    }

    fn attach_fdt_to_objects(&mut self, fdt_id: u32) -> Option<()> {
        let fdt_receiver = self.fdt_receivers.get_mut(&fdt_id)?;
        let fdt_instance = fdt_receiver.fdt_instance()?;
        for obj in &mut self.objects {
            obj.1.attach_fdt(fdt_id, fdt_instance);
        }

        Some(())
    }

    fn push_obj(&mut self, pkt: &alc::AlcPkt) -> Result<()> {
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
                if pkt.lct.close_object {
                    return Ok(());
                }
                self.create_obj(&pkt.lct.toi);
                obj = self.objects.get_mut(&pkt.lct.toi);
            }

            let obj = match obj {
                Some(obj) => obj.as_mut(),
                None => return Err(FluteError::new("Bug ? Object not found")),
            };

            assert!(obj.state == objectreceiver::State::Receiving);
            obj.push(pkt).ok();
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
        while self.objects_completed.len() > self.config.gc_max_objects_completed {
            let toi = self.objects_completed.pop_first().unwrap();
            self.objects.remove(&toi);
        }
    }

    fn gc_object_error(&mut self) {
        while self.objects_error.len() > self.config.gc_max_objects_error {
            let toi = self.objects_error.pop_first().unwrap();
            self.objects.remove(&toi);
        }
    }

    fn create_obj(&mut self, toi: &u128) {
        let session = self.writer.create_session(&self.tsi, toi);
        let mut obj = Box::new(ObjectReceiver::new(toi, session));

        for fdt in &mut self.fdt_receivers {
            if fdt.1.state() == fdtreceiver::State::Complete {
                let fdt_instance = fdt.1.fdt_instance();
                if fdt_instance.is_some() {
                    let success = obj.attach_fdt(fdt.0.clone(), fdt_instance.unwrap());
                    if success {
                        log::info!("FDT attached during object creation");
                        break;
                    }
                }
            }
        }

        self.objects.insert(toi.clone(), obj);
    }
}
