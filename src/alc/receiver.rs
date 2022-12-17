use super::alc;
use super::lct;
use super::objectreceiver::ObjectReceiver;
use crate::tools::error::FluteError;
use crate::tools::error::Result;
use std::collections::HashMap;

pub struct Receiver {
    tsi: u64,
    objects: HashMap<u128, Box<ObjectReceiver>>,
}

impl Receiver {
    pub fn new(tsi: u64) -> Receiver {
        Receiver {
            tsi,
            objects: HashMap::new(),
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
                // todo close this receiver
                return Ok(true);
            }

            return Err(FluteError::new("FDT pkt received without FDT Extension"));
        }

        Ok(true)
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
        let obj = Box::new(ObjectReceiver::new());
        self.objects.insert(toi.clone(), obj);
    }
}
