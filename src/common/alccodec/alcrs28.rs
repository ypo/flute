use super::AlcCodec;
use crate::{
    common::{alc, lct, oti, pkt},
    error::FluteError,
};

pub struct AlcRS28 {}

impl AlcCodec for AlcRS28 {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
        /*0                   1                   2                   3
         0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |   HET = 64    |    HEL = 3    |                               |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
        |                      Transfer Length (L)                      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |   Encoding Symbol Length (E)  | MaxBlkLen (B) |     max_n     |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+*/

        let ext_header_l: u64 =
            (lct::Ext::Fti as u64) << 56 | 3u64 << 48 | transfer_length & 0xFFFFFFFFFFFF;
        let max_n: u32 =
            (oti.max_number_of_parity_symbols + oti.maximum_source_block_length) & 0xFF;
        let e_b_n: u32 = (oti.encoding_symbol_length as u32) << 16
            | (oti.maximum_source_block_length & 0xFF) << 8
            | max_n;
        data.extend(ext_header_l.to_be_bytes());
        data.extend(e_b_n.to_be_bytes());
        lct::inc_hdr_len(data, 3);
    }

    fn get_fti(
        &self,
        data: &[u8],
        lct_header: &lct::LCTHeader,
    ) -> crate::error::Result<Option<(oti::Oti, u64)>> {
        let fti = match lct::get_ext(data, lct_header, lct::Ext::Fti as u8)? {
            Some(fti) => fti,
            None => return Ok(None),
        };

        if fti.len() != 12 {
            return Err(FluteError::new("Wrong extension size"));
        }

        debug_assert!(fti[0] == lct::Ext::Fti as u8);
        if fti[1] != 3 {
            return Err(FluteError::new("Wrong header extension"));
        }

        let transfer_length =
            u64::from_be_bytes(fti[0..8].as_ref().try_into().unwrap()) & 0xFFFFFFFFFFFF;
        let encoding_symbol_length = u16::from_be_bytes(fti[8..10].as_ref().try_into().unwrap());

        let maximum_source_block_length = fti[10];
        let num_encoding_symbols = fti[11];

        let oti = oti::Oti {
            fec_encoding_id: oti::FECEncodingID::ReedSolomonGF28,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: num_encoding_symbols as u32
                - maximum_source_block_length as u32,
            scheme_specific: None,
            inband_fti: true,
        };

        Ok(Some((oti, transfer_length)))
    }

    fn add_fec_payload_id(&self, data: &mut Vec<u8>, _oti: &oti::Oti, pkt: &pkt::Pkt) {
        /*
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |     Source Block Number (24)                 | Enc. Symb. ID  |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */

        let sbn = pkt.sbn & 0xFFFFFF;
        let esi = pkt.esi & 0xFF;

        let header: u32 = (sbn << 8) | esi & 0xFF;
        data.extend(header.to_be_bytes());
    }

    fn get_fec_payload_id(
        &self,
        pkt: &alc::AlcPkt,
        _oti: &oti::Oti,
    ) -> crate::error::Result<alc::PayloadID> {
        self.get_fec_inline_payload_id(pkt)
    }

    fn get_fec_inline_payload_id(&self, pkt: &alc::AlcPkt) -> crate::error::Result<alc::PayloadID> {
        let data = &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset];
        let arr: [u8; 4] = match data.try_into() {
            Ok(arr) => arr,
            Err(e) => return Err(FluteError::new(e.to_string())),
        };
        let payload_id_header = u32::from_be_bytes(arr);
        let sbn = payload_id_header >> 8;
        let esi = payload_id_header & 0xFF;
        Ok(alc::PayloadID {
            esi,
            sbn,
            source_block_length: None,
        })
    }

    fn fec_payload_id_block_length(&self) -> usize {
        4
    }
}
