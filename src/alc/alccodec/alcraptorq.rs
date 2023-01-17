use super::AlcCodec;
use crate::{
    alc::{alc, lct, oti, pkt},
    error::FluteError,
};

pub struct AlcRaptorQ {}

impl AlcCodec for AlcRaptorQ {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
        /*
         +-
        | FTI <127 8bits|  LEN (8bit)   |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |                      Transfer Length (F)                      |
        +               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |               |    Reserved   |           Symbol Size (T)     |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |       Z       |              N                |       Al      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        | PADDING (16 bits)   ??        |

        Transfer Length (F): 40-bit unsigned integer
        Symbol Size (T): 16-bit unsigned integer.
        The number of source blocks (Z): 8-bit unsigned integer.
        The number of sub-blocks (N): 16-bit unsigned integer.
        A symbol alignment parameter (Al): 8-bit unsigned integer.
        */
        let ext_header: u16 = (lct::Ext::Fti as u16) << 8 | 4u16;
        let transfer_header: u64 = (transfer_length << 24) | oti.encoding_symbol_length as u64;

        assert!(oti.raptorq_scheme_specific.is_some());
        let raptorq = oti.raptorq_scheme_specific.as_ref().unwrap();

        let padding: u16 = 0;

        data.extend(ext_header.to_be_bytes());
        data.extend(transfer_header.to_be_bytes());
        data.push(raptorq.source_block_length);
        data.extend(raptorq.sub_blocks_length.to_be_bytes());
        data.push(raptorq.symbol_alignment);
        data.extend(padding.to_be_bytes());
        lct::inc_hdr_len(data, 4);
    }

    fn get_fti(
        &self,
        data: &[u8],
        lct_header: &lct::LCTHeader,
    ) -> crate::error::Result<Option<(oti::Oti, u64)>> {
        let fti = match lct::get_ext(data.as_ref(), &lct_header, lct::Ext::Fti)? {
            Some(fti) => fti,
            None => return Ok(None),
        };

        if fti.len() != 16 {
            return Err(FluteError::new("Wrong extension size"));
        }

        todo!()
    }

    fn add_fec_payload_id(&self, _data: &mut Vec<u8>, _oti: &oti::Oti, _pkt: &pkt::Pkt) {
        todo!()
    }

    fn get_fec_payload_id(
        &self,
        _pkt: &alc::AlcPkt,
        _oti: &oti::Oti,
    ) -> crate::error::Result<alc::PayloadID> {
        todo!()
    }

    fn fec_payload_id_block_length(&self) -> usize {
        todo!()
    }
}
