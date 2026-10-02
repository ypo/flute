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
        RFC 5053 §3.2.3 <https://www.rfc-editor.org/rfc/rfc5053.html#section-3.2.3>
         0                   1                   2                   3
         0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |   HET = 64    |    HEL = 4    |                               |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+                               +
        |                      Transfer Length (F)                      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |           Reserved            |        Symbol Size (T)        |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |             Z                 |      N        |       Al      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

        Transfer Length (F): 48-bit unsigned integer
        Reserved: 16 bits
        Symbol Size (T): 16-bit unsigned integer.
        The number of source blocks (Z): 16-bit unsigned integer.
        The number of sub-blocks (N): 8-bit unsigned integer.
        A symbol alignment parameter (Al): 8-bit unsigned integer.
        */
        let len: u8 = 4;
        let ext_header: u16 = (lct::Ext::Fti as u16) << 8 | len as u16;
        // 48-bit F followed by 16 reserved bits
        let transfer_header: u64 = transfer_length << 16;

        debug_assert!(oti.scheme_specific.is_some());
        if let SchemeSpecific::Raptor(raptor) = oti.scheme_specific.as_ref().unwrap() {
            data.extend(ext_header.to_be_bytes());
            data.extend(transfer_header.to_be_bytes());
            data.extend(oti.encoding_symbol_length.to_be_bytes());
            data.extend(raptor.source_blocks_length.to_be_bytes());
            data.push(raptor.sub_blocks_length);
            data.push(raptor.symbol_alignment);
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

        let transfer_length = u64::from_be_bytes(fti[2..10].as_ref().try_into().unwrap()) >> 16;
        let symbol_size = u16::from_be_bytes(fti[10..12].as_ref().try_into().unwrap());
        let z = u16::from_be_bytes(fti[12..14].as_ref().try_into().unwrap());
        let n = fti[14];
        let al = fti[15];

        if symbol_size == 0 {
            return Err(FluteError::new("Symbol size is null"));
        }

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

#[cfg(test)]
mod tests {
    use super::AlcRaptor;
    use crate::common::{
        alccodec::AlcCodec,
        lct,
        oti::{self, SchemeSpecific},
    };

    #[test]
    pub fn test_raptor_fti_rfc5053_layout() {
        crate::tests::init();

        let mut oti = oti::Oti::new_raptor(1400, 60, 4, 2, 4).unwrap();
        if let Some(SchemeSpecific::Raptor(raptor)) = oti.scheme_specific.as_mut() {
            raptor.source_blocks_length = 0x0102;
        }
        let transfer_length: u64 = 0x1234_5678_9ABC;

        let mut data = Vec::new();
        lct::push_lct_header(&mut data, 0, &0, 1, &2, 1, false, false);
        let ext_offset = data.len();
        AlcRaptor {}.add_fti(&mut data, &oti, transfer_length);

        let expected: [u8; 16] = [
            lct::Ext::Fti as u8,
            4, // HEL
            0x12,
            0x34,
            0x56,
            0x78,
            0x9A,
            0xBC, // F (48 bits)
            0x00,
            0x00, // Reserved
            0x05,
            0x78, // T = 1400
            0x01,
            0x02, // Z
            2,    // N
            4,    // Al
        ];
        assert_eq!(&data[ext_offset..], &expected);

        let lct_header = lct::parse_lct_header(&data).unwrap();
        let (decoded_oti, decoded_transfer_length) =
            AlcRaptor {}.get_fti(&data, &lct_header).unwrap().unwrap();
        assert_eq!(decoded_transfer_length, transfer_length);
        assert_eq!(decoded_oti.encoding_symbol_length, 1400);
        match decoded_oti.scheme_specific {
            Some(SchemeSpecific::Raptor(raptor)) => {
                assert_eq!(raptor.source_blocks_length, 0x0102);
                assert_eq!(raptor.sub_blocks_length, 2);
                assert_eq!(raptor.symbol_alignment, 4);
            }
            _ => panic!("Raptor scheme specific expected"),
        }
    }
}
