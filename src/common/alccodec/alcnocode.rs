use super::AlcCodec;
use crate::{
    common::{alc, lct, oti, pkt},
    error::FluteError,
};

pub struct AlcNoCode {}

impl AlcCodec for AlcNoCode {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
        // https://tools.ietf.org/html/rfc5445
        /*
        +-
        | FTI  <127 8bits | LEN    (8bit)      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |                      Transfer Length                          |
        +                               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |                               |           Reserved            |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-++
        |    Encoding Symbol Length     | Max. Source Block Length (MSB)|
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        | Max. Source Block Length (LSB)|
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+*/
        let ext_header: u16 = (lct::Ext::Fti as u16) << 8 | 4u16;
        let transfer_header: u64 = transfer_length << 16;
        let esl: u16 = oti.encoding_symbol_length;
        let sbl_msb: u16 = ((oti.maximum_source_block_length >> 16) & 0xFFFF) as u16;
        let sbl_lsb: u16 = (oti.maximum_source_block_length & 0xFFFF) as u16;

        data.extend(ext_header.to_be_bytes());
        data.extend(transfer_header.to_be_bytes());
        data.extend(esl.to_be_bytes());
        data.extend(sbl_msb.to_be_bytes());
        data.extend(sbl_lsb.to_be_bytes());
        lct::inc_hdr_len(data, 4);
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
            return Err(FluteError::new(format!(
                "Wrong exten header size {} != 4 for FTI",
                fti[1]
            )));
        }

        let transfer_length = u64::from_be_bytes(fti[2..10].as_ref().try_into().unwrap()) >> 16;
        let encoding_symbol_length = u16::from_be_bytes(fti[10..12].as_ref().try_into().unwrap());
        let maximum_source_block_length =
            u32::from_be_bytes(fti[12..16].as_ref().try_into().unwrap());

        let oti = oti::Oti {
            fec_encoding_id: oti::FECEncodingID::NoCode,
            fec_instance_id: 0,
            maximum_source_block_length,
            encoding_symbol_length,
            max_number_of_parity_symbols: 0,
            scheme_specific: None,
            inband_fti: true,
        };

        Ok(Some((oti, transfer_length)))
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

    fn add_fec_payload_id(&self, data: &mut Vec<u8>, _oti: &oti::Oti, pkt: &pkt::Pkt) {
        let sbn = pkt.sbn;
        let esi = pkt.esi;
        /*
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |     Source Block Number  16 bits | Enc. Symb. ID  16 bits     |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */
        let header: u32 = ((sbn & 0xFFFF) << 16) | esi & 0xFFFF;
        data.extend(header.to_be_bytes());
    }

    fn fec_payload_id_block_length(&self) -> usize {
        4
    }
}
