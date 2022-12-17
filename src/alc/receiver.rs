use super::objectreceiver::ObjectReceiver;
use crate::tools::error::Result;
use std::collections::HashMap;
use super::alc;

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

    pub fn push(alc_pkt: &Vec<u8>) -> Result<()> {

        let alc = alc::parse_alc_pkt(alc_pkt)?;

        Ok(())
    }
}
