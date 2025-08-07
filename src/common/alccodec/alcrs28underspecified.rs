use super::AlcCodec;
use crate::{
    common::{alc, lct, oti, pkt},
    error::FluteError,
};

pub struct AlcRS28UnderSpecified {}

impl AlcCodec for AlcRS28UnderSpecified {
    fn add_fti(&self, data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
        /*
        * 0                   1                   2                   3
          0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         |                      Transfer Length                          |
         +                               +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         |                               |         FEC Instance ID       |
         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         |    Encoding Symbol Length     |  Maximum Source Block Length  |
         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         | Max. Num. of Encoding Symbols |
         +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        */

        let ext_header: u16 = (lct::Ext::Fti as u16) << 8 | 4u16;
        let transfer_header_fec_id: u64 = (transfer_length << 16) | oti.fec_instance_id as u64;
        let esl: u16 = oti.encoding_symbol_length;
        let sbl: u16 = ((oti.maximum_source_block_length) & 0xFFFF) as u16;
        let mne: u16 = (oti.max_number_of_parity_symbols + oti.maximum_source_block_length) as u16;

        data.extend(ext_header.to_be_bytes());
        data.extend(transfer_header_fec_id.to_be_bytes());
        data.extend(esl.to_be_bytes());
        data.extend(sbl.to_be_bytes());
        data.extend(mne.to_be_bytes());
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
        let fec_instance_id = u16::from_be_bytes(fti[8..10].as_ref().try_into().unwrap());
        let encoding_symbol_length = u16::from_be_bytes(fti[10..12].as_ref().try_into().unwrap());
        let maximum_source_block_length =
            u16::from_be_bytes(fti[12..14].as_ref().try_into().unwrap());
        let num_encoding_symbols = u16::from_be_bytes(fti[14..16].as_ref().try_into().unwrap());

        let oti = oti::Oti {
            fec_encoding_id: oti::FECEncodingID::ReedSolomonGF28UnderSpecified,
            fec_instance_id,
            maximum_source_block_length: maximum_source_block_length as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols: (num_encoding_symbols as u32).checked_sub(maximum_source_block_length as u32).unwrap_or_default(),
            scheme_specific: None,
            inband_fti: true,
        };

        Ok(Some((oti, transfer_length)))
    }

    fn add_fec_payload_id(&self, data: &mut Vec<u8>, _oti: &oti::Oti, pkt: &pkt::Pkt) {
        /*0                   1                   2                   3
             0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
            +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
            |                     Source Block Number                       |
            +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
            |      Source Block Length      |       Encoding Symbol ID      |
            +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        */
        let sbn = pkt.sbn;
        let source_block_length = pkt.source_block_length as u16;
        let esi = pkt.esi as u16;

        data.extend(sbn.to_be_bytes());
        data.extend(source_block_length.to_be_bytes());
        data.extend(esi.to_be_bytes());
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
        let arr: [u8; 8] = match data.try_into() {
            Ok(arr) => arr,
            Err(e) => return Err(FluteError::new(e.to_string())),
        };

        /*0                   1                   2                   3
             0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
            +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
            |                     Source Block Number                       |
            +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
            |      Source Block Length      |       Encoding Symbol ID      |
            +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        */
        let payload_id_header = u64::from_be_bytes(arr);
        let sbn = ((payload_id_header >> 32) & 0xFFFFFFFF) as u32;
        let source_block_length = ((payload_id_header >> 16) & 0xFFFF) as u32;
        let esi = ((payload_id_header) & 0xFFFF) as u32;

        Ok(alc::PayloadID {
            sbn,
            esi,
            source_block_length: Some(source_block_length),
        })
    }

    fn fec_payload_id_block_length(&self) -> usize {
        8
    }
}
