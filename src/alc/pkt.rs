use super::lct;
use crate::tools::error::Result;

#[derive(Debug)]
pub struct Pkt {
    pub payload: Vec<u8>,
    pub transfer_length: u64,
    pub esi: u32,
    pub snb: u32,
    pub toi: u128,
    pub fdt_id: Option<u32>,
    pub cenc: lct::CENC,
    pub inband_cenc: bool,
}

/// Write ALC packet to a destination
pub trait PktWriter {
    fn write(&self, data: &Vec<u8>) -> Result<usize>;
}
