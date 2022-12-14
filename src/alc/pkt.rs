use super::lct;

#[derive(Debug)]
pub struct Pkt {
    pub payload: Vec<u8>,
    pub transfer_length: u64,
    /* 
    pub block_length: u32,
    */
    pub esi: u32,
    pub snb: u32,
    pub toi: u128,
    pub fdt_id: Option<u32>,
    pub cenc: lct::CENC,
    pub inband_cenc: bool,
   // pub is_source_symbol: bool
}
