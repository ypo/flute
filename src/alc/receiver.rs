use super::alc;
use super::fdtreceiver;
use super::fdtreceiver::FdtReceiver;
use super::lct;
use super::objectreceiver::ObjectReceiver;
use super::objectwriter::ObjectWriter;
use crate::tools::error::FluteError;
use crate::tools::error::Result;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

pub struct Receiver {
    tsi: u64,
    objects: HashMap<u128, Box<ObjectReceiver>>,
    fdt_receivers: BTreeMap<u32, Box<FdtReceiver>>,
    writer: Rc<dyn ObjectWriter>,
}

impl Receiver {
    pub fn new(tsi: u64, writer: Rc<dyn ObjectWriter>) -> Receiver {
        Receiver {
            tsi,
            objects: HashMap::new(),
            fdt_receivers: BTreeMap::new(),
            writer,
        }
    }

    pub fn push(&mut self, alc_pkt: &alc::AlcPkt) -> Result<bool> {
        assert!(self.tsi == alc_pkt.lct.tsi);

        match alc_pkt.lct.toi {
            toi if toi == lct::TOI_FDT => self.push_fdt_obj(alc_pkt),
            _ => self.push_obj(alc_pkt),
        }
    }

    fn push_fdt_obj(&mut self, alc_pkt: &alc::AlcPkt) -> Result<bool> {
        let fdt_ext = alc::find_ext_fdt(&alc_pkt)?;
        if fdt_ext.is_none() {
            if alc_pkt.lct.close_object {
                return Ok(true);
            }

            if alc_pkt.lct.close_session {
                // TODO close this receiver
                return Ok(true);
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
                return Ok(true);
            }

            fdt_receiver.push(alc_pkt).ok();
            if fdt_receiver.state() != fdtreceiver::State::Complete {
                return Ok(true);
            }
        }

        log::info!("FDT Received !");
        self.attach_fdt_to_objects(fdt_ext.fdt_instance_id);

        Ok(true)
    }

    fn attach_fdt_to_objects(&mut self, fdt_id: u32) -> Option<()> {
        let fdt_receiver = self.fdt_receivers.get_mut(&fdt_id)?;
        let fdt_instance = fdt_receiver.fdt_instance()?;
        for obj in &mut self.objects {
            obj.1.attach_fdt(fdt_id, fdt_instance);
        }

        Some(())
    }

    fn push_obj(&mut self, pkt: &alc::AlcPkt) -> Result<bool> {
        let mut obj = self.objects.get_mut(&pkt.lct.toi);
        if obj.is_none() {
            if pkt.lct.close_object {
                return Ok(true);
            }
            self.create_obj(&pkt.lct.toi);
            obj = self.objects.get_mut(&pkt.lct.toi);
        }

        let obj = match obj {
            Some(obj) => obj.as_mut(),
            None => return Err(FluteError::new("Bug ? Object not found")),
        };

        obj.push(pkt)
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
