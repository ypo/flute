use super::AlcCodec;
use crate::{
    common::{
        alc, lct,
        oti::{self, ReedSolomonGF2MSchemeSpecific, SchemeSpecific},
        pkt,
    },
    error::FluteError,
};

pub struct AlcRS2m {}

impl AlcCodec for AlcRS2m {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
        /*  0                   1                   2                   3
         0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |   HET = 64    |    HEL = 4    |                               |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
        |                      Transfer Length (L)                      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |       m       |       G       |   Encoding Symbol Length (E)  |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |  Max Source Block Length (B)  |  Max Nb Enc. Symbols (max_n)  |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+*/

        if let SchemeSpecific::ReedSolomon(scheme_specific) = oti.scheme_specific.as_ref().unwrap()
        {
            let ext_header_l: u64 =
                (lct::Ext::Fti as u64) << 56 | 4u64 << 48 | transfer_length & 0xFFFFFFFFFFFF;

            let b = oti.maximum_source_block_length as u16;
            let max_n = (oti.max_number_of_parity_symbols + oti.maximum_source_block_length) as u16;

            data.extend(ext_header_l.to_be_bytes());
            data.push(scheme_specific.m);
            data.push(scheme_specific.g);
            data.extend(oti.encoding_symbol_length.to_be_bytes());
            data.extend(b.to_be_bytes());
            data.extend(max_n.to_be_bytes());
            lct::inc_hdr_len(data, 4);
        } else {
            debug_assert!(false);
        }
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

        if fti.len() != 16 {
            return Err(FluteError::new("Wrong extension size"));
        }

        debug_assert!(fti[0] == lct::Ext::Fti as u8);
        if fti[1] != 4 {
            return Err(FluteError::new("Wrong extension"));
        }

        let transfer_length =
            u64::from_be_bytes(fti[0..8].as_ref().try_into().unwrap()) & 0xFFFFFFFFFFFF;
        let m = fti[8];
        let g = fti[9];
        let encoding_symbol_length = u16::from_be_bytes(fti[10..12].as_ref().try_into().unwrap());
        let b = u16::from_be_bytes(fti[12..14].as_ref().try_into().unwrap());
        let max_n = u16::from_be_bytes(fti[14..16].as_ref().try_into().unwrap());

        let oti = oti::Oti {
            fec_encoding_id: oti::FECEncodingID::ReedSolomonGF2M,
            fec_instance_id: 0,
            maximum_source_block_length: b as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: max_n as u32 - b as u32,
            scheme_specific: Some(SchemeSpecific::ReedSolomon(ReedSolomonGF2MSchemeSpecific {
                g: match g {
                    0 => 1,
                    g => g,
                },
                m: match m {
                    0 => 8,
                    m => m,
                },
            })),
            inband_fti: true,
        };

        Ok(Some((oti, transfer_length)))
    }

    fn add_fec_payload_id(&self, data: &mut Vec<u8>, oti: &oti::Oti, pkt: &pkt::Pkt) {
        let m = oti
            .scheme_specific
            .as_ref()
            .map(|f| match f {
                SchemeSpecific::ReedSolomon(s) => s.m,
                _ => 8,
            })
            .unwrap_or(8);

        let sbn = pkt.sbn;
        let esi = pkt.esi;

        /*
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |     Source Block Number (32-m                  | Enc. Symb. ID |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */
        let header: u32 = (sbn << m) | esi & 0xFF;
        data.extend(header.to_be_bytes());
    }

    fn get_fec_payload_id(
        &self,
        pkt: &alc::AlcPkt,
        oti: &oti::Oti,
    ) -> crate::error::Result<alc::PayloadID> {
        /*
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |     Source Block Number (32-m                  | Enc. Symb. ID |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */
        let data = &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset];
        let arr: [u8; 4] = match data.try_into() {
            Ok(arr) => arr,
            Err(e) => return Err(FluteError::new(e.to_string())),
        };
        let payload_id_header = u32::from_be_bytes(arr);

        let m = oti
            .scheme_specific
            .as_ref()
            .map(|f| match f {
                SchemeSpecific::ReedSolomon(s) => s.m,
                _ => 8,
            })
            .unwrap_or(8);

        let sbn = payload_id_header >> m;
        let esi_mask = (1u32 << m) - 1u32;
        let esi = payload_id_header & esi_mask;

        Ok(alc::PayloadID {
            esi,
            sbn,
            source_block_length: None,
        })
    }

    fn get_fec_inline_payload_id(
        &self,
        _pkt: &alc::AlcPkt,
    ) -> crate::error::Result<alc::PayloadID> {
        Err(FluteError::new("not supported"))
    }

    fn fec_payload_id_block_length(&self) -> usize {
        4
    }
}
