use std::time::SystemTime;

use super::{lct, oti, pkt::Pkt};
use crate::tools::{self, error::FluteError, error::Result};

pub struct AlcPkt {
    pub lct: lct::LCTHeader,
    pub oti: Option<oti::Oti>,
    pub transfer_length: Option<u64>,
    pub cenc: Option<lct::CENC>,
    pub alc_header: Vec<u8>,
    pub payload: Vec<u8>,
    pub snb: u32,
    pub esi: u32,
    pub server_time: Option<SystemTime>,
}

pub struct PayloadID {
    pub snb: u32,
    pub esi: u32,
    pub source_block_length: Option<u32>,
}

pub fn create_alc_pkt(
    oti: &oti::Oti,
    cci: &u128,
    tsi: u64,
    pkt: &Pkt,
    now: Option<&SystemTime>,
) -> Vec<u8> {
    let mut data = Vec::new();
    lct::push_lct_header(
        &mut data,
        0,
        &cci,
        tsi,
        &pkt.toi,
        oti.fec as u8,
        pkt.close_object,
    );

    if pkt.toi == lct::TOI_FDT {
        assert!(pkt.fdt_id.is_some());
        push_fdt(&mut data, 2, pkt.fdt_id.unwrap())
    }

    if pkt.cenc != lct::CENC::Null && pkt.inband_cenc {
        push_cenc(&mut data, pkt.cenc as u8);
    }

    if now.is_some() {
        push_sct(&mut data, now.unwrap());
    }

    match oti.fec {
        oti::FECEncodingID::NoCode => {
            if pkt.toi == lct::TOI_FDT || oti.inband_oti {
                push_no_code_oti(&mut data, oti, pkt.transfer_length);
            }
            push_fec_payload_id_16x16(&mut data, pkt.snb as u16, pkt.esi as u16);
        }
        oti::FECEncodingID::ReedSolomonGF28 => todo!(),
        oti::FECEncodingID::ReedSolomonGF2M => todo!(),
    }

    push_payload(&mut data, pkt);
    data
}

pub fn parse_alc_pkt(data: &Vec<u8>) -> Result<AlcPkt> {
    let lct_header = lct::parse_lct_header(data)?;

    let fec: oti::FECEncodingID = lct_header
        .cp
        .try_into()
        .map_err(|_| FluteError::new(format!("Codepoint {} not supported", lct_header.cp)))?;

    let alc_header_length: usize = match fec {
        oti::FECEncodingID::NoCode => 4,
        oti::FECEncodingID::ReedSolomonGF2M => 4,
        oti::FECEncodingID::ReedSolomonGF28 => 8,
    };

    if alc_header_length + lct_header.len > data.len() {
        return Err(FluteError::new("Wrong size of ALC packet"));
    }

    let alc_header = &data[lct_header.len..(alc_header_length + lct_header.len)];
    let payload = &data[(alc_header_length + lct_header.len)..];

    let fti = lct::get_ext(data, &lct_header, lct::EXT::Fti)?;
    let mut oti: Option<oti::Oti> = None;
    let mut transfer_length: Option<u64> = None;
    if fti.is_some() {
        let fti = fti.unwrap();
        let res = match fec {
            oti::FECEncodingID::NoCode => parse_no_code_oti(fti).ok(),
            oti::FECEncodingID::ReedSolomonGF2M => todo!(),
            oti::FECEncodingID::ReedSolomonGF28 => todo!(),
        };
        if let Some((o, t)) = res {
            oti = Some(o);
            transfer_length = Some(t)
        };
    }

    Ok(AlcPkt {
        lct: lct_header,
        oti: oti,
        transfer_length: transfer_length,
        cenc: None,
        snb: 0,
        esi: 0,
        server_time: None,
        payload: payload.to_vec(),
        alc_header: alc_header.to_vec(),
    })
}

pub fn parse_payload_id(pkt: &AlcPkt, m: Option<u8>) -> Result<PayloadID> {
    match pkt.lct.cp.try_into() {
        Ok(oti::FECEncodingID::NoCode) => parse_fec_payload_id_16x16(&pkt.alc_header),
        Ok(oti::FECEncodingID::ReedSolomonGF2M) => {
            parse_fec_payload_id_m(&pkt.alc_header, m.unwrap_or_default())
        }
        Ok(oti::FECEncodingID::ReedSolomonGF28) => {
            parse_fec_payload_id_block_systematic(&pkt.alc_header)
        }
        Err(_) => Err(FluteError::new(format!(
            "Code point {} is not supported",
            pkt.lct.cp
        ))),
    }
}

fn push_fdt(data: &mut Vec<u8>, version: u8, fdt_id: u32) {
    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |   HET = 192   |   V   |          FDT Instance ID              |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */
    let ext = (lct::EXT::Fdt as u32) << 24 | (version as u32) << 20 | fdt_id;
    data.extend(ext.to_be_bytes());
    lct::inc_hdr_len(data, 1);
}

fn push_cenc(data: &mut Vec<u8>, cenc: u8) {
    /*
     0                   1                   2                   3
     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |   HET = 193   |     CENC      |          Reserved             |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
      */
    let ext = (lct::EXT::Cenc as u32) << 24 | (cenc as u32) << 16;
    data.extend(ext.to_be_bytes());
    lct::inc_hdr_len(data, 1);
}

fn push_sct(data: &mut Vec<u8>, time: &std::time::SystemTime) {
    /*
     0                   1                   2                   3
     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |     HET = 2   |    HEL >= 1   |         Use (bit field)       |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |                       first time value                        |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    ...            (other time values (optional)                  ...
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+   */

    /*
     Use (bit field)                                       3
      6   7   8   9   0   1   2   3   4   5   6   7   8   9   0   1
    +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
    |SCT|SCT|ERT|SLC|   reserved    |          PI-specific          |
    |Hi |Low|   |   |    by LCT     |              use              |
    +---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+---+
    */

    /* HEL | SCT HIGH | SCT LOW */
    let header: u32 = (lct::EXT::Time as u32) << 24 | (3u32 << 16) | (1u32 << 15) | (1u32 << 14);

    // Convert UTC to NTP
    let (seconds, rest) = match tools::system_time_to_ntp(time) {
        Ok(res) => res,
        Err(_) => return,
    };
    data.extend(header.to_be_bytes());
    data.extend(seconds.to_be_bytes());
    data.extend(rest.to_be_bytes());
    lct::inc_hdr_len(data, 3);
}

fn push_no_code_oti(data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
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

    let ext_header: u16 = (lct::EXT::Fti as u16) << 8 | 4u16;
    let transfer_header: u64 = transfer_length << 16;
    let esl: u16 = oti.encoding_symbol_length as u16;
    let sbl_msb: u16 = ((oti.maximum_source_block_length >> 16) & 0xFFFF) as u16;
    let sbl_lsb: u16 = (oti.maximum_source_block_length & 0xFFFF) as u16;

    data.extend(ext_header.to_be_bytes());
    data.extend(transfer_header.to_be_bytes());
    data.extend(esl.to_be_bytes());
    data.extend(sbl_msb.to_be_bytes());
    data.extend(sbl_lsb.to_be_bytes());
    lct::inc_hdr_len(data, 4);
}

fn parse_no_code_oti(fti: &[u8]) -> Result<(oti::Oti, u64)> {
    if fti.len() != 16 {
        return Err(FluteError::new("Wrong extension size"));
    }

    assert!(fti[0] == lct::EXT::Fti as u8);
    assert!(fti[1] == 4);

    let mut transfer_length: [u8; 8] = [0; 8];
    transfer_length.copy_from_slice(&fti[2..10]);
    let transfer_length = u64::from_be_bytes(transfer_length) >> 16;

    let mut encoding_symbol_length: [u8; 2] = [0; 2];
    encoding_symbol_length.copy_from_slice(&fti[10..12]);
    let encoding_symbol_length = u16::from_be_bytes(encoding_symbol_length);

    let mut maximum_source_block_length: [u8; 4] = [0; 4];
    maximum_source_block_length.copy_from_slice(&fti[12..16]);
    let maximum_source_block_length = u32::from_be_bytes(maximum_source_block_length);

    let oti = oti::Oti {
        fec: oti::FECEncodingID::NoCode,
        fec_instance_id: 0,
        maximum_source_block_length,
        encoding_symbol_length,
        max_number_of_parity_symbols: 0,
        reed_solomon_m: None,
        inband_oti: true,
    };

    Ok((oti, transfer_length))
}

fn push_fec_payload_id_16x16(data: &mut Vec<u8>, snb: u16, esi: u16) {
    log::info!("Write snb {} esi {}", snb, esi);
    /*
       +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
       |     Source Block Number       |      Encoding Symbol ID       |
       +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    */
    let fec_payload_id = (snb as u32) << 16 | (esi as u32);
    data.extend(fec_payload_id.to_be_bytes());
}

fn parse_fec_payload_id_16x16(data: &Vec<u8>) -> Result<PayloadID> {
    assert!(data.len() == 4);
    let arr: [u8; 4] = match data.as_slice().try_into() {
        Ok(arr) => arr,
        Err(e) => return Err(FluteError::new(e.to_string())),
    };

    /*
           +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
           |     Source Block Number       |      Encoding Symbol ID       |
           +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    */
    let payload_id_header = u32::from_be_bytes(arr);
    let snb = (payload_id_header >> 16) & 0xFFFF;
    let esi = (payload_id_header) & 0xFFFF;

    Ok(PayloadID {
        snb,
        esi,
        source_block_length: None,
    })
}

fn parse_fec_payload_id_block_systematic(data: &Vec<u8>) -> Result<PayloadID> {
    assert!(data.len() == 8);
    let arr: [u8; 8] = match data.as_slice().try_into() {
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
    let snb = ((payload_id_header >> 32) & 0xFFFFFFFF) as u32;
    let source_block_length = ((payload_id_header >> 16) & 0xFFFF) as u32;
    let esi = ((payload_id_header) & 0xFFFF) as u32;

    Ok(PayloadID {
        snb,
        esi,
        source_block_length: Some(source_block_length),
    })
}

fn parse_fec_payload_id_m(data: &Vec<u8>, m: u8) -> Result<PayloadID> {
    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |     Source Block Number (32-m                  | Enc. Symb. ID |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */
    let arr: [u8; 4] = match data.as_slice().try_into() {
        Ok(arr) => arr,
        Err(e) => return Err(FluteError::new(e.to_string())),
    };
    let payload_id_header = u32::from_be_bytes(arr);

    let snb = payload_id_header >> m;
    let esi_mask = (1u32 << m) - 1u32;
    let esi = payload_id_header & esi_mask;

    Ok(PayloadID {
        esi,
        snb,
        source_block_length: None,
    })
}

fn push_payload(data: &mut Vec<u8>, pkt: &Pkt) {
    data.extend(pkt.payload.iter());
}

#[cfg(test)]
mod tests {
    use crate::alc::lct;
    use crate::alc::oti;
    use crate::alc::pkt;

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_alc_create() {
        init();

        let oti: oti::Oti = Default::default();
        let cci: u128 = 0x804754755879;
        let tsi: u64 = 0x055789451234;

        let pkt = pkt::Pkt {
            payload: vec!['h' as u8, 'e' as u8, 'l' as u8, 'l' as u8, 'o' as u8],
            esi: 1,
            snb: 2,
            toi: 3,
            fdt_id: None,
            cenc: lct::CENC::Null,
            inband_cenc: true,
            transfer_length: 5,
            close_object: false,
        };

        let alc_pkt = super::create_alc_pkt(&oti, &cci, tsi, &pkt, None);
        let decoded_pkt = super::parse_alc_pkt(&alc_pkt).unwrap();
        assert!(decoded_pkt.lct.toi == pkt.toi);
        assert!(decoded_pkt.lct.cci == cci);
        assert!(decoded_pkt.lct.tsi == tsi);
    }
}
