use super::{
    alc::{AlcPkt, PayloadID},
    lct, oti, pkt,
};
use crate::tools::error::Result;

pub mod alcnocode;
pub mod alcrs28;
pub mod alcrs28smallblocksystematic;
pub mod alcrs2m;

pub trait AlcCodec {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64);
    fn get_fti(&self, data: &[u8], lct_header: &lct::LCTHeader) -> Result<Option<(oti::Oti, u64)>>;
    fn add_fec_payload_id(&self, data: &mut Vec<u8>, oti: &oti::Oti, pkt: &pkt::Pkt);
    fn get_fec_payload_id(&self, pkt: &AlcPkt, oti: &oti::Oti) -> Result<PayloadID>;
    fn fec_payload_id_block_length(&self) -> usize;
}

impl dyn AlcCodec {
    pub fn instance(fec: oti::FECEncodingID) -> &'static dyn AlcCodec {
        const NOCODE: alcnocode::AlcNoCode = alcnocode::AlcNoCode {};
        const ALCRS28: alcrs28::AlcRS28 = alcrs28::AlcRS28 {};
        const ALCRS2M: alcrs2m::AlcRS2m = alcrs2m::AlcRS2m {};
        const ALCRS28SMALLBLOCKSYSTEMATIC:
            alcrs28smallblocksystematic::AlcRS28SmallBlockSystematic =
            alcrs28smallblocksystematic::AlcRS28SmallBlockSystematic {};

        match fec {
            oti::FECEncodingID::NoCode => &NOCODE,
            oti::FECEncodingID::ReedSolomonGF2M => &ALCRS2M,
            oti::FECEncodingID::ReedSolomonGF28 => &ALCRS28,
            oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic => &ALCRS28SMALLBLOCKSYSTEMATIC,
        }
    }
}
