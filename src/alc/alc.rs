use super::{lct, oti, pkt::Pkt};
use crate::tools::{self, error::FluteError, error::Result};
use std::time::SystemTime;

#[derive(Debug)]
pub struct AlcPkt<'a> {
    pub lct: lct::LCTHeader,
    pub oti: Option<oti::Oti>,
    pub transfer_length: Option<u64>,
    pub cenc: Option<lct::CENC>,
    pub server_time: Option<SystemTime>,
    pub data: &'a [u8],
    pub data_alc_header_offset: usize,
    pub data_payload_offset: usize,
    pub fdt_info: Option<ExtFDT>,
}

#[derive(Debug)]
pub struct AlcPktCache {
    pub lct: lct::LCTHeader,
    pub oti: Option<oti::Oti>,
    pub transfer_length: Option<u64>,
    pub cenc: Option<lct::CENC>,
    pub server_time: Option<SystemTime>,
    pub data_alc_header_offset: usize,
    pub data_payload_offset: usize,
    pub data: Vec<u8>,
    pub fdt_info: Option<ExtFDT>,
}

pub struct PayloadID {
    pub snb: u32,
    pub esi: u32,
    pub source_block_length: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ExtFDT {
    pub version: u32,
    pub fdt_instance_id: u32,
}

impl<'a> AlcPkt<'a> {
    pub fn to_cache(&self) -> AlcPktCache {
        AlcPktCache {
            lct: self.lct.clone(),
            oti: self.oti.clone(),
            transfer_length: self.transfer_length,
            cenc: self.cenc.clone(),
            server_time: self.server_time.clone(),
            data_alc_header_offset: self.data_alc_header_offset,
            data_payload_offset: self.data_payload_offset,
            data: self.data.to_vec(),
            fdt_info: self.fdt_info.clone(),
        }
    }
}

impl<'a> AlcPktCache {
    pub fn to_pkt(&'a self) -> AlcPkt<'a> {
        AlcPkt {
            lct: self.lct.clone(),
            oti: self.oti.clone(),
            transfer_length: self.transfer_length,
            cenc: self.cenc.clone(),
            server_time: self.server_time.clone(),
            data_alc_header_offset: self.data_alc_header_offset,
            data_payload_offset: self.data_payload_offset,
            data: self.data.as_ref(),
            fdt_info: self.fdt_info.clone(),
        }
    }
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
        oti.fec_encoding_id as u8,
        pkt.close_object,
    );

    if pkt.toi == lct::TOI_FDT {
        assert!(pkt.fdt_id.is_some());
        push_fdt(&mut data, 2, pkt.fdt_id.unwrap())
    }

    // In case of FDT, we must push CENC if CENC is not null
    if pkt.toi == lct::TOI_FDT && (pkt.cenc != lct::CENC::Null) || pkt.inband_cenc {
        push_cenc(&mut data, pkt.cenc as u8);
    }

    if now.is_some() {
        push_sct(&mut data, now.unwrap());
    }

    match oti.fec_encoding_id {
        oti::FECEncodingID::NoCode => {
            if pkt.toi == lct::TOI_FDT || oti.inband_oti {
                push_no_code_oti(&mut data, oti, pkt.transfer_length);
            }
            push_fec_payload_id_m(&mut data, pkt.snb, pkt.esi as u16, 16);
        }
        oti::FECEncodingID::ReedSolomonGF28 => {
            if pkt.toi == lct::TOI_FDT || oti.inband_oti {
                push_general_oti(&mut data, oti, pkt.transfer_length);
            }
            push_fec_payload_id_m(&mut data, pkt.snb, pkt.esi as u16, 8);
        }
        oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic => {
            if pkt.toi == lct::TOI_FDT || oti.inband_oti {
                push_small_block_systematic_oti(&mut data, oti, pkt.transfer_length);
            }
            push_fec_payload_id_block_systematic(
                &mut data,
                pkt.snb,
                pkt.esi as u16,
                pkt.source_block_length as u16,
            );
        }
        oti::FECEncodingID::ReedSolomonGF2M => todo!(),
    }

    push_payload(&mut data, pkt);
    data
}

pub fn parse_alc_pkt(data: &[u8]) -> Result<AlcPkt> {
    let lct_header = lct::parse_lct_header(data)?;

    let fec: oti::FECEncodingID = lct_header
        .cp
        .try_into()
        .map_err(|_| FluteError::new(format!("Codepoint {} not supported", lct_header.cp)))?;

    let alc_payload_id_length: usize = match fec {
        oti::FECEncodingID::NoCode => 4,
        oti::FECEncodingID::ReedSolomonGF28 => 4,
        oti::FECEncodingID::ReedSolomonGF2M => 4,
        oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic => 8,
    };

    if alc_payload_id_length + lct_header.len > data.len() {
        return Err(FluteError::new("Wrong size of ALC packet"));
    }

    let fti = lct::get_ext(data.as_ref(), &lct_header, lct::EXT::Fti)?;
    let mut oti: Option<oti::Oti> = None;
    let mut transfer_length: Option<u64> = None;
    if fti.is_some() {
        let fti = fti.unwrap();
        let res = match fec {
            oti::FECEncodingID::NoCode => parse_no_code_oti(fti).ok(),
            oti::FECEncodingID::ReedSolomonGF28 => {
                parse_general_oti(fti, oti::FECEncodingID::ReedSolomonGF28).ok()
            }
            oti::FECEncodingID::ReedSolomonGF2M => todo!(),
            oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic => {
                parse_small_block_systematic_oti(fti, fec).ok()
            }
        };
        if let Some((o, t)) = res {
            oti = Some(o);
            transfer_length = Some(t)
        };
    }

    let data_alc_header_offset = lct_header.len;
    let data_payload_offset = alc_payload_id_length + lct_header.len;

    let cenc = lct::get_ext(data.as_ref(), &lct_header, lct::EXT::Cenc)?;
    let cenc = match cenc {
        Some(ext) => parse_cenc(ext).ok(),
        None => None,
    };

    let mut fdt_info: Option<ExtFDT> = None;
    if lct_header.toi == lct::TOI_FDT {
        let fdt = lct::get_ext(data.as_ref(), &lct_header, lct::EXT::Fdt)?;
        fdt_info = match fdt {
            Some(ext) => parse_ext_fdt(ext)?,
            None => None,
        };
    }

    Ok(AlcPkt {
        lct: lct_header,
        oti: oti,
        transfer_length: transfer_length,
        cenc: cenc,
        server_time: None,
        data: data.as_ref(),
        data_alc_header_offset,
        data_payload_offset,
        fdt_info,
    })
}

pub fn parse_payload_id(pkt: &AlcPkt, oti: &oti::Oti) -> Result<PayloadID> {
    match pkt.lct.cp.try_into() {
        Ok(oti::FECEncodingID::NoCode) => parse_fec_payload_id_m(
            &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset],
            16,
        ),
        Ok(oti::FECEncodingID::ReedSolomonGF28) => parse_fec_payload_id_m(
            &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset],
            8,
        ),
        Ok(oti::FECEncodingID::ReedSolomonGF2M) => parse_fec_payload_id_m(
            &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset],
            oti.reed_solomon_m.unwrap_or_default(),
        ),
        Ok(oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic) => {
            parse_fec_payload_id_block_systematic(
                &pkt.data[pkt.data_alc_header_offset..pkt.data_payload_offset],
            )
        }
        Err(_) => Err(FluteError::new(format!(
            "Code point {} is not supported",
            pkt.lct.cp
        ))),
    }
}

fn parse_ext_fdt(ext: &[u8]) -> Result<Option<ExtFDT>> {
    if ext.len() != 4 {
        return Err(FluteError::new("Wrong size of FDT Extension"));
    }

    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |   HET = 192   |   V   |          FDT Instance ID              |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */

    let mut fdt_bytes: [u8; 4] = [0; 4];
    fdt_bytes.copy_from_slice(&ext);
    let fdt_bytes = u32::from_be_bytes(fdt_bytes);

    let version = (fdt_bytes >> 20) & 0xF;
    let fdt_instance_id = fdt_bytes & 0xFFFF;

    Ok(Some(ExtFDT {
        version,
        fdt_instance_id,
    }))
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

fn parse_cenc(ext: &[u8]) -> Result<lct::CENC> {
    if ext.len() != 4 {
        return Err(FluteError::new("Wrong extension size"));
    }
    ext[1]
        .try_into()
        .map_err(|_| FluteError::new("CENC not supported"))
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
        fec_encoding_id: oti::FECEncodingID::NoCode,
        fec_instance_id: 0,
        maximum_source_block_length,
        encoding_symbol_length,
        max_number_of_parity_symbols: 0,
        reed_solomon_m: None,
        inband_oti: true,
    };

    Ok((oti, transfer_length))
}

/// rfc5510 Using the General EXT_FTI Format
fn push_general_oti(data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
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
        (lct::EXT::Fti as u64) << 56 | 3u64 << 48 | transfer_length & 0xFFFFFFFFFFFF;
    let max_n: u32 = (oti.max_number_of_parity_symbols + oti.maximum_source_block_length) & 0xFF;
    let e_b_n: u32 = (oti.encoding_symbol_length as u32) << 16
        | (oti.maximum_source_block_length & 0xFF) << 8
        | max_n;
    data.extend(ext_header_l.to_be_bytes());
    data.extend(e_b_n.to_be_bytes());
    lct::inc_hdr_len(data, 3);
}

fn parse_general_oti(fti: &[u8], fec_encoding_id: oti::FECEncodingID) -> Result<(oti::Oti, u64)> {
    if fti.len() != 12 {
        return Err(FluteError::new("Wrong extension size"));
    }

    assert!(fti[0] == lct::EXT::Fti as u8);
    assert!(fti[1] == 3);

    let mut transfer_length: [u8; 8] = [0; 8];
    transfer_length.copy_from_slice(&fti[0..8]);
    let transfer_length = u64::from_be_bytes(transfer_length) & 0xFFFFFFFFFFFF;

    let mut encoding_symbol_length: [u8; 2] = [0; 2];
    encoding_symbol_length.copy_from_slice(&fti[8..10]);
    let encoding_symbol_length = u16::from_be_bytes(encoding_symbol_length);

    let maximum_source_block_length = fti[10];
    let num_encoding_symbols = fti[11];

    let oti = oti::Oti {
        fec_encoding_id: fec_encoding_id,
        fec_instance_id: 0,
        maximum_source_block_length: maximum_source_block_length as u32,
        encoding_symbol_length,
        max_number_of_parity_symbols: num_encoding_symbols as u32
            - maximum_source_block_length as u32,
        reed_solomon_m: None,
        inband_oti: true,
    };

    Ok((oti, transfer_length))
}

///
/// Small Block Systematic FEC Scheme
/// https://tools.ietf.org/html/rfc5445
fn push_small_block_systematic_oti(data: &mut Vec<u8>, oti: &oti::Oti, transfer_length: u64) {
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

    let ext_header: u16 = (lct::EXT::Fti as u16) << 8 | 4u16;
    let transfer_header_fec_id: u64 = (transfer_length << 16) | oti.fec_instance_id as u64;
    let esl: u16 = oti.encoding_symbol_length as u16;
    let sbl: u16 = ((oti.maximum_source_block_length) & 0xFFFF) as u16;
    let mne: u16 = (oti.max_number_of_parity_symbols + oti.maximum_source_block_length) as u16;

    data.extend(ext_header.to_be_bytes());
    data.extend(transfer_header_fec_id.to_be_bytes());
    data.extend(esl.to_be_bytes());
    data.extend(sbl.to_be_bytes());
    data.extend(mne.to_be_bytes());
    lct::inc_hdr_len(data, 4);
}

fn parse_small_block_systematic_oti(
    fti: &[u8],
    fec_encoding_id: oti::FECEncodingID,
) -> Result<(oti::Oti, u64)> {
    if fti.len() != 16 {
        return Err(FluteError::new("Wrong extension size"));
    }

    assert!(fti[0] == lct::EXT::Fti as u8);
    assert!(fti[1] == 4);

    let mut transfer_length: [u8; 8] = [0; 8];
    transfer_length.copy_from_slice(&fti[2..10]);
    let transfer_length = u64::from_be_bytes(transfer_length) >> 16;

    let mut fec_instance_id: [u8; 2] = [0; 2];
    fec_instance_id.copy_from_slice(&fti[8..10]);
    let fec_instance_id = u16::from_be_bytes(fec_instance_id);

    let mut encoding_symbol_length: [u8; 2] = [0; 2];
    encoding_symbol_length.copy_from_slice(&fti[10..12]);
    let encoding_symbol_length = u16::from_be_bytes(encoding_symbol_length);

    let mut maximum_source_block_length: [u8; 2] = [0; 2];
    maximum_source_block_length.copy_from_slice(&fti[12..14]);
    let maximum_source_block_length = u16::from_be_bytes(maximum_source_block_length);

    let mut num_encoding_symbols: [u8; 2] = [0; 2];
    num_encoding_symbols.copy_from_slice(&fti[14..16]);
    let num_encoding_symbols = u16::from_be_bytes(num_encoding_symbols);

    let oti = oti::Oti {
        fec_encoding_id: fec_encoding_id,
        fec_instance_id: fec_instance_id,
        maximum_source_block_length: maximum_source_block_length as u32,
        encoding_symbol_length,
        max_number_of_parity_symbols: num_encoding_symbols as u32
            - maximum_source_block_length as u32,
        reed_solomon_m: None,
        inband_oti: true,
    };

    Ok((oti, transfer_length))
}

fn push_fec_payload_id_block_systematic(
    data: &mut Vec<u8>,
    snb: u32,
    esi: u16,
    source_block_length: u16,
) {
    /*0                   1                   2                   3
         0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |                     Source Block Number                       |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
        |      Source Block Length      |       Encoding Symbol ID      |
        +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    */
    data.extend(snb.to_be_bytes());
    data.extend(source_block_length.to_be_bytes());
    data.extend(esi.to_be_bytes());
}

fn parse_fec_payload_id_block_systematic(data: &[u8]) -> Result<PayloadID> {
    assert!(data.len() == 8);
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
    let snb = ((payload_id_header >> 32) & 0xFFFFFFFF) as u32;
    let source_block_length = ((payload_id_header >> 16) & 0xFFFF) as u32;
    let esi = ((payload_id_header) & 0xFFFF) as u32;

    Ok(PayloadID {
        snb,
        esi,
        source_block_length: Some(source_block_length),
    })
}

fn push_fec_payload_id_m(data: &mut Vec<u8>, snb: u32, esi: u16, m: u8) {
    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |     Source Block Number (32-m                  | Enc. Symb. ID |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */
    let header: u32 = (snb << m) | (esi as u32) & 0xFF;
    data.extend(header.to_be_bytes());
}

fn parse_fec_payload_id_m(data: &[u8], m: u8) -> Result<PayloadID> {
    /*
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    |     Source Block Number (32-m                  | Enc. Symb. ID |
    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
     */
    let arr: [u8; 4] = match data.try_into() {
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

    #[test]
    pub fn test_alc_create() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let cci: u128 = 0x804754755879;
        let tsi: u64 = 0x055789451234;
        let payload = vec!['h' as u8, 'e' as u8, 'l' as u8, 'l' as u8, 'o' as u8];
        let transfer_length = payload.len() as u64;

        let pkt = pkt::Pkt {
            payload: payload,
            esi: 1,
            snb: 2,
            toi: 3,
            fdt_id: None,
            cenc: lct::CENC::Null,
            inband_cenc: true,
            transfer_length: transfer_length,
            close_object: false,
            source_block_length: 1,
        };

        let alc_pkt = super::create_alc_pkt(&oti, &cci, tsi, &pkt, None);
        let decoded_pkt = super::parse_alc_pkt(&alc_pkt).unwrap();
        assert!(decoded_pkt.lct.toi == pkt.toi);
        assert!(decoded_pkt.lct.cci == cci);
        assert!(decoded_pkt.lct.tsi == tsi);
    }
}
