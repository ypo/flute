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

fn parse_fec_payload_id(pkt: &alc::AlcPkt, m: u8) -> crate::error::Result<alc::PayloadID> {
    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |     Source Block Number (32-m                  | Enc. Symb. ID |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */
    if !(2..=16).contains(&m) {
        return Err(FluteError::new(format!(
            "Reed-Solomon GF(2^m): m={} is not between 2 and 16",
            m
        )));
    }

    let data = &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset];
    let arr: [u8; 4] = match data.try_into() {
        Ok(arr) => arr,
        Err(e) => return Err(FluteError::new(e.to_string())),
    };
    let payload_id_header = u32::from_be_bytes(arr);

    let sbn = payload_id_header >> m;
    let esi_mask = (1u32 << m) - 1u32;
    let esi = payload_id_header & esi_mask;

    Ok(alc::PayloadID {
        esi,
        sbn,
        source_block_length: None,
    })
}

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

        let scheme_specific = oti.reed_solomon_gf2m_scheme();
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
        let scheme_specific = ReedSolomonGF2MSchemeSpecific::parse(fti[8], fti[9])?;
        let encoding_symbol_length = u16::from_be_bytes(fti[10..12].as_ref().try_into().unwrap());
        let b = u16::from_be_bytes(fti[12..14].as_ref().try_into().unwrap());
        let max_n = u16::from_be_bytes(fti[14..16].as_ref().try_into().unwrap());
        let max_number_of_parity_symbols = (max_n as u32).saturating_sub(b as u32);

        let oti = oti::Oti {
            fec_encoding_id: oti::FECEncodingID::ReedSolomonGF2M,
            fec_instance_id: 0,
            maximum_source_block_length: b as u32,
            encoding_symbol_length,
            max_number_of_parity_symbols,
            scheme_specific: Some(SchemeSpecific::ReedSolomon(scheme_specific)),
            inband_fti: true,
        };

        Ok(Some((oti, transfer_length)))
    }

    fn add_fec_payload_id(&self, data: &mut Vec<u8>, oti: &oti::Oti, pkt: &pkt::Pkt) {
        let m = oti.reed_solomon_gf2m_scheme().m as u32;
        debug_assert!((2..=16).contains(&m));
        let m = m.clamp(2, 16);

        let sbn = pkt.sbn;
        let esi = pkt.esi;
        debug_assert!(sbn <= u32::MAX >> m);
        debug_assert!(esi < 1u32 << m);

        /*
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |     Source Block Number (32-m                  | Enc. Symb. ID |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
         */
        let header: u32 = ((sbn & (u32::MAX >> m)) << m) | (esi & ((1u32 << m) - 1));
        data.extend(header.to_be_bytes());
    }

    fn get_fec_payload_id(
        &self,
        pkt: &alc::AlcPkt,
        oti: &oti::Oti,
    ) -> crate::error::Result<alc::PayloadID> {
        parse_fec_payload_id(pkt, oti.reed_solomon_gf2m_scheme().m)
    }

    fn get_fec_inline_payload_id(
        &self,
        pkt: &alc::AlcPkt,
    ) -> crate::error::Result<alc::PayloadID> {
        // m is only known from the FEC OTI
        match pkt.oti.as_ref() {
            Some(oti) => parse_fec_payload_id(pkt, oti.reed_solomon_gf2m_scheme().m),
            None => Err(FluteError::new(
                "Reed-Solomon GF(2^m): FEC Payload ID cannot be parsed without EXT_FTI",
            )),
        }
    }

    fn fec_payload_id_block_length(&self) -> usize {
        4
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::AlcRS2m;
    use crate::common::{
        alc,
        alccodec::AlcCodec,
        lct,
        oti::{self, ReedSolomonGF2MSchemeSpecific, SchemeSpecific},
        pkt, Profile,
    };

    fn create_pkt(sbn: u32, esi: u32) -> pkt::Pkt {
        pkt::Pkt {
            payload: vec![1, 2, 3, 4],
            transfer_length: 1000,
            esi,
            sbn,
            toi: 3,
            fdt_id: None,
            cenc: lct::Cenc::Null,
            inband_cenc: false,
            close_object: false,
            source_block_length: 10,
            sender_current_time: false,
        }
    }

    #[test]
    pub fn test_rs2m_payload_id() {
        crate::tests::init();

        for (m, sbn, esi, header) in [
            (16u8, 0x1234u32, 300u32, 0x1234_012Cu32),
            (8, 0x12_3456, 0xAB, 0x1234_56AB),
            (4, 0x123_4567, 0xE, 0x1234_567E),
            (2, 0x3FFF_FFFF, 2, 0xFFFF_FFFE),
        ] {
            let max_parity = ((1u32 << m) - 2) as u16;
            let oti = oti::Oti::new_reed_solomon_rs2m(8, 1, max_parity, m).unwrap();
            let data =
                alc::new_alc_pkt(&oti, &0, 1, &create_pkt(sbn, esi), Profile::RFC6726, SystemTime::now());
            let alc_pkt = alc::parse_alc_pkt(&data).unwrap();
            assert_eq!(
                &data[alc_pkt.data_alc_header_offset..alc_pkt.data_payload_offset],
                &header.to_be_bytes()
            );

            let payload_id = alc::parse_payload_id(&alc_pkt, &oti).unwrap();
            assert_eq!((payload_id.sbn, payload_id.esi), (sbn, esi), "m={}", m);

            // m is read from the in-band FTI
            let payload_id = alc::get_fec_inline_payload_id(&alc_pkt).unwrap();
            assert_eq!((payload_id.sbn, payload_id.esi), (sbn, esi), "m={}", m);
        }
    }

    #[test]
    pub fn test_rs2m_inline_payload_id_without_fti() {
        crate::tests::init();

        let mut oti = oti::Oti::new_reed_solomon_rs2m(1400, 64, 20, 16).unwrap();
        oti.inband_fti = false;
        let data =
            alc::new_alc_pkt(&oti, &0, 1, &create_pkt(1, 70), Profile::RFC6726, SystemTime::now());
        let alc_pkt = alc::parse_alc_pkt(&data).unwrap();
        assert!(alc_pkt.oti.is_none());
        assert!(alc::get_fec_inline_payload_id(&alc_pkt).is_err());

        let payload_id = alc::parse_payload_id(&alc_pkt, &oti).unwrap();
        assert_eq!((payload_id.sbn, payload_id.esi), (1, 70));
    }

    #[test]
    pub fn test_rs2m_fti() {
        crate::tests::init();

        let oti = oti::Oti::new_reed_solomon_rs2m(1400, 64, 20, 16).unwrap();
        let mut data = Vec::new();
        lct::push_lct_header(&mut data, 0, &0, 1, &2, 2, false, false);
        let ext_offset = data.len();
        AlcRS2m {}.add_fti(&mut data, &oti, 1000);
        let lct_header = lct::parse_lct_header(&data).unwrap();

        let (decoded_oti, transfer_length) =
            AlcRS2m {}.get_fti(&data, &lct_header).unwrap().unwrap();
        assert_eq!(transfer_length, 1000);
        assert_eq!(decoded_oti.encoding_symbol_length, 1400);
        assert_eq!(decoded_oti.maximum_source_block_length, 64);
        assert_eq!(decoded_oti.max_number_of_parity_symbols, 20);
        let scheme = decoded_oti.reed_solomon_gf2m_scheme();
        assert_eq!((scheme.m, scheme.g), (16, 1));

        // 0 means default values
        data[ext_offset + 8] = 0;
        data[ext_offset + 9] = 0;
        let (decoded_oti, _) = AlcRS2m {}.get_fti(&data, &lct_header).unwrap().unwrap();
        let scheme = decoded_oti.reed_solomon_gf2m_scheme();
        assert_eq!((scheme.m, scheme.g), (8, 1));

        for m in [1, 17, 32, 255] {
            data[ext_offset + 8] = m;
            assert!(AlcRS2m {}.get_fti(&data, &lct_header).is_err(), "m={}", m);
        }
    }

    #[test]
    pub fn test_rs2m_payload_id_invalid_m() {
        crate::tests::init();

        let mut oti = oti::Oti::new_reed_solomon_rs2m(1400, 64, 20, 16).unwrap();
        let data =
            alc::new_alc_pkt(&oti, &0, 1, &create_pkt(1, 1), Profile::RFC6726, SystemTime::now());
        let alc_pkt = alc::parse_alc_pkt(&data).unwrap();

        oti.scheme_specific = Some(SchemeSpecific::ReedSolomon(ReedSolomonGF2MSchemeSpecific {
            m: 32,
            g: 1,
        }));
        assert!(alc::parse_payload_id(&alc_pkt, &oti).is_err());
    }
}
