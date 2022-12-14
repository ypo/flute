use super::{lct, oti, pkt::Pkt};
use crate::tools::error::{FluteError, Result};

struct BlockID {
    snb: u32,
    esi: u32,
}

struct PayloadID {
    snb: u32,
    esi: u32,
    snb_length: u32,
}

pub fn create_alc_pkt(
    oti: &oti::Oti,
    cci: &u128,
    tsi: u64,
    pkt: &Pkt,
    now: Option<u64>,
) -> Vec<u8> {
    let mut data = Vec::new();
    lct::push_lct_header(&mut data, 0, &cci, tsi, &pkt.toi, oti.fec as u8);

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

pub fn parse_alc_pkt(data: &Vec<u8>) -> Result<lct::LCTHeader> {
    let lct_header = lct::parse_lct_header(data)?;
    log::info!("LCT Header={:?}", lct_header);
    Ok(lct_header)
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

fn push_sct(data: &mut Vec<u8>, time: u64) {
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
    let one_second_in_us = 1000000u64;
    let seconds_utc = time / one_second_in_us;
    let seconds_ntp = seconds_utc as u32 + 2208988800u32;
    let rest_ntp = (((time - (seconds_utc * one_second_in_us)) * (1u64 << 32)) / 1000000u64) as u32;

    data.extend(header.to_be_bytes());
    data.extend(seconds_ntp.to_be_bytes());
    data.extend(rest_ntp.to_be_bytes());
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

fn push_fec_payload_id_16x16(data: &mut Vec<u8>, snb: u16, esi: u16) {
    /*
       +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
       |     Source Block Number       |      Encoding Symbol ID       |
       +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    */
    let fec_payload_id = (snb as u32) << 16 | (esi as u32);
    data.extend(fec_payload_id.to_be_bytes());
    lct::inc_hdr_len(data, 1);
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
        };

        let alc_pkt = super::create_alc_pkt(&oti, &cci, tsi, &pkt, None);
        let decoded_pkt = super::parse_alc_pkt(&alc_pkt).unwrap();
        assert!(decoded_pkt.toi == pkt.toi);
        assert!(decoded_pkt.cci == cci);
        assert!(decoded_pkt.tsi == tsi);
    }
}
