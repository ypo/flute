
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum FECEncodingID {
    FECNoCode = 0,
    FECSmallLargeExpandable = 128,
    FECReedSolomonGF28 = 129,
    FECReedSolomonGF2M = 2,
    FECLDPCStaircase = 3,
}

pub struct Oti {
    pub fec: FECEncodingID, 
    pub fec_instance_id: u16,
    pub maximum_source_block_length: u32,
    pub encoding_symbol_length: u16,     
    pub max_number_of_encoding_symbols: u32, 
    pub reed_solomon_m: u8,
    pub g: u8,
    pub valid: bool,
    pub inband_oti: bool,
}
