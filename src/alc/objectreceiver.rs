use super::alc;
use crate::tools::error::Result;

pub struct ObjectReceiver {}

impl ObjectReceiver {
    pub fn new() -> ObjectReceiver {
        ObjectReceiver {}
    }

    pub fn push(&mut self, pkt: &alc::AlcPkt) -> Result<bool> {
        Ok(true)
    }
}
