use super::lct;

#[derive(Debug)]
pub struct Pkt {
    pub payload: Vec<u8>,
    pub transfer_length: u64,
    pub esi: u32,
    pub sbn: u32,
    pub toi: u128,
    pub fdt_id: Option<u32>,
    pub cenc: lct::Cenc,
    pub inband_cenc: bool,
    pub close_object: bool,
    pub source_block_length: u32,
    pub sender_current_time: bool,
}
