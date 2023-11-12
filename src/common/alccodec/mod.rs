use super::{
    alc::{AlcPkt, PayloadID},
    lct, oti, pkt,
};
use crate::tools::error::Result;

mod alcnocode;
mod alcraptor;
mod alcraptorq;
mod alcrs28;
mod alcrs28underspecified;
mod alcrs2m;

pub trait AlcCodec {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64);
    fn get_fti(&self, data: &[u8], lct_header: &lct::LCTHeader) -> Result<Option<(oti::Oti, u64)>>;
    fn add_fec_payload_id(&self, data: &mut Vec<u8>, oti: &oti::Oti, pkt: &pkt::Pkt);
    fn get_fec_payload_id(&self, pkt: &AlcPkt, oti: &oti::Oti) -> Result<PayloadID>;
    fn get_fec_inline_payload_id(&self, pkt: &AlcPkt) -> Result<PayloadID>;
    fn fec_payload_id_block_length(&self) -> usize;
}

impl dyn AlcCodec {
    pub fn instance(fec: oti::FECEncodingID) -> &'static dyn AlcCodec {
        const NOCODE: alcnocode::AlcNoCode = alcnocode::AlcNoCode {};
        const ALCRS28: alcrs28::AlcRS28 = alcrs28::AlcRS28 {};
        const ALCRS2M: alcrs2m::AlcRS2m = alcrs2m::AlcRS2m {};
        const ALCRS28UNDERSPECIFIED: alcrs28underspecified::AlcRS28UnderSpecified =
            alcrs28underspecified::AlcRS28UnderSpecified {};
        const ALCRAPTORQ: alcraptorq::AlcRaptorQ = alcraptorq::AlcRaptorQ {};
        const ALCRAPTOR: alcraptor::AlcRaptor = alcraptor::AlcRaptor {};

        match fec {
            oti::FECEncodingID::NoCode => &NOCODE,
            oti::FECEncodingID::ReedSolomonGF2M => &ALCRS2M,
            oti::FECEncodingID::ReedSolomonGF28 => &ALCRS28,
            oti::FECEncodingID::ReedSolomonGF28UnderSpecified => &ALCRS28UNDERSPECIFIED,
            oti::FECEncodingID::RaptorQ => &ALCRAPTORQ,
            oti::FECEncodingID::Raptor => &ALCRAPTOR,
        }
    }
}
