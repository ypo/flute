use super::AlcCodec;
use crate::{
    common::{
        alc, lct,
        oti::{self, SchemeSpecific},
        pkt,
    },
    error::FluteError,
};

pub struct AlcRaptor {}

impl AlcCodec for AlcRaptor {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
        /*
         +-
        | FTI <127 8bits|  LEN (8bit)   |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |                      Transfer Length (F)                      |
        +               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |               |    Reserved   |           Symbol Size (T)     |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |             Z                 |      N        |       Al      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        | PADDING (16 bits)   ??        |

        Transfer Length (F): 40-bit unsigned integer
        Symbol Size (T): 16-bit unsigned integer.
        The number of source blocks (Z): 16-bit unsigned integer.
        The number of sub-blocks (N): 8-bit unsigned integer.
        A symbol alignment parameter (Al): 8-bit unsigned integer.
        */
        let len: u8 = 4;
        let ext_header: u16 = (lct::Ext::Fti as u16) << 8 | len as u16;
        let transfer_header: u64 =
            (transfer_length << 24) | (oti.encoding_symbol_length as u64 & 0xFFFF);

        debug_assert!(oti.scheme_specific.is_some());
        if let SchemeSpecific::Raptor(raptor) = oti.scheme_specific.as_ref().unwrap() {
            let padding: u16 = 0;
            data.extend(ext_header.to_be_bytes());
            data.extend(transfer_header.to_be_bytes());
            data.extend(raptor.source_blocks_length.to_be_bytes());
            data.extend(raptor.sub_blocks_length.to_be_bytes());
            data.extend(raptor.symbol_alignment.to_be_bytes());
            data.extend(padding.to_be_bytes());
            lct::inc_hdr_len(data, len);
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

        let transfer_length = u64::from_be_bytes(fti[2..10].as_ref().try_into().unwrap()) >> 24;
        let symbol_size = u16::from_be_bytes(fti[8..10].as_ref().try_into().unwrap());
        let z = u16::from_be_bytes(fti[10..12].as_ref().try_into().unwrap());
        let n = fti[12];
        let al = fti[13];

        if z == 0 {
            return Err(FluteError::new("Z is null"));
        }

        if al == 0 {
            return Err(FluteError::new("AL must be at least 1"));
        }

        if symbol_size % al as u16 != 0 {
            return Err(FluteError::new("Symbol size is not properly aligned"));
        }

        let block_size = num_integer::div_ceil(transfer_length, z as u64);
        let maximum_source_block_length = num_integer::div_ceil(block_size, symbol_size as u64);

        let oti = oti::Oti {
            fec_encoding_id: oti::FECEncodingID::Raptor,
            fec_instance_id: 0,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length: symbol_size,
            max_number_of_parity_symbols: 0, // Unknown for RaptorQ
            scheme_specific: Some(SchemeSpecific::Raptor(oti::RaptorSchemeSpecific {
                source_blocks_length: z,
                sub_blocks_length: n,
                symbol_alignment: al,
            })),
            inband_fti: true,
        };

        Ok(Some((oti, transfer_length)))
    }

    fn add_fec_payload_id(&self, data: &mut Vec<u8>, _oti: &oti::Oti, pkt: &pkt::Pkt) {
        /*
         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |     Source Block Number       |      Encoding Symbol ID       |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */

        let payload_id = (pkt.sbn & 0xFFFFu32) << 16 | pkt.esi & 0xFFFFu32;
        data.extend(payload_id.to_be_bytes());
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
        let sbn = payload_id_header >> 16;
        let esi = payload_id_header & 0xFFFF;
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
